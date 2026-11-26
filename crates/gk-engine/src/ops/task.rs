// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Tasks: create, read, change and move.
//!
//! Rules: only the title is needed; a new task gets the project's next
//! number, its first to-do status, its default type and no priority, at the
//! bottom of its column; numbers never come back; subtasks stay in their
//! parent's project, at most 5 levels, without loops; a start after the due
//! date is refused; entering a done or cancelled status closes a task and
//! leaving it reopens it; a task moved to another project takes its subtasks
//! along, gets new keys there, and its old keys keep finding it.
//!
//! Subtasks keep an order of their own under their parent (`subtask_rank`),
//! apart from their places on the board: a new subtask comes last, and
//! `task.reorder_subtask` moves one within the list.

use gk_api::ErrorCode;
use gk_api::ops::common::{Actor, ActorKind, Category, Glyph, Hue, Priority};
use gk_api::ops::task::{
    Checklist, Claim, LabelRef, MoveCheck, MoveMapping, MoveMatch, MoveRequiredField, MovedKey,
    StatusRef, SubtaskReorderInput, Task, TaskChange, TaskCreate, TaskCreateInput, TaskField,
    TaskGet, TaskGetInput, TaskList, TaskMove, TaskMoveCheck, TaskMoveCheckInput, TaskMoveInput,
    TaskPart, TaskRef, TaskReorderSubtask, TaskUpdate, TaskUpdateInput, TypeRef,
};
use gk_core::{Day, DurationUnits, Id, Rank, StatusCategory, fold};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::description::{self, CONFLICTS_SQL, Owner};
use super::fields::{self, TASK, id_value, priority_name, text};
use super::lookup::{self, List, ProjectRow, Target, TaskRow, b, id_at, opt_id_at};
use super::project::{enum_text, parse_enum};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// Most levels of subtasks: a task and four levels below it.
pub(crate) const MAX_LEVELS: usize = 5;
/// Most tasks one read returns.
const MAX_KEYS: usize = 20;
/// Most tasks one move check looks at, as many as one bulk change takes.
const MAX_MOVE_CHECK: usize = 500;

pub(super) fn register(r: &mut Registry) {
    r.query::<TaskGet, _>(desc::<TaskGet>(), |q, input: TaskGetInput| {
        if input.keys.is_empty() || input.keys.len() > MAX_KEYS {
            return Err(messages::invalid(
                "keys",
                "task-get-count",
                &[("max", MAX_KEYS.to_string())],
            ));
        }
        let units = units(q.tx)?;
        let mut items = Vec::with_capacity(input.keys.len());
        for (i, key) in input.keys.iter().enumerate() {
            let t = lookup::task(q.tx, &format!("keys[{i}]"), key)?;
            let mut task = load(q.tx, t.id, units, q.now)?;
            add_parts(q.tx, &q.env.zone, &mut task, t.id, &input.include)?;
            items.push(task);
        }
        Ok(TaskList {
            total: items.len() as u64,
            items,
        })
    });
    r.command::<TaskCreate, _>(desc::<TaskCreate>(), create);
    r.command::<TaskUpdate, _>(desc::<TaskUpdate>(), update);
    r.command::<TaskMove, _>(desc::<TaskMove>(), move_task);
    r.query::<TaskMoveCheck, _>(desc::<TaskMoveCheck>(), |q, input: TaskMoveCheckInput| {
        move_check(q.tx, &input)
    });
    r.command::<TaskReorderSubtask, _>(desc::<TaskReorderSubtask>(), reorder_subtask);
}

/// The workspace's working day and week, for durations.
pub(crate) fn units(tx: &Transaction<'_>) -> Result<DurationUnits, Failure> {
    let s = super::settings::load(tx)?;
    Ok(DurationUnits::new(s.hours_per_day, s.days_per_week).unwrap_or_default())
}

pub(crate) fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    crate::messages::invalid(field, e.message_id(), &e.message_args())
}

// ---------------------------------------------------------------------
// Labels and checklists

/// The task's label IDs, sorted.
pub(crate) fn label_ids(tx: &Transaction<'_>, task: Id) -> Result<Vec<Id>, Failure> {
    let mut stmt =
        tx.prepare_cached("SELECT label_id FROM task_labels WHERE task_id = ?1 ORDER BY label_id")?;
    let ids = stmt
        .query_map([b(task)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Label IDs as history records them: a sorted list, or nothing.
pub(crate) fn labels_value(ids: &[Id]) -> Option<Value> {
    if ids.is_empty() {
        return None;
    }
    let mut ids: Vec<Id> = ids.to_vec();
    ids.sort_by_key(|id| *id.as_bytes());
    ids.dedup();
    Some(Value::Array(
        ids.iter().map(|id| Value::String(id.to_string())).collect(),
    ))
}

/// Writes the task's labels without recording them.
pub(crate) fn write_labels(w: &WriteCtx<'_>, task: Id, ids: &[Id]) -> Result<(), Failure> {
    w.tx()
        .execute("DELETE FROM task_labels WHERE task_id = ?1", [b(task)])?;
    for id in ids {
        w.tx().execute(
            "INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
            params![b(task), b(*id)],
        )?;
    }
    fields::touch(w, TASK, task, "labels")
}

/// Gives the task exactly these labels and records the change.
pub(crate) fn set_labels(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    ids: &[Id],
) -> Result<bool, Failure> {
    let before = labels_value(&label_ids(w.tx(), task)?);
    let after = labels_value(ids);
    if before == after {
        return Ok(false);
    }
    write_labels(w, task, ids)?;
    w.record_field(TASK, task, Some(project), "labels", before, after);
    Ok(true)
}

/// Keeps the task's search entry and checklist counts in step with its
/// title and description.
pub(crate) fn refresh_text(w: &WriteCtx<'_>, task: Id) -> Result<(), Failure> {
    let (project, title, description): (Vec<u8>, String, String) = w.tx().query_row(
        "SELECT project_id, title, description FROM tasks WHERE id = ?1",
        [b(task)],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let analysis = gk_markdown::analyze(&description);
    super::links::index_refs(
        w.tx(),
        super::links::Source::Task(task),
        &analysis.references,
    )?;
    let total = analysis.checklist.len();
    let done = analysis.checklist.iter().filter(|i| i.checked).count();
    w.tx().execute(
        "UPDATE tasks SET checklist_done = ?2, checklist_total = ?3 WHERE id = ?1",
        params![
            b(task),
            i64::try_from(done).unwrap_or(i64::MAX),
            i64::try_from(total).unwrap_or(i64::MAX)
        ],
    )?;
    let body = super::custom_field::indexed_body(w.tx(), task, &analysis.plain_text)?;
    w.tx().execute(
        "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body, updated_at)
         VALUES ('task', ?1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (kind, entity_id) DO UPDATE SET project_id = excluded.project_id,
           title = excluded.title, body = excluded.body, updated_at = excluded.updated_at",
        params![b(task), project, title, body, w.now()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------
// Reading

/// Whether the task `t` is blocked: a task that blocks it is still open.
pub(crate) const BLOCKED_SQL: &str = "EXISTS (SELECT 1 FROM task_relations r
      JOIN tasks o ON o.id = r.from_task
     WHERE r.to_task = t.id AND r.kind = 'blocks' AND o.deleted_at IS NULL
       AND o.closed_at IS NULL)";
/// The live comments of the task `t`.
pub(crate) const COMMENTS_SQL: &str =
    "(SELECT count(*) FROM comments c WHERE c.task_id = t.id AND c.deleted_at IS NULL)";
/// The live files of the task `t` and of its live comments.
pub(crate) const FILES_SQL: &str = "(SELECT count(*) FROM attachments a
     WHERE a.deleted_at IS NULL
       AND ((a.owner_kind = 'task' AND a.owner_id = t.id)
         OR (a.owner_kind = 'comment' AND a.owner_id IN
               (SELECT c.id FROM comments c WHERE c.task_id = t.id AND c.deleted_at IS NULL))))";
/// The live subtasks of the task `t`.
pub(crate) const SUBTASKS_SQL: &str =
    "(SELECT count(*) FROM tasks c WHERE c.parent_id = t.id AND c.deleted_at IS NULL)";
/// The closed live subtasks of the task `t`.
pub(crate) const CLOSED_SUBTASKS_SQL: &str = "(SELECT count(*) FROM tasks c
     WHERE c.parent_id = t.id AND c.deleted_at IS NULL AND c.closed_at IS NOT NULL)";

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

/// The claim stored in `session`/`name`/`until`, if it still lasts at
/// `now`.
pub(crate) fn live_claim(
    session: Option<Id>,
    name: Option<String>,
    until: Option<i64>,
    now: i64,
) -> Option<Claim> {
    match (session, until) {
        (Some(session), Some(until)) if until > now => Some(Claim {
            agent: name.unwrap_or_default(),
            session: session.to_string(),
            until: rfc3339(until),
        }),
        _ => None,
    }
}

/// One task as outputs carry it, with the claim as it stands at `now`.
pub(crate) fn load(
    tx: &Transaction<'_>,
    id: Id,
    units: DurationUnits,
    now: i64,
) -> Result<Task, Failure> {
    let sql = format!(
        "SELECT t.number, p.key, t.title, t.description, s.id, s.name, s.category,
                y.id, y.name, y.glyph, t.priority, t.parent_id, t.start_date, t.due_date,
                t.estimate_min, t.progress_override, t.spent_min, t.checklist_done,
                t.checklist_total, t.rank, t.created_by_kind, t.created_by_name, t.created_at,
                t.edited_at, t.updated_at, t.status_entered_at, t.closed_at, t.version,
                {SUBTASKS_SQL}, {CLOSED_SUBTASKS_SQL}, {BLOCKED_SQL}, {COMMENTS_SQL}, {FILES_SQL},
                t.claim_session, t.claim_name, t.claim_until, t.description_rev, {CONFLICTS_SQL}
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
          WHERE t.id = ?1"
    );
    let row = tx.query_row(&sql, [b(id)], |r| {
        let estimate: Option<i64> = r.get(14)?;
        Ok((
            Task {
                key: format!("{}-{}", r.get::<_, String>(1)?, r.get::<_, i64>(0)?),
                title: r.get(2)?,
                project: r.get(1)?,
                status: StatusRef {
                    id: id_at(r, 4)?.to_string(),
                    name: r.get(5)?,
                    category: parse_enum(&r.get::<_, String>(6)?).unwrap_or(Category::Todo),
                },
                task_type: TypeRef {
                    id: id_at(r, 7)?.to_string(),
                    name: r.get(8)?,
                    glyph: parse_enum(&r.get::<_, String>(9)?).unwrap_or(Glyph::Task),
                },
                priority: parse_enum(priority_name(r.get(10)?)).unwrap_or(Priority::None),
                labels: Vec::new(),
                fields: Vec::new(),
                parent: None,
                fix_version: None,
                description: r.get(3)?,
                description_revision: opt_id_at(r, 36)?.map(|id| id.to_string()),
                conflict_copies: count(r.get(37)?),
                start_date: r.get(12)?,
                due_date: r.get(13)?,
                estimate: estimate.map(|m| gk_core::format_duration(m, units)),
                estimate_minutes: estimate,
                progress: r
                    .get::<_, Option<i64>>(15)?
                    .and_then(|p| u8::try_from(p).ok()),
                spent_minutes: r.get(16)?,
                checklist: Checklist {
                    done: u32::try_from(r.get::<_, i64>(17)?).unwrap_or(0),
                    total: u32::try_from(r.get::<_, i64>(18)?).unwrap_or(0),
                },
                progress_percent: 0,
                totals: None,
                subtask_count: Checklist {
                    done: count(r.get(29)?),
                    total: count(r.get(28)?),
                },
                blocked: r.get(30)?,
                comment_count: count(r.get(31)?),
                file_count: count(r.get(32)?),
                claim: live_claim(opt_id_at(r, 33)?, r.get(34)?, r.get(35)?, now),
                subtasks: None,
                links: None,
                comments: None,
                files: None,
                checklist_items: None,
                revisions: None,
                time_entries: None,
                backlinks: None,
                references: None,
                reminder: None,
                recurrence: None,
                rank: r.get(19)?,
                old_keys: Vec::new(),
                created_by: Actor {
                    kind: if r.get::<_, String>(20)? == "agent" {
                        ActorKind::Agent
                    } else {
                        ActorKind::User
                    },
                    name: r.get(21)?,
                },
                created_at: rfc3339(r.get(22)?),
                edited_at: rfc3339(r.get(23)?),
                updated_at: rfc3339(r.get(24)?),
                status_entered_at: rfc3339(r.get(25)?),
                closed_at: r.get::<_, Option<i64>>(26)?.map(rfc3339),
                version: r.get(27)?,
                id: id.to_string(),
            },
            opt_id_at(r, 11)?,
        ))
    })?;
    let (mut task, parent) = row;
    if let Some(parent) = parent {
        let (project, number): (Vec<u8>, i64) = tx.query_row(
            "SELECT project_id, number FROM tasks WHERE id = ?1",
            [b(parent)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let project = Id::from_slice(&project)
            .map_err(|e| Failure::Store(gk_store::StoreError::Corrupt(e.to_string())))?;
        task.parent = Some(lookup::task_key(tx, project, number)?);
    }
    task.labels = task_labels(tx, id)?;
    task.recurrence = super::recurrence::of_task(tx, id)?;
    task.fix_version = super::version::of_task(tx, id)?;
    task.fields = super::custom_field::task_values(tx, id)?;
    task.progress_percent = super::rollup::progress(tx, id)?;
    if task.subtask_count.total > 0 {
        task.totals = super::rollup::totals(tx, id)?;
    }
    let mut stmt = tx.prepare_cached(
        "SELECT project_key || '-' || number FROM task_key_aliases WHERE task_id = ?1
          ORDER BY project_key, number",
    )?;
    task.old_keys = stmt
        .query_map([b(id)], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(task)
}

/// The task's labels in the project's order.
pub(crate) fn task_labels(tx: &Transaction<'_>, id: Id) -> Result<Vec<LabelRef>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT l.id, l.name, l.hue FROM task_labels tl JOIN labels l ON l.id = tl.label_id
          WHERE tl.task_id = ?1 ORDER BY l.rank, l.id",
    )?;
    let labels = stmt
        .query_map([b(id)], |r| {
            Ok(LabelRef {
                id: id_at(r, 0)?.to_string(),
                name: r.get(1)?,
                hue: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Hue::Grey),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(labels)
}

/// Adds the parts `include` asks for to `task`.
pub(crate) fn add_parts(
    tx: &Transaction<'_>,
    zone: &crate::time::Zone,
    task: &mut Task,
    id: Id,
    include: &[TaskPart],
) -> Result<(), Failure> {
    for part in include {
        match part {
            TaskPart::Subtasks => task.subtasks = Some(subtasks(tx, id)?),
            TaskPart::Links => task.links = Some(super::relation::links(tx, id)?),
            TaskPart::Comments => task.comments = Some(super::comment::list(tx, id, false)?),
            TaskPart::Files => task.files = Some(super::file::of_task(tx, id)?),
            TaskPart::Checklist => {
                task.checklist_items = Some(description::checklist_items(&task.description));
            }
            TaskPart::Revisions => {
                task.revisions = Some(description::revisions(tx, Owner::Task(id))?);
            }
            TaskPart::Time => {
                task.time_entries = Some(super::time::of_task(tx, id, units(tx)?)?);
            }
            TaskPart::Reminder => task.reminder = super::reminder::of_task(tx, id, zone)?,
            TaskPart::Backlinks => task.backlinks = Some(super::links::task_backlinks(tx, id)?),
            TaskPart::References => {
                task.references = Some(super::links::task_references(tx, id)?);
            }
        }
    }
    Ok(())
}

/// Another task as links and subtask lists show it.
pub(crate) fn task_ref(tx: &Transaction<'_>, id: Id) -> Result<TaskRef, Failure> {
    Ok(tx.query_row(
        "SELECT p.key || '-' || t.number, t.title, s.id, s.name, s.category, t.closed_at
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id
          WHERE t.id = ?1",
        [b(id)],
        |r| {
            Ok(TaskRef {
                key: r.get(0)?,
                title: r.get(1)?,
                status: StatusRef {
                    id: id_at(r, 2)?.to_string(),
                    name: r.get(3)?,
                    category: parse_enum(&r.get::<_, String>(4)?).unwrap_or(Category::Todo),
                },
                closed: r.get::<_, Option<i64>>(5)?.is_some(),
                id: id.to_string(),
            })
        },
    )?)
}

/// The live subtasks of `id`, in their own order.
fn subtasks(tx: &Transaction<'_>, id: Id) -> Result<Vec<TaskRef>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT t.id FROM tasks t JOIN statuses s ON s.id = t.status_id
          WHERE t.parent_id = ?1 AND t.deleted_at IS NULL
          ORDER BY t.subtask_rank, s.rank, t.rank, t.id",
    )?;
    let ids = stmt
        .query_map([b(id)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter().map(|c| task_ref(tx, c)).collect()
}

/// The task after a change, with `include` added.
pub(crate) fn changed_with(
    w: &WriteCtx<'_>,
    id: Id,
    warnings: Vec<String>,
    include: &[TaskPart],
) -> Result<TaskChange, Failure> {
    let mut task = load(w.tx(), id, units(w.tx())?, w.now())?;
    add_parts(w.tx(), w.zone(), &mut task, id, include)?;
    Ok(TaskChange {
        task,
        group: w.group_if_changed(),
        warnings,
        description_save: None,
        moved: Vec::new(),
        duplicates_closed: Vec::new(),
    })
}

pub(crate) fn changed(
    w: &WriteCtx<'_>,
    id: Id,
    warnings: Vec<String>,
) -> Result<TaskChange, Failure> {
    changed_with(w, id, warnings, &[])
}

// ---------------------------------------------------------------------
// Values

/// A date input: `YYYY-MM-DD`, or a natural form (`today`, `fri`, `next
/// mon`, `in 3d`, `+3d`, `dec 1`) read in the workspace's time zone.
pub(crate) fn date_input(w: &WriteCtx<'_>, field: &str, text: &str) -> Result<Day, Failure> {
    parse_date(w.tx(), w.zone(), w.now(), field, text)
}

/// [`date_input`] outside a command: `now` and `zone` give "today".
pub(crate) fn parse_date(
    tx: &Transaction<'_>,
    zone: &crate::time::Zone,
    now: i64,
    field: &str,
    text: &str,
) -> Result<Day, Failure> {
    let trimmed = text.trim();
    match trimmed.parse::<Day>() {
        Ok(day) => return Ok(day),
        // Written as a date, but not a real one: say what is wrong with it.
        Err(e)
            if trimmed.len() >= 5
                && trimmed[..4].bytes().all(|c| c.is_ascii_digit())
                && trimmed.as_bytes()[4] == b'-' =>
        {
            return Err(invalid(field, e));
        }
        Err(_) => {}
    }
    let cal = super::query::calendar(tx, zone, now)?;
    gk_query::parse_natural(trimmed, &cal).ok_or_else(|| {
        crate::messages::invalid(field, "date-invalid", &[("text", trimmed.to_owned())])
    })
}

fn duration(field: &str, text: &str, units: DurationUnits) -> Result<i64, Failure> {
    gk_core::parse_minutes(text, units).map_err(|e| invalid(field, e))
}

fn priority_value(p: Priority) -> Option<Value> {
    Some(Value::String(enum_text(p)))
}

pub(crate) fn category_of(tx: &Transaction<'_>, status: Id) -> Result<StatusCategory, Failure> {
    let text: String = tx.query_row(
        "SELECT category FROM statuses WHERE id = ?1",
        [b(status)],
        |r| r.get(0),
    )?;
    Ok(text.parse().unwrap_or(StatusCategory::Todo))
}

/// The project's first to-do status, else its first status.
pub(crate) fn first_status(tx: &Transaction<'_>, project: &ProjectRow) -> Result<Id, Failure> {
    let statuses = lookup::all(tx, List::Statuses, project.id)?;
    statuses
        .iter()
        .find(|s| s.kind == "todo")
        .or_else(|| statuses.first())
        .map(|s| s.id)
        .ok_or_else(|| {
            Failure::Op(messages::error(
                ErrorCode::Invalid,
                "project-no-statuses",
                &[("project", project.key.clone())],
            ))
        })
}

pub(crate) fn default_type(tx: &Transaction<'_>, project: &ProjectRow) -> Result<Id, Failure> {
    let found: Option<Id> = tx
        .query_row(
            "SELECT id FROM task_types WHERE project_id = ?1
              ORDER BY is_default DESC, rank, id LIMIT 1",
            [b(project.id)],
            |r| id_at(r, 0),
        )
        .optional()?;
    found.ok_or_else(|| {
        Failure::Op(messages::error(
            ErrorCode::Invalid,
            "project-no-types",
            &[("project", project.key.clone())],
        ))
    })
}

pub(crate) fn resolve_labels(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    field: &str,
    names: &[String],
) -> Result<Vec<Id>, Failure> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            lookup::named(tx, List::Labels, project, &format!("{field}[{i}]"), n).map(|l| l.id)
        })
        .collect()
}

/// The levels from the top task down to `id` (1 for a task without a
/// parent).
pub(crate) fn level_of(tx: &Transaction<'_>, id: Id) -> Result<usize, Failure> {
    let mut level = 1;
    let mut next = Some(id);
    while let Some(current) = next {
        next = tx
            .query_row(
                "SELECT parent_id FROM tasks WHERE id = ?1",
                [b(current)],
                |r| opt_id_at(r, 0),
            )
            .optional()?
            .flatten();
        if next.is_some() {
            level += 1;
        }
        if level > 64 {
            break;
        }
    }
    Ok(level)
}

/// The task `id` and all its live subtasks, parents first.
pub(crate) fn descendants(tx: &Transaction<'_>, id: Id) -> Result<Vec<Id>, Failure> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let mut stmt = tx.prepare_cached(
            "SELECT id FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL
              ORDER BY subtask_rank, rank, id",
        )?;
        let children = stmt
            .query_map([b(out[i])], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?;
        out.extend(children);
        i += 1;
        if out.len() > 100_000 {
            break;
        }
    }
    Ok(out)
}

/// Levels below `id`: 0 without subtasks.
fn height(tx: &Transaction<'_>, id: Id) -> Result<usize, Failure> {
    let mut stmt =
        tx.prepare_cached("SELECT id FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL")?;
    let children = stmt
        .query_map([b(id)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut most = 0;
    for c in children {
        most = most.max(1 + height(tx, c)?);
    }
    Ok(most)
}

/// Checks that `task` (with its subtasks) may go under `parent`. `key` is
/// the task's key; `None` for a task not made yet.
pub(crate) fn check_parent(
    tx: &Transaction<'_>,
    task: Option<Id>,
    project: Id,
    parent: &TaskRow,
    key: Option<&str>,
) -> Result<(), Failure> {
    if parent.project != project {
        return Err(match key {
            Some(key) => messages::invalid(
                "parent",
                "task-parent-other-project",
                &[("task", key.to_owned())],
            ),
            None => messages::invalid("parent", "task-new-parent-other-project", &[]),
        });
    }
    let below = match task {
        Some(t) => {
            if descendants(tx, t)?.contains(&parent.id) {
                return Err(messages::invalid(
                    "parent",
                    "task-parent-loop",
                    &[("task", key.unwrap_or_default().to_owned())],
                ));
            }
            height(tx, t)?
        }
        None => 0,
    };
    let levels = level_of(tx, parent.id)? + 1 + below;
    if levels > MAX_LEVELS {
        return Err(messages::invalid(
            "parent",
            "task-too-deep",
            &[
                ("max", MAX_LEVELS.to_string()),
                ("levels", levels.to_string()),
            ],
        ));
    }
    Ok(())
}

fn check_dates(start: Option<&Value>, due: Option<&Value>) -> Result<(), Failure> {
    let day_of = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .and_then(|s| s.parse::<Day>().ok())
    };
    gk_core::check_date_range(day_of(start), day_of(due)).map_err(|e| invalid("start_date", e))
}

/// The task's place in the column of `status` in `project`.
fn column_target(
    tx: &Transaction<'_>,
    status: Id,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<gk_api::ops::common::Edge>,
) -> Result<Option<Target>, Failure> {
    let in_column = |field: &str, text: &str| -> Result<Id, Failure> {
        let t = lookup::task(tx, field, text)?;
        if t.status != status {
            return Err(messages::invalid(
                field,
                "task-not-in-column",
                &[("task", text.to_owned())],
            ));
        }
        Ok(t.id)
    };
    let before = before.map(|t| in_column("before", t)).transpose()?;
    let after = after.map(|t| in_column("after", t)).transpose()?;
    Target::from_parts(before, after, at)
}

/// New ranks for placing `task` in the column of `status`; ranks of other
/// tasks that had to be spread out are recorded here.
pub(crate) fn rank_in_column(
    w: &mut WriteCtx<'_>,
    project: Id,
    status: Id,
    task: Id,
    target: Target,
) -> Result<Rank, Failure> {
    let siblings = lookup::ranked(
        w.tx(),
        "tasks",
        "status_id = ?1 AND deleted_at IS NULL",
        &b(status),
    )?;
    let mut mine = Rank::first();
    for (id, rank) in lookup::place(&siblings, task, target)? {
        if id == task {
            mine = rank;
        } else {
            fields::set(w, TASK, id, Some(project), "rank", text(rank.as_str()))?;
        }
    }
    Ok(mine)
}

/// The live subtasks of `parent` as `(id, subtask rank)`, in their order;
/// a missing rank reads as empty, which makes [`lookup::place`] re-spread.
fn subtask_ranks(tx: &Transaction<'_>, parent: Id) -> Result<Vec<(Id, String)>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, coalesce(subtask_rank, '') FROM tasks
          WHERE parent_id = ?1 AND deleted_at IS NULL ORDER BY subtask_rank, rank, id",
    )?;
    let rows = stmt
        .query_map([b(parent)], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The subtask rank that places `task` among the subtasks of `parent` at
/// `target`; subtask ranks of others that had to be spread out are recorded
/// here.
fn rank_among_subtasks(
    w: &mut WriteCtx<'_>,
    project: Id,
    parent: Id,
    task: Id,
    target: Target,
) -> Result<Rank, Failure> {
    let siblings = subtask_ranks(w.tx(), parent)?;
    let mut mine = Rank::first();
    for (id, rank) in lookup::place(&siblings, task, target)? {
        if id == task {
            mine = rank;
        } else {
            fields::set(
                w,
                TASK,
                id,
                Some(project),
                "subtask_rank",
                text(rank.as_str()),
            )?;
        }
    }
    Ok(mine)
}

/// Makes `task` the last subtask of `parent`, or takes it out of its
/// parent's list with `None`. Nothing changes when the parent stays the
/// same. The parent is checked already.
pub(crate) fn set_parent(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    parent: Option<Id>,
) -> Result<(), Failure> {
    let current: Option<Vec<u8>> = w.tx().query_row(
        "SELECT parent_id FROM tasks WHERE id = ?1",
        [b(task)],
        |r| r.get(0),
    )?;
    if current.and_then(|c| Id::from_slice(&c).ok()) == parent {
        return Ok(());
    }
    let pid = Some(project);
    fields::set(w, TASK, task, pid, "parent", parent.and_then(id_value))?;
    let rank = match parent {
        Some(p) => text(rank_among_subtasks(w, project, p, task, Target::Bottom)?.as_str()),
        None => None,
    };
    fields::set(w, TASK, task, pid, "subtask_rank", rank)?;
    Ok(())
}

/// Moves a subtask within its parent's list; returns the parent with its
/// subtasks.
fn reorder_subtask(
    w: &mut WriteCtx<'_>,
    input: SubtaskReorderInput,
) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(messages::not_found(
            "task-not-found",
            &[("task", input.key.clone())],
        ))
    })?;
    lookup::ensure_active(&project)?;
    lookup::check_version(&task, input.expected_version, &input.key)?;
    super::claim::guard(w, task.id, &input.key, input.override_claim)?;
    let Some(parent) = task.parent else {
        let error =
            messages::invalid_at("key", "task-not-a-subtask", &[("task", input.key.clone())]);
        return Err(messages::hint(error, "task-not-a-subtask-hint", &[]).into());
    };
    let sibling = |w: &WriteCtx<'_>, field: &str, text: &str| -> Result<Id, Failure> {
        let other = lookup::task(w.tx(), field, text)?;
        if other.parent != Some(parent) || other.id == task.id {
            return Err(messages::invalid(
                field,
                "task-not-a-sibling",
                &[("task", text.to_owned())],
            ));
        }
        Ok(other.id)
    };
    let before = input
        .before
        .as_deref()
        .map(|t| sibling(w, "before", t))
        .transpose()?;
    let after = input
        .after
        .as_deref()
        .map(|t| sibling(w, "after", t))
        .transpose()?;
    let target = Target::from_parts(before, after, input.at)?.ok_or_else(|| {
        Failure::Op(messages::hint(
            messages::error(ErrorCode::Invalid, "task-subtask-where", &[]),
            "task-subtask-where-hint",
            &[],
        ))
    })?;
    let siblings = subtask_ranks(w.tx(), parent)?;
    if !already_at(&siblings, task.id, target) {
        let rank = rank_among_subtasks(w, task.project, parent, task.id, target)?;
        fields::set(
            w,
            TASK,
            task.id,
            Some(task.project),
            "subtask_rank",
            text(rank.as_str()),
        )?;
    }
    changed_with(w, parent, Vec::new(), &[TaskPart::Subtasks])
}

/// Whether `moving` already stands where `target` would put it in `list`,
/// and every rank in the list is set.
fn already_at(list: &[(Id, String)], moving: Id, target: Target) -> bool {
    if list.iter().any(|(_, r)| r.is_empty()) {
        return false;
    }
    let Some(now) = list.iter().position(|(id, _)| *id == moving) else {
        return false;
    };
    let others: Vec<Id> = list
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| *id != moving)
        .collect();
    let wanted = match target {
        Target::Top => Some(0),
        Target::Bottom => Some(others.len()),
        Target::Before(o) => others.iter().position(|id| *id == o),
        Target::After(o) => others.iter().position(|id| *id == o).map(|i| i + 1),
    };
    wanted == Some(now)
}

/// Open subtasks of `task`, for the warning when it closes.
fn open_subtasks(tx: &Transaction<'_>, task: Id) -> Result<i64, Failure> {
    Ok(tx.query_row(
        "SELECT count(*) FROM tasks t JOIN statuses s ON s.id = t.status_id
          WHERE t.parent_id = ?1 AND t.deleted_at IS NULL AND s.category IN ('todo', 'in_progress')",
        [b(task)],
        |r| r.get(0),
    )?)
}

// ---------------------------------------------------------------------
// Commands

pub(crate) fn create(w: &mut WriteCtx<'_>, input: TaskCreateInput) -> Result<TaskChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let title = gk_core::check_title(&input.title).map_err(|e| invalid("title", e))?;
    let description = input.description.unwrap_or_default();
    gk_core::check_markdown("description", &description).map_err(|e| invalid("description", e))?;
    let status = match &input.status {
        Some(s) => lookup::named(w.tx(), List::Statuses, &project, "status", s)?.id,
        None => first_status(w.tx(), &project)?,
    };
    let task_type = match &input.task_type {
        Some(t) => lookup::named(w.tx(), List::Types, &project, "type", t)?.id,
        None => default_type(w.tx(), &project)?,
    };
    let labels = resolve_labels(w.tx(), &project, "labels", &input.labels)?;
    let fix_version = input
        .fix_version
        .as_deref()
        .map(|v| super::version::resolve_for_task(w.tx(), &project, "fix_version", v))
        .transpose()?;
    let parent = match &input.parent {
        Some(p) => {
            let parent = lookup::task(w.tx(), "parent", p)?;
            check_parent(w.tx(), None, project.id, &parent, None)?;
            Some(parent.id)
        }
        None => None,
    };
    let start = input
        .start_date
        .as_deref()
        .map(|d| date_input(w, "start_date", d))
        .transpose()?;
    let due = input
        .due_date
        .as_deref()
        .map(|d| date_input(w, "due_date", d))
        .transpose()?;
    gk_core::check_date_range(start, due).map_err(|e| invalid("start_date", e))?;
    let due = match due {
        Some(d) => Some(d.to_string()),
        None => type_due_date(w, task_type, start.map(|d| d.to_string()).as_deref())?,
    };
    let units = units(w.tx())?;
    let estimate = input
        .estimate
        .as_deref()
        .map(|e| duration("estimate", e, units))
        .transpose()?;
    let target = column_target(
        w.tx(),
        status,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?
    .unwrap_or(Target::Bottom);
    let id = insert(
        w,
        NewTask {
            project: project.id,
            project_key: project.key.clone(),
            title,
            description,
            status,
            task_type,
            priority: priority_index(input.priority.unwrap_or_default()),
            parent,
            start_date: start.map(|d| d.to_string()),
            due_date: due,
            estimate,
            progress: None,
            labels,
            target,
        },
    )?;
    if fix_version.is_some() {
        super::version::set_task(w, id, project.id, fix_version)?;
    }
    super::custom_field::apply_input(w, id, &project, &input.fields, true)?;
    changed(w, id, Vec::new())
}

/// The due date a new task of type `task_type` gets when none is given:
/// the type's number of days after today, in the person's time zone; none
/// when the type sets none or the start date comes after it.
pub(crate) fn type_due_date(
    w: &WriteCtx<'_>,
    task_type: Id,
    start: Option<&str>,
) -> Result<Option<String>, Failure> {
    let days: Option<i64> = w
        .tx()
        .query_row(
            "SELECT due_offset_days FROM task_types WHERE id = ?1",
            [b(task_type)],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(days) = days else {
        return Ok(None);
    };
    let due = w
        .zone()
        .day_of(w.now())
        .add_days(days)
        .map_err(|e| invalid("due_date", e))?
        .to_string();
    Ok(match start {
        Some(s) if s > due.as_str() => None,
        _ => Some(due),
    })
}

/// The fields of a task about to be written.
#[derive(Debug, Clone)]
pub(crate) struct NewTask {
    pub(crate) project: Id,
    pub(crate) project_key: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) status: Id,
    pub(crate) task_type: Id,
    pub(crate) priority: i64,
    pub(crate) parent: Option<Id>,
    pub(crate) start_date: Option<String>,
    pub(crate) due_date: Option<String>,
    pub(crate) estimate: Option<i64>,
    pub(crate) progress: Option<i64>,
    pub(crate) labels: Vec<Id>,
    pub(crate) target: Target,
}

/// Refuses a new task to an agent session that created as many as the
/// workspace's limit on new tasks a day allows in the last 24 hours, this
/// command's included. Without a limit, every new task is allowed.
fn check_new_task_cap(w: &WriteCtx<'_>) -> Result<(), Failure> {
    const DAY_MS: i64 = 86_400_000;
    let caller = w.caller();
    let (true, Some(session)) = (caller.is_agent(), caller.session) else {
        return Ok(());
    };
    let Some(max) = super::settings::load(w.tx())?
        .agent_limits
        .new_tasks_per_day
    else {
        return Ok(());
    };
    let max = u64::from(max);
    let (earlier, oldest): (i64, Option<i64>) = w.tx().query_row(
        "SELECT count(*), min(at) FROM activity
          WHERE session_id = ?1 AND at > ?2 AND entity_kind = 'task'
            AND json_extract(changes, '$.fields.created.after') = 1",
        params![session.as_bytes().as_slice(), w.now() - DAY_MS],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let made = u64::try_from(earlier).unwrap_or(0) + w.created_so_far(TASK) as u64;
    if made < max {
        return Ok(());
    }
    let wait_ms = oldest.map_or(DAY_MS, |t| t + DAY_MS - w.now()).max(1000);
    let retry_after = (wait_ms + 999) / 1000;
    let args = [("count", made.to_string()), ("max", max.to_string())];
    Err(Failure::Op(
        messages::hint(
            messages::tag(
                gk_api::OpError::limit(messages::text("task-day-limit", &args), max),
                "task-day-limit",
                &args,
            ),
            "task-day-limit-hint",
            &[("hours", ((wait_ms + 3_599_999) / 3_600_000).to_string())],
        )
        .with_detail("retry_after", retry_after),
    ))
}

/// Writes a new task with the project's next number, its labels and its
/// search entry, and records its creation. The fields are checked already.
pub(crate) fn insert(w: &mut WriteCtx<'_>, t: NewTask) -> Result<Id, Failure> {
    check_new_task_cap(w)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let rank = rank_in_column(w, t.project, t.status, id, t.target)?;
    let subtask_rank = t
        .parent
        .map(|p| rank_among_subtasks(w, t.project, p, id, Target::Bottom))
        .transpose()?;
    let number: i64 = w.tx().query_row(
        "UPDATE projects SET next_number = next_number + 1, updated_at = ?2 WHERE id = ?1
         RETURNING next_number - 1",
        params![b(t.project), w.now()],
        |r| r.get(0),
    )?;
    let category = category_of(w.tx(), t.status)?;
    let closed_at = category.is_closed().then(|| w.now());
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO tasks (id, project_id, number, title, title_fold, description, status_id,
                            status_entered_at, type_id, priority, parent_id, rank, start_date,
                            due_date, estimate_min, created_at, created_by_kind, created_by_name,
                            edited_at, updated_at, closed_at, progress_override, subtask_rank)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?8, ?16, ?17,
                 ?8, ?8, ?18, ?19, ?20)",
        params![
            b(id),
            b(t.project),
            number,
            t.title,
            fold(&t.title),
            t.description,
            b(t.status),
            w.now(),
            b(t.task_type),
            t.priority,
            t.parent.map(b),
            rank.as_str(),
            t.start_date,
            t.due_date,
            t.estimate,
            caller.actor.kind.as_str(),
            caller.actor.name,
            closed_at,
            t.progress,
            subtask_rank.as_ref().map(Rank::as_str)
        ],
    )?;
    write_labels(w, id, &t.labels)?;
    description::initial(w, Owner::Task(id), &t.description)?;
    w.tx()
        .execute("UPDATE tasks SET version = 1 WHERE id = ?1", [b(id)])?;
    refresh_text(w, id)?;
    let key = format!("{}-{number}", t.project_key);
    let pid = Some(t.project);
    w.record_field(TASK, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(TASK, id, pid, "key", None, text(key));
    w.record_field(TASK, id, pid, "title", None, text(t.title));
    w.record_field(TASK, id, pid, "status", None, id_value(t.status));
    Ok(id)
}

pub(crate) fn priority_index(p: Priority) -> i64 {
    match p {
        Priority::None => 0,
        Priority::Low => 1,
        Priority::Normal => 2,
        Priority::High => 3,
        Priority::Urgent => 4,
    }
}

pub(crate) fn update(w: &mut WriteCtx<'_>, input: TaskUpdateInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(messages::not_found(
            "task-not-found",
            &[("task", input.key.clone())],
        ))
    })?;
    lookup::ensure_active(&project)?;
    lookup::check_version(&task, input.expected_version, &input.key)?;
    super::claim::guard(w, task.id, &input.key, input.override_claim)?;
    let clears = |f: TaskField| input.clear.contains(&f);
    for (given, field, name) in [
        (
            input.description.is_some() || !input.description_edits.is_empty(),
            TaskField::Description,
            "description",
        ),
        (input.parent.is_some(), TaskField::Parent, "parent"),
        (
            input.start_date.is_some(),
            TaskField::StartDate,
            "start_date",
        ),
        (input.due_date.is_some(), TaskField::DueDate, "due_date"),
        (input.estimate.is_some(), TaskField::Estimate, "estimate"),
        (input.progress.is_some(), TaskField::Progress, "progress"),
        (
            input.labels.is_some() || !input.add_labels.is_empty(),
            TaskField::Labels,
            "labels",
        ),
        (
            input.fix_version.is_some(),
            TaskField::FixVersion,
            "fix_version",
        ),
    ] {
        if given && clears(field) {
            return Err(messages::invalid(
                "clear",
                "field-given-and-cleared",
                &[("field", name.to_owned())],
            ));
        }
    }
    if input.description.is_some() && !input.description_edits.is_empty() {
        return Err(messages::invalid(
            "description_edits",
            "task-description-and-edits",
            &[],
        ));
    }
    if input.base_revision.is_some() && input.description.is_none() {
        return Err(messages::invalid(
            "base_revision",
            "task-base-revision-alone",
            &[],
        ));
    }
    let (id, pid) = (task.id, Some(project.id));
    let mut text_changed = false;
    let mut warnings = Vec::new();
    let mut description_save = None;
    if let Some(title) = &input.title {
        let title = gk_core::check_title(title).map_err(|e| invalid("title", e))?;
        text_changed |= fields::set(w, TASK, id, pid, "title", text(title))?;
    }
    if let Some(description) = &input.description {
        gk_core::check_markdown("description", description)
            .map_err(|e| invalid("description", e))?;
        let saved = description::save(
            w,
            Owner::Task(id),
            pid,
            description,
            input.base_revision.as_deref(),
            "normal",
        )?;
        if let Some(copy) = &saved.conflict_copy {
            warnings.push(messages::text(
                "task-description-conflict",
                &[("copy", copy.to_string())],
            ));
        }
        description_save = Some(saved);
        super::draft::clear(w, gk_api::ops::draft::DraftKind::Task, id)?;
    }
    if !input.description_edits.is_empty() {
        description_save = Some(description::save_edits(
            w,
            Owner::Task(id),
            pid,
            &input.description_edits,
        )?);
    }
    if clears(TaskField::Description) {
        description_save = Some(description::save(
            w,
            Owner::Task(id),
            pid,
            "",
            None,
            "normal",
        )?);
    }
    if let Some(r) = &input.resolve_conflict {
        description_save = Some(description::resolve(
            w,
            Owner::Task(id),
            pid,
            &r.copy,
            r.action,
            r.text.as_deref(),
            None,
        )?);
        super::draft::clear(w, gk_api::ops::draft::DraftKind::Task, id)?;
    }
    if let Some(t) = &input.task_type {
        let t = lookup::named(w.tx(), List::Types, &project, "type", t)?;
        fields::set(w, TASK, id, pid, "type", id_value(t.id))?;
    }
    if let Some(p) = input.priority {
        fields::set(w, TASK, id, pid, "priority", priority_value(p))?;
    }
    let labels_given = input.labels.is_some()
        || !input.add_labels.is_empty()
        || !input.remove_labels.is_empty()
        || clears(TaskField::Labels);
    if labels_given {
        let mut ids = match &input.labels {
            Some(names) => resolve_labels(w.tx(), &project, "labels", names)?,
            None if clears(TaskField::Labels) => Vec::new(),
            None => label_ids(w.tx(), id)?,
        };
        for l in resolve_labels(w.tx(), &project, "add_labels", &input.add_labels)? {
            if !ids.contains(&l) {
                ids.push(l);
            }
        }
        let remove = resolve_labels(w.tx(), &project, "remove_labels", &input.remove_labels)?;
        ids.retain(|l| !remove.contains(l));
        set_labels(w, id, project.id, &ids)?;
    }
    if let Some(v) = &input.fix_version {
        let current = fields::read(w.tx(), TASK, id, super::version::TASK_FIELD)?;
        let version = super::version::resolve_for_task(w.tx(), &project, "fix_version", v)?;
        if current != id_value(version) {
            super::version::set_task(w, id, project.id, Some(version))?;
        }
    }
    if clears(TaskField::FixVersion) {
        super::version::set_task(w, id, project.id, None)?;
    }
    if let Some(p) = &input.parent {
        let parent = lookup::task(w.tx(), "parent", p)?;
        check_parent(w.tx(), Some(id), project.id, &parent, Some(&input.key))?;
        set_parent(w, id, project.id, Some(parent.id))?;
    }
    if clears(TaskField::Parent) {
        set_parent(w, id, project.id, None)?;
    }
    let start = match (&input.start_date, clears(TaskField::StartDate)) {
        (Some(d), _) => Some(text(date_input(w, "start_date", d)?.to_string())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let due = match (&input.due_date, clears(TaskField::DueDate)) {
        (Some(d), _) => Some(text(date_input(w, "due_date", d)?.to_string())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let dates_given = start.is_some() || due.is_some();
    if dates_given {
        let start_after = match &start {
            Some(v) => v.clone(),
            None => fields::read(w.tx(), TASK, id, "start_date")?,
        };
        let due_after = match &due {
            Some(v) => v.clone(),
            None => fields::read(w.tx(), TASK, id, "due_date")?,
        };
        check_dates(start_after.as_ref(), due_after.as_ref())?;
        // A range moved later past its old due date needs the due date
        // written first, or the start would pass it in between.
        let due_first = super::schedule::due_first(w.tx(), id, start_after.as_ref())?;
        let (start, due) = (
            start.map(|v| ("start_date", v)),
            due.map(|v| ("due_date", v)),
        );
        let writes = if due_first {
            [due, start]
        } else {
            [start, due]
        };
        for (field, v) in writes.into_iter().flatten() {
            fields::set(w, TASK, id, pid, field, v)?;
        }
    }
    if let Some(e) = &input.estimate {
        let minutes = duration("estimate", e, units(w.tx())?)?;
        fields::set(w, TASK, id, pid, "estimate", Some(Value::from(minutes)))?;
    }
    if clears(TaskField::Estimate) {
        fields::set(w, TASK, id, pid, "estimate", None)?;
    }
    if let Some(p) = input.progress {
        if p > 100 {
            return Err(messages::invalid("progress", "task-progress-range", &[]));
        }
        fields::set(w, TASK, id, pid, "progress", Some(Value::from(p)))?;
    }
    if clears(TaskField::Progress) {
        fields::set(w, TASK, id, pid, "progress", None)?;
    }
    if !input.fields.is_empty() {
        super::custom_field::apply_input(w, id, &project, &input.fields, false)?;
    }
    if text_changed {
        refresh_text(w, id)?;
    }
    let moved = if input.move_dependents && dates_given {
        super::schedule::move_dependents(w, id, input.override_claim, &mut warnings)?
    } else {
        Vec::new()
    };
    let mut change = changed(w, id, warnings)?;
    change.description_save = description_save;
    change.moved = moved;
    Ok(change)
}

pub(crate) fn move_task(w: &mut WriteCtx<'_>, input: TaskMoveInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let from = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(messages::not_found(
            "task-not-found",
            &[("task", input.key.clone())],
        ))
    })?;
    lookup::ensure_active(&from)?;
    lookup::check_version(&task, input.expected_version, &input.key)?;
    super::claim::guard(w, task.id, &input.key, input.override_claim)?;
    let mut warnings = Vec::new();
    let mut closed_duplicates = Vec::new();
    let to = match &input.project {
        Some(p) => {
            let to = lookup::project(w.tx(), "project", p)?;
            lookup::ensure_active(&to)?;
            to
        }
        None => from.clone(),
    };
    if to.id != from.id {
        warnings.extend(move_to_project(
            w,
            &task,
            &to,
            input.status.as_deref(),
            &input.mapping,
        )?);
        if !input.fields.is_empty() {
            for id in descendants(w.tx(), task.id)? {
                super::custom_field::apply_moved_input(w, id, &to, &input.fields)?;
            }
        }
    } else if !input.fields.is_empty() {
        if input.project.is_none() {
            return Err(messages::invalid(
                "fields",
                "task-move-fields-without-project",
                &[],
            ));
        }
        // Already in that project (a bulk move of tasks from several): the
        // values are set as an edit would set them.
        super::custom_field::apply_input(w, task.id, &from, &input.fields, false)?;
    }
    let current = lookup::task_by_id(w.tx(), task.id)?.unwrap_or(task.clone());
    let status = match &input.status {
        Some(s) => lookup::named(w.tx(), List::Statuses, &to, "status", s)?.id,
        None => current.status,
    };
    let status_changes = status != current.status;
    let target = column_target(
        w.tx(),
        status,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?;
    let target = match (target, status_changes) {
        (Some(t), _) => Some(t),
        (None, true) => Some(Target::Bottom),
        (None, false) => None,
    };
    let pid = Some(to.id);
    if status_changes {
        let before = category_of(w.tx(), current.status)?;
        let after = category_of(w.tx(), status)?;
        fields::set(w, TASK, task.id, pid, "status", id_value(status))?;
        let closed: Option<i64> =
            fields::read(w.tx(), TASK, task.id, "closed_at")?.and_then(|v| v.as_i64());
        let closed_after = gk_core::closed_at_after_move(before, after, closed, w.now());
        fields::set(
            w,
            TASK,
            task.id,
            pid,
            "closed_at",
            closed_after.map(Value::from),
        )?;
        if after.is_closed() && !before.is_closed() {
            let open = open_subtasks(w.tx(), task.id)?;
            if open > 0 {
                warnings.push(messages::text(
                    "task-open-subtasks",
                    &[("task", input.key.clone()), ("count", open.to_string())],
                ));
            }
            let (blockers, hidden) = super::relation::open_blockers(w.tx(), task.id)?;
            if !blockers.is_empty() {
                warnings.push(messages::text(
                    "task-still-blocked",
                    &[
                        ("task", input.key.clone()),
                        ("blockers", blockers.join(", ")),
                        ("count", blockers.len().to_string()),
                    ],
                ));
            } else if hidden > 0 {
                // Blocked only by tasks outside the caller's project: say
                // so without naming them.
                warnings.push(messages::text(
                    "task-still-blocked-outside",
                    &[("task", input.key.clone())],
                ));
            }
            let duplicates = open_duplicates(w.tx(), task.id)?;
            match input.close_duplicates {
                Some(true) => {
                    for (id, key) in duplicates {
                        close_duplicate(w, id, status)?;
                        closed_duplicates.push(key);
                    }
                }
                None if !duplicates.is_empty() => {
                    let keys: Vec<String> = duplicates.into_iter().map(|(_, k)| k).collect();
                    warnings.push(messages::text(
                        "task-open-duplicates",
                        &[
                            ("task", input.key.clone()),
                            ("count", keys.len().to_string()),
                            ("duplicates", keys.join(", ")),
                        ],
                    ));
                }
                _ => {}
            }
        }
    }
    if let Some(target) = target {
        let rank = rank_in_column(w, to.id, status, task.id, target)?;
        fields::set(w, TASK, task.id, pid, "rank", text(rank.as_str()))?;
    }
    let mut out = changed(w, task.id, warnings)?;
    out.duplicates_closed = closed_duplicates;
    Ok(out)
}

/// The open, live tasks marked as duplicates of task `id` that the caller
/// may change, with their keys, by key.
fn open_duplicates(tx: &Transaction<'_>, id: Id) -> Result<Vec<(Id, String)>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT d.id, p.key || '-' || d.number, p.id FROM task_relations r
           JOIN tasks d ON d.id = r.from_task JOIN projects p ON p.id = d.project_id
          WHERE r.to_task = ?1 AND r.kind = 'duplicates' AND d.deleted_at IS NULL
            AND d.closed_at IS NULL AND p.deleted_at IS NULL AND p.state = 'active'
          ORDER BY p.key, d.number",
    )?;
    let rows = stmt
        .query_map([b(id)], |r| {
            Ok((id_at(r, 0)?, r.get::<_, String>(1)?, id_at(r, 2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (task, key, project) in rows {
        if crate::scope::allows(tx, project)? {
            out.push((task, key));
        }
    }
    Ok(out)
}

/// Closes duplicate `id` as its original was closed into `status`: into
/// that status when it is in the same project, else into its project's
/// status of the same name, else the first of the same category, else the
/// first closed one.
fn close_duplicate(w: &mut WriteCtx<'_>, id: Id, status: Id) -> Result<(), Failure> {
    let target: Option<Id> = w
        .tx()
        .query_row(
            "SELECT c.id FROM tasks d JOIN statuses o ON o.id = ?2
               JOIN statuses c ON c.project_id = d.project_id
              WHERE d.id = ?1 AND c.category IN ('done', 'cancelled')
              ORDER BY c.id <> o.id, c.name_fold <> o.name_fold, c.category <> o.category,
                       c.rank, c.id
              LIMIT 1",
            params![b(id), b(status)],
            |r| id_at(r, 0),
        )
        .optional()?;
    let Some(target) = target else {
        return Ok(());
    };
    move_task(
        w,
        TaskMoveInput {
            key: id.to_string(),
            status: Some(target.to_string()),
            close_duplicates: Some(false),
            ..TaskMoveInput::default()
        },
    )
    .map(drop)
}

/// Gives task `id` a project and a number together (one statement, as the
/// pair is unique), recording both.
pub(crate) fn set_home(
    w: &mut WriteCtx<'_>,
    id: Id,
    record_under: Option<Id>,
    project: Option<Value>,
    number: Option<Value>,
) -> Result<(), Failure> {
    let before_project = fields::read(w.tx(), TASK, id, "project")?;
    let before_number = fields::read(w.tx(), TASK, id, "number")?;
    let project_id: Id = project
        .as_ref()
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Failure::Op(messages::storage("storage-task-no-project", &[])))?;
    let n = number
        .as_ref()
        .and_then(Value::as_i64)
        .ok_or_else(|| Failure::Op(messages::storage("storage-task-no-number", &[])))?;
    // The key the task leaves keeps finding it, also when an undo takes it
    // back home; the key it takes is nobody's alias any more.
    let (old_key, old_number): (String, i64) = w.tx().query_row(
        "SELECT p.key, t.number FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.id = ?1",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let new_key: String = w.tx().query_row(
        "SELECT key FROM projects WHERE id = ?1",
        [b(project_id)],
        |r| r.get(0),
    )?;
    w.tx().execute(
        "UPDATE tasks SET project_id = ?2, number = ?3 WHERE id = ?1",
        params![b(id), b(project_id), n],
    )?;
    if (old_key.as_str(), old_number) != (new_key.as_str(), n) {
        w.tx().execute(
            "INSERT INTO task_key_aliases (project_key, number, task_id) VALUES (?1, ?2, ?3)
             ON CONFLICT (project_key, number) DO UPDATE SET task_id = excluded.task_id",
            params![old_key, old_number, b(id)],
        )?;
    }
    w.tx().execute(
        "DELETE FROM task_key_aliases WHERE project_key = ?1 AND number = ?2",
        params![new_key, n],
    )?;
    fields::touch(w, TASK, id, "project")?;
    w.record_field(TASK, id, record_under, "project", before_project, project);
    w.record_field(TASK, id, record_under, "number", before_number, number);
    Ok(())
}

/// Moves `task` and its subtasks from project `from` to `to`: new numbers,
/// the old keys kept as aliases, statuses, types and labels matched by
/// `mapping`, else by name, else the target's defaults, with a warning for
/// each that had no match.
fn move_to_project(
    w: &mut WriteCtx<'_>,
    task: &TaskRow,
    to: &ProjectRow,
    status: Option<&str>,
    mapping: &MoveMapping,
) -> Result<Vec<String>, Failure> {
    let mut warnings = Vec::new();
    let to_statuses = lookup::all(w.tx(), List::Statuses, to.id)?;
    let to_types = lookup::all(w.tx(), List::Types, to.id)?;
    let to_labels = lookup::all(w.tx(), List::Labels, to.id)?;
    let statuses = Mapped::new(
        w.tx(),
        List::Statuses,
        to,
        "mapping.statuses",
        &mapping.statuses,
        &to_statuses,
    )?;
    let types = Mapped::new(
        w.tx(),
        List::Types,
        to,
        "mapping.types",
        &mapping.types,
        &to_types,
    )?;
    let labels = Mapped::new(
        w.tx(),
        List::Labels,
        to,
        "mapping.labels",
        &mapping.labels,
        &to_labels,
    )?;
    let fallback_status = first_status(w.tx(), to)?;
    let fallback_type = default_type(w.tx(), to)?;
    let name_of = |items: &[lookup::Named], id: Id| {
        items
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.name.clone())
            .unwrap_or_default()
    };
    let root_status = status
        .map(|s| lookup::named(w.tx(), List::Statuses, to, "status", s).map(|n| n.id))
        .transpose()?;
    let pid = Some(to.id);
    let mut warn = |warning: String| {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    };
    for id in descendants(w.tx(), task.id)? {
        let (status_name, status_cat, type_name, closed): (String, String, String, Option<i64>) =
            w.tx().query_row(
                "SELECT s.name, s.category, y.name, t.closed_at
                   FROM tasks t JOIN statuses s ON s.id = t.status_id
                   JOIN task_types y ON y.id = t.type_id WHERE t.id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        let new_number: i64 = w.tx().query_row(
            "UPDATE projects SET next_number = next_number + 1, updated_at = ?2 WHERE id = ?1
             RETURNING next_number - 1",
            params![b(to.id), w.now()],
            |r| r.get(0),
        )?;
        set_home(w, id, pid, id_value(to.id), Some(Value::from(new_number)))?;
        let new_status = match (id == task.id, root_status) {
            (true, Some(s)) => s,
            _ => statuses.get(&status_name).unwrap_or_else(|| {
                warn(messages::text(
                    "task-status-fallback",
                    &[
                        ("status", status_name.clone()),
                        ("project", to.key.clone()),
                        ("target", name_of(&to_statuses, fallback_status)),
                    ],
                ));
                fallback_status
            }),
        };
        fields::set(w, TASK, id, pid, "status", id_value(new_status))?;
        let before: StatusCategory = status_cat.parse().unwrap_or(StatusCategory::Todo);
        let after = category_of(w.tx(), new_status)?;
        let closed_after = gk_core::closed_at_after_move(before, after, closed, w.now());
        fields::set(w, TASK, id, pid, "closed_at", closed_after.map(Value::from))?;
        let rank = rank_in_column(w, to.id, new_status, id, Target::Bottom)?;
        fields::set(w, TASK, id, pid, "rank", text(rank.as_str()))?;
        let new_type = types.get(&type_name).unwrap_or_else(|| {
            warn(messages::text(
                "task-type-fallback",
                &[
                    ("type", type_name.clone()),
                    ("project", to.key.clone()),
                    ("target", name_of(&to_types, fallback_type)),
                ],
            ));
            fallback_type
        });
        fields::set(w, TASK, id, pid, "type", id_value(new_type))?;
        let mut kept = Vec::new();
        for label in task_labels(w.tx(), id)? {
            match labels.get(&label.name) {
                Some(l) => {
                    if !kept.contains(&l) {
                        kept.push(l);
                    }
                }
                None => warn(messages::text(
                    "task-label-left-off",
                    &[("label", label.name.clone()), ("project", to.key.clone())],
                )),
            }
        }
        set_labels(w, id, to.id, &kept)?;
        if let Some(key) = super::version::drop_unreachable_task(w, id, to.id)? {
            warn(messages::text(
                "task-version-left-off",
                &[("version", key), ("project", to.key.clone())],
            ));
        }
        for warning in super::custom_field::follow_move(w, id, to)? {
            warn(warning);
        }
        super::recurrence::follow_move(w, id, to.id)?;
        if id == task.id && task.parent.is_some() {
            set_parent(w, id, to.id, None)?;
        }
        refresh_text(w, id)?;
    }
    Ok(warnings)
}

/// How the statuses, types or labels of moving tasks find their match in
/// the target project: the name the mapping gives, else the same name.
struct Mapped<'a> {
    /// The mapping, by folded source name, already looked up.
    given: Vec<(String, Id)>,
    target: &'a [lookup::Named],
}

impl<'a> Mapped<'a> {
    /// Looks up every target the mapping names, so a mistyped one is an
    /// error even when no moving task needs it.
    fn new(
        tx: &Transaction<'_>,
        list: List,
        to: &ProjectRow,
        field: &str,
        mapping: &std::collections::BTreeMap<String, String>,
        target: &'a [lookup::Named],
    ) -> Result<Self, Failure> {
        let given = mapping
            .iter()
            .map(|(from, name)| {
                lookup::named(tx, list, to, &format!("{field}.{from}"), name)
                    .map(|n| (fold(from), n.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { given, target })
    }

    fn get(&self, name: &str) -> Option<Id> {
        let name = fold(name);
        self.given
            .iter()
            .find(|(from, _)| *from == name)
            .map(|(_, id)| *id)
            .or_else(|| {
                self.target
                    .iter()
                    .find(|t| fold(&t.name) == name)
                    .map(|t| t.id)
            })
    }
}

/// What moving the tasks `input` names to its project would do, changing
/// nothing.
fn move_check(tx: &Transaction<'_>, input: &TaskMoveCheckInput) -> Result<MoveCheck, Failure> {
    if input.keys.is_empty() || input.keys.len() > MAX_MOVE_CHECK {
        return Err(messages::invalid(
            "keys",
            "task-move-check-count",
            &[("max", MAX_MOVE_CHECK.to_string())],
        ));
    }
    let to = lookup::project(tx, "project", &input.project)?;
    lookup::ensure_active(&to)?;
    let to_statuses = lookup::all(tx, List::Statuses, to.id)?;
    let to_types = lookup::all(tx, List::Types, to.id)?;
    let to_labels = lookup::all(tx, List::Labels, to.id)?;
    let mut next: i64 = tx.query_row(
        "SELECT next_number FROM projects WHERE id = ?1",
        [b(to.id)],
        |r| r.get(0),
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut keys = Vec::new();
    let (mut statuses, mut types, mut labels) = (Vec::new(), Vec::new(), Vec::new());
    let mut fields: Vec<MoveMatch> = Vec::new();
    let mut versions: Vec<MoveMatch> = Vec::new();
    let mut required_fields: Vec<MoveRequiredField> = Vec::new();
    let fallback_type = default_type(tx, &to)?;
    fn count(list: &mut Vec<MoveMatch>, target: &[lookup::Named], name: &str) {
        let folded = fold(name);
        match list.iter_mut().find(|m| fold(&m.name) == folded) {
            Some(m) => m.tasks += 1,
            None => list.push(MoveMatch {
                name: name.to_owned(),
                tasks: 1,
                target: target
                    .iter()
                    .find(|t| fold(&t.name) == folded)
                    .map(|t| t.name.clone()),
            }),
        }
    }
    for (i, key) in input.keys.iter().enumerate() {
        let t = lookup::task(tx, &format!("keys[{i}]"), key)?;
        if t.project == to.id || seen.contains(&t.id) {
            continue;
        }
        for id in descendants(tx, t.id)? {
            if !seen.insert(id) {
                continue;
            }
            let (key, status, task_type): (String, String, String) = tx.query_row(
                "SELECT p.key || '-' || t.number, s.name, y.name
                   FROM tasks t JOIN projects p ON p.id = t.project_id
                   JOIN statuses s ON s.id = t.status_id
                   JOIN task_types y ON y.id = t.type_id WHERE t.id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            keys.push(MovedKey {
                from: key,
                to: format!("{}-{next}", to.key),
            });
            next += 1;
            count(&mut statuses, &to_statuses, &status);
            count(&mut types, &to_types, &task_type);
            let new_type = to_types
                .iter()
                .find(|t| fold(&t.name) == fold(&task_type))
                .or_else(|| to_types.iter().find(|t| t.id == fallback_type))
                .map(|t| t.name.clone())
                .unwrap_or_default();
            for (name, has_default) in super::custom_field::move_required(tx, id, &to, &new_type)? {
                match required_fields.iter_mut().find(|r| r.name == name) {
                    Some(r) => r.tasks += 1,
                    None => required_fields.push(MoveRequiredField {
                        name,
                        tasks: 1,
                        has_default,
                    }),
                }
            }
            for label in task_labels(tx, id)? {
                count(&mut labels, &to_labels, &label.name);
            }
            if let Some((name, kept)) = super::version::move_match(tx, id, to.id)? {
                match versions.iter_mut().find(|m| m.name == name) {
                    Some(m) => m.tasks += 1,
                    None => versions.push(MoveMatch {
                        target: kept.then(|| name.clone()),
                        name,
                        tasks: 1,
                    }),
                }
            }
            for (name, target) in super::custom_field::move_matches(tx, id, &to)? {
                let folded = fold(&name);
                match fields
                    .iter_mut()
                    .find(|m: &&mut MoveMatch| fold(&m.name) == folded)
                {
                    Some(m) => {
                        m.tasks += 1;
                        // One that some tasks keep and others don't is
                        // shown as left off.
                        if target.is_none() {
                            m.target = None;
                        }
                    }
                    None => fields.push(MoveMatch {
                        name,
                        tasks: 1,
                        target,
                    }),
                }
            }
        }
    }
    let name_of = |items: &[lookup::Named], id: Id| {
        items
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.name.clone())
            .unwrap_or_default()
    };
    Ok(MoveCheck {
        project: to.key.clone(),
        keys,
        statuses,
        types,
        labels,
        fields,
        versions,
        required_fields,
        fallback_status: name_of(&to_statuses, first_status(tx, &to)?),
        fallback_type: name_of(&to_types, fallback_type),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_recorded_sorted_or_not_at_all() {
        assert_eq!(labels_value(&[]), None);
        let (a, b) = (Id::generate(), Id::generate());
        assert_eq!(labels_value(&[b, a]), labels_value(&[a, b, a]));
    }
}
