// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Storage: the SQLite schema and its migrations, connections (one writer and
//! a pool of readers), repositories, the full-text index and snapshots.
//!
//! # One workspace, several processes
//!
//! The app, the `glyphkeel` command and every `glyphkeel mcp` open the same
//! workspace database themselves; there is no server between them. Each
//! process opens one [`Store`]:
//!
//! - **One writer thread** owns the only write connection. Every command is
//!   one short `BEGIN IMMEDIATE` transaction ([`Store::write`]); a lock
//!   conflict with another process is waited out by a short-sleep busy
//!   handler and, failing that, retried whole.
//! - **A pool of readers** (4) serves queries, each in one read transaction
//!   ([`Store::read`]).
//! - **A change watcher** ([`Store::watch_changes`]) polls
//!   `PRAGMA data_version` and reports other processes' commits as history
//!   rows.
//! - **Checkpoints** ([`Store::checkpointer`], [`CheckpointSchedule`]) keep
//!   the write-ahead log small; only the holder of the jobs lock runs them.
//! - **Schema versions** are guarded on open and at the start of every write
//!   ([`SchemaState`], [`StoreError::WorkspaceUpgraded`]).
//!
//! # Example
//!
//! ```
//! use gk_store::{Store, StoreConfig, StoreError, WorkspaceLayout};
//!
//! # fn main() -> Result<(), StoreError> {
//! # let dir = tempfile::tempdir().unwrap();
//! let layout = WorkspaceLayout::new(dir.path().join("Personal"));
//! let store = Store::open(&layout, StoreConfig::default())?;
//! store.write("settings.set", |tx| {
//!     tx.execute(
//!         "INSERT INTO settings (key, value, updated_at) VALUES ('trash_days', '30', 0)",
//!         [],
//!     )?;
//!     Ok::<_, StoreError>(())
//! })?;
//! let days: String = store.read(|tx| {
//!     Ok::<_, StoreError>(tx.query_row(
//!         "SELECT value FROM settings WHERE key = 'trash_days'",
//!         [],
//!         |r| r.get(0),
//!     )?)
//! })?;
//! assert_eq!(days, "30");
//! # Ok(())
//! # }
//! ```

mod changes;
mod checkpoint;
mod connection;
mod error;
mod files;
mod layout;
mod readers;
mod requests;
mod schema;
mod store;
mod writer;

pub use changes::{
    ActivityRecord, ChangeBatch, ChangeWatcher, MAX_ROWS_PER_POLL, OwnGroups, WatcherHandle,
};
pub use checkpoint::{
    CHECKPOINT_BUSY_BUDGET, CheckpointOutcome, CheckpointPolicy, CheckpointSchedule, Checkpointer,
};
pub use connection::{COLLATION, JOURNAL_SIZE_LIMIT, collate};
pub use error::{Result, StoreError, WriteError};
pub use files::{StoredFile, file_path, has_file, read_file, remove_file, sha256_hex, store_file};
pub use layout::{
    DATABASE_FILE, DEFAULT_WORKSPACE, Location, LockFile, WORKSPACE_ENV, WorkspaceLayout, data_dir,
};
pub use requests::{REQUEST_WINDOW_MS, RequestKey, purge_requests, remember, remembered};
pub use schema::{SCHEMA_VERSION, SchemaState};
pub use store::{Store, StoreConfig, open_raw};
pub use writer::{SlowWrite, WriterStats};

/// The connection type handed to read and write closures.
pub use rusqlite;
