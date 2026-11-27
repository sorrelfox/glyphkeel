// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The storage error type.

use std::path::PathBuf;

use rusqlite::ErrorCode;

/// Something went wrong in the storage layer. The engine maps these to the
/// error codes that users and agents see.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Another program held the write lock for longer than the busy budget,
    /// even after the transaction was retried.
    #[error("the workspace is busy: another program kept it locked; try again")]
    Busy,
    /// A newer program upgraded the schema while this one was running.
    #[error(
        "the workspace was upgraded to schema version {found} while this program (schema version {expected}) was running; restart it"
    )]
    WorkspaceUpgraded {
        /// The schema version now in the file.
        found: i64,
        /// The version this program was built for.
        expected: i64,
    },
    /// The database was replaced by a backup while this program was
    /// running.
    #[error("the workspace was restored from a backup while this program was running; restart it")]
    WorkspaceReplaced,
    /// The workspace was made by a newer program.
    #[error(
        "the workspace has schema version {found}, but this program supports only up to {supported}; install a newer Glyphkeel"
    )]
    TooNew {
        /// The schema version in the file.
        found: i64,
        /// The newest version this program knows.
        supported: i64,
    },
    /// The workspace has an older schema and opening was not allowed to
    /// upgrade it.
    #[error("the workspace has schema version {found} and must be upgraded to {current} first")]
    NeedsUpgrade {
        /// The schema version in the file.
        found: i64,
        /// The version this program needs.
        current: i64,
    },
    /// The database file belongs to something else.
    #[error("{0} is not a Glyphkeel workspace database")]
    NotAWorkspace(PathBuf),
    /// The file system does not support the write-ahead log (for example a
    /// network share).
    #[error("{0} is on a file system that does not support the write-ahead log; use a local disk")]
    NoWal(PathBuf),
    /// A workspace name that cannot be a folder name.
    #[error(
        "\"{0}\" cannot be a workspace name: use letters, digits, spaces, '-' or '_', at most 64 characters"
    )]
    InvalidName(String),
    /// A workspace was named, but there is no data folder to find it in
    /// (no home folder).
    #[error("there is no data folder to find the workspace in: no home folder is set")]
    NoDataDir,
    /// The store was closed, or its writer thread stopped.
    #[error("the workspace is closed")]
    Closed,
    /// A schema migration failed; nothing of it was kept.
    #[error("upgrading the workspace to schema version {version} failed: {source}")]
    Migration {
        /// The version that could not be reached.
        version: i64,
        /// What SQLite reported.
        #[source]
        source: rusqlite::Error,
    },
    /// Stored data that does not have the expected shape.
    #[error("the workspace database holds invalid data: {0}")]
    Corrupt(String),
    /// A file-system operation failed.
    #[error("{action} {path}: {source}")]
    Io {
        /// What was being done ("creating", "locking", …).
        action: &'static str,
        /// The file or folder.
        path: PathBuf,
        /// The operating system's error.
        #[source]
        source: std::io::Error,
    },
    /// The workspace is encrypted and no passphrase was given for it.
    #[error("{0} is encrypted: give its passphrase to open it")]
    PassphraseRequired(PathBuf),
    /// The workspace is encrypted and none of the passphrases given opens
    /// it.
    #[error("the passphrase does not open {0}")]
    WrongPassphrase(PathBuf),
    /// The workspace is encrypted, or should be, and this build of the
    /// program has no encryption.
    #[error("this build of Glyphkeel cannot open or make encrypted workspaces")]
    EncryptionUnavailable,
    /// The operation needs the workspace to itself, and another program has
    /// it open.
    #[error("another program has {0} open; close it and try again")]
    InUse(PathBuf),
    /// Any other SQLite error.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

impl StoreError {
    /// Whether this is a lock conflict with another connection: either
    /// [`StoreError::Busy`] or SQLite's `SQLITE_BUSY` / `SQLITE_LOCKED`.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        match self {
            Self::Busy => true,
            Self::Sqlite(rusqlite::Error::SqliteFailure(e, _)) => {
                matches!(e.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
            }
            _ => false,
        }
    }

    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// The error type of a write closure passed to
/// [`Store::write`](crate::Store::write).
///
/// The writer retries a whole transaction when it fails because another
/// program holds the lock, so it must be able to recognise that case in the
/// caller's own error type. [`StoreError`] implements it; the engine's error
/// type implements it by delegating to the `StoreError` it wraps.
pub trait WriteError: From<StoreError> + Send + 'static {
    /// Whether the error is a lock conflict worth retrying.
    fn is_busy(&self) -> bool;
}

impl WriteError for StoreError {
    fn is_busy(&self) -> bool {
        StoreError::is_busy(self)
    }
}

/// Result type of this crate.
pub type Result<T, E = StoreError> = std::result::Result<T, E>;
