// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The digest ("while you were away"): what changed without the person
//! since a moment, read from history in SQL.
//!
//! - agent sessions: their changes (`activity` rows by agents, by session),
//!   the commands, what was reverted since, and the tasks touched;
//! - automation rules: their runs, failures and changes (`automation_runs`),
//!   the history groups of the runs and the latest runs;
//! - personal reminders and notifications of rules that came due and
//!   weren't acknowledged, and open tasks whose due day began in the
//!   window;
//! - changes the person made through the command line.
//!
//! Changes the person made in the app are not in it for the person: they
//! saw them. For an agent catching up they are, and its own session isn't.
//!
//! The starting moment is the caller's `since`; else, for the person, their
//! marker; for an agent, the end of its previous session of the same name;
//! else the last 24 hours.
//!
//! **The marker** (`personal_marks`, mark `digest_seen`) is the moment up
//! to which the person has seen the digest. It moves only when the person
//! dismisses the digest (`digest.dismiss`), to the `until` of the digest
//! they saw, so changes that came while they read it show next time. It
//! never moves back. It is personal, kept per person in the workspace (the
//! person at this computer is `me`), and is not history: it can't be
//! undone, and moving it changes no data.
//!
//! A caller limited to a project sees only that project and its
//! sub-projects: history rows, runs, reminders and due dates of other
//! projects, and rows of no project, are left out.

use std::collections::HashSet;

use gk_api::ops::digest::{
    DIGEST_DEFAULT_LIMIT, DIGEST_MAX_GROUPS, DIGEST_MAX_LIMIT, Digest, DigestCli, DigestCounts,
    DigestDismiss, DigestDismissInput, DigestDismissed, DigestDueTask, DigestGet, DigestInput,
    DigestRule, DigestSession, DigestSince,
};
use gk_api::ops::history::ActivityTask;
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params, params_from_iter};

use super::desc;
use super::lookup::{self, b, id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::{Zone, rfc3339};
use crate::write::WriteCtx;

/// The person at this computer, as marks name them. There are no accounts
/// yet; a workspace shared by several people would name each.
const PERSON: &str = "me";
/// The mark of the digest.
const SEEN: &str = "digest_seen";
/// How far back the digest reaches when nothing else says.
const LAST_DAY_MS: i64 = 24 * 3_600_000;

/// The task an `activity` row `a` belongs to: the task itself, or the task
/// of a comment, time entry or file (also a file on a comment).
const TASK_OF_ROW: &str = "CASE a.entity_kind
    WHEN 'task' THEN a.entity_id
    WHEN 'comment' THEN (SELECT c.task_id FROM comments c WHERE c.id = a.entity_id)
    WHEN 'time_entry' THEN (SELECT e.task_id FROM time_entries e WHERE e.id = a.entity_id)
    WHEN 'attachment' THEN (SELECT CASE f.owner_kind
                   WHEN 'task' THEN f.owner_id
                   WHEN 'comment' THEN (SELECT c.task_id FROM comments c WHERE c.id = f.owner_id)
                 END FROM attachments f WHERE f.id = a.entity_id)
  END";

pub(super) fn register(r: &mut Registry) {
    r.query::<DigestGet, _>(desc::<DigestGet>(), get);
    r.command::<DigestDismiss, _>(desc::<DigestDismiss>(), dismiss);
}

/// The person's marker, if they ever dismissed the digest.
pub(crate) fn marker(tx: &Transaction<'_>) -> Result<Option<i64>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT at FROM personal_marks WHERE person = ?1 AND mark = ?2",
            params![PERSON, SEEN],
            |r| r.get(0),
        )
        .optional()?)
}

/// The moment `text` names: a moment, or the start of a day in words.
fn moment(q: &QueryCtx<'_>, field: &str, text: &str) -> Result<i64, Failure> {
    if let Some(at) = q.env.zone.parse_moment(text) {
        return Ok(at);
    }
    let day = super::task::parse_date(q.tx, &q.env.zone, q.now, field, text).map_err(|_| {
        Failure::Op(messages::hint(
            messages::invalid_at(field, "digest-not-a-moment", &[("text", text.to_owned())]),
            "digest-moment-hint",
            &[],
        ))
    })?;
    Ok(q.env.zone.moment(day, 0))
}

/// Where the digest starts, and why there.
fn start(q: &QueryCtx<'_>, input: &DigestInput) -> Result<(i64, DigestSince), Failure> {
    if let Some(text) = &input.since {
        return Ok((moment(q, "since", text)?, DigestSince::Given));
    }
    if q.caller.is_agent() {
        let previous: Option<i64> = q.tx.query_row(
            "SELECT max(coalesce(ended_at, last_seen_at)) FROM agent_sessions
              WHERE display_name = ?1 AND id IS NOT ?2 AND started_at <= ?3",
            params![q.caller.actor.name, q.caller.session.map(b), q.now],
            |r| r.get(0),
        )?;
        // Just after it: its own last change is not news to it.
        if let Some(at) = previous {
            return Ok((at + 1, DigestSince::PreviousSession));
        }
    } else if let Some(at) = marker(q.tx)? {
        return Ok((at, DigestSince::Marker));
    }
    Ok((q.now - LAST_DAY_MS, DigestSince::LastDay))
}

/// The SQL that keeps rows of the digest's projects, on the column
/// `column`, with its parameters: the project and everything below it,
/// trashed projects too, as in the activity log. Nothing when the digest
/// covers every project.
fn project_filter(column: &str, root: Option<Id>) -> (String, Vec<Sql>) {
    match root {
        None => (String::new(), Vec::new()),
        Some(root) => (
            format!(
                " AND {column} IN (WITH RECURSIVE tree(id) AS (
                    SELECT ? UNION SELECT p.id FROM projects p JOIN tree ON p.parent_id = tree.id)
                  SELECT id FROM tree)"
            ),
            vec![Sql::Blob(b(root))],
        ),
    }
}

/// The digest's window and filter, shared by its parts.
struct Window {
    since: i64,
    until: i64,
    /// The project the digest covers, with its sub-projects; `None`: all.
    root: Option<Id>,
    limit: usize,
}

impl Window {
    /// `since`, `until` and the project's parameters, then `more`.
    fn args(&self, column: &str, more: &[Sql]) -> (String, Vec<Sql>) {
        let (filter, project) = project_filter(column, self.root);
        let mut args = vec![Sql::Integer(self.since), Sql::Integer(self.until)];
        args.extend(project);
        args.extend(more.iter().cloned());
        (filter, args)
    }
}

fn get(q: &QueryCtx<'_>, input: DigestInput) -> Result<Digest, Failure> {
    let limit = input.limit.unwrap_or(DIGEST_DEFAULT_LIMIT);
    if !(1..=DIGEST_MAX_LIMIT).contains(&limit) {
        return Err(messages::invalid(
            "limit",
            "input-limit",
            &[("max", DIGEST_MAX_LIMIT.to_string())],
        ));
    }
    let (since, since_from) = start(q, &input)?;
    let root = match &input.project {
        Some(p) => Some(lookup::project(q.tx, "project", p)?.id),
        None => crate::scope::root(),
    };
    let w = Window {
        since,
        until: q.now,
        root,
        limit: limit as usize,
    };
    let own = if q.caller.is_agent() {
        q.caller.session
    } else {
        None
    };
    let agents: Vec<DigestSession> = sessions(q.tx, &w)?
        .into_iter()
        .filter(|s| own.is_none_or(|own| s.id != own.to_string()))
        .collect();
    let automation = rules(q.tx, &w)?;
    let reminders = reminders(q, &w)?;
    let (became_due, became_due_total) = became_due(q, &w)?;
    let cli = by_person(q.tx, &w, "cli")?;
    let app = if q.caller.is_agent() {
        by_person(q.tx, &w, "app")?
    } else {
        DigestCli::default()
    };
    let mut counts = DigestCounts {
        sessions: agents.len() as u64,
        agent_changes: agents.iter().map(|s| s.changes).sum(),
        rules: automation.len() as u64,
        runs: automation.iter().map(|r| r.runs).sum(),
        failures: automation.iter().map(|r| r.failures).sum(),
        reminders: reminders.len() as u64,
        became_due: became_due_total,
        cli_changes: cli.changes,
        app_changes: app.changes,
        items: 0,
    };
    counts.items = counts.sessions
        + counts.rules
        + counts.reminders
        + counts.became_due
        + u64::from(counts.cli_changes > 0)
        + u64::from(counts.app_changes > 0);
    let mut agents = agents;
    agents.truncate(w.limit);
    let mut automation = automation;
    automation.truncate(w.limit);
    let mut reminders = reminders;
    reminders.truncate(w.limit);
    Ok(Digest {
        since: rfc3339(since),
        since_from,
        until: rfc3339(w.until),
        agents,
        automation,
        reminders,
        became_due,
        cli,
        app,
        counts,
    })
}

/// The tasks the `activity` rows matching `filter` touched, the most
/// recently touched first: up to the limit, and how many in all.
fn touched(
    tx: &Transaction<'_>,
    w: &Window,
    filter: &str,
    more: &[Sql],
) -> Result<(Vec<ActivityTask>, u64), Failure> {
    let (project, args) = w.args("a.project_id", more);
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT t.id, p.key || '-' || t.number, t.title, max(a.seq) AS last
           FROM activity a
           JOIN tasks t ON t.id = ({TASK_OF_ROW})
           JOIN projects p ON p.id = t.project_id
          WHERE a.at >= ? AND a.at <= ?{project} AND {filter}
          GROUP BY t.id ORDER BY last DESC"
    ))?;
    let mut tasks = Vec::new();
    let mut total = 0u64;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;
    while let Some(r) = rows.next()? {
        total += 1;
        if tasks.len() < w.limit {
            tasks.push(ActivityTask {
                id: id_at(r, 0)?.to_string(),
                key: r.get(1)?,
                title: r.get(2)?,
            });
        }
    }
    Ok((tasks, total))
}

/// The agent sessions that changed something in the window, the latest
/// first.
fn sessions(tx: &Transaction<'_>, w: &Window) -> Result<Vec<DigestSession>, Failure> {
    let (project, args) = w.args("a.project_id", &[]);
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT a.session_id, count(*), count(DISTINCT a.group_id),
                sum(a.reverted_by IS NOT NULL), max(a.at),
                s.display_name, s.client_id, s.started_at, s.ended_at
           FROM activity a JOIN agent_sessions s ON s.id = a.session_id
          WHERE a.at >= ? AND a.at <= ?{project}
            AND a.actor_kind = 'agent' AND a.session_id IS NOT NULL
          GROUP BY a.session_id ORDER BY max(a.at) DESC, a.session_id"
    ))?;
    let found = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            Ok((
                id_at(r, 0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, Option<i64>>(8)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(found.len());
    for (id, changes, groups, reverted, last, name, client, started, ended) in found {
        let (tasks, task_count) = if out.len() < w.limit {
            touched(tx, w, "a.session_id = ?", &[Sql::Blob(b(id))])?
        } else {
            (Vec::new(), 0)
        };
        out.push(DigestSession {
            id: id.to_string(),
            name,
            client,
            running: ended.is_none(),
            started_at: rfc3339(started),
            last_at: rfc3339(last),
            changes: count(changes),
            reverted: count(reverted),
            groups: count(groups),
            tasks,
            task_count,
        });
    }
    Ok(out)
}

fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// The project a run belongs to: its task's, else its rule's.
const RUN_PROJECT: &str = "coalesce((SELECT t.project_id FROM tasks t WHERE t.id = u.task_id),
    (SELECT r.project_id FROM automation_rules r WHERE r.id = u.rule_id))";

/// The automation rules that ran in the window, the latest first.
fn rules(tx: &Transaction<'_>, w: &Window) -> Result<Vec<DigestRule>, Failure> {
    let (project, args) = w.args(RUN_PROJECT, &[]);
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT u.rule_id, count(*), sum(u.result = 'failed'), sum(u.result = 'limited'),
                sum(u.changes), max(u.at)
           FROM automation_runs u
          WHERE u.at >= ? AND u.at <= ?{project}
          GROUP BY u.rule_id ORDER BY max(u.at) DESC, u.rule_id"
    ))?;
    let found = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            Ok((
                id_at(r, 0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(found.len());
    for (rule, runs, failures, limited, changes, last) in found {
        let one = [Sql::Blob(b(rule))];
        let (project_filter, args) = w.args(RUN_PROJECT, &one);
        let filter = format!("u.at >= ? AND u.at <= ?{project_filter} AND u.rule_id = ?");
        // The run log: the latest runs, the first giving the rule's name.
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT u.id, u.rule_id, u.rule_name, u.at, u.result, u.trigger_kind,
                    (SELECT p.key || '-' || t.number FROM tasks t
                       JOIN projects p ON p.id = t.project_id
                      WHERE t.id = u.task_id AND t.deleted_at IS NULL),
                    u.group_id, u.changes, u.depth, u.error
               FROM automation_runs u WHERE {filter}
              ORDER BY u.at DESC, u.rowid DESC LIMIT {}",
            w.limit
        ))?;
        let recent = stmt
            .query_map(params_from_iter(args.iter()), super::automation::run_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let name = recent
            .first()
            .map(|r| r.rule_name.clone())
            .unwrap_or_default();
        let last_error: Option<String> = tx
            .query_row(
                &format!(
                    "SELECT u.error FROM automation_runs u
                      WHERE {filter} AND u.result <> 'done' AND u.error IS NOT NULL
                      ORDER BY u.at DESC, u.rowid DESC LIMIT 1"
                ),
                params_from_iter(args.iter()),
                |r| r.get(0),
            )
            .optional()?;
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT u.group_id FROM automation_runs u
              WHERE {filter} AND u.group_id IS NOT NULL
              ORDER BY u.at DESC, u.rowid DESC LIMIT {DIGEST_MAX_GROUPS}"
        ))?;
        let groups = stmt
            .query_map(params_from_iter(args.iter()), |r| id_at(r, 0))?
            .map(|g| g.map(|g| g.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT t.id, p.key || '-' || t.number, t.title, max(u.at) AS last
               FROM automation_runs u JOIN tasks t ON t.id = u.task_id
               JOIN projects p ON p.id = t.project_id
              WHERE {filter} GROUP BY t.id ORDER BY last DESC, t.id"
        ))?;
        let mut tasks = Vec::new();
        let mut task_count = 0u64;
        let mut rows = stmt.query(params_from_iter(args.iter()))?;
        while let Some(r) = rows.next()? {
            task_count += 1;
            if tasks.len() < w.limit {
                tasks.push(ActivityTask {
                    id: id_at(r, 0)?.to_string(),
                    key: r.get(1)?,
                    title: r.get(2)?,
                });
            }
        }
        let project: Option<String> = tx
            .query_row(
                "SELECT p.key FROM automation_rules r JOIN projects p ON p.id = r.project_id
                  WHERE r.id = ?1",
                [b(rule)],
                |r| r.get(0),
            )
            .optional()?;
        out.push(DigestRule {
            id: rule.to_string(),
            name,
            project,
            runs: count(runs),
            failures: count(failures),
            limited: count(limited),
            changes: count(changes),
            last_at: rfc3339(last),
            last_error: last_error.and_then(|e| serde_json::from_str(&e).ok()),
            groups,
            tasks,
            task_count,
            recent,
        });
    }
    Ok(out)
}

/// The projects the digest covers, for filtering rows read in Rust;
/// `None`: all.
fn covered(tx: &Transaction<'_>, w: &Window) -> Result<Option<HashSet<Id>>, Failure> {
    let Some(root) = w.root else {
        return Ok(None);
    };
    let mut stmt = tx.prepare_cached(
        "WITH RECURSIVE tree(id) AS (
           SELECT ?1 UNION SELECT p.id FROM projects p JOIN tree ON p.parent_id = tree.id)
         SELECT id FROM tree",
    )?;
    let ids = stmt
        .query_map([b(root)], |r| id_at(r, 0))?
        .collect::<Result<HashSet<_>, _>>()?;
    Ok(Some(ids))
}

/// Reminders and rules' notifications that came due in the window.
fn reminders(q: &QueryCtx<'_>, w: &Window) -> Result<Vec<gk_api::ops::time::DueReminder>, Failure> {
    let projects = covered(q.tx, w)?;
    Ok(
        super::reminder::came_due(q.tx, &q.env.zone, w.since, w.until)?
            .into_iter()
            .filter(|(project, _)| projects.as_ref().is_none_or(|p| p.contains(project)))
            .map(|(_, reminder)| reminder)
            .collect(),
    )
}

/// Open tasks of active projects whose due date came in the window: their
/// due day started at the window's start or later, and not after today. A
/// task already due when the window started isn't listed. Up to the limit,
/// and how many.
fn became_due(q: &QueryCtx<'_>, w: &Window) -> Result<(Vec<DigestDueTask>, u64), Failure> {
    let zone: &Zone = &q.env.zone;
    let start = zone.day_of(w.since);
    let first = if zone.moment(start, 0) >= w.since {
        start
    } else {
        start.add_days(1).unwrap_or(start)
    };
    let first = first.to_string();
    let today = zone.day_of(w.until).to_string();
    let (project, mut args) = project_filter("t.project_id", w.root);
    args.splice(0..0, [Sql::Text(first), Sql::Text(today.clone())]);
    let mut stmt = q.tx.prepare_cached(&format!(
        "SELECT t.id, p.key || '-' || t.number, t.title, t.due_date, t.project_id
           FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.due_date >= ? AND t.due_date <= ?{project}
            AND t.deleted_at IS NULL AND t.closed_at IS NULL
            AND p.deleted_at IS NULL AND p.state = 'active'
          ORDER BY t.due_date, p.key, t.number"
    ))?;
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;
    while let Some(r) = rows.next()? {
        if !crate::scope::sees(id_at(r, 4)?) {
            continue;
        }
        total += 1;
        if out.len() < w.limit {
            let due: String = r.get(3)?;
            out.push(DigestDueTask {
                task: ActivityTask {
                    id: id_at(r, 0)?.to_string(),
                    key: r.get(1)?,
                    title: r.get(2)?,
                },
                overdue: due < today,
                due_date: due,
            });
        }
    }
    Ok((out, total))
}

/// What the person changed through the program `via` (`cli` or `app`) in
/// the window.
fn by_person(tx: &Transaction<'_>, w: &Window, via: &str) -> Result<DigestCli, Failure> {
    let filter = format!("a.actor_kind = 'user' AND a.via = '{via}'");
    let filter = filter.as_str();
    let (project, args) = w.args("a.project_id", &[]);
    let (changes, group_count, last): (i64, i64, Option<i64>) = tx.query_row(
        &format!(
            "SELECT count(*), count(DISTINCT a.group_id), max(a.at) FROM activity a
              WHERE a.at >= ? AND a.at <= ?{project} AND {filter}"
        ),
        params_from_iter(args.iter()),
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if changes == 0 {
        return Ok(DigestCli::default());
    }
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT a.group_id FROM activity a
          WHERE a.at >= ? AND a.at <= ?{project} AND {filter}
          GROUP BY a.group_id ORDER BY max(a.seq) DESC LIMIT {DIGEST_MAX_GROUPS}"
    ))?;
    let groups = stmt
        .query_map(params_from_iter(args.iter()), |r| id_at(r, 0))?
        .map(|g| g.map(|g| g.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let (tasks, task_count) = touched(tx, w, filter, &[])?;
    Ok(DigestCli {
        changes: count(changes),
        group_count: count(group_count),
        groups,
        tasks,
        task_count,
        last_at: last.map(rfc3339),
    })
}

fn dismiss(w: &mut WriteCtx<'_>, input: DigestDismissInput) -> Result<DigestDismissed, Failure> {
    let now = w.now();
    let at = match &input.at {
        Some(text) => w.zone().parse_moment(text).ok_or_else(|| {
            Failure::Op(messages::hint(
                messages::invalid_at("at", "digest-not-a-moment", &[("text", text.clone())]),
                "digest-moment-hint",
                &[],
            ))
        })?,
        None => now,
    };
    // Never ahead of now, and never back.
    let at = at.min(now).max(marker(w.tx())?.unwrap_or(i64::MIN));
    w.tx().execute(
        "INSERT INTO personal_marks (person, mark, at) VALUES (?1, ?2, ?3)
         ON CONFLICT (person, mark) DO UPDATE SET at = excluded.at",
        params![PERSON, SEEN, at],
    )?;
    Ok(DigestDismissed {
        marker: rfc3339(at),
    })
}
