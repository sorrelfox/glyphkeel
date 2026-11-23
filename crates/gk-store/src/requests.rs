// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Answers to repeated requests.
//!
//! A command sent with a client's own request ID is answered once; a repeat
//! of the same operation, caller and ID within [`REQUEST_WINDOW_MS`] gets the
//! first answer back without running again, whichever process receives it.
//! Both the lookup and the record run inside the command's write
//! transaction, so two processes can't both run the same request.

use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::Result;

/// How long an answer is kept: 24 hours, in milliseconds.
pub const REQUEST_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

/// Identifies one request: the operation, who sent it and the client's ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestKey<'a> {
    /// The operation (`task.create`).
    pub op: &'a str,
    /// The caller: an agent session's ID, or the program and the person.
    pub caller: &'a str,
    /// The client's request ID.
    pub request_id: &'a str,
}

/// The answer recorded for `key` within the window before `now`
/// (milliseconds since 1970 UTC), as JSON text.
pub fn remembered(tx: &Transaction<'_>, key: RequestKey<'_>, now: i64) -> Result<Option<String>> {
    Ok(tx
        .query_row(
            "SELECT output FROM request_log
              WHERE op = ?1 AND caller = ?2 AND request_id = ?3 AND at > ?4",
            params![key.op, key.caller, key.request_id, now - REQUEST_WINDOW_MS],
            |r| r.get(0),
        )
        .optional()?)
}

/// Records `output` (JSON text) as the answer to `key` at `now`, and purges
/// answers older than the window.
pub fn remember(tx: &Transaction<'_>, key: RequestKey<'_>, output: &str, now: i64) -> Result<()> {
    purge_requests(tx, now)?;
    tx.execute(
        "INSERT INTO request_log (op, caller, request_id, output, at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (op, caller, request_id) DO UPDATE SET output = excluded.output, at = excluded.at",
        params![key.op, key.caller, key.request_id, output, now],
    )?;
    Ok(())
}

/// Removes answers older than the window before `now`; returns how many.
pub fn purge_requests(tx: &Transaction<'_>, now: i64) -> Result<usize> {
    Ok(tx.execute(
        "DELETE FROM request_log WHERE at <= ?1",
        [now - REQUEST_WINDOW_MS],
    )?)
}
