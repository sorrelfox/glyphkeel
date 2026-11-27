// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Snapshots of the database file, and putting one back.
//!
//! - [`vacuum_into`] writes a compact copy of the live database while other
//!   processes keep writing (`VACUUM INTO` reads one snapshot).
//! - [`check_integrity`] runs SQLite's integrity check on a copy.
//! - [`upgrade_copy`] brings a copy made by an older version up to the
//!   current schema.
//! - [`replace_database`] puts a copy's contents into the live database with
//!   SQLite's online backup API. It works through SQLite's own locks, so
//!   other processes with the workspace open see either the old or the new
//!   contents, never a mix. It raises the database's *generation*, and every
//!   process's writer refuses to write once the generation it opened with
//!   has changed ([`StoreError::WorkspaceReplaced`]).
//! - [`put_back_before_upgrade`] does the same with the copy made just
//!   before a schema upgrade that then failed.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, params};

use crate::connection::{self, Purpose};
use crate::error::{Result, StoreError};
use crate::layout::WorkspaceLayout;
use crate::schema::{self, MIGRATIONS, SCHEMA_VERSION, SchemaState};

/// The `meta` key of the database's generation.
const GENERATION_KEY: &str = "generation";

/// The generation of the database on `conn`: 0 until a restore raised it.
pub(crate) fn generation(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT coalesce((SELECT CAST(value AS INTEGER) FROM meta WHERE key = ?1), 0)",
        [GENERATION_KEY],
        |r| r.get(0),
    )?)
}

fn read_only(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        return Err(StoreError::io(
            "opening",
            path,
            io::Error::from(io::ErrorKind::NotFound),
        ));
    }
    crate::cipher::open_file(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

/// Opens the database file at `path` (a snapshot or a copy) for reading,
/// with its key when it is encrypted.
pub fn open_read_only(path: &Path) -> Result<Connection> {
    read_only(path)
}

/// The generation of the database file at `path`.
pub fn generation_of(path: &Path) -> Result<i64> {
    generation(&read_only(path)?)
}

/// The schema version (`user_version`) of the database file at `path`.
pub fn schema_version_of(path: &Path) -> Result<i64> {
    schema::user_version(&read_only(path)?)
}

/// Writes a compact copy of the database `database` to `target`, which
/// must not exist yet. Other processes may keep writing meanwhile; the copy
/// holds one consistent state. The copy uses a rollback journal, so it can
/// be read where no `-wal` file can be made. The copy of an encrypted
/// database is encrypted with the same key.
pub fn vacuum_into(database: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        return Err(StoreError::io(
            "writing",
            target,
            io::Error::from(io::ErrorKind::AlreadyExists),
        ));
    }
    let conn = connection::open(database, Purpose::Write, Duration::from_secs(5))?;
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])?;
    drop(conn);
    let copy = crate::cipher::open_file(target, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let _: String = copy.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    Ok(())
}

/// SQLite's integrity check of the database file at `path` (`quick`: the
/// faster check that skips index contents). Empty when the file is sound;
/// otherwise the problems SQLite lists.
pub fn check_integrity(path: &Path, quick: bool) -> Result<Vec<String>> {
    let conn = read_only(path)?;
    let pragma = if quick {
        "PRAGMA quick_check"
    } else {
        "PRAGMA integrity_check"
    };
    let mut stmt = conn.prepare(pragma)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(if rows == ["ok"] { Vec::new() } else { rows })
}

/// Upgrades the database copy at `path` to this program's schema. Returns
/// the version it had. A copy made by a newer program is refused.
pub fn upgrade_copy(path: &Path) -> Result<i64> {
    let mut conn = crate::cipher::open_file(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    match schema::state(&conn, SCHEMA_VERSION)? {
        SchemaState::Current => Ok(SCHEMA_VERSION),
        SchemaState::Newer(found) => Err(StoreError::TooNew {
            found,
            supported: SCHEMA_VERSION,
        }),
        SchemaState::Empty => Err(StoreError::NotAWorkspace(path.to_path_buf())),
        SchemaState::Older(_) => schema::migrate(&mut conn, MIGRATIONS, |_| Ok(())),
    }
}

/// Replaces the contents of the workspace database at `layout` with the
/// database file `source` (a checked copy at the current schema), through
/// SQLite's online backup API, and raises the generation so that every
/// process that had the old contents open stops writing. `source` is
/// changed: it gets the new generation first.
pub fn replace_database(
    layout: &WorkspaceLayout,
    source: &Path,
    busy_budget: Duration,
) -> Result<()> {
    let found = schema_version_of(source)?;
    if found != SCHEMA_VERSION {
        return Err(StoreError::Corrupt(format!(
            "the copy to restore has schema version {found}, not {SCHEMA_VERSION}"
        )));
    }
    copy_into_live(layout, source, busy_budget)
}

/// Puts the database file `source` (a copy made just before a schema
/// upgrade, at that older version) back into the workspace at `layout`
/// after the upgrade failed, so the workspace is exactly as it was before.
/// Works like [`replace_database`] but keeps the copy's own schema version.
/// The caller holds the migrate lock. `source` is changed: it gets the new
/// generation first.
pub fn put_back_before_upgrade(
    layout: &WorkspaceLayout,
    source: &Path,
    busy_budget: Duration,
) -> Result<()> {
    let found = schema_version_of(source)?;
    if found == 0 || found > SCHEMA_VERSION {
        return Err(StoreError::Corrupt(format!(
            "the copy to put back has schema version {found}"
        )));
    }
    copy_into_live(layout, source, busy_budget)
}

fn copy_into_live(layout: &WorkspaceLayout, source: &Path, busy_budget: Duration) -> Result<()> {
    // The backup API copies only between files of the same kind: a copy
    // made before the workspace was encrypted, or decrypted, is made like
    // it first.
    crate::cipher::match_encryption(source, &layout.database())?;
    let mut live = connection::open(&layout.database(), Purpose::Write, busy_budget)?;
    let next = generation(&live)?.max(generation_of(source)?) + 1;
    {
        let src = crate::cipher::open_file(source, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        src.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![GENERATION_KEY, next.to_string()],
        )?;
    }
    let src = read_only(source)?;
    {
        use rusqlite::backup::{Backup, StepResult};
        let backup = Backup::new(&src, &mut live)?;
        let started = std::time::Instant::now();
        // All pages in one step, so the destination's write lock is taken
        // once and nobody sees a half-copied database.
        loop {
            match backup.step(-1)? {
                StepResult::Done => break,
                StepResult::More => {}
                StepResult::Busy | StepResult::Locked => {
                    if started.elapsed() > busy_budget {
                        return Err(StoreError::Busy);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                _ => {}
            }
        }
    }
    let _: String = live.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    let now = generation(&live)?;
    if now != next {
        return Err(StoreError::Corrupt(format!(
            "the restored database has generation {now}, not {next}"
        )));
    }
    Ok(())
}

/// How [`copy_blob`] put a file in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobCopy {
    /// The target was there already.
    Present,
    /// A hard link to the source (same file system).
    Linked,
    /// A copy, written to a temporary name, synced and renamed.
    Copied,
}

/// Puts the immutable stored file `source` at `target` unless a file of
/// the same size is there already: a hard link where the file system
/// allows it, else a synced copy.
pub fn copy_blob(source: &Path, target: &Path) -> io::Result<BlobCopy> {
    let size = fs::metadata(source)?.len();
    if fs::metadata(target).is_ok_and(|m| m.is_file() && m.len() == size) {
        return Ok(BlobCopy::Present);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_file(target);
    if fs::hard_link(source, target).is_ok() {
        return Ok(BlobCopy::Linked);
    }
    copy_synced(source, target)?;
    Ok(BlobCopy::Copied)
}

/// Copies `source` to `target` through a temporary name, flushed to disk
/// before the rename: where a hard link can't be made, such as on another
/// volume or a file system without links.
fn copy_synced(source: &Path, target: &Path) -> io::Result<()> {
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let temp = target.with_file_name(format!(".{name}.part"));
    // Written and flushed through the one handle: a handle opened again
    // only for reading can't flush on Windows, and the copy doesn't take
    // over a read-only flag from the source.
    let mut from = fs::File::open(source)?;
    let mut to = fs::File::create(&temp)?;
    io::copy(&mut from, &mut to)?;
    to.sync_all()?;
    drop(to);
    fs::rename(&temp, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Store, StoreConfig};

    #[test]
    fn a_snapshot_is_sound_and_a_restore_raises_the_generation() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let store = Store::open(&layout, StoreConfig::default()).unwrap();
        store
            .write("test", |tx| {
                tx.execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES ('trash_days', '7', 0)",
                    [],
                )?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
        let copy = dir.path().join("copy.db");
        vacuum_into(&layout.database(), &copy).unwrap();
        assert!(check_integrity(&copy, false).unwrap().is_empty());
        assert_eq!(schema_version_of(&copy).unwrap(), SCHEMA_VERSION);
        assert!(vacuum_into(&layout.database(), &copy).is_err());

        store
            .write("test", |tx| {
                tx.execute(
                    "UPDATE settings SET value = '9' WHERE key = 'trash_days'",
                    [],
                )?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
        replace_database(&layout, &copy, Duration::from_secs(5)).unwrap();
        let value: String = store
            .read(|tx| {
                Ok::<_, StoreError>(tx.query_row(
                    "SELECT value FROM settings WHERE key = 'trash_days'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(value, "7");
        let refused = store.write("test", |tx| {
            tx.execute("UPDATE settings SET value = '1'", [])?;
            Ok::<_, StoreError>(())
        });
        assert!(matches!(refused, Err(StoreError::WorkspaceReplaced)));
        drop(store);
        let reopened = Store::open(&layout, StoreConfig::default()).unwrap();
        reopened
            .write("test", |tx| {
                tx.execute("UPDATE settings SET value = '2'", [])?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
        assert_eq!(generation_of(&layout.database()).unwrap(), 1);
    }

    #[test]
    fn blobs_are_linked_or_copied_once() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a");
        fs::write(&source, b"content").unwrap();
        let target = dir.path().join("b").join("a");
        assert_ne!(copy_blob(&source, &target).unwrap(), BlobCopy::Present);
        assert_eq!(copy_blob(&source, &target).unwrap(), BlobCopy::Present);
        assert_eq!(fs::read(&target).unwrap(), b"content");
    }

    #[test]
    fn a_blob_that_cant_be_linked_is_copied_and_synced() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a");
        fs::write(&source, b"content").unwrap();
        // A read-only source still gives a copy that can be flushed and
        // replaced.
        let mut readonly = fs::metadata(&source).unwrap().permissions();
        readonly.set_readonly(true);
        fs::set_permissions(&source, readonly).unwrap();
        let target = dir.path().join("b").join("a");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        copy_synced(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"content");
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, [std::ffi::OsString::from("a")]);
    }
}
