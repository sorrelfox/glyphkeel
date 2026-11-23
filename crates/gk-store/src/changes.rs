// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Seeing changes made by other processes.
//!
//! Every committed command appends `activity` rows with increasing sequence
//! numbers. A [`ChangeWatcher`] keeps its own connection and asks SQLite for
//! `PRAGMA data_version`, which changes whenever another connection — in
//! this process or another — has committed. Only then does it read the
//! `activity` rows after the last sequence number it has seen. A poll that
//! finds nothing new costs a few microseconds, so polling every 250 ms is
//! cheap and gives a change on screen within half a second, without file
//! watchers or any connection between processes.
//!
//! The process's own commands are announced when they commit, so the
//! watcher skips their rows: the engine registers each command's history
//! group in [`OwnGroups`] *before* committing it.

use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gk_core::Id;
use rusqlite::{Connection, Row, params};

use crate::connection::{self, Purpose};
use crate::error::{Result, StoreError};

/// How many history rows one poll reads at most; the rest follow on the next
/// poll, which then runs whether or not `data_version` changed again.
pub const MAX_ROWS_PER_POLL: usize = 1000;

/// How many of the process's own recent history groups are remembered.
const OWN_GROUPS_KEPT: usize = 4096;

/// One `activity` row: one entity changed by one command. Long texts are
/// not in here; readers fetch what they show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityRecord {
    /// Position in the workspace's history.
    pub seq: i64,
    /// The command this change belongs to.
    pub group_id: Id,
    /// When it happened, in milliseconds since 1970 UTC.
    pub at: i64,
    /// The operation, such as `task.move`.
    pub op: String,
    /// `user` or `agent`.
    pub actor_kind: String,
    /// "Me" or the agent's name.
    pub actor_name: String,
    /// `app`, `cli` or `mcp`.
    pub via: String,
    /// The agent session, for agents.
    pub session_id: Option<Id>,
    /// The project the entity belongs to, if any.
    pub project_id: Option<Id>,
    /// What kind of entity changed (`task`, `comment`, …).
    pub entity_kind: String,
    /// Which entity changed.
    pub entity_id: Id,
    /// The changed fields as JSON text: `{"status": ["To do", "In progress"]}`.
    pub changes: String,
}

/// New history rows found by one poll.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeBatch {
    /// Rows from other processes (and other stores in this one), in order.
    pub records: Vec<ActivityRecord>,
    /// The highest sequence number seen so far, including skipped own rows.
    pub last_seq: i64,
}

/// The history groups this process committed itself, so watchers can skip
/// them. Cloning shares the set.
#[derive(Debug, Clone, Default)]
pub struct OwnGroups(Arc<Mutex<OwnGroupsInner>>);

#[derive(Debug, Default)]
struct OwnGroupsInner {
    order: VecDeque<Id>,
    set: HashSet<Id>,
}

impl OwnGroups {
    /// Marks `group` as this process's own. Call it before the command
    /// commits, so no watcher can see the rows first.
    pub fn insert(&self, group: Id) {
        let mut inner = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if inner.set.insert(group) {
            inner.order.push_back(group);
            if inner.order.len() > OWN_GROUPS_KEPT
                && let Some(old) = inner.order.pop_front()
            {
                inner.set.remove(&old);
            }
        }
    }

    fn contains(&self, group: &Id) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set
            .contains(group)
    }
}

/// Watches the workspace for commits by other connections.
///
/// Each [`poll`](Self::poll) asks SQLite for `PRAGMA data_version`, which
/// changes whenever another connection (in this process or another) has
/// committed; only then does it read the `activity` rows after the last
/// sequence number seen. Rows of groups registered in [`OwnGroups`] are
/// skipped, because the engine announced them when it committed them.
#[derive(Debug)]
pub struct ChangeWatcher {
    conn: Connection,
    data_version: i64,
    last_seq: i64,
    more_pending: bool,
    own: OwnGroups,
}

impl ChangeWatcher {
    pub(crate) fn open(
        database: &Path,
        busy_budget: Duration,
        own: OwnGroups,
        after_seq: Option<i64>,
    ) -> Result<Self> {
        let conn = connection::open(database, Purpose::Read, busy_budget)?;
        let data_version = read_data_version(&conn)?;
        let last_seq = match after_seq {
            Some(seq) => seq,
            None => conn.query_row("SELECT coalesce(max(seq), 0) FROM activity", [], |r| {
                r.get(0)
            })?,
        };
        Ok(Self {
            conn,
            data_version,
            last_seq,
            more_pending: after_seq.is_some(),
            own,
        })
    }

    /// The highest sequence number seen so far.
    #[must_use]
    pub fn last_seq(&self) -> i64 {
        self.last_seq
    }

    /// Checks for new commits. `None` when nothing was committed since the
    /// last poll; otherwise the new history rows, without this process's
    /// own (the list may then be empty).
    pub fn poll(&mut self) -> Result<Option<ChangeBatch>> {
        let data_version = read_data_version(&self.conn)?;
        if data_version == self.data_version && !self.more_pending {
            return Ok(None);
        }
        self.data_version = data_version;

        let tx = self.conn.transaction()?;
        let rows: Vec<ActivityRecord> = {
            let mut stmt = tx.prepare_cached(
                "SELECT seq, group_id, at, op, actor_kind, actor_name, via, session_id,
                        project_id, entity_kind, entity_id, changes
                   FROM activity WHERE seq > ?1 ORDER BY seq LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![self.last_seq, MAX_ROWS_PER_POLL as i64], record)?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        drop(tx);

        self.more_pending = rows.len() == MAX_ROWS_PER_POLL;
        if let Some(last) = rows.last() {
            self.last_seq = last.seq;
        }
        let records = rows
            .into_iter()
            .filter(|r| !self.own.contains(&r.group_id))
            .collect();
        Ok(Some(ChangeBatch {
            records,
            last_seq: self.last_seq,
        }))
    }

    /// Polls on a background thread every `interval` and passes every
    /// non-empty batch, and every error, to `on_change`. Polling stops when
    /// the returned handle is dropped.
    pub fn spawn(
        mut self,
        interval: Duration,
        mut on_change: impl FnMut(Result<ChangeBatch>) + Send + 'static,
    ) -> Result<WatcherHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("gk-store-watcher".into())
            .spawn(move || {
                while !stop_flag.load(Ordering::Acquire) {
                    match self.poll() {
                        Ok(Some(batch)) if !batch.records.is_empty() => on_change(Ok(batch)),
                        Ok(_) => {}
                        Err(e) => on_change(Err(e)),
                    }
                    if !self.more_pending {
                        thread::park_timeout(interval);
                    }
                }
            })
            .map_err(|e| StoreError::io("starting the change watcher for", "", e))?;
        Ok(WatcherHandle {
            stop,
            thread: Some(thread),
        })
    }
}

/// Keeps a spawned [`ChangeWatcher`] running; dropping it stops the thread.
#[derive(Debug)]
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

fn read_data_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "data_version", |r| r.get(0))?)
}

fn id(row: &Row<'_>, index: usize) -> rusqlite::Result<Id> {
    let bytes: Vec<u8> = row.get(index)?;
    Id::from_slice(&bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Blob, Box::new(e))
    })
}

fn optional_id(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Id>> {
    let bytes: Option<Vec<u8>> = row.get(index)?;
    bytes
        .map(|b| {
            Id::from_slice(&b).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Blob,
                    Box::new(e),
                )
            })
        })
        .transpose()
}

fn record(row: &Row<'_>) -> rusqlite::Result<ActivityRecord> {
    Ok(ActivityRecord {
        seq: row.get(0)?,
        group_id: id(row, 1)?,
        at: row.get(2)?,
        op: row.get(3)?,
        actor_kind: row.get(4)?,
        actor_name: row.get(5)?,
        via: row.get(6)?,
        session_id: optional_id(row, 7)?,
        project_id: optional_id(row, 8)?,
        entity_kind: row.get(9)?,
        entity_id: id(row, 10)?,
        changes: row.get(11)?,
    })
}
