// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Opening a connection: the settings every connection gets.
//!
//! Several processes (the app, the command line, one agent server per agent)
//! open the same database file. That works because of the write-ahead log
//! and these settings:
//!
//! - `synchronous=FULL`: a committed change survives a power cut.
//! - Our own busy handler: when another process holds the write lock, sleep
//!   0.5–1 ms with jitter and try again, up to a total budget (5 s by
//!   default). SQLite's built-in `busy_timeout` backs off to long sleeps,
//!   which made waiting writers in other processes unfair to each other.
//! - `journal_size_limit` of 32 MiB: the WAL file left after a checkpoint
//!   is cut back to that size.
//! - `foreign_keys=ON`, `temp_store=MEMORY`.
//! - The `gk_unicode` collation (see [`COLLATION`]).
//!
//! - The key, for an encrypted workspace (see [`crate::cipher`]).
//!
//! The automatic checkpoint stays at SQLite's default.

use std::cmp::Ordering;
use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use icu_collator::options::CollatorOptions;
use icu_collator::{CollatorBorrowed, CollatorPreferences};
use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags};

use crate::error::Result;

/// Name of the collation that sorts text by the Unicode Collation Algorithm
/// (root order), registered on every connection: `ORDER BY name COLLATE
/// gk_unicode`. Text that compares equal under the algorithm is ordered by
/// its bytes, so sorting is always the same.
pub const COLLATION: &str = "gk_unicode";

/// Size the WAL file is cut back to after a checkpoint, in bytes.
pub const JOURNAL_SIZE_LIMIT: i64 = 32 * 1024 * 1024;

/// Prepared statements each connection keeps for reuse. One command can
/// run a few dozen different statements per task it touches (a task's
/// row, its labels, search entry, history, rules); with the library's
/// default of 16 they push each other out and every call parses its SQL
/// again.
pub(crate) const STATEMENT_CACHE: usize = 256;

/// What a connection is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Purpose {
    /// The one connection that writes; may create the file.
    Write,
    /// Reads only (`query_only`), for the reader pool and the change watcher.
    Read,
    /// Runs checkpoints.
    Checkpoint,
}

static COLLATOR: LazyLock<Option<CollatorBorrowed<'static>>> = LazyLock::new(|| {
    CollatorBorrowed::try_new(CollatorPreferences::default(), CollatorOptions::default()).ok()
});

/// Compares two strings as the `gk_unicode` collation does.
#[must_use]
pub fn collate(a: &str, b: &str) -> Ordering {
    match COLLATOR.as_ref() {
        Some(collator) => collator.compare(a, b).then_with(|| a.cmp(b)),
        None => a.cmp(b),
    }
}

/// Opens `path` with the settings above. `busy_budget` is the longest the
/// busy handler waits for a lock in one call.
pub(crate) fn open(path: &Path, purpose: Purpose, busy_budget: Duration) -> Result<Connection> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if purpose == Purpose::Write {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let mut conn = Connection::open_with_flags(path, flags)?;
    // An encrypted file needs its key before any other statement.
    crate::cipher::prepare(&conn, path)?;
    // The busy handler first, so the settings below can wait for locks too.
    conn.busy_handler(Some(busy_handler(busy_budget)))?;
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = FULL;
         PRAGMA temp_store = MEMORY;
         PRAGMA journal_size_limit = {JOURNAL_SIZE_LIMIT};"
    ))?;
    if purpose == Purpose::Read {
        conn.execute_batch("PRAGMA query_only = ON;")?;
    }
    conn.create_collation(COLLATION, collate)?;
    conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
    // One plan per statement, whatever values are bound. Without it SQLite
    // compiles a cached statement again whenever a bound value might
    // change its plan (with the statistics this build keeps, nearly every
    // lookup by a bound ID), which costs more than running it. Queries
    // that need a partial index whose condition compares with a constant
    // write that constant into their SQL.
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_QPSG, true)?;
    Ok(conn)
}

/// A busy handler that sleeps 0.5–1 ms with jitter between attempts and
/// gives up once `budget` has passed since the first attempt.
fn busy_handler(budget: Duration) -> impl FnMut(i32) -> bool + Send + 'static {
    let mut started = Instant::now();
    let mut rng = Jitter::new();
    move |attempt| {
        if attempt == 0 {
            started = Instant::now();
        }
        let Some(left) = budget.checked_sub(started.elapsed()) else {
            return false;
        };
        if left.is_zero() {
            return false;
        }
        let sleep = Duration::from_micros(500 + rng.below(501));
        thread::sleep(sleep.min(left));
        true
    }
}

/// A tiny xorshift generator for sleep jitter; quality does not matter, only
/// that two processes do not retry in lockstep.
#[derive(Debug)]
pub(crate) struct Jitter(u64);

impl Jitter {
    pub(crate) fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let seed = nanos
            ^ u64::from(std::process::id()).rotate_left(32)
            ^ COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, AtomicOrdering::Relaxed);
        Self(seed | 1)
    }

    pub(crate) fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collation_is_unicode_aware_and_total() {
        let mut names = vec![
            "zebra",
            "Äpfel",
            "apple",
            "Zebra",
            "Ölung",
            "Яблоко",
            "ябло",
            "日本",
            "10",
            "9",
        ];
        names.sort_by(|a, b| collate(a, b));
        let pos = |s: &str| names.iter().position(|n| *n == s).unwrap();
        // Accents and case are secondary to the letters themselves.
        assert!(pos("Äpfel") < pos("apple"));
        assert!(pos("apple") < pos("Ölung"));
        assert!(pos("Ölung") < pos("zebra"));
        assert!(pos("zebra") < pos("Zebra"));
        assert!(pos("ябло") < pos("Яблоко"));
        // Equal under the algorithm, still a fixed order.
        assert_ne!(collate("e\u{301}", "\u{e9}"), Ordering::Equal);
        assert_eq!(collate("x", "x"), Ordering::Equal);
    }

    #[test]
    fn jitter_stays_in_range() {
        let mut j = Jitter::new();
        assert!((0..1000).all(|_| j.below(501) <= 500));
    }
}
