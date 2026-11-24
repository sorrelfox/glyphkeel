// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Errors inside the engine, and how they reach callers as [`OpError`].

use gk_api::{ErrorCode, OpError};
use gk_store::{StoreError, WriteError};

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

/// The [`OpError`] for a storage error.
#[must_use]
pub fn store_error(e: &StoreError) -> OpError {
    match e {
        StoreError::Busy => OpError::new(ErrorCode::Busy, e.to_string())
            .with_hint("Another program is writing to the workspace; try again in a moment."),
        StoreError::WorkspaceUpgraded { found, expected } => {
            OpError::new(ErrorCode::WorkspaceUpgraded, e.to_string())
                .with_hint("Restart the program; a newer version is now needed.")
                .with_detail("found", *found)
                .with_detail("expected", *expected)
        }
        StoreError::WorkspaceReplaced => OpError::new(ErrorCode::WorkspaceUpgraded, e.to_string())
            .with_hint("Restart the program; the workspace was restored from a backup.")
            .with_detail("restored", true),
        StoreError::TooNew { found, supported } => {
            OpError::new(ErrorCode::WorkspaceTooNew, e.to_string())
                .with_hint(
                    "Install the Glyphkeel version that made this workspace, or a newer one.",
                )
                .with_detail("found", *found)
                .with_detail("supported", *supported)
        }
        StoreError::NeedsUpgrade { found, current } => OpError::storage(e.to_string())
            .with_hint("Open the app once to upgrade this workspace, or run `glyphkeel migrate`.")
            .with_detail("found", *found)
            .with_detail("current", *current),
        StoreError::InvalidName(_) => OpError::invalid(e.to_string()),
        _ => OpError::storage(e.to_string()),
    }
}
