// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The workspace folder and its lock files.
//!
//! ```text
//! <data>/workspaces/<name>/
//!   workspace.db  workspace.db-wal  workspace.db-shm
//!   files/ab/abcdef…     stored files, by SHA-256
//!   cache/thumbs/…       rebuildable
//!   locks/app.lock  jobs.lock  migrate.lock
//!   exports/             default target of an export
//! ```
//!
//! Locks are advisory operating-system locks on the files in `locks/`: they
//! disappear when the process that held them dies.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::{Result, StoreError};

/// File name of the database inside a workspace folder.
pub const DATABASE_FILE: &str = "workspace.db";

/// Paths inside one workspace folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLayout {
    root: PathBuf,
}

impl WorkspaceLayout {
    /// The layout of the workspace folder at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The workspace called `name` under the data folder: on Linux
    /// `data_dir` is `$XDG_DATA_HOME/glyphkeel` and the result
    /// `<data_dir>/workspaces/<name>`. The name must be usable as a folder
    /// name on every platform.
    pub fn in_data_dir(data_dir: &Path, name: &str) -> Result<Self> {
        check_name(name)?;
        Ok(Self::new(data_dir.join("workspaces").join(name)))
    }

    /// The workspace folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The database file.
    #[must_use]
    pub fn database(&self) -> PathBuf {
        self.root.join(DATABASE_FILE)
    }

    /// The write-ahead log next to the database.
    #[must_use]
    pub fn wal(&self) -> PathBuf {
        self.root.join(format!("{DATABASE_FILE}-wal"))
    }

    /// The folder of stored files.
    #[must_use]
    pub fn files_dir(&self) -> PathBuf {
        self.root.join("files")
    }

    /// Where the file with this SHA-256 (64 lower-case hexadecimal digits)
    /// is stored: `files/<first two digits>/<sha256>`. `None` for anything
    /// that is not such a hash, so a crafted value can never name a path
    /// outside the folder.
    #[must_use]
    pub fn blob_path(&self, sha256: &str) -> Option<PathBuf> {
        let valid = sha256.len() == 64
            && sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        valid.then(|| self.files_dir().join(&sha256[..2]).join(sha256))
    }

    /// The rebuildable cache.
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Thumbnails, inside the cache.
    #[must_use]
    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache_dir().join("thumbs")
    }

    /// The folder of lock files.
    #[must_use]
    pub fn locks_dir(&self) -> PathBuf {
        self.root.join("locks")
    }

    /// Held by the desktop app: one app per workspace; reminders and timers.
    #[must_use]
    pub fn app_lock(&self) -> PathBuf {
        self.locks_dir().join("app.lock")
    }

    /// Held by the process that runs background jobs, including the WAL
    /// checkpoints.
    #[must_use]
    pub fn jobs_lock(&self) -> PathBuf {
        self.locks_dir().join("jobs.lock")
    }

    /// Held briefly while the schema is created or upgraded.
    #[must_use]
    pub fn migrate_lock(&self) -> PathBuf {
        self.locks_dir().join("migrate.lock")
    }

    /// Default target of exports.
    #[must_use]
    pub fn exports_dir(&self) -> PathBuf {
        self.root.join("exports")
    }

    /// Creates the folder and its sub-folders where they are missing.
    pub fn create_dirs(&self) -> Result<()> {
        for dir in [
            self.root.clone(),
            self.files_dir(),
            self.thumbs_dir(),
            self.locks_dir(),
            self.exports_dir(),
        ] {
            fs::create_dir_all(&dir).map_err(|e| StoreError::io("creating", dir, e))?;
        }
        Ok(())
    }
}

fn check_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.chars().count() <= 64
        && !name.starts_with(['.', ' '])
        && !name.ends_with(['.', ' '])
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(StoreError::InvalidName(name.to_owned()))
    }
}

/// An exclusive advisory lock on a file, released when dropped or when the
/// process ends, however it ends.
#[derive(Debug)]
pub struct LockFile {
    file: File,
    path: PathBuf,
}

impl LockFile {
    /// Takes the lock if nobody holds it; `None` if another process (or
    /// another `LockFile` in this one) does.
    pub fn try_acquire(path: &Path) -> Result<Option<Self>> {
        let file = open_lock_file(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self {
                file,
                path: path.to_owned(),
            })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(StoreError::io("locking", path, e)),
        }
    }

    /// Waits until the lock is free and takes it.
    pub fn acquire(path: &Path) -> Result<Self> {
        let file = open_lock_file(path)?;
        file.lock()
            .map_err(|e| StoreError::io("locking", path, e))?;
        Ok(Self {
            file,
            path: path.to_owned(),
        })
    }

    /// The lock file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // Closing the file releases the lock too; unlocking first makes the
        // hand-over immediate even if the handle were shared.
        let _ = self.file.unlock();
    }
}

fn open_lock_file(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| StoreError::io("creating", parent, e))?;
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| StoreError::io("opening", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let layout =
            WorkspaceLayout::in_data_dir(Path::new("/data/glyphkeel"), "Personal").unwrap();
        assert_eq!(
            layout.root(),
            Path::new("/data/glyphkeel/workspaces/Personal")
        );
        assert_eq!(
            layout.database(),
            Path::new("/data/glyphkeel/workspaces/Personal/workspace.db")
        );
        assert_eq!(
            layout.jobs_lock(),
            Path::new("/data/glyphkeel/workspaces/Personal/locks/jobs.lock")
        );
        let sha = "ab".repeat(32);
        assert_eq!(
            layout.blob_path(&sha).unwrap(),
            layout.files_dir().join("ab").join(&sha)
        );
    }

    #[test]
    fn blob_paths_only_for_hashes() {
        let layout = WorkspaceLayout::new("/w");
        assert!(layout.blob_path("../../etc/passwd").is_none());
        assert!(layout.blob_path(&"AB".repeat(32)).is_none());
        assert!(layout.blob_path(&"ab".repeat(31)).is_none());
    }

    #[test]
    fn workspace_names() {
        for good in ["Personal", "work-2026", "Мой проект", "a_b c"] {
            assert!(check_name(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a\\b",
            "trailing.",
            " lead",
            "a:b",
            "x\u{0}",
        ] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn locks_are_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locks/jobs.lock");
        let first = LockFile::try_acquire(&path).unwrap().unwrap();
        assert!(LockFile::try_acquire(&path).unwrap().is_none());
        drop(first);
        assert!(LockFile::try_acquire(&path).unwrap().is_some());
    }
}
