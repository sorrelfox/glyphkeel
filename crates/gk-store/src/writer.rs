// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The writer thread: the process's one write connection.
//!
//! Commands arrive on a channel and run one at a time, each in a
//! `BEGIN IMMEDIATE` transaction, which takes the write lock up front so two
//! processes never deadlock upgrading a read lock. After taking the lock the
//! writer checks the schema version, so a process that a newer program
//! upgraded underneath stops writing.
//!
//! Write transactions must be short: no file I/O, waiting or slow work
//! between `BEGIN` and `COMMIT`. A transaction that holds the lock longer
//! than [`StoreConfig::slow_write`](crate::StoreConfig) is logged and counted.
//! One that fails because another program kept the lock past the busy
//! handler's budget is retried whole a few times before the caller gets
//! [`StoreError::Busy`].

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::connection::Jitter;
use crate::error::{Result, StoreError, WriteError};
use crate::schema;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Counters about the writer, for diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriterStats {
    /// Committed transactions.
    pub commits: u64,
    /// Transactions retried because of a lock conflict.
    pub busy_retries: u64,
    /// Transactions given up because of lock conflicts.
    pub busy_failures: u64,
    /// Committed transactions that held the lock longer than the limit.
    pub slow_commits: u64,
    /// The longest time any committed transaction held the lock.
    pub longest_hold: Duration,
    /// The most recent slow transaction.
    pub last_slow: Option<SlowWrite>,
    /// When the last transaction committed.
    pub last_commit_at: Option<Instant>,
}

/// A write transaction that held the lock too long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlowWrite {
    /// The operation's name, as passed to [`Store::write`](crate::Store::write).
    pub op: &'static str,
    /// How long it held the write lock.
    pub held: Duration,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WriteSettings {
    pub(crate) schema_version: i64,
    /// The database's generation when it was opened; a restore from a
    /// backup raises it.
    pub(crate) generation: i64,
    pub(crate) busy_retries: u32,
    pub(crate) slow_write: Duration,
}

#[derive(Debug)]
pub(crate) struct Writer {
    sender: Option<mpsc::Sender<Job>>,
    thread: Option<JoinHandle<()>>,
    stats: Arc<Mutex<WriterStats>>,
    settings: WriteSettings,
}

impl Writer {
    /// Starts the writer thread with `conn`. `on_exit` runs on the thread
    /// with the connection when the writer stops.
    pub(crate) fn start(
        mut conn: Connection,
        settings: WriteSettings,
        on_exit: impl FnOnce(&mut Connection) + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let thread = thread::Builder::new()
            .name("gk-store-writer".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    job(&mut conn);
                }
                on_exit(&mut conn);
            })
            .map_err(|e| StoreError::io("starting the writer thread for", "", e))?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
            stats: Arc::new(Mutex::new(WriterStats::default())),
            settings,
        })
    }

    pub(crate) fn stats(&self) -> WriterStats {
        self.stats
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn write<R, E, F>(&self, op: &'static str, f: F) -> Result<R, E>
    where
        R: Send + 'static,
        E: WriteError,
        F: FnMut(&Transaction<'_>) -> Result<R, E> + Send + 'static,
    {
        let sender = self.sender.as_ref().ok_or(StoreError::Closed)?;
        let (reply, response) = mpsc::sync_channel(1);
        let settings = self.settings;
        let stats = Arc::clone(&self.stats);
        let job: Job = Box::new(move |conn| {
            // A panic in `f` unwinds through the transaction, which rolls it
            // back, and is passed on to the caller; the writer keeps running.
            let result =
                panic::catch_unwind(AssertUnwindSafe(|| run(conn, op, f, settings, &stats)));
            let _ = reply.send(result);
        });
        sender.send(job).map_err(|_| StoreError::Closed)?;
        match response.recv().map_err(|_| StoreError::Closed)? {
            Ok(result) => result,
            Err(payload) => panic::resume_unwind(payload),
        }
    }

    /// Stops the thread after the queued commands and waits for it.
    pub(crate) fn stop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run<R, E, F>(
    conn: &mut Connection,
    op: &'static str,
    mut f: F,
    settings: WriteSettings,
    stats: &Mutex<WriterStats>,
) -> Result<R, E>
where
    E: WriteError,
    F: FnMut(&Transaction<'_>) -> Result<R, E>,
{
    let mut retries = 0;
    let mut jitter = Jitter::new();
    loop {
        match attempt(conn, op, &mut f, settings, stats) {
            Err(e) if e.is_busy() => {
                let mut s = stats.lock().unwrap_or_else(PoisonError::into_inner);
                if retries >= settings.busy_retries {
                    s.busy_failures += 1;
                    tracing::warn!(
                        op,
                        retries,
                        "write transaction gave up: the workspace stayed locked"
                    );
                    return Err(E::from(StoreError::Busy));
                }
                s.busy_retries += 1;
                drop(s);
                retries += 1;
                thread::sleep(Duration::from_micros(1000 + jitter.below(4000)));
            }
            other => return other,
        }
    }
}

fn attempt<R, E, F>(
    conn: &mut Connection,
    op: &'static str,
    f: &mut F,
    settings: WriteSettings,
    stats: &Mutex<WriterStats>,
) -> Result<R, E>
where
    E: WriteError,
    F: FnMut(&Transaction<'_>) -> Result<R, E>,
{
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(StoreError::from)?;
    let locked_at = Instant::now();
    let found = schema::user_version(&tx)?;
    if found != settings.schema_version {
        return Err(E::from(StoreError::WorkspaceUpgraded {
            found,
            expected: settings.schema_version,
        }));
    }
    if crate::snapshot::generation(&tx)? != settings.generation {
        return Err(E::from(StoreError::WorkspaceReplaced));
    }
    // Dropping `tx` on an error rolls it back.
    let value = f(&tx)?;
    tx.commit().map_err(StoreError::from)?;
    let held = locked_at.elapsed();

    let mut s = stats.lock().unwrap_or_else(PoisonError::into_inner);
    s.commits += 1;
    s.longest_hold = s.longest_hold.max(held);
    s.last_commit_at = Some(Instant::now());
    if held > settings.slow_write {
        s.slow_commits += 1;
        s.last_slow = Some(SlowWrite { op, held });
        tracing::warn!(
            op,
            held_ms = held.as_secs_f64() * 1e3,
            "write transaction held the workspace lock too long"
        );
    }
    Ok(value)
}
