// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Keeping the write-ahead log small.
//!
//! SQLite's automatic checkpoint copies the log back into the database but
//! never shrinks the log file, and while readers overlap it cannot restart
//! the log at all: under load the file grew past 130 MB in seconds. So the
//! process that holds `locks/jobs.lock` also runs `wal_checkpoint(TRUNCATE)`
//! on its own connection:
//!
//! - when the log passes a size threshold (checked every few seconds),
//!   after a quiet period, and when the process closes;
//! - with a short busy budget (150 ms): a `TRUNCATE` checkpoint holds the
//!   write lock while it waits for readers, so a long budget would stall
//!   every writer in every process;
//! - if it cannot finish (a reader is still on an old snapshot), it gives up
//!   and tries again a little later.
//!
//! [`CheckpointSchedule`] implements that timing; the caller only has to
//! call [`CheckpointSchedule::tick`] regularly while it holds the lock.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::connection::{self, Purpose};
use crate::error::{Result, StoreError};

/// The busy budget of checkpoint connections.
pub const CHECKPOINT_BUSY_BUDGET: Duration = Duration::from_millis(150);

/// What a checkpoint achieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointOutcome {
    /// The log was copied into the database and truncated to zero bytes.
    Truncated {
        /// Pages that were in the log.
        log_frames: i64,
    },
    /// Another connection was in the way; nothing to worry about, try again
    /// later.
    Busy,
}

/// A connection that runs checkpoints. Open one with
/// [`Store::checkpointer`](crate::Store::checkpointer), only in the process
/// that holds the jobs lock.
#[derive(Debug)]
pub struct Checkpointer {
    conn: Connection,
    wal: PathBuf,
}

impl Checkpointer {
    pub(crate) fn open(database: &Path, wal: PathBuf) -> Result<Self> {
        let conn = connection::open(database, Purpose::Checkpoint, CHECKPOINT_BUSY_BUDGET)?;
        Ok(Self { conn, wal })
    }

    /// The size of the log file in bytes (0 when there is none).
    #[must_use]
    pub fn wal_bytes(&self) -> u64 {
        fs::metadata(&self.wal).map_or(0, |m| m.len())
    }

    /// Runs `wal_checkpoint(TRUNCATE)` once.
    pub fn truncate(&self) -> Result<CheckpointOutcome> {
        let result = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            });
        match result {
            Ok((0, log_frames)) => Ok(CheckpointOutcome::Truncated { log_frames }),
            Ok(_) => Ok(CheckpointOutcome::Busy),
            Err(e) => {
                let e = StoreError::from(e);
                if e.is_busy() {
                    Ok(CheckpointOutcome::Busy)
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// When to checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointPolicy {
    /// Checkpoint once the log is larger than this.
    pub threshold_bytes: u64,
    /// How often to look at the log size.
    pub check_every: Duration,
    /// Checkpoint a non-empty log once no write has committed for this long.
    pub idle_after: Duration,
    /// How soon to try again after [`CheckpointOutcome::Busy`].
    pub retry_after: Duration,
}

impl Default for CheckpointPolicy {
    /// 24 MiB, checked every 5 s; when idle for 30 s; retry after 1 s.
    fn default() -> Self {
        Self {
            threshold_bytes: 24 * 1024 * 1024,
            check_every: Duration::from_secs(5),
            idle_after: Duration::from_secs(30),
            retry_after: Duration::from_secs(1),
        }
    }
}

impl CheckpointPolicy {
    /// Whether a log of `wal_bytes` should be checkpointed now, when the
    /// last write committed `idle_for` ago.
    #[must_use]
    pub fn is_due(&self, wal_bytes: u64, idle_for: Duration) -> bool {
        wal_bytes > self.threshold_bytes || (wal_bytes > 0 && idle_for >= self.idle_after)
    }
}

/// Applies a [`CheckpointPolicy`] over time.
#[derive(Debug, Clone)]
pub struct CheckpointSchedule {
    policy: CheckpointPolicy,
    next_check: Instant,
}

impl CheckpointSchedule {
    /// A schedule whose first check is due at once.
    #[must_use]
    pub fn new(policy: CheckpointPolicy) -> Self {
        Self {
            policy,
            next_check: Instant::now(),
        }
    }

    /// Checks the log if a check is due at `now` and checkpoints when the
    /// policy says so. `idle_for` is the time since the last write commit
    /// that this process knows of. Returns what a checkpoint did, or `None`
    /// when none ran.
    pub fn tick(
        &mut self,
        now: Instant,
        idle_for: Duration,
        checkpointer: &Checkpointer,
    ) -> Result<Option<CheckpointOutcome>> {
        if now < self.next_check {
            return Ok(None);
        }
        self.next_check = now + self.policy.check_every;
        if !self.policy.is_due(checkpointer.wal_bytes(), idle_for) {
            return Ok(None);
        }
        let outcome = checkpointer.truncate()?;
        if outcome == CheckpointOutcome::Busy {
            self.next_check = now + self.policy.retry_after;
            tracing::debug!("checkpoint postponed: a reader is still on an old snapshot");
        }
        Ok(Some(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy() {
        let p = CheckpointPolicy::default();
        assert!(!p.is_due(0, Duration::from_secs(3600)));
        assert!(!p.is_due(1024, Duration::from_secs(1)));
        assert!(p.is_due(1024, Duration::from_secs(30)));
        assert!(p.is_due(25 * 1024 * 1024, Duration::ZERO));
    }
}
