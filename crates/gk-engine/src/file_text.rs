// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The text inside attached files, for search.
//!
//! Every stored file content (a blob) whose text has not been read by the
//! current extraction ([`gk_extract::VERSION`]) waits in `file_texts`'s
//! absence. The process holding the jobs lock reads waiting files a few at
//! a time in its background turns ([`Engine::index_files`]), newest first,
//! so a file just attached is searchable within seconds; the command line,
//! which has no background turns, reads the file it attached before it
//! exits. Files attached before the text was read at all are waiting the
//! same way, so the first start after the upgrade reads them all, and
//! `file.reindex` makes every file wait again. Either can stop at any
//! moment and go on later: what has been read is kept.
//!
//! Extraction runs on a thread of its own ([`gk_extract::spawn`]) within
//! the limits of [`gk_extract::Limits`]; a turn never waits for it longer
//! than its budget. The readers of text and Office files watch the time
//! limit themselves and stop; the PDF reader can't be interrupted inside
//! a page, so a PDF still running a moment past the limit is left to
//! finish on its own (a thread can't be stopped from outside). Only one
//! such thread may be left at a time: while it runs, no other PDF is
//! started and PDFs keep waiting. A file that ran out of time is recorded
//! as `timed-out` and not tried again until `file.reindex`.
//!
//! The text is written to `file_texts` and to the search document of every
//! live attachment of that content (`search_docs.body`), in the database
//! only. These writes keep an index in step and change nothing a person
//! made, so they are not commands and leave no history.

use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use gk_api::OpError;
use gk_api::ops::file::{FileIndexInput, FileIndexStatus};
use gk_extract::{Extracted, Limits, Running, Skipped};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};

use crate::engine::{Engine, Role};
use crate::failure::store_error;
use crate::registry::Registry;

/// How many waiting files one look at the queue takes.
const BATCH: u32 = 32;
/// How long past the time limit an extraction may take to stop by itself
/// before it is left running (at most the time limit itself).
const GRACE: Duration = Duration::from_secs(1);

/// The extractions of this process: the one being waited for, across the
/// turns, and the ones that ran out of time and were left running.
#[derive(Debug, Default)]
pub(crate) struct Extractions {
    current: Option<InFlight>,
    abandoned: Vec<InFlight>,
}

impl Extractions {
    /// Forgets the abandoned extractions that have ended, and tells whether
    /// one still runs.
    fn one_abandoned(&mut self) -> bool {
        self.abandoned
            .retain(|a| a.running.wait(Duration::ZERO).is_none());
        !self.abandoned.is_empty()
    }
}

/// A file whose text is being read.
#[derive(Debug)]
pub(crate) struct InFlight {
    sha256: String,
    started: Instant,
    running: Running,
}

/// What one turn of [`Engine::index_files`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileIndexTurn {
    /// Files read in this turn.
    pub read: u64,
    /// Files still waiting after it (one being read included).
    pub waiting: u64,
}

/// A file waiting to be read.
#[derive(Debug, Clone)]
struct Waiting {
    sha256: String,
    size: u64,
    mime: String,
    name: String,
}

pub(crate) fn register(r: &mut Registry) {
    use gk_api::ops::file::{FileIndexGet, FileReindex};
    r.query::<FileIndexGet, _>(
        crate::ops::desc::<FileIndexGet>(),
        |q, _: FileIndexInput| Ok(status(q.tx)?),
    );
    r.command::<FileReindex, _>(crate::ops::desc::<FileReindex>(), |w, _: FileIndexInput| {
        w.tx()
            .execute("UPDATE file_texts SET version = 0 WHERE version > 0", [])?;
        Ok(status(w.tx())?)
    });
}

/// The counts of `file.index_status`.
pub(crate) fn status(tx: &Transaction<'_>) -> Result<FileIndexStatus, StoreError> {
    let version = i64::from(gk_extract::VERSION);
    let (files, read, with_text, skipped): (i64, i64, i64, i64) = tx.query_row(
        "SELECT count(*),
                count(x.sha256) FILTER (WHERE x.version >= ?1),
                count(x.sha256) FILTER (WHERE x.version >= ?1 AND x.outcome = 'text'),
                count(x.sha256) FILTER (WHERE x.version >= ?1 AND x.outcome = 'none')
           FROM blobs b LEFT JOIN file_texts x ON x.sha256 = b.sha256
          WHERE EXISTS (SELECT 1 FROM attachments a WHERE a.sha256 = b.sha256)",
        [version],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let n = |v: i64| u64::try_from(v).unwrap_or(0);
    Ok(FileIndexStatus {
        files: n(files),
        read: n(read),
        waiting: n(files - read),
        with_text: n(with_text),
        skipped: n(skipped),
    })
}

/// The type the stored files of PDFs have (read from their content).
const PDF: &str = "application/pdf";

/// Up to `limit` waiting files, newest first; without PDFs when `no_pdf`.
fn waiting(tx: &Transaction<'_>, limit: u32, no_pdf: bool) -> Result<Vec<Waiting>, StoreError> {
    let mut stmt = tx.prepare_cached(
        "SELECT sha256, size, mime, name FROM (
           SELECT b.sha256, b.size, b.created_at,
                  (SELECT a.mime FROM attachments a WHERE a.sha256 = b.sha256
                    ORDER BY a.created_at DESC LIMIT 1) AS mime,
                  (SELECT a.file_name FROM attachments a WHERE a.sha256 = b.sha256
                    ORDER BY a.created_at DESC LIMIT 1) AS name
             FROM blobs b LEFT JOIN file_texts x ON x.sha256 = b.sha256
            WHERE (x.sha256 IS NULL OR x.version < ?1)
              AND EXISTS (SELECT 1 FROM attachments a WHERE a.sha256 = b.sha256))
          WHERE NOT (?3 AND mime IS ?4)
          ORDER BY created_at DESC, sha256
          LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(
            params![i64::from(gk_extract::VERSION), limit, no_pdf, PDF],
            |r| {
                Ok(Waiting {
                    sha256: r.get(0)?,
                    size: u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                    mime: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    name: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Stores what extraction found in `sha256` and puts the text into the
/// search documents of its live attachments.
fn record(
    store: &gk_store::Store,
    sha256: &str,
    found: &Extracted,
    at: i64,
) -> Result<(), StoreError> {
    let (outcome, reason, text, truncated) = match found {
        Extracted::Text { text, truncated } => ("text", None, text.clone(), *truncated),
        Extracted::Skipped(why) => ("none", Some(why.as_str()), String::new(), false),
        Extracted::Unsupported => ("unsupported", None, String::new(), false),
    };
    let sha = sha256.to_owned();
    store.write("file.index", move |tx| {
        let stored = tx.execute(
            "INSERT INTO file_texts (sha256, version, outcome, reason, text, truncated,
                                     extracted_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7 WHERE EXISTS (SELECT 1 FROM blobs WHERE sha256 = ?1)
             ON CONFLICT (sha256) DO UPDATE SET version = excluded.version,
               outcome = excluded.outcome, reason = excluded.reason, text = excluded.text,
               truncated = excluded.truncated, extracted_at = excluded.extracted_at",
            params![
                sha,
                i64::from(gk_extract::VERSION),
                outcome,
                reason,
                text,
                truncated,
                at
            ],
        )?;
        if stored > 0 {
            tx.execute(
                "UPDATE search_docs SET body = ?2
                  WHERE kind = 'file' AND body <> ?2
                    AND entity_id IN (SELECT id FROM attachments WHERE sha256 = ?1)",
                params![sha, text],
            )?;
        }
        Ok::<_, StoreError>(())
    })
}

impl Engine {
    /// The limits each file is read within.
    fn extract_limits(&self) -> Limits {
        self.file_text_limits
    }

    /// One turn of reading waiting files, for at most about `budget`: first
    /// the result of a file still being read from the turn before, then
    /// more files, newest first. Returns what was read and what still
    /// waits. A file larger than the limit, of a kind that is not read, or
    /// gone from the store is recorded without being read.
    pub fn index_files(&self, budget: Duration) -> Result<FileIndexTurn, OpError> {
        let deadline = Instant::now() + budget;
        let limits = self.extract_limits();
        let mut turn = FileIndexTurn::default();
        let mut queue: Vec<Waiting> = Vec::new();
        let cut_off = limits.max_time + GRACE.min(limits.max_time);
        let mut runs = self
            .extracting
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(current) = runs.current.take() {
                let left = deadline.saturating_duration_since(Instant::now());
                let until_limit = cut_off.saturating_sub(current.started.elapsed());
                match current.running.wait(left.min(until_limit)) {
                    Some(found) => {
                        self.store_text(&current.sha256, &found)?;
                        turn.read += 1;
                    }
                    None if current.started.elapsed() >= cut_off => {
                        tracing::warn!(sha256 = %current.sha256, "reading a file's text took too long; it is left to end on its own");
                        self.store_text(&current.sha256, &Extracted::Skipped(Skipped::TimedOut))?;
                        turn.read += 1;
                        runs.abandoned.push(current);
                    }
                    None => {
                        runs.current = Some(current);
                        break;
                    }
                }
            }
            if Instant::now() >= deadline {
                break;
            }
            let no_pdf = runs.one_abandoned();
            if queue.is_empty() {
                queue = self
                    .store
                    .read(|tx| waiting(tx, BATCH, no_pdf))
                    .map_err(|e| store_error(&e))?;
                queue.reverse();
            }
            let Some(next) = queue.pop() else {
                break;
            };
            if no_pdf && next.mime == PDF {
                continue;
            }
            if let Some(started) = self.start_reading(&next, &limits)? {
                runs.current = Some(started);
            } else {
                turn.read += 1;
            }
        }
        let busy = u64::from(runs.current.is_some());
        drop(runs);
        let waiting = self
            .store
            .read(status)
            .map_err(|e| store_error(&e))?
            .waiting;
        turn.waiting = waiting.max(busy);
        Ok(turn)
    }

    /// Starts reading `file`; `None` when it was recorded at once.
    fn start_reading(&self, file: &Waiting, limits: &Limits) -> Result<Option<InFlight>, OpError> {
        let layout = &self.env.layout;
        if file.size > limits.max_input as u64 {
            let found = match gk_store::read_file(layout, &file.sha256, 8) {
                Ok(head) if gk_extract::format_of(&head, &file.mime, &file.name).is_some() => {
                    Extracted::Skipped(Skipped::TooLarge)
                }
                _ => Extracted::Unsupported,
            };
            self.store_text(&file.sha256, &found)?;
            return Ok(None);
        }
        let bytes = match gk_store::read_file(layout, &file.sha256, limits.max_input as u64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(sha256 = %file.sha256, error = %e, "a stored file could not be read for search");
                self.store_text(&file.sha256, &Extracted::Skipped(Skipped::Failed))?;
                return Ok(None);
            }
        };
        if gk_extract::format_of(&bytes, &file.mime, &file.name).is_none() {
            self.store_text(&file.sha256, &Extracted::Unsupported)?;
            return Ok(None);
        }
        let Some(running) = gk_extract::spawn(Arc::from(bytes), &file.mime, &file.name, limits)
        else {
            self.store_text(&file.sha256, &Extracted::Skipped(Skipped::Failed))?;
            return Ok(None);
        };
        Ok(Some(InFlight {
            sha256: file.sha256.clone(),
            started: Instant::now(),
            running,
        }))
    }

    fn store_text(&self, sha256: &str, found: &Extracted) -> Result<(), OpError> {
        record(&self.store, sha256, found, (self.clock)()).map_err(|e| store_error(&e))
    }

    /// Reads the text of the stored file `sha256` now, if it waits: the
    /// command line's way, which has no background turns. Waits at most
    /// the time limit.
    pub(crate) fn index_file_now(&self, sha256: &str) {
        if self.role != Role::Cli {
            return;
        }
        let limits = self.extract_limits();
        let file = self.store.read(|tx| waiting_one(tx, sha256));
        let Ok(Some(file)) = file else {
            return;
        };
        let result = (|| -> Result<(), OpError> {
            if let Some(started) = self.start_reading(&file, &limits)? {
                let found = started
                    .running
                    .wait(limits.max_time)
                    .unwrap_or(Extracted::Skipped(Skipped::TimedOut));
                self.store_text(sha256, &found)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!(error = %e, "the text of an attached file was not read");
        }
    }
}

fn waiting_one(tx: &Transaction<'_>, sha256: &str) -> Result<Option<Waiting>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT b.sha256, b.size, a.mime, a.file_name
               FROM blobs b JOIN attachments a ON a.sha256 = b.sha256
               LEFT JOIN file_texts x ON x.sha256 = b.sha256
              WHERE b.sha256 = ?1 AND (x.sha256 IS NULL OR x.version < ?2)
              ORDER BY a.created_at DESC LIMIT 1",
            params![sha256, i64::from(gk_extract::VERSION)],
            |r| {
                Ok(Waiting {
                    sha256: r.get(0)?,
                    size: u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                    mime: r.get(2)?,
                    name: r.get(3)?,
                })
            },
        )
        .optional()?)
}
