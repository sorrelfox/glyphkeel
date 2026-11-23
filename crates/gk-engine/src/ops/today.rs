// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Today: overdue tasks, tasks due today and tasks in progress (each task
//! in one list only, the first that fits), today's personal reminders,
//! agents' claims, recent changes and the timer. "Today" is the local day
//! in the workspace's time zone. Archived projects are left out unless the
//! input names one.

use gk_api::ops::common::{Actor, ActorKind};
use gk_api::ops::today::{RecentTask, Today, TodayCounts, TodayGet, TodayInput};
use gk_core::Id;
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params_from_iter};

use super::desc;
use super::lookup::{self, b, id_at};
use super::query::listed;
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;

/// Tasks per list unless asked otherwise.
const DEFAULT_LIMIT: u32 = 50;
/// How far back "recent" reaches.
const RECENT_MS: i64 = 24 * 3_600_000;

pub(super) fn register(r: &mut Registry) {
    r.query::<TodayGet, _>(desc::<TodayGet>(), today);
}

/// The SQL condition on `p` (projects) and its parameters for the scope.
fn scope(tx: &Transaction<'_>, project: Option<&str>) -> Result<(String, Vec<Sql>), Failure> {
    match project {
        None => Ok(("p.state = 'active'".to_owned(), Vec::new())),
        Some(text) => {
            let p = lookup::project(tx, "project", text)?;
            let ids = super::project::subtree(tx, p.id)?;
            let marks = vec!["?"; ids.len()].join(", ");
            Ok((
                format!("p.id IN ({marks})"),
                ids.into_iter().map(|id| Sql::Blob(b(id))).collect(),
            ))
        }
    }
}

/// The IDs of the live tasks of the scope matching `filter`, in `order`.
fn ids(
    tx: &Transaction<'_>,
    scope: &(String, Vec<Sql>),
    filter: &str,
    args: &[Sql],
    order: &str,
) -> Result<Vec<Id>, Failure> {
    let sql = format!(
        "SELECT t.id FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id
          WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL AND {} AND {filter}
          ORDER BY {order}",
        scope.0
    );
    let mut stmt = tx.prepare(&sql)?;
    let params: Vec<&Sql> = scope.1.iter().chain(args.iter()).collect();
    let rows = stmt
        .query_map(params_from_iter(params), |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Who changed task `id` last, and when.
fn last_change(tx: &Transaction<'_>, id: Id) -> Result<Option<(Actor, i64)>, Failure> {
    Ok(tx
        .query_row(
            "SELECT actor_kind, actor_name, at FROM activity
              WHERE entity_kind = 'task' AND entity_id = ?1 ORDER BY seq DESC LIMIT 1",
            [b(id)],
            |r| {
                Ok((
                    Actor {
                        kind: if r.get::<_, String>(0)? == "agent" {
                            ActorKind::Agent
                        } else {
                            ActorKind::User
                        },
                        name: r.get(1)?,
                    },
                    r.get(2)?,
                ))
            },
        )
        .optional()?)
}

fn count(ids: &[Id]) -> u64 {
    ids.len() as u64
}

fn today(q: &QueryCtx<'_>, input: TodayInput) -> Result<Today, Failure> {
    let tx = q.tx;
    let (limit, _) = super::search::paging(input.limit, None, DEFAULT_LIMIT)?;
    let limit = limit as usize;
    let scope = scope(tx, input.project.as_deref())?;
    let today = q.env.zone.day_of(q.now);
    let date = Sql::Text(today.to_string());
    let open = "t.closed_at IS NULL";

    let overdue = ids(
        tx,
        &scope,
        &format!("{open} AND t.due_date < ?"),
        std::slice::from_ref(&date),
        "t.due_date, t.priority DESC, p.key, t.number",
    )?;
    let due_today = ids(
        tx,
        &scope,
        &format!("{open} AND t.due_date = ?"),
        std::slice::from_ref(&date),
        "t.priority DESC, p.key, t.number",
    )?;
    let in_progress = ids(
        tx,
        &scope,
        &format!(
            "{open} AND s.category = 'in_progress' AND (t.due_date IS NULL OR t.due_date > ?)"
        ),
        std::slice::from_ref(&date),
        "p.key, s.rank, t.rank, t.number",
    )?;
    let claimed = ids(
        tx,
        &scope,
        "t.claim_session IS NOT NULL AND t.claim_until > ?",
        &[Sql::Integer(q.now)],
        "t.claim_until, p.key, t.number",
    )?;
    let recent = ids(
        tx,
        &scope,
        "t.updated_at >= ?",
        &[Sql::Integer(q.now - RECENT_MS)],
        "t.updated_at DESC, p.key, t.number",
    )?;
    let all_open = ids(tx, &scope, open, &[], "t.id")?;

    let end_of_today = today
        .add_days(1)
        .map_or(i64::MAX, |d| q.env.zone.moment(d, 0) - 1);
    let mut reminders = super::reminder::personal_due(tx, &q.env.zone, q.now, end_of_today)?;
    if input.project.is_some() {
        let mut kept = Vec::new();
        for r in reminders {
            if let Ok(id) = r.task.id.parse::<Id>()
                && in_scope(tx, &scope, id)?
            {
                kept.push(r);
            }
        }
        reminders = kept;
    }

    let page = |list: &[Id]| -> Result<Vec<_>, Failure> {
        list.iter()
            .take(limit)
            .map(|id| listed(tx, *id, q.now))
            .collect()
    };
    let mut recent_items = Vec::new();
    for id in recent.iter().take(limit) {
        let task = listed(tx, *id, q.now)?;
        let (changed_by, at) = last_change(tx, *id)?.unwrap_or_else(|| {
            (
                task.created_by.clone(),
                tx.query_row(
                    "SELECT updated_at FROM tasks WHERE id = ?1",
                    [b(*id)],
                    |r| r.get(0),
                )
                .unwrap_or(q.now),
            )
        });
        recent_items.push(RecentTask {
            task,
            changed_by,
            changed_at: rfc3339(at),
        });
    }
    let counts = TodayCounts {
        today: count(&overdue) + count(&due_today) + count(&in_progress),
        overdue: count(&overdue),
        due_today: count(&due_today),
        in_progress: count(&in_progress),
        reminders: reminders.len() as u64,
        claimed: count(&claimed),
        recent: count(&recent),
        open: count(&all_open),
    };
    reminders.truncate(limit);
    let timer = super::time::current(tx, q.now, super::task::units(tx)?)?;
    Ok(Today {
        date: today.to_string(),
        overdue: page(&overdue)?,
        due_today: page(&due_today)?,
        in_progress: page(&in_progress)?,
        reminders,
        claimed: page(&claimed)?,
        recent: recent_items,
        timer,
        counts,
    })
}

/// Whether task `id` (open or closed) is in the scope.
fn in_scope(tx: &Transaction<'_>, scope: &(String, Vec<Sql>), id: Id) -> Result<bool, Failure> {
    let sql = format!(
        "SELECT 1 FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.id = ? AND {}",
        scope.0
    );
    let mut params = vec![Sql::Blob(b(id))];
    params.extend(scope.1.iter().cloned());
    Ok(tx
        .query_row(&sql, params_from_iter(params), |_| Ok(()))
        .optional()?
        .is_some())
}
