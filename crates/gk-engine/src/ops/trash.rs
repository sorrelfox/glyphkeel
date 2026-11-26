// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The trash: deleting tasks, wiki pages and projects into it, listing it,
//! restoring from it and purging it.
//!
//! Rules:
//! - Every row a deletion removes gets `deleted_at` and the deletion's
//!   history group in `deleted_group`, and history records `trashed` on it,
//!   so Undo and revert bring it back like any other change.
//! - A task takes its live subtasks with it, a page the pages inside it, a
//!   project its live sub-projects, tasks and pages (only the projects get
//!   history rows; the rows inside share their group and come back with
//!   them). What belongs to a task (comments, time entries, files, links)
//!   stays as it is and is hidden with the task.
//! - An entry of the trash is a deleted item whose parent did not go in
//!   the same deletion. Restoring it brings back everything of its subtree
//!   that went with it. An item whose task, page or project is itself in
//!   the trash can't come back on its own.
//! - Purging deletes the rows for good, with everything that hangs off
//!   them, removes their text from earlier history rows and records only
//!   that something was purged. Stored files no attachment uses any more
//!   are removed after the commit.
//! - Agents with the standard profile move at most the workspace's limits
//!   of items per call and per hour to the trash (when they are set), and
//!   need the full profile to delete a project or purge anything.

use std::collections::HashSet;

use gk_api::ops::common::{DEFAULT_LIMIT, MAX_LIMIT};
use gk_api::ops::project::{ProjectDelete, ProjectDeleteInput, ProjectDeleted};
use gk_api::ops::task::{TaskDelete, TaskDeleteInput};
use gk_api::ops::trash::{
    TrashChange, TrashEntry, TrashKind, TrashList, TrashListInput, TrashListOp, TrashPurge,
    TrashPurgeInput, TrashRestore, TrashRestoreInput,
};
use gk_api::ops::wiki::{WikiDelete, WikiDeleteInput};
use gk_api::{OpError, Profile};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as SqlValue;
use gk_store::rusqlite::{OptionalExtension, Transaction, params, params_from_iter};
use serde_json::{Value, json};

use super::comment::actor;
use super::fields::{PROJECT, TASK, WIKI};
use super::lookup::{self, b, id_at, opt_id_at};
use super::{comment, desc, file, links, project_delete, settings, time, wiki};
use crate::context::Context;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's field for an item in the trash.
pub(crate) const TRASHED: &str = "trashed";
/// History's field for a purged item; it can't be reverted.
pub(crate) const PURGED: &str = "purged";
/// Most tasks one deletion names.
const MAX_TASKS: usize = 500;
/// Most entries one restore or purge names.
const MAX_ITEMS: usize = 100;
const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;

pub(super) fn register(r: &mut Registry) {
    r.command::<TaskDelete, _>(desc::<TaskDelete>(), delete_tasks);
    r.command::<WikiDelete, _>(desc::<WikiDelete>(), delete_page);
    r.command::<ProjectDelete, _>(desc::<ProjectDelete>(), delete_project);
    r.query::<TrashListOp, _>(desc::<TrashListOp>(), list);
    r.command::<TrashRestore, _>(desc::<TrashRestore>(), restore);
    r.engine_op::<TrashPurge, _>(desc::<TrashPurge>(), |engine, ctx, input| {
        engine.purge_trash(ctx, input)
    });
}

// ---------------------------------------------------------------------
// Profiles

/// The agent's profile after the workspace's cap; `None` for the person.
pub(crate) fn agent_profile(
    tx: &Transaction<'_>,
    ctx: &Context,
) -> Result<Option<Profile>, StoreError> {
    if !ctx.is_agent() {
        return Ok(None);
    }
    let cap = settings::load(tx)?.agent_max_profile;
    Ok(Some(
        ctx.profile.unwrap_or(Profile::Standard).capped_by(cap),
    ))
}

/// Refuses an agent below the full profile.
fn ensure_full(w: &WriteCtx<'_>, message: &str) -> Result<(), Failure> {
    match agent_profile(w.tx(), w.caller())? {
        Some(p) if p < Profile::Full => Err(Failure::Op(
            messages::hint(
                messages::tag(
                    OpError::forbidden(messages::text(message, &[])),
                    message,
                    &[],
                ),
                "trash-needs-full-hint",
                &[],
            )
            .with_detail("profile", p.as_str()),
        )),
        _ => Ok(()),
    }
}

/// The caps on what a standard agent moves to the trash: `count` items in
/// this command, and with those of the last hour. Called before the
/// command's history is written.
pub(crate) fn check_agent_caps(
    tx: &Transaction<'_>,
    ctx: &Context,
    now: i64,
    count: u64,
) -> Result<(), Failure> {
    if count == 0 {
        return Ok(());
    }
    let Some(profile) = agent_profile(tx, ctx)? else {
        return Ok(());
    };
    if profile >= Profile::Full {
        return Ok(());
    }
    let limits = settings::load(tx)?.agent_limits;
    if let Some(per_call) = limits.trash_items_per_call.map(u64::from)
        && count > per_call
    {
        let args = [("count", count.to_string()), ("max", per_call.to_string())];
        return Err(Failure::Op(messages::hint(
            messages::tag(
                OpError::limit(messages::text("trash-call-limit", &args), per_call),
                "trash-call-limit",
                &args,
            ),
            "trash-call-limit-hint",
            &[],
        )));
    }
    let (Some(session), Some(per_hour)) = (ctx.session, limits.trash_items_per_hour) else {
        return Ok(());
    };
    let per_hour = u64::from(per_hour);
    let (earlier, oldest): (i64, Option<i64>) = tx.query_row(
        "SELECT count(*), min(at) FROM activity
          WHERE session_id = ?1 AND at > ?2
            AND json_extract(changes, '$.fields.trashed.after') = 1",
        params![session.as_bytes().as_slice(), now - HOUR_MS],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let total = u64::try_from(earlier).unwrap_or(0) + count;
    if total > per_hour {
        let wait_ms = oldest.map_or(HOUR_MS, |t| t + HOUR_MS - now).max(1000);
        let retry_after = (wait_ms + 999) / 1000;
        let args = [
            ("count", earlier.to_string()),
            ("max", per_hour.to_string()),
        ];
        return Err(Failure::Op(
            messages::hint(
                messages::tag(
                    OpError::limit(messages::text("trash-hour-limit", &args), per_hour),
                    "trash-hour-limit",
                    &args,
                ),
                "trash-hour-limit-hint",
                &[("seconds", retry_after.to_string())],
            )
            .with_detail("retry_after", retry_after),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Rows in and out of the trash

fn table_of(entity: &str) -> &'static str {
    match entity {
        PROJECT => "projects",
        WIKI => "wiki_pages",
        _ => "tasks",
    }
}

/// `trashed` as history records it for the row `id` of `entity` (a task,
/// project or page): `true` while in the trash, absent while live.
pub(crate) fn raw_trashed(
    tx: &Transaction<'_>,
    entity: &str,
    id: Id,
) -> Result<Option<Value>, StoreError> {
    let trashed: Option<bool> = tx
        .query_row(
            &format!(
                "SELECT deleted_at IS NOT NULL FROM {} WHERE id = ?1",
                table_of(entity)
            ),
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(trashed.filter(|t| *t).map(Value::Bool))
}

/// `imported` as history records it for project `id`: `true` while live.
pub(crate) fn raw_imported(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM projects WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(live.filter(|l| *l).map(Value::Bool))
}

/// Moves the row `id` of `entity` (task, project or page) into the trash
/// or back, recording it as `field` (`trashed`, or `imported` for an
/// imported project, which is live while true). Returns whether it changed.
pub(crate) fn set_row(
    w: &mut WriteCtx<'_>,
    entity: &str,
    id: Id,
    project: Option<Id>,
    field: &str,
    trashed: bool,
) -> Result<bool, Failure> {
    let table = table_of(entity);
    let now_trashed: Option<bool> = w
        .tx()
        .query_row(
            &format!("SELECT deleted_at IS NOT NULL FROM {table} WHERE id = ?1"),
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    let Some(now_trashed) = now_trashed else {
        return Ok(false);
    };
    if now_trashed == trashed {
        return Ok(false);
    }
    if trashed {
        // A trashed page keeps its brief flag, so it is the brief again
        // when it comes back; the brief is read from live pages only.
        w.tx().execute(
            &format!("UPDATE {table} SET deleted_at = ?2, deleted_group = ?3 WHERE id = ?1"),
            params![b(id), w.now(), b(w.group())],
        )?;
        if entity == PROJECT {
            // The project's live tasks and pages go with it, in its group,
            // so everything that hides trashed tasks and pages hides them.
            for table in ["tasks", "wiki_pages"] {
                w.tx().execute(
                    &format!(
                        "UPDATE {table} SET deleted_at = ?2, deleted_group = ?3
                          WHERE project_id = ?1 AND deleted_at IS NULL"
                    ),
                    params![b(id), w.now(), b(w.group())],
                )?;
            }
        }
    } else {
        if entity == WIKI {
            wiki::ensure_title_free(w.tx(), id)?;
        }
        if entity == PROJECT {
            let group: Option<Vec<u8>> = w.tx().query_row(
                "SELECT deleted_group FROM projects WHERE id = ?1",
                [b(id)],
                |r| r.get(0),
            )?;
            if let Some(group) = group {
                for table in ["tasks", "wiki_pages"] {
                    w.tx().execute(
                        &format!(
                            "UPDATE {table} SET deleted_at = NULL, deleted_group = NULL
                              WHERE project_id = ?1 AND deleted_group = ?2"
                        ),
                        params![b(id), group],
                    )?;
                }
            }
        }
        w.tx().execute(
            &format!("UPDATE {table} SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1"),
            [b(id)],
        )?;
    }
    if entity == WIKI {
        wiki::reindex(w, id)?;
    }
    let mark = |t: bool| t.then_some(Value::Bool(true));
    let (before, after) = if field == TRASHED {
        (mark(now_trashed), mark(trashed))
    } else {
        (mark(!now_trashed), mark(!trashed))
    };
    w.record_field(entity, id, project, field, before, after);
    Ok(true)
}

/// The rows below `root` in `table` (by `parent_id`), each after its
/// parent; `filter` is added to the condition (`deleted_at IS NULL`, or a
/// group test with `?2`).
fn below(
    tx: &Transaction<'_>,
    table: &str,
    root: Id,
    filter: &str,
    group: Option<Id>,
) -> Result<Vec<Id>, Failure> {
    let sql = format!("SELECT id FROM {table} WHERE parent_id = ?1 AND {filter} ORDER BY id");
    let mut stmt = tx.prepare_cached(&sql)?;
    let mut out = Vec::new();
    let mut queue = vec![root];
    while let Some(parent) = queue.pop() {
        let children: Vec<Id> = if let Some(g) = group {
            stmt.query_map(params![b(parent), b(g)], |r| id_at(r, 0))?
                .collect::<Result<_, _>>()?
        } else {
            stmt.query_map(params![b(parent)], |r| id_at(r, 0))?
                .collect::<Result<_, _>>()?
        };
        for c in children {
            if !out.contains(&c) && c != root {
                out.push(c);
                queue.push(c);
            }
        }
        if out.len() > 1_000_000 {
            break;
        }
    }
    Ok(out)
}

/// The project of the row `id` in `table`.
fn project_of(tx: &Transaction<'_>, table: &str, id: Id) -> Result<Id, Failure> {
    Ok(tx.query_row(
        &format!("SELECT project_id FROM {table} WHERE id = ?1"),
        [b(id)],
        |r| id_at(r, 0),
    )?)
}

// ---------------------------------------------------------------------
// Deleting

fn delete_tasks(w: &mut WriteCtx<'_>, input: TaskDeleteInput) -> Result<TrashChange, Failure> {
    if input.keys.is_empty() || input.keys.len() > MAX_TASKS {
        return Err(messages::invalid(
            "keys",
            "trash-tasks-count",
            &[("max", MAX_TASKS.to_string())],
        ));
    }
    let mut roots = Vec::new();
    for (i, key) in input.keys.iter().enumerate() {
        let t = lookup::task(w.tx(), &format!("keys[{i}]"), key)?;
        if !roots.iter().any(|r: &lookup::TaskRow| r.id == t.id) {
            roots.push(t);
        }
    }
    for t in &roots {
        if let Some(p) = lookup::project_by_id(w.tx(), t.project)? {
            lookup::ensure_active(&p)?;
        }
    }
    let ids: HashSet<Id> = roots.iter().map(|t| t.id).collect();
    time::stop_before_trash(w, |tx, task| {
        // The timer's task, if live, or one of its ancestors is named.
        let mut at = Some(task);
        let mut first = true;
        while let Some(id) = at {
            let row: Option<(Option<Vec<u8>>, bool)> = tx
                .query_row(
                    "SELECT parent_id, deleted_at IS NULL FROM tasks WHERE id = ?1",
                    [b(id)],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((parent, live)) = row else {
                return Ok(false);
            };
            if first && !live {
                return Ok(false);
            }
            first = false;
            if ids.contains(&id) {
                return Ok(true);
            }
            at = parent.and_then(|p| Id::from_slice(&p).ok());
        }
        Ok(false)
    })?;
    let mut total = 0;
    let mut entries = Vec::new();
    for t in &roots {
        // A task named with one of its ancestors went with that one.
        if set_row(w, TASK, t.id, Some(t.project), TRASHED, true)? {
            total += 1;
            for child in below(w.tx(), "tasks", t.id, "deleted_at IS NULL", None)? {
                if set_row(w, TASK, child, Some(t.project), TRASHED, true)? {
                    total += 1;
                }
            }
            entries.push(t.id);
        }
    }
    finish_change(w, entries, total)
}

fn delete_page(w: &mut WriteCtx<'_>, input: WikiDeleteInput) -> Result<TrashChange, Failure> {
    let page = wiki::find_page(w.tx(), input.project.as_deref(), "page", &input.page)?;
    if let Some(p) = lookup::project_by_id(w.tx(), page.project)? {
        lookup::ensure_active(&p)?;
        lookup::ensure_wiki_on(w.tx(), &p)?;
    }
    let mut total = 0;
    if set_row(w, WIKI, page.id, Some(page.project), TRASHED, true)? {
        total += 1;
        for child in below(w.tx(), "wiki_pages", page.id, "deleted_at IS NULL", None)? {
            if set_row(w, WIKI, child, Some(page.project), TRASHED, true)? {
                total += 1;
            }
        }
    }
    finish_change(w, vec![page.id], total)
}

fn delete_project(
    w: &mut WriteCtx<'_>,
    input: ProjectDeleteInput,
) -> Result<ProjectDeleted, Failure> {
    ensure_full(w, "trash-project-needs-full")?;
    let root = lookup::project(w.tx(), "project", &input.project)?;
    let mut projects = vec![root.id];
    projects.extend(below(
        w.tx(),
        "projects",
        root.id,
        "deleted_at IS NULL",
        None,
    )?);
    let counts = project_delete::counts(w.tx(), &projects)?;
    if let Some(expected) = input.expected_task_count
        && expected != counts.tasks
    {
        let args = [
            ("project", root.key.clone()),
            ("count", counts.tasks.to_string()),
            ("expected", expected.to_string()),
        ];
        return Err(Failure::Op(
            messages::hint(
                messages::conflict("trash-project-count", &args),
                "trash-project-count-hint",
                &[],
            )
            .with_detail("tasks", counts.tasks),
        ));
    }
    let set: HashSet<Id> = projects.iter().copied().collect();
    time::stop_before_trash(w, |tx, task| {
        let project: Option<Vec<u8>> = tx
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [b(task)],
                |r| r.get(0),
            )
            .optional()?;
        Ok(project
            .and_then(|p| Id::from_slice(&p).ok())
            .is_some_and(|p| set.contains(&p)))
    })?;
    let mut keys = Vec::new();
    for p in &projects {
        let key: String =
            w.tx()
                .query_row("SELECT key FROM projects WHERE id = ?1", [b(*p)], |r| {
                    r.get(0)
                })?;
        keys.push(key);
        set_row(w, PROJECT, *p, Some(*p), TRASHED, true)?;
    }
    Ok(ProjectDeleted {
        project: root.key,
        projects: keys,
        tasks: counts.tasks,
        comments: counts.comments,
        files: counts.files,
        group: w.group().to_string(),
    })
}

fn finish_change(w: &WriteCtx<'_>, roots: Vec<Id>, total: u64) -> Result<TrashChange, Failure> {
    let days = settings::load(w.tx())?.trash_days;
    let mut items = Vec::new();
    for id in roots {
        if let Some(found) = locate(w.tx(), id)? {
            items.push(entry(w.tx(), &found, days)?);
        }
    }
    Ok(TrashChange {
        items,
        total,
        group: w.group_if_changed(),
    })
}

// ---------------------------------------------------------------------
// Finding entries

/// An item in the trash: its kind, ID and the facts every kind has.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    pub(crate) kind: TrashKind,
    pub(crate) id: Id,
    pub(crate) deleted_at: i64,
    pub(crate) group: Option<Id>,
}

/// The trashed item with ID `id`, in whichever table holds it.
pub(crate) fn locate(tx: &Transaction<'_>, id: Id) -> Result<Option<Found>, Failure> {
    for kind in TrashKind::ALL {
        let row: Option<(Option<i64>, Option<Vec<u8>>)> = tx
            .query_row(
                &format!(
                    "SELECT deleted_at, deleted_group FROM {} WHERE id = ?1",
                    kind_table(kind)
                ),
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((deleted_at, group)) = row {
            return Ok(deleted_at.map(|deleted_at| Found {
                kind,
                id,
                deleted_at,
                group: group.and_then(|g| Id::from_slice(&g).ok()),
            }));
        }
    }
    Ok(None)
}

const fn kind_table(kind: TrashKind) -> &'static str {
    match kind {
        TrashKind::Project => "projects",
        TrashKind::Task => "tasks",
        TrashKind::Comment => "comments",
        TrashKind::File => "attachments",
        TrashKind::TimeEntry => "time_entries",
        TrashKind::WikiPage => "wiki_pages",
    }
}

fn kind_of(text: &str) -> TrashKind {
    match text {
        "project" => TrashKind::Project,
        "comment" => TrashKind::Comment,
        "file" => TrashKind::File,
        "time_entry" => TrashKind::TimeEntry,
        "wiki_page" => TrashKind::WikiPage,
        _ => TrashKind::Task,
    }
}

/// Every entry of the trash: kind, ID, deletion time and its project.
const ENTRIES_SQL: &str = "
SELECT 'project' AS kind, p.id AS id, p.deleted_at AS at, p.id AS project_id
  FROM projects p
 WHERE p.deleted_at IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM projects q
                    WHERE q.id = p.parent_id AND q.deleted_group = p.deleted_group)
UNION ALL
SELECT 'task', t.id, t.deleted_at, t.project_id
  FROM tasks t
 WHERE t.deleted_at IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM tasks q
                    WHERE q.id = t.parent_id AND q.deleted_group = t.deleted_group)
   AND NOT EXISTS (SELECT 1 FROM projects q
                    WHERE q.id = t.project_id AND q.deleted_group = t.deleted_group)
UNION ALL
SELECT 'comment', c.id, c.deleted_at, t.project_id
  FROM comments c JOIN tasks t ON t.id = c.task_id
 WHERE c.deleted_at IS NOT NULL
UNION ALL
SELECT 'file', a.id, a.deleted_at,
       CASE a.owner_kind
         WHEN 'task' THEN (SELECT project_id FROM tasks WHERE id = a.owner_id)
         WHEN 'comment' THEN (SELECT t.project_id FROM comments c JOIN tasks t ON t.id = c.task_id
                               WHERE c.id = a.owner_id)
         WHEN 'wiki_page' THEN (SELECT project_id FROM wiki_pages WHERE id = a.owner_id)
         ELSE a.owner_id END
  FROM attachments a
 WHERE a.deleted_at IS NOT NULL
UNION ALL
SELECT 'time_entry', e.id, e.deleted_at, t.project_id
  FROM time_entries e JOIN tasks t ON t.id = e.task_id
 WHERE e.deleted_at IS NOT NULL
UNION ALL
SELECT 'wiki_page', w.id, w.deleted_at, w.project_id
  FROM wiki_pages w
 WHERE w.deleted_at IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM wiki_pages q
                    WHERE q.id = w.parent_id AND q.deleted_group = w.deleted_group)
   AND NOT EXISTS (SELECT 1 FROM projects q
                    WHERE q.id = w.project_id AND q.deleted_group = w.deleted_group)";

fn found_row(tx: &Transaction<'_>, kind: &str, id: Id, at: i64) -> Result<Found, Failure> {
    let kind = kind_of(kind);
    let group: Option<Vec<u8>> = tx.query_row(
        &format!(
            "SELECT deleted_group FROM {} WHERE id = ?1",
            kind_table(kind)
        ),
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(Found {
        kind,
        id,
        deleted_at: at,
        group: group.and_then(|g| Id::from_slice(&g).ok()),
    })
}

/// The project row, live or trashed, that `text` names (key or ID).
fn any_project(tx: &Transaction<'_>, text: &str) -> Result<Id, Failure> {
    let text = text.trim();
    let by: Option<Vec<u8>> = tx
        .query_row(
            "SELECT id FROM projects WHERE key = ?1 OR id = ?2",
            params![
                text.to_uppercase(),
                text.parse::<Id>().ok().map(b).unwrap_or_default()
            ],
            |r| r.get(0),
        )
        .optional()?;
    match by.and_then(|v| Id::from_slice(&v).ok()) {
        Some(id) if crate::scope::sees(id) => Ok(id),
        _ => lookup::project(tx, "project", text).map(|p| p.id),
    }
}

fn list(q: &QueryCtx<'_>, input: TrashListInput) -> Result<TrashList, Failure> {
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(messages::invalid(
            "limit",
            "list-limit",
            &[("max", MAX_LIMIT.to_string())],
        ));
    }
    let offset: u64 = match &input.cursor {
        Some(c) => c
            .parse()
            .map_err(|_| messages::invalid("cursor", "list-cursor", &[]))?,
        None => 0,
    };
    let mut filters = Vec::new();
    let mut args: Vec<SqlValue> = Vec::new();
    if let Some(p) = &input.project {
        filters.push("project_id = ?".to_owned());
        args.push(SqlValue::Blob(b(any_project(q.tx, p)?)));
    }
    // A caller limited to a project sees the trash of it and its
    // sub-projects only.
    if let Some(projects) = crate::scope::projects() {
        let marks = vec!["?"; projects.len()].join(", ");
        filters.push(format!("project_id IN ({marks})"));
        args.extend(projects.into_iter().map(|p| SqlValue::Blob(b(p))));
    }
    if !input.kinds.is_empty() {
        let marks = vec!["?"; input.kinds.len()].join(", ");
        filters.push(format!("kind IN ({marks})"));
        args.extend(
            input
                .kinds
                .iter()
                .map(|k| SqlValue::Text(k.as_str().to_owned())),
        );
    }
    let filter = if filters.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", filters.join(" AND "))
    };
    let total: i64 = q.tx.query_row(
        &format!("SELECT count(*) FROM ({ENTRIES_SQL}) {filter}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;
    let mut page_args = args.clone();
    page_args.push(SqlValue::Integer(i64::from(limit) + 1));
    page_args.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
    let mut stmt = q.tx.prepare(&format!(
        "SELECT kind, id, at FROM ({ENTRIES_SQL}) {filter}
          ORDER BY at DESC, kind, id LIMIT ? OFFSET ?"
    ))?;
    let rows = stmt
        .query_map(params_from_iter(page_args.iter()), |r| {
            Ok((r.get::<_, String>(0)?, id_at(r, 1)?, r.get::<_, i64>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let days = settings::load(q.tx)?.trash_days;
    let mut items = Vec::new();
    for (kind, id, at) in rows.iter().take(limit as usize) {
        let found = found_row(q.tx, kind, *id, *at)?;
        items.push(entry(q.tx, &found, days)?);
    }
    let next_cursor =
        (rows.len() > limit as usize).then(|| (offset + u64::from(limit)).to_string());
    Ok(TrashList {
        items,
        total: u64::try_from(total).unwrap_or(0),
        next_cursor,
        trash_days: days,
    })
}

fn excerpt(text: &str) -> String {
    let flat: String = gk_markdown::plain_text(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: String = flat.chars().take(80).collect();
    if flat.chars().count() > 80 {
        out.push('…');
    }
    out
}

/// The trash entry of `found`.
pub(crate) fn entry(tx: &Transaction<'_>, found: &Found, days: u32) -> Result<TrashEntry, Failure> {
    let id = found.id;
    let mut e = TrashEntry {
        id: id.to_string(),
        kind: found.kind,
        title: String::new(),
        key: None,
        project: None,
        task: None,
        minutes: None,
        size: None,
        deleted_at: rfc3339(found.deleted_at),
        deleted_by: None,
        purge_at: rfc3339(found.deleted_at + i64::from(days) * DAY_MS),
        contains: 0,
        group: found.group.map(|g| g.to_string()).unwrap_or_default(),
    };
    let task_key = |task: Vec<u8>| -> Result<(String, String), Failure> {
        Ok(tx.query_row(
            "SELECT p.key || '-' || t.number, p.key FROM tasks t
               JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
            [task],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    };
    let same_group = "deleted_group = ?2";
    match found.kind {
        TrashKind::Project => {
            let (key, name): (String, String) = tx.query_row(
                "SELECT key, name FROM projects WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            e.title = name;
            e.project = Some(key.clone());
            e.key = Some(key);
            let mut projects = vec![id];
            if let Some(g) = found.group {
                projects.extend(below(tx, "projects", id, same_group, Some(g))?);
            }
            let mut contains = projects.len() as u64 - 1;
            for p in &projects {
                let n: i64 = tx.query_row(
                    "SELECT (SELECT count(*) FROM tasks WHERE project_id = ?1
                               AND deleted_group IS ?2)
                          + (SELECT count(*) FROM wiki_pages WHERE project_id = ?1
                               AND deleted_group IS ?2)",
                    params![b(*p), found.group.map(b)],
                    |r| r.get(0),
                )?;
                contains += u64::try_from(n).unwrap_or(0);
            }
            e.contains = contains;
        }
        TrashKind::Task => {
            let (title, task): (String, Vec<u8>) =
                tx.query_row("SELECT title, id FROM tasks WHERE id = ?1", [b(id)], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
            let (key, project) = task_key(task)?;
            e.title = title;
            e.key = Some(key);
            e.project = Some(project);
            if let Some(g) = found.group {
                e.contains = below(tx, "tasks", id, same_group, Some(g))?.len() as u64;
            }
        }
        TrashKind::Comment => {
            let (body, task): (String, Vec<u8>) = tx.query_row(
                "SELECT body, task_id FROM comments WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let (key, project) = task_key(task)?;
            e.title = excerpt(&body);
            e.task = Some(key);
            e.project = Some(project);
        }
        TrashKind::File => {
            let (name, size, kind, owner): (String, i64, String, Vec<u8>) = tx.query_row(
                "SELECT file_name, size, owner_kind, owner_id FROM attachments WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            e.title = name;
            e.size = Some(u64::try_from(size).unwrap_or(0));
            let task = match kind.as_str() {
                "task" => Some(owner.clone()),
                "comment" => tx
                    .query_row(
                        "SELECT task_id FROM comments WHERE id = ?1",
                        [&owner],
                        |r| r.get::<_, Vec<u8>>(0),
                    )
                    .optional()?,
                _ => None,
            };
            if let Some(task) = task {
                let (key, project) = task_key(task)?;
                e.task = Some(key);
                e.project = Some(project);
            } else {
                let table = if kind == "wiki_page" {
                    "SELECT p.key FROM wiki_pages w JOIN projects p ON p.id = w.project_id
                      WHERE w.id = ?1"
                } else {
                    "SELECT key FROM projects WHERE id = ?1"
                };
                e.project = tx.query_row(table, [&owner], |r| r.get(0)).optional()?;
            }
        }
        TrashKind::TimeEntry => {
            let (note, minutes, task): (String, i64, Vec<u8>) = tx.query_row(
                "SELECT note, minutes, task_id FROM time_entries WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let (key, project) = task_key(task)?;
            e.title = note;
            e.minutes = Some(u32::try_from(minutes).unwrap_or(0));
            e.task = Some(key);
            e.project = Some(project);
        }
        TrashKind::WikiPage => {
            let (title, project): (String, String) = tx.query_row(
                "SELECT w.title, p.key FROM wiki_pages w JOIN projects p ON p.id = w.project_id
                  WHERE w.id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            e.title = title;
            e.project = Some(project);
            if let Some(g) = found.group {
                e.contains = below(tx, "wiki_pages", id, same_group, Some(g))?.len() as u64;
            }
        }
    }
    if let Some(g) = found.group {
        e.deleted_by = tx
            .query_row(
                "SELECT actor_kind, actor_name FROM activity WHERE group_id = ?1 LIMIT 1",
                [b(g)],
                |r| Ok(actor(&r.get::<_, String>(0)?, r.get(1)?)),
            )
            .optional()?;
    }
    Ok(e)
}

fn parse_items(field: &str, items: &[String], max: usize) -> Result<Vec<Id>, Failure> {
    if items.is_empty() || items.len() > max {
        return Err(messages::invalid(
            field,
            "trash-items-count",
            &[("max", max.to_string())],
        ));
    }
    let mut out = Vec::new();
    for (i, text) in items.iter().enumerate() {
        let id: Id = text.trim().parse().map_err(|_| {
            messages::invalid(
                &format!("{field}[{i}]"),
                "trash-item-not-found",
                &[("item", text.clone())],
            )
        })?;
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn not_in_trash(item: Id) -> Failure {
    Failure::Op(messages::hint(
        messages::not_found("trash-item-not-found", &[("item", item.to_string())]),
        "trash-item-not-found-hint",
        &[],
    ))
}

// ---------------------------------------------------------------------
// Restoring

/// Refuses to restore `found` while what holds it is in the trash too, or
/// its project is archived.
fn ensure_home(tx: &Transaction<'_>, found: &Found) -> Result<(), Failure> {
    let id = found.id;
    let first = |message: &str, what: Option<String>| -> Failure {
        let args = [
            ("item", id.to_string()),
            ("container", what.unwrap_or_default()),
        ];
        Failure::Op(messages::hint(
            messages::conflict(message, &args),
            "trash-restore-first-hint",
            &[],
        ))
    };
    let live_task = |task: Vec<u8>| -> Result<bool, Failure> {
        Ok(tx
            .query_row(
                "SELECT t.deleted_at IS NULL AND p.deleted_at IS NULL FROM tasks t
                   JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                [task],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false))
    };
    let project = match found.kind {
        TrashKind::Project => {
            let parent: Option<Vec<u8>> = tx.query_row(
                "SELECT parent_id FROM projects WHERE id = ?1",
                [b(id)],
                |r| r.get(0),
            )?;
            if let Some(parent) = parent {
                let key: Option<String> = tx
                    .query_row(
                        "SELECT key FROM projects WHERE id = ?1 AND deleted_at IS NOT NULL",
                        [&parent],
                        |r| r.get(0),
                    )
                    .optional()?;
                if key.is_some() {
                    return Err(first("trash-restore-project-first", key));
                }
            }
            None
        }
        TrashKind::Task => {
            let (project, parent): (Id, Option<Id>) = tx.query_row(
                "SELECT project_id, parent_id FROM tasks WHERE id = ?1",
                [b(id)],
                |r| Ok((id_at(r, 0)?, opt_id_at(r, 1)?)),
            )?;
            if let Some(parent) = parent
                && !live_task(b(parent))?
            {
                let key: Option<String> = tx
                    .query_row(
                        "SELECT p.key || '-' || t.number FROM tasks t
                           JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                        [b(parent)],
                        |r| r.get(0),
                    )
                    .optional()?;
                return Err(first("trash-restore-task-first", key));
            }
            Some(project)
        }
        TrashKind::Comment | TrashKind::TimeEntry => {
            let table = kind_table(found.kind);
            let task: Vec<u8> = tx.query_row(
                &format!("SELECT task_id FROM {table} WHERE id = ?1"),
                [b(id)],
                |r| r.get(0),
            )?;
            if !live_task(task.clone())? {
                let key: Option<String> = tx
                    .query_row(
                        "SELECT p.key || '-' || t.number FROM tasks t
                           JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                        [&task],
                        |r| r.get(0),
                    )
                    .optional()?;
                return Err(first("trash-restore-task-first", key));
            }
            Some(project_of(
                tx,
                "tasks",
                Id::from_slice(&task)
                    .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?,
            )?)
        }
        TrashKind::File => {
            let (kind, owner): (String, Vec<u8>) = tx.query_row(
                "SELECT owner_kind, owner_id FROM attachments WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let live: bool = match kind.as_str() {
                "task" => live_task(owner.clone())?,
                "comment" => tx
                    .query_row(
                        "SELECT c.deleted_at IS NULL AND t.deleted_at IS NULL FROM comments c
                           JOIN tasks t ON t.id = c.task_id WHERE c.id = ?1",
                        [&owner],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or(false),
                "wiki_page" => tx
                    .query_row(
                        "SELECT deleted_at IS NULL FROM wiki_pages WHERE id = ?1",
                        [&owner],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or(false),
                _ => tx
                    .query_row(
                        "SELECT deleted_at IS NULL FROM projects WHERE id = ?1",
                        [&owner],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or(false),
            };
            if !live {
                return Err(first("trash-restore-owner-first", None));
            }
            None
        }
        TrashKind::WikiPage => {
            let (project, parent): (Id, Option<Id>) = tx.query_row(
                "SELECT project_id, parent_id FROM wiki_pages WHERE id = ?1",
                [b(id)],
                |r| Ok((id_at(r, 0)?, opt_id_at(r, 1)?)),
            )?;
            if let Some(parent) = parent {
                let title: Option<String> = tx
                    .query_row(
                        "SELECT title FROM wiki_pages WHERE id = ?1 AND deleted_at IS NOT NULL",
                        [b(parent)],
                        |r| r.get(0),
                    )
                    .optional()?;
                if title.is_some() {
                    return Err(first("trash-restore-page-first", title));
                }
            }
            Some(project)
        }
    };
    if let Some(project) = project {
        match lookup::project_by_id(tx, project)? {
            Some(p) => lookup::ensure_active(&p)?,
            None => {
                let key: Option<String> = tx
                    .query_row(
                        "SELECT key FROM projects WHERE id = ?1",
                        [b(project)],
                        |r| r.get(0),
                    )
                    .optional()?;
                return Err(first("trash-restore-project-first", key));
            }
        }
    }
    Ok(())
}

fn restore(w: &mut WriteCtx<'_>, input: TrashRestoreInput) -> Result<TrashChange, Failure> {
    let ids = parse_items("items", &input.items, MAX_ITEMS)?;
    let days = settings::load(w.tx())?.trash_days;
    let mut items = Vec::new();
    let mut total = 0;
    for id in ids {
        let found = locate(w.tx(), id)?.ok_or_else(|| not_in_trash(id))?;
        ensure_home(w.tx(), &found)?;
        items.push(entry(w.tx(), &found, days)?);
        total += restore_one(w, &found)?;
    }
    Ok(TrashChange {
        items,
        total,
        group: w.group_if_changed(),
    })
}

/// Brings `found` back with what went into the trash with it. Returns how
/// many items came back.
fn restore_one(w: &mut WriteCtx<'_>, found: &Found) -> Result<u64, Failure> {
    let id = found.id;
    let subtree = |w: &WriteCtx<'_>, table: &str| -> Result<Vec<Id>, Failure> {
        match found.group {
            Some(g) => below(w.tx(), table, id, "deleted_group = ?2", Some(g)),
            None => Ok(Vec::new()),
        }
    };
    let mut n = 0;
    match found.kind {
        TrashKind::Project => {
            let mut all = vec![id];
            all.extend(subtree(w, "projects")?);
            for p in all {
                n += u64::from(set_row(w, PROJECT, p, Some(p), TRASHED, false)?);
            }
        }
        TrashKind::Task => {
            let project = project_of(w.tx(), "tasks", id)?;
            let mut all = vec![id];
            all.extend(subtree(w, "tasks")?);
            for t in all {
                n += u64::from(set_row(w, TASK, t, Some(project), TRASHED, false)?);
            }
        }
        TrashKind::WikiPage => {
            let project = project_of(w.tx(), "wiki_pages", id)?;
            let mut all = vec![id];
            all.extend(subtree(w, "wiki_pages")?);
            for p in all {
                n += u64::from(set_row(w, WIKI, p, Some(project), TRASHED, false)?);
            }
        }
        TrashKind::Comment => {
            let project = item_project(w.tx(), found)?;
            comment::set_live(w, id, project, TRASHED, None)?;
            n += 1;
        }
        TrashKind::File => {
            let project = item_project(w.tx(), found)?;
            file::set_live(w, id, project, TRASHED, None)?;
            n += 1;
        }
        TrashKind::TimeEntry => {
            let project = item_project(w.tx(), found)?;
            time::set_live(w, id, project, TRASHED, None)?;
            n += 1;
        }
    }
    Ok(n)
}

fn item_project(tx: &Transaction<'_>, found: &Found) -> Result<Option<Id>, Failure> {
    let sql = match found.kind {
        TrashKind::Comment => {
            "SELECT t.project_id FROM comments c JOIN tasks t ON t.id = c.task_id WHERE c.id = ?1"
        }
        TrashKind::TimeEntry => {
            "SELECT t.project_id FROM time_entries e JOIN tasks t ON t.id = e.task_id
              WHERE e.id = ?1"
        }
        _ => return Ok(None),
    };
    Ok(tx
        .query_row(sql, [b(found.id)], |r| id_at(r, 0))
        .optional()?)
}

// ---------------------------------------------------------------------
// Purging

/// What a purge removed, for the stored files to delete after the commit.
#[derive(Debug, Default)]
pub(crate) struct Purged {
    /// Stored files whose last attachment went.
    pub(crate) unused_files: Vec<String>,
}

/// Replaces the values of every history row about `ids` with a purged
/// marker: who and when stay, what is gone.
fn scrub(w: &WriteCtx<'_>, ids: &[Id]) -> Result<(), Failure> {
    let mut stmt = w.tx().prepare_cached(
        "UPDATE activity SET changes = '{\"fields\":{},\"purged\":true}', inverse = NULL
          WHERE entity_id = ?1",
    )?;
    for id in ids {
        stmt.execute([b(*id)])?;
    }
    Ok(())
}

fn ids_of(tx: &Transaction<'_>, sql: &str, owner: Id) -> Result<Vec<Id>, Failure> {
    let mut stmt = tx.prepare_cached(sql)?;
    let out = stmt
        .query_map([b(owner)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// Deletes task `task` and everything that hangs off it for good; its
/// subtasks must be gone already.
fn purge_task(w: &WriteCtx<'_>, task: Id, files: &mut Vec<String>) -> Result<(), Failure> {
    let tx = w.tx();
    let mut gone = vec![task];
    let comments = ids_of(tx, "SELECT id FROM comments WHERE task_id = ?1", task)?;
    for c in &comments {
        gone.extend(ids_of(
            tx,
            "SELECT id FROM attachments WHERE owner_kind = 'comment' AND owner_id = ?1",
            *c,
        )?);
    }
    gone.extend(&comments);
    gone.extend(ids_of(
        tx,
        "SELECT id FROM attachments WHERE owner_kind = 'task' AND owner_id = ?1",
        task,
    )?);
    gone.extend(ids_of(
        tx,
        "SELECT id FROM time_entries WHERE task_id = ?1",
        task,
    )?);
    gone.extend(ids_of(
        tx,
        "SELECT id FROM task_relations WHERE from_task = ?1 OR to_task = ?1",
        task,
    )?);
    scrub(w, &gone)?;
    tx.execute("DELETE FROM search_docs WHERE task_id = ?1", [b(task)])?;
    project_delete::clear_task(w, task, files)?;
    tx.execute("DELETE FROM tasks WHERE id = ?1", [b(task)])?;
    Ok(())
}

/// Deletes the task tree at `root` for good: subtasks first. Live
/// subtasks (there should be none) are moved to the top level instead.
fn purge_task_tree(w: &WriteCtx<'_>, root: Id, files: &mut Vec<String>) -> Result<u64, Failure> {
    let tx = w.tx();
    tx.execute(
        "UPDATE tasks SET parent_id = NULL, subtask_rank = NULL
          WHERE parent_id = ?1 AND deleted_at IS NULL",
        [b(root)],
    )?;
    let mut all = below(tx, "tasks", root, "deleted_at IS NOT NULL", None)?;
    all.insert(0, root);
    for t in &all {
        tx.execute(
            "UPDATE tasks SET parent_id = NULL, subtask_rank = NULL
              WHERE parent_id = ?1 AND deleted_at IS NULL",
            [b(*t)],
        )?;
    }
    for t in all.iter().rev() {
        purge_task(w, *t, files)?;
    }
    Ok(all.len() as u64)
}

fn collect(
    tx: &Transaction<'_>,
    sql: &str,
    owner: Id,
    files: &mut Vec<String>,
) -> Result<(), Failure> {
    let mut stmt = tx.prepare_cached(sql)?;
    let shas = stmt
        .query_map([b(owner)], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for sha in shas {
        if !files.contains(&sha) {
            files.push(sha);
        }
    }
    Ok(())
}

fn purge_comment(w: &WriteCtx<'_>, id: Id, files: &mut Vec<String>) -> Result<(), Failure> {
    let tx = w.tx();
    let mut gone = ids_of(
        tx,
        "SELECT id FROM attachments WHERE owner_kind = 'comment' AND owner_id = ?1",
        id,
    )?;
    collect(
        tx,
        "SELECT sha256 FROM attachments WHERE owner_kind = 'comment' AND owner_id = ?1",
        id,
        files,
    )?;
    for a in &gone {
        tx.execute(
            "DELETE FROM search_docs WHERE kind = 'file' AND entity_id = ?1",
            [b(*a)],
        )?;
    }
    gone.push(id);
    scrub(w, &gone)?;
    for sql in [
        "DELETE FROM attachments WHERE owner_kind = 'comment' AND owner_id = ?1",
        "DELETE FROM comment_revisions WHERE comment_id = ?1",
        "DELETE FROM drafts WHERE owner_kind = 'comment' AND owner_id = ?1",
        "UPDATE comments SET reply_to = NULL WHERE reply_to = ?1",
        "DELETE FROM search_docs WHERE kind = 'comment' AND entity_id = ?1",
        "DELETE FROM comments WHERE id = ?1",
    ] {
        tx.execute(sql, [b(id)])?;
    }
    links::forget(tx, links::Source::Comment(id))?;
    Ok(())
}

fn purge_file(w: &WriteCtx<'_>, id: Id, files: &mut Vec<String>) -> Result<(), Failure> {
    let tx = w.tx();
    collect(
        tx,
        "SELECT sha256 FROM attachments WHERE id = ?1",
        id,
        files,
    )?;
    scrub(w, &[id])?;
    tx.execute(
        "DELETE FROM search_docs WHERE kind = 'file' AND entity_id = ?1",
        [b(id)],
    )?;
    tx.execute("DELETE FROM attachments WHERE id = ?1", [b(id)])?;
    Ok(())
}

fn purge_time(w: &WriteCtx<'_>, id: Id) -> Result<(), Failure> {
    scrub(w, &[id])?;
    w.tx()
        .execute("DELETE FROM time_entries WHERE id = ?1", [b(id)])?;
    Ok(())
}

fn purge_page_tree(w: &WriteCtx<'_>, root: Id, files: &mut Vec<String>) -> Result<u64, Failure> {
    let tx = w.tx();
    let mut all = below(tx, "wiki_pages", root, "deleted_at IS NOT NULL", None)?;
    all.insert(0, root);
    for p in &all {
        tx.execute(
            "UPDATE wiki_pages SET parent_id = NULL WHERE parent_id = ?1 AND deleted_at IS NULL",
            [b(*p)],
        )?;
    }
    for p in all.iter().rev() {
        let mut gone = ids_of(
            tx,
            "SELECT id FROM attachments WHERE owner_kind = 'wiki_page' AND owner_id = ?1",
            *p,
        )?;
        collect(
            tx,
            "SELECT sha256 FROM attachments WHERE owner_kind = 'wiki_page' AND owner_id = ?1",
            *p,
            files,
        )?;
        gone.push(*p);
        scrub(w, &gone)?;
        for sql in [
            "DELETE FROM attachments WHERE owner_kind = 'wiki_page' AND owner_id = ?1",
            "DELETE FROM wiki_redirects WHERE page_id = ?1",
            "DELETE FROM wiki_revisions WHERE page_id = ?1",
            "DELETE FROM drafts WHERE owner_kind IN ('wiki_page', 'new_wiki_page')
               AND owner_id = ?1",
            "DELETE FROM search_docs WHERE kind = 'wiki' AND entity_id = ?1",
            "DELETE FROM wiki_pages WHERE id = ?1",
        ] {
            tx.execute(sql, [b(*p)])?;
        }
        links::forget(tx, links::Source::Wiki(*p))?;
    }
    Ok(all.len() as u64)
}

/// Purges the trash entry `found` and records it. Returns how many items
/// went.
fn purge_one(
    w: &mut WriteCtx<'_>,
    found: &Found,
    expired: bool,
    out: &mut Purged,
) -> Result<u64, Failure> {
    let id = found.id;
    let mut files = Vec::new();
    let (entity, project, n, key): (&str, Option<Id>, u64, Option<String>) = match found.kind {
        TrashKind::Project => {
            let key: Option<String> = w
                .tx()
                .query_row("SELECT key FROM projects WHERE id = ?1", [b(id)], |r| {
                    r.get(0)
                })
                .optional()?;
            let n = project_delete::purge_projects(w, id, &mut files)?;
            (PROJECT, Some(id), n, key)
        }
        TrashKind::Task => {
            let project = project_of(w.tx(), "tasks", id)?;
            let key: Option<String> = w
                .tx()
                .query_row(
                    "SELECT p.key || '-' || t.number FROM tasks t
                       JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                    [b(id)],
                    |r| r.get(0),
                )
                .optional()?;
            let n = purge_task_tree(w, id, &mut files)?;
            (TASK, Some(project), n, key)
        }
        TrashKind::Comment => {
            let project = item_project(w.tx(), found)?;
            purge_comment(w, id, &mut files)?;
            (comment::ENTITY, project, 1, None)
        }
        TrashKind::File => {
            purge_file(w, id, &mut files)?;
            (file::ENTITY, None, 1, None)
        }
        TrashKind::TimeEntry => {
            let project = item_project(w.tx(), found)?;
            purge_time(w, id)?;
            (time::ENTRY, project, 1, None)
        }
        TrashKind::WikiPage => {
            let project = project_of(w.tx(), "wiki_pages", id)?;
            let n = purge_page_tree(w, id, &mut files)?;
            (WIKI, Some(project), n, None)
        }
    };
    out.unused_files.extend(file::drop_unused_rows(w, &files)?);
    let mut record = json!({ "items": n });
    if let Some(key) = key {
        record["key"] = Value::String(key);
    }
    if expired {
        record["expired"] = Value::Bool(true);
    }
    w.record_field(entity, id, project, PURGED, Some(record), None);
    Ok(n)
}

/// Purges the entries `input` names (or all), inside a command. The
/// stored files to remove after the commit go to `out`.
pub(crate) fn purge(
    w: &mut WriteCtx<'_>,
    input: &TrashPurgeInput,
    out: &mut Purged,
) -> Result<TrashChange, Failure> {
    let days = settings::load(w.tx())?.trash_days;
    let roots: Vec<Found> = if input.all {
        if w.caller().is_agent() {
            return Err(Failure::Op(messages::hint(
                messages::tag(
                    OpError::forbidden(messages::text("trash-purge-all-agents", &[])),
                    "trash-purge-all-agents",
                    &[],
                ),
                "trash-purge-all-agents-hint",
                &[],
            )));
        }
        if !input.items.is_empty() {
            return Err(messages::invalid("items", "trash-purge-all-and-items", &[]));
        }
        all_entries(w.tx(), None)?
    } else {
        let ids = parse_items("items", &input.items, MAX_ITEMS)?;
        let mut roots = Vec::new();
        for id in ids {
            roots.push(locate(w.tx(), id)?.ok_or_else(|| not_in_trash(id))?);
        }
        roots
    };
    purge_found(w, roots, days, false, out)
}

fn purge_found(
    w: &mut WriteCtx<'_>,
    mut roots: Vec<Found>,
    days: u32,
    expired: bool,
    out: &mut Purged,
) -> Result<TrashChange, Failure> {
    // Items first, then the tasks and pages that may hold them, projects
    // last: an entry inside one purged earlier is skipped.
    let order = |k: TrashKind| match k {
        TrashKind::Comment | TrashKind::File | TrashKind::TimeEntry => 0,
        TrashKind::Task | TrashKind::WikiPage => 1,
        TrashKind::Project => 2,
    };
    roots.sort_by_key(|f| order(f.kind));
    let mut items = Vec::new();
    let mut total = 0;
    let mut seen = HashSet::new();
    for found in roots {
        if !seen.insert(found.id) {
            continue;
        }
        // Still there and still in the trash?
        let Some(now) = locate(w.tx(), found.id)? else {
            continue;
        };
        items.push(entry(w.tx(), &now, days)?);
        total += purge_one(w, &now, expired, out)?;
    }
    Ok(TrashChange {
        items,
        total,
        group: w.group_if_changed(),
    })
}

/// Every entry of the trash deleted before `before` (all when `None`).
fn all_entries(tx: &Transaction<'_>, before: Option<i64>) -> Result<Vec<Found>, Failure> {
    let mut stmt = tx.prepare(&format!(
        "SELECT kind, id, at FROM ({ENTRIES_SQL}) WHERE at < ?1 ORDER BY at, id"
    ))?;
    let rows = stmt
        .query_map([before.unwrap_or(i64::MAX)], |r| {
            Ok((r.get::<_, String>(0)?, id_at(r, 1)?, r.get::<_, i64>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(kind, id, at)| found_row(tx, &kind, id, at))
        .collect()
}

/// Purges what has been in the trash longer than the `trash_days`
/// setting, as of `now`.
pub(crate) fn purge_expired(
    w: &mut WriteCtx<'_>,
    out: &mut Purged,
) -> Result<TrashChange, Failure> {
    let days = settings::load(w.tx())?.trash_days;
    let cutoff = w.now() - i64::from(days) * DAY_MS;
    let roots = all_entries(w.tx(), Some(cutoff))?;
    purge_found(w, roots, days, true, out)
}
