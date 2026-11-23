// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Schema versions and migrations.
//!
//! The schema version is SQLite's `user_version`. Each migration is a
//! numbered SQL file in `migrations/`, compiled into the program; they run
//! forward only, in order, each in its own write transaction that also sets
//! the new version, so a crash leaves the database at the last complete
//! version. A version-0 file with no tables is a new workspace; version 0
//! with tables is some other database.
//!
//! A program refuses a database newer than it knows. An older one is
//! upgraded only when the caller allows it (the app does, after a backup);
//! otherwise opening fails with [`StoreError::NeedsUpgrade`].

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::{Result, StoreError};

/// The schema version this program is built for.
pub const SCHEMA_VERSION: i64 = 3;

/// One migration: the SQL that takes the schema from `version - 1` to
/// `version`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) sql: &'static str,
}

/// Every migration, in order. The last one's version is [`SCHEMA_VERSION`].
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_requests_and_names.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../migrations/0003_subtask_order.sql"),
    },
];

/// The schema of a database file compared with this program's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    /// A new, empty file.
    Empty,
    /// The version this program is built for.
    Current,
    /// An older version, which needs migrating.
    Older(i64),
    /// A newer version, which this program must not touch.
    Newer(i64),
}

/// Reads the schema version of the database on `conn`.
pub(crate) fn state(conn: &Connection, current: i64) -> Result<SchemaState> {
    let version = user_version(conn)?;
    Ok(match version {
        0 => {
            let objects: i64 =
                conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
            if objects == 0 {
                SchemaState::Empty
            } else {
                return Err(StoreError::NotAWorkspace(
                    conn.path().unwrap_or_default().into(),
                ));
            }
        }
        v if v == current => SchemaState::Current,
        v if v < current => SchemaState::Older(v),
        v => SchemaState::Newer(v),
    })
}

/// The `user_version` of the database on `conn`.
pub(crate) fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// Runs every migration in `migrations` above the database's version.
/// `init` runs inside the transaction that creates the schema of a new
/// workspace (from version 0), to fill in its first rows. Returns the version
/// the database had before.
///
/// Safe against another process migrating at the same time: each step
/// re-reads the version after taking the write lock and skips what is
/// already done. Callers still hold the migrate lock so only one process does
/// the work.
pub(crate) fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    init: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<()>,
) -> Result<i64> {
    let before = user_version(conn)?;
    if let Some(last) = migrations.last()
        && before > last.version
    {
        return Err(StoreError::TooNew {
            found: before,
            supported: last.version,
        });
    }
    let mut init = Some(init);
    for migration in migrations {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = user_version(&tx)?;
        if version >= migration.version {
            continue;
        }
        if version != migration.version - 1 {
            return Err(StoreError::Corrupt(format!(
                "schema version {version} cannot be upgraded by migration {}",
                migration.version
            )));
        }
        let run_init = if version == 0 { init.take() } else { None };
        let step = (|| {
            tx.execute_batch(migration.sql)?;
            tx.pragma_update(None, "user_version", migration.version)?;
            if let Some(init) = run_init {
                init(&tx)?;
            }
            check_foreign_keys(&tx)
        })();
        step.map_err(|source| StoreError::Migration {
            version: migration.version,
            source,
        })?;
        tx.commit()?;
        tracing::info!(version = migration.version, "workspace schema upgraded");
    }
    Ok(before)
}

/// Fails if any row breaks a foreign key: a migration must leave none.
fn check_foreign_keys(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let table: String = row.get(0)?;
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
            Some(format!("a row of {table} breaks a foreign key")),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            sql: "CREATE TABLE a (x INTEGER) STRICT;",
        },
        Migration {
            version: 2,
            sql: "ALTER TABLE a ADD COLUMN y TEXT;",
        },
    ];

    fn memory() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn empty_then_current() {
        let mut conn = memory();
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Empty);
        let before = migrate(&mut conn, TEST_MIGRATIONS, |tx| {
            tx.execute("INSERT INTO a (x) VALUES (7)", []).map(|_| ())
        })
        .unwrap();
        assert_eq!(before, 0);
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Current);
        let x: i64 = conn.query_row("SELECT x FROM a", [], |r| r.get(0)).unwrap();
        assert_eq!(x, 7);
    }

    #[test]
    fn older_is_upgraded_without_init() {
        let mut conn = memory();
        migrate(&mut conn, &TEST_MIGRATIONS[..1], |_| Ok(())).unwrap();
        assert_eq!(state(&conn, 2).unwrap(), SchemaState::Older(1));
        let before = migrate(&mut conn, TEST_MIGRATIONS, |_| {
            panic!("init runs only for new workspaces")
        })
        .unwrap();
        assert_eq!(before, 1);
        assert_eq!(user_version(&conn).unwrap(), 2);
        conn.execute("INSERT INTO a (x, y) VALUES (1, 'b')", [])
            .unwrap();
    }

    #[test]
    fn newer_is_refused() {
        let mut conn = memory();
        migrate(&mut conn, TEST_MIGRATIONS, |_| Ok(())).unwrap();
        assert_eq!(state(&conn, 1).unwrap(), SchemaState::Newer(2));
        assert!(matches!(
            migrate(&mut conn, &TEST_MIGRATIONS[..1], |_| Ok(())),
            Err(StoreError::TooNew {
                found: 2,
                supported: 1
            })
        ));
    }

    #[test]
    fn a_failed_migration_keeps_nothing() {
        let mut conn = memory();
        let broken = [
            TEST_MIGRATIONS[0],
            Migration {
                version: 2,
                sql: "CREATE TABLE b (x INTEGER); THIS IS NOT SQL;",
            },
        ];
        let err = migrate(&mut conn, &broken, |_| Ok(())).unwrap_err();
        assert!(
            matches!(err, StoreError::Migration { version: 2, .. }),
            "{err}"
        );
        assert_eq!(user_version(&conn).unwrap(), 1);
        let b: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'b'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(b, 0);
    }

    #[test]
    fn a_foreign_database_is_not_a_workspace() {
        let conn = memory();
        conn.execute_batch("CREATE TABLE other (x)").unwrap();
        assert!(matches!(state(&conn, 1), Err(StoreError::NotAWorkspace(_))));
    }

    #[test]
    fn the_real_schema_applies_cleanly() {
        let mut conn = memory();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        migrate(&mut conn, MIGRATIONS, |_| Ok(())).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(MIGRATIONS.last().unwrap().version, SCHEMA_VERSION);
        assert!(
            MIGRATIONS
                .windows(2)
                .all(|w| w[1].version == w[0].version + 1)
        );
    }
}
