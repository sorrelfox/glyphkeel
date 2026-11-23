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
//!
//! # Which workspace
//!
//! The app, the command line and the agent server find the workspace the
//! same way ([`Location::find`]): the one they are told (`--workspace`),
//! else the one `GLYPHKEEL_WORKSPACE` names, else the default workspace
//! [`DEFAULT_WORKSPACE`] in the data folder ([`data_dir`]). A value with a
//! path separator, or one that starts with `.` or `~`, is a folder; any
//! other value is a name in the data folder.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::{Result, StoreError};

/// File name of the database inside a workspace folder.
pub const DATABASE_FILE: &str = "workspace.db";

/// The name of the workspace opened when nothing else is asked for. It is
/// created on first use.
pub const DEFAULT_WORKSPACE: &str = "Personal";

/// The environment variable that names another workspace than the default
/// one, by name or by path.
pub const WORKSPACE_ENV: &str = "GLYPHKEEL_WORKSPACE";

/// The program's data folder: `$XDG_DATA_HOME/glyphkeel` (normally
/// `~/.local/share/glyphkeel`) on Linux, `%LOCALAPPDATA%\Glyphkeel` on
/// Windows. `None` when no home folder can be found.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    let name = if cfg!(windows) {
        "Glyphkeel"
    } else {
        "glyphkeel"
    };
    Some(base.data_local_dir().join(name))
}

/// Where a workspace is: a name in the data folder or a folder anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A workspace in the data folder, by name.
    Named(String),
    /// A workspace folder anywhere.
    Path(PathBuf),
}

impl Location {
    /// The workspace to open: `arg` when given (the program's
    /// `--workspace`), else the value of [`WORKSPACE_ENV`], else the
    /// default workspace. Empty values count as not given.
    #[must_use]
    pub fn find(arg: Option<&str>) -> Self {
        let env = std::env::var(WORKSPACE_ENV).ok();
        Self::choose(arg, env.as_deref())
    }

    /// As [`Location::find`], with the environment's value passed in.
    #[must_use]
    pub fn choose(arg: Option<&str>, env: Option<&str>) -> Self {
        fn given(v: Option<&str>) -> Option<&str> {
            v.map(str::trim).filter(|v| !v.is_empty())
        }
        given(arg)
            .or_else(|| given(env))
            .map_or_else(|| Self::Named(DEFAULT_WORKSPACE.to_owned()), Self::parse)
    }

    /// A folder when `value` has a path separator, starts with `.` or `~`,
    /// or is absolute; otherwise a name in the data folder.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let is_path = value.contains('/')
            || value.contains(std::path::MAIN_SEPARATOR)
            || value.starts_with(['.', '~'])
            || Path::new(value).is_absolute();
        if is_path {
            Self::Path(PathBuf::from(value))
        } else {
            Self::Named(value.to_owned())
        }
    }

    /// Whether this is the default workspace by name.
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self, Self::Named(name) if name == DEFAULT_WORKSPACE)
    }

    /// The workspace folder's layout. A named workspace lives under
    /// `data_dir` (normally [`data_dir`]); a leading `~` of a path is the
    /// home folder.
    ///
    /// # Errors
    /// [`StoreError::InvalidName`] for a name that can't be a folder name,
    /// [`StoreError::NoDataDir`] for a name without a data folder.
    pub fn layout(&self, data_dir: Option<&Path>) -> Result<WorkspaceLayout> {
        match self {
            Self::Path(path) => Ok(WorkspaceLayout::new(expand_home(path))),
            Self::Named(name) => {
                let data_dir = data_dir.ok_or(StoreError::NoDataDir)?;
                WorkspaceLayout::in_data_dir(data_dir, name)
            }
        }
    }
}

fn expand_home(path: &Path) -> PathBuf {
    let home = || directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
    if path == Path::new("~") {
        return home().unwrap_or_else(|| path.to_path_buf());
    }
    match path.strip_prefix("~") {
        Ok(rest) => home().map_or_else(|| path.to_path_buf(), |h| h.join(rest)),
        Err(_) => path.to_path_buf(),
    }
}

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

    /// Copies of the database made before a schema upgrade.
    #[must_use]
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
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
    fn the_workspace_is_the_argument_then_the_environment_then_personal() {
        assert_eq!(
            Location::choose(None, None),
            Location::Named("Personal".to_owned())
        );
        assert!(Location::choose(None, Some("  ")).is_default());
        assert_eq!(
            Location::choose(None, Some("Work")),
            Location::Named("Work".to_owned())
        );
        assert_eq!(
            Location::choose(Some("Side"), Some("Work")),
            Location::Named("Side".to_owned())
        );
        assert_eq!(
            Location::choose(Some("./ws"), None),
            Location::Path(PathBuf::from("./ws"))
        );
        assert_eq!(
            Location::choose(None, Some("/tmp/ws")),
            Location::Path(PathBuf::from("/tmp/ws"))
        );
        assert_eq!(Location::parse("."), Location::Path(PathBuf::from(".")));
        assert_eq!(
            Location::parse("~/ws"),
            Location::Path(PathBuf::from("~/ws"))
        );
    }

    #[test]
    fn named_workspaces_live_under_the_data_folder() {
        let data = Path::new("/data/glyphkeel");
        let layout = Location::choose(None, None).layout(Some(data)).unwrap();
        assert_eq!(
            layout.root(),
            Path::new("/data/glyphkeel/workspaces/Personal")
        );
        assert!(matches!(
            Location::choose(None, None).layout(None),
            Err(StoreError::NoDataDir)
        ));
        assert!(matches!(
            Location::Named("a:b".to_owned()).layout(Some(data)),
            Err(StoreError::InvalidName(_))
        ));
        let path = Location::parse("/tmp/ws").layout(None).unwrap();
        assert_eq!(path.root(), Path::new("/tmp/ws"));
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
