// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Errors inside the engine, and how they reach callers as [`OpError`].

use gk_api::{ErrorCode, OpError};
use gk_store::{StoreError, WriteError};

use crate::messages;

/// An operation failed, either by its own rules or in storage. Inside a
/// write transaction the writer must tell lock conflicts apart, so storage
/// errors are kept as they are until the operation returns.
#[derive(Debug)]
pub enum Failure {
    /// Refused by the operation's rules.
    Op(OpError),
    /// Storage failed.
    Store(StoreError),
}

impl From<OpError> for Failure {
    fn from(e: OpError) -> Self {
        Self::Op(e)
    }
}

impl From<StoreError> for Failure {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<gk_store::rusqlite::Error> for Failure {
    fn from(e: gk_store::rusqlite::Error) -> Self {
        Self::Store(StoreError::from(e))
    }
}

impl WriteError for Failure {
    fn is_busy(&self) -> bool {
        matches!(self, Self::Store(e) if e.is_busy())
    }
}

impl From<Failure> for OpError {
    fn from(f: Failure) -> Self {
        match f {
            Failure::Op(e) => e,
            Failure::Store(e) => store_error(&e),
        }
    }
}

/// The [`OpError`] for a storage error, with its message and hint from the
/// message file.
#[must_use]
pub fn store_error(e: &StoreError) -> OpError {
    let path = |p: &std::path::Path| ("path", p.display().to_string());
    match e {
        StoreError::Busy => messages::hint(
            messages::error(ErrorCode::Busy, "storage-busy", &[]),
            "storage-busy-hint",
            &[],
        ),
        StoreError::WorkspaceUpgraded { found, expected } => messages::hint(
            messages::error(
                ErrorCode::WorkspaceUpgraded,
                "storage-upgraded",
                &[
                    ("found", found.to_string()),
                    ("expected", expected.to_string()),
                ],
            ),
            "storage-upgraded-hint",
            &[],
        )
        .with_detail("found", *found)
        .with_detail("expected", *expected),
        StoreError::WorkspaceReplaced => messages::hint(
            messages::error(ErrorCode::WorkspaceUpgraded, "storage-replaced", &[]),
            "storage-replaced-hint",
            &[],
        )
        .with_detail("restored", true),
        StoreError::TooNew { found, supported } => messages::hint(
            messages::error(
                ErrorCode::WorkspaceTooNew,
                "storage-too-new",
                &[
                    ("found", found.to_string()),
                    ("supported", supported.to_string()),
                ],
            ),
            "storage-too-new-hint",
            &[],
        )
        .with_detail("found", *found)
        .with_detail("supported", *supported),
        StoreError::NeedsUpgrade { found, current } => messages::hint(
            messages::storage(
                "storage-needs-upgrade",
                &[
                    ("found", found.to_string()),
                    ("current", current.to_string()),
                ],
            ),
            "storage-needs-upgrade-hint",
            &[],
        )
        .with_detail("found", *found)
        .with_detail("current", *current),
        StoreError::InvalidName(name) => messages::error(
            ErrorCode::Invalid,
            "storage-invalid-name",
            &[("name", name.clone())],
        ),
        StoreError::NotAWorkspace(p) => messages::storage("storage-not-a-workspace", &[path(p)]),
        StoreError::NoWal(p) => messages::storage("storage-no-wal", &[path(p)]),
        StoreError::NoDataDir => messages::storage("storage-no-data-dir", &[]),
        StoreError::Closed => messages::storage("storage-closed", &[]),
        StoreError::Migration { version, source } => messages::storage(
            "storage-migration",
            &[
                ("version", version.to_string()),
                ("error", source.to_string()),
            ],
        ),
        StoreError::Corrupt(detail) => {
            messages::storage("storage-corrupt", &[("error", detail.clone())])
        }
        StoreError::Io {
            path: p, source, ..
        } => messages::storage("storage-io", &[path(p), ("error", source.to_string())]),
        StoreError::Sqlite(source) => {
            messages::storage("storage-sqlite", &[("error", source.to_string())])
        }
    }
}
