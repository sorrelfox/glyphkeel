// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! [`Store`]: one open workspace database in one process.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gk_core::Id;
use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::changes::{ChangeWatcher, OwnGroups};
use crate::checkpoint::Checkpointer;
use crate::connection::{self, Purpose};
use crate::error::{Result, StoreError, WriteError};
use crate::layout::{LockFile, WorkspaceLayout};
use crate::readers::ReaderPool;
use crate::schema::{self, MIGRATIONS, SCHEMA_VERSION, SchemaState};
use crate::writer::{WriteSettings, Writer, WriterStats};

/// How a [`Store`] is opened and behaves.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Read connections in the pool (4).
    pub readers: usize,
    /// Longest wait for another process's lock in one attempt (5 s).
    pub busy_budget: Duration,
    /// How often a write that ran out of busy budget is retried (3).
    pub busy_retries: u32,
    /// Write transactions holding the lock longer than this are logged
    /// (50 ms).
    pub slow_write: Duration,
    /// Whether an older schema may be upgraded on open (no). The app allows
    /// it after making a backup; the command line and the agent server do
    /// not.
    pub allow_upgrade: bool,
    /// The program version recorded in a new workspace.
    pub app_version: String,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            readers: 4,
            busy_budget: Duration::from_secs(5),
            busy_retries: 3,
            slow_write: Duration::from_millis(50),
            allow_upgrade: false,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// An open workspace database: one writer thread and a pool of readers.
///
/// Every process opens the workspace itself; this type is safe to share
/// between the process's threads (`Store` is `Sync`), and several processes
/// may each have one open on the same folder.
#[derive(Debug)]
pub struct Store {
    layout: WorkspaceLayout,
    config: StoreConfig,
    workspace_id: Id,
    writer: Writer,
    readers: ReaderPool,
    own_groups: OwnGroups,
}

impl Store {
    /// The schema state of the workspace at `layout` without opening it for
    /// writing, so the caller can make a backup before an upgrade.
    pub fn probe(layout: &WorkspaceLayout) -> Result<SchemaState> {
        let database = layout.database();
        if !database.exists() {
            return Ok(SchemaState::Empty);
        }
        let conn = connection::open(&database, Purpose::Read, Duration::from_secs(5))?;
        schema::state(&conn, SCHEMA_VERSION)
    }

    /// The stored text of workspace setting `key`, read before the store is
    /// opened (for the backup made before a schema upgrade); `None` when
    /// there is no database or no such row.
    pub fn probe_setting(layout: &WorkspaceLayout, key: &str) -> Result<Option<String>> {
        let database = layout.database();
        if !database.exists() {
            return Ok(None);
        }
        let conn = connection::open(&database, Purpose::Read, Duration::from_secs(5))?;
        Ok(conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Opens the workspace at `layout`, creating the folder and the database
    /// if they don't exist yet. A new database gets the current schema; an
    /// older one is upgraded only if [`StoreConfig::allow_upgrade`] is set.
    pub fn open(layout: &WorkspaceLayout, config: StoreConfig) -> Result<Self> {
        layout.create_dirs()?;
        let database = layout.database();
        let mut conn = connection::open(&database, Purpose::Write, config.busy_budget)?;
        let mode: String =
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::NoWal(database));
        }

        match schema::state(&conn, SCHEMA_VERSION)? {
            SchemaState::Current => {}
            SchemaState::Newer(found) => {
                return Err(StoreError::TooNew {
                    found,
                    supported: SCHEMA_VERSION,
                });
            }
            SchemaState::Older(found) if !config.allow_upgrade => {
                return Err(StoreError::NeedsUpgrade {
                    found,
                    current: SCHEMA_VERSION,
                });
            }
            SchemaState::Empty | SchemaState::Older(_) => {
                let _lock = LockFile::acquire(&layout.migrate_lock())?;
                let version = config.app_version.clone();
                schema::migrate(&mut conn, MIGRATIONS, |tx| init_workspace(tx, &version))?;
            }
        }
        let workspace_id = read_workspace_id(&conn)?;
        let generation = crate::snapshot::generation(&conn)?;
        optimize(&conn, "open");

        let readers = (0..config.readers.max(1))
            .map(|_| connection::open(&database, Purpose::Read, config.busy_budget))
            .collect::<Result<Vec<_>>>()?;
        let settings = WriteSettings {
            schema_version: SCHEMA_VERSION,
            generation,
            busy_retries: config.busy_retries,
            slow_write: config.slow_write,
        };
        let writer = Writer::start(conn, settings, |conn| optimize(conn, "close"))?;
        Ok(Self {
            layout: layout.clone(),
            config,
            workspace_id,
            writer,
            readers: ReaderPool::new(readers),
            own_groups: OwnGroups::default(),
        })
    }

    /// The workspace folder.
    #[must_use]
    pub fn layout(&self) -> &WorkspaceLayout {
        &self.layout
    }

    /// The workspace's own ID, made when it was created.
    #[must_use]
    pub fn workspace_id(&self) -> Id {
        self.workspace_id
    }

    /// Runs `f` in a write transaction on the writer thread and commits it
    /// if `f` returns `Ok`. Blocks until done.
    ///
    /// `op` names the operation in logs and diagnostics (`task.create`).
    /// `f` may run more than once: when another program holds the lock past
    /// the busy budget, the whole transaction is retried, so `f` must not
    /// have effects outside the transaction. It must also be quick: no file
    /// I/O or waiting inside.
    ///
    /// Fails with [`StoreError::WorkspaceUpgraded`] once a newer program has
    /// upgraded the schema, and with [`StoreError::Busy`] when the retries
    /// ran out.
    pub fn write<R, E, F>(&self, op: &'static str, f: F) -> Result<R, E>
    where
        R: Send + 'static,
        E: WriteError,
        F: FnMut(&Transaction<'_>) -> Result<R, E> + Send + 'static,
    {
        self.writer.write(op, f)
    }

    /// Runs `f` in a read transaction on a pooled connection: everything it
    /// reads comes from one snapshot. Waits if every reader is busy. Keep it
    /// short; long exports read in chunks.
    pub fn read<R, E>(&self, f: impl FnOnce(&Transaction<'_>) -> Result<R, E>) -> Result<R, E>
    where
        E: From<StoreError>,
    {
        self.readers.read(f)
    }

    /// The number of read connections.
    #[must_use]
    pub fn reader_count(&self) -> usize {
        self.readers.size()
    }

    /// The writer's counters.
    #[must_use]
    pub fn writer_stats(&self) -> WriterStats {
        self.writer.stats()
    }

    /// The set of this process's own history groups, shared with every
    /// [`ChangeWatcher`] made by [`Store::watch_changes`].
    #[must_use]
    pub fn own_groups(&self) -> &OwnGroups {
        &self.own_groups
    }

    /// A watcher for commits by other connections, on its own connection.
    /// It reports history rows after `after_seq`, or after the newest row
    /// when `None`.
    pub fn watch_changes(&self, after_seq: Option<i64>) -> Result<ChangeWatcher> {
        ChangeWatcher::open(
            &self.layout.database(),
            self.config.busy_budget,
            self.own_groups.clone(),
            after_seq,
        )
    }

    /// A connection for checkpoints. Only the process holding the jobs lock
    /// should run them.
    pub fn checkpointer(&self) -> Result<Checkpointer> {
        Checkpointer::open(&self.layout.database(), self.layout.wal())
    }

    /// Finishes queued writes, runs `PRAGMA optimize` and closes the
    /// connections. Dropping the store does the same.
    pub fn close(mut self) {
        self.writer.stop();
    }
}

/// Brings the query planner's statistics up to date where a table has
/// grown or shrunk a lot since they were taken, as after an import: with
/// statistics from when a table was nearly empty, SQLite picks plans that
/// scan it once per row. `0x10000` checks every table, not only the ones
/// this connection read (the readers have their own connections); the limit
/// keeps each analysis to a sample, so this stays quick on a big workspace.
fn optimize(conn: &Connection, when: &str) {
    if let Err(e) = conn.execute_batch("PRAGMA analysis_limit = 1000; PRAGMA optimize = 0x10002;") {
        tracing::debug!(error = %e, when, "optimize failed");
    }
}

fn init_workspace(tx: &Transaction<'_>, app_version: &str) -> rusqlite::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let mut stmt = tx.prepare("INSERT INTO meta (key, value) VALUES (?1, ?2)")?;
    stmt.execute(["workspace_id", &Id::generate_at(now).to_string()])?;
    stmt.execute(["created_at", &now.to_string()])?;
    stmt.execute(["created_by_version", app_version])?;
    Ok(())
}

fn read_workspace_id(conn: &Connection) -> Result<Id> {
    let text: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'workspace_id'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| StoreError::Corrupt("the workspace has no ID".into()))?;
    text.parse()
        .map_err(|_| StoreError::Corrupt(format!("the workspace ID \"{text}\" is malformed")))
}

/// Opens (or creates) `path` with the store's connection settings,
/// read-write, without a schema check or threads. For tests and tools that
/// stand in for another process.
#[doc(hidden)]
pub fn open_raw(path: &Path) -> Result<Connection> {
    connection::open(path, Purpose::Write, Duration::from_secs(5))
}
