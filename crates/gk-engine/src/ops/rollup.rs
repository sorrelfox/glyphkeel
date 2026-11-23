// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Values computed when a task is read: its progress, and the totals over
//! its subtasks.

use gk_api::ops::task::Totals;
use gk_core::{Id, SubtaskProgress};
use gk_store::rusqlite::{OptionalExtension, Transaction};

use super::lookup::{b, id_at};
use crate::failure::Failure;

/// Levels of subtasks followed; deeper ones count as not started. Subtasks
/// go at most 5 levels deep, so this only guards against damaged data.
const MAX_DEPTH: usize = 16;

/// The task's progress in percent, as `gk_core::progress` defines it.
pub(crate) fn progress(tx: &Transaction<'_>, id: Id) -> Result<u8, Failure> {
    progress_at(tx, id, 0)
}

fn progress_at(tx: &Transaction<'_>, id: Id, depth: usize) -> Result<u8, Failure> {
    let row: Option<(Option<i64>, bool, i64, i64)> = tx
        .query_row(
            "SELECT progress_override, closed_at IS NOT NULL, checklist_done, checklist_total
               FROM tasks WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((by_hand, closed, done, total)) = row else {
        return Ok(0);
    };
    let by_hand = by_hand.and_then(|p| u8::try_from(p.clamp(0, 100)).ok());
    let checklist = (
        u32::try_from(done).unwrap_or(0),
        u32::try_from(total).unwrap_or(0),
    );
    if by_hand.is_some() || closed || checklist.1 > 0 {
        return Ok(gk_core::progress(by_hand, closed, checklist, &[]));
    }
    let mut stmt = tx.prepare_cached(
        "SELECT id, estimate_min, closed_at IS NOT NULL FROM tasks
          WHERE parent_id = ?1 AND deleted_at IS NULL",
    )?;
    let children = stmt
        .query_map([b(id)], |r| {
            Ok((
                id_at(r, 0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, bool>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut subtasks = Vec::with_capacity(children.len());
    for (child, estimate, closed) in children {
        let percent = if closed {
            100
        } else if depth < MAX_DEPTH {
            progress_at(tx, child, depth + 1)?
        } else {
            0
        };
        subtasks.push(SubtaskProgress {
            percent,
            estimate_minutes: estimate,
            closed,
        });
    }
    Ok(gk_core::progress(None, false, (0, 0), &subtasks))
}

/// Estimate, logged time and what remains over the task and all its live
/// subtasks; `None` for a task without subtasks.
pub(crate) fn totals(tx: &Transaction<'_>, id: Id) -> Result<Option<Totals>, Failure> {
    let all = super::task::descendants(tx, id)?;
    if all.len() < 2 {
        return Ok(None);
    }
    let (mut estimate, mut spent) = (0i64, 0i64);
    let mut stmt =
        tx.prepare_cached("SELECT coalesce(estimate_min, 0), spent_min FROM tasks WHERE id = ?1")?;
    for t in all {
        let (e, s): (i64, i64) = stmt.query_row([b(t)], |r| Ok((r.get(0)?, r.get(1)?)))?;
        estimate = estimate.saturating_add(e);
        spent = spent.saturating_add(s);
    }
    Ok(Some(Totals {
        estimate_minutes: estimate,
        spent_minutes: spent,
        remaining_minutes: estimate.saturating_sub(spent),
    }))
}
