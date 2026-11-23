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

use gk_api::OpError;
use gk_api::ops::common::{Actor, ActorKind, Category, Glyph, Hue, Priority};
use gk_api::ops::task::{
    Checklist, LabelRef, StatusRef, Task, TaskChange, TaskCreate, TaskCreateInput, TaskField,
    TaskGet, TaskGetInput, TaskList, TaskMove, TaskMoveInput, TaskUpdate, TaskUpdateInput, TypeRef,
};
use gk_core::{Day, DurationUnits, Id, Rank, StatusCategory, fold};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK, id_value, priority_name, text};
use super::lookup::{self, List, ProjectRow, Target, TaskRow, b, id_at, opt_id_at};
use super::project::{enum_text, parse_enum};
use crate::failure::Failure;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// Most levels of subtasks: a task and four levels below it.
const MAX_LEVELS: usize = 5;
/// Most tasks one read returns.
const MAX_KEYS: usize = 20;

pub(super) fn register(r: &mut Registry) {
    r.query::<TaskGet, _>(desc::<TaskGet>(), |q, input: TaskGetInput| {
        if input.keys.is_empty() || input.keys.len() > MAX_KEYS {
            return Err(OpError::invalid_at(
                "keys",
                format!("Ask for 1 to {MAX_KEYS} tasks at once."),
            )
            .into());
        }
        let units = units(q.tx)?;
        let mut items = Vec::with_capacity(input.keys.len());
        for (i, key) in input.keys.iter().enumerate() {
            let t = lookup::task(q.tx, &format!("keys[{i}]"), key)?;
            items.push(load(q.tx, t.id, units)?);
        }
        Ok(TaskList {
            total: items.len() as u64,
            items,
        })
    });
    r.command::<TaskCreate, _>(desc::<TaskCreate>(), create);
    r.command::<TaskUpdate, _>(desc::<TaskUpdate>(), update);
    r.command::<TaskMove, _>(desc::<TaskMove>(), move_task);
}

/// The workspace's working day and week, for durations.
pub(crate) fn units(tx: &Transaction<'_>) -> Result<DurationUnits, Failure> {
    let s = super::settings::load(tx)?;
    Ok(DurationUnits::new(s.hours_per_day, s.days_per_week).unwrap_or_default())
}

fn invalid(field: &str, e: impl std::fmt::Display) -> Failure {
    let text = e.to_string();
    let mut chars = text.chars();
    let message = chars.next().map_or_else(String::new, |c| {
        format!("{}{}.", c.to_uppercase(), chars.as_str())
    });
    Failure::Op(OpError::invalid_at(field, message))
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

/// Ticked and all task-list items of a Markdown text (`- [ ]`, `* [x]`,
/// `1. [ ]`), outside fenced code blocks.
pub(crate) fn checklist(markdown: &str) -> (u32, u32) {
    let (mut done, mut total) = (0, 0);
    let mut fence: Option<&str> = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        for marker in ["```", "~~~"] {
            if trimmed.starts_with(marker) {
                fence = match fence {
                    None => Some(marker),
                    Some(m) if m == marker => None,
                    other => other,
                };
            }
        }
        if fence.is_some() {
            continue;
        }
        let rest = if let Some(r) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            r
        } else {
            let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
            match trimmed[digits..]
                .strip_prefix(". ")
                .or_else(|| trimmed[digits..].strip_prefix(") "))
            {
                Some(r) if digits > 0 => r,
                _ => continue,
            }
        };
        if rest.starts_with("[ ]") {
            total += 1;
        } else if rest.starts_with("[x]") || rest.starts_with("[X]") {
            total += 1;
            done += 1;
        }
    }
    (done, total)
}

/// Keeps the task's search entry and checklist counts in step with its
/// title and description.
pub(crate) fn refresh_text(w: &WriteCtx<'_>, task: Id) -> Result<(), Failure> {
    let (project, title, description): (Vec<u8>, String, String) = w.tx().query_row(
        "SELECT project_id, title, description FROM tasks WHERE id = ?1",
        [b(task)],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let (done, total) = checklist(&description);
    w.tx().execute(
        "UPDATE tasks SET checklist_done = ?2, checklist_total = ?3 WHERE id = ?1",
        params![b(task), done, total],
    )?;
    w.tx().execute(
        "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body, updated_at)
         VALUES ('task', ?1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (kind, entity_id) DO UPDATE SET project_id = excluded.project_id,
           title = excluded.title, body = excluded.body, updated_at = excluded.updated_at",
        params![b(task), project, title, description, w.now()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------
// Reading

/// One task as outputs carry it.
pub(crate) fn load(tx: &Transaction<'_>, id: Id, units: DurationUnits) -> Result<Task, Failure> {
    let row = tx.query_row(
        "SELECT t.number, p.key, t.title, t.description, s.id, s.name, s.category,
                y.id, y.name, y.glyph, t.priority, t.parent_id, t.start_date, t.due_date,
                t.estimate_min, t.progress_override, t.spent_min, t.checklist_done,
                t.checklist_total, t.rank, t.created_by_kind, t.created_by_name, t.created_at,
                t.edited_at, t.updated_at, t.status_entered_at, t.closed_at, t.version
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
          WHERE t.id = ?1",
        [b(id)],
        |r| {
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
                    parent: None,
                    description: r.get(3)?,
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
        },
    )?;
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

fn changed(w: &WriteCtx<'_>, id: Id, warnings: Vec<String>) -> Result<TaskChange, Failure> {
    Ok(TaskChange {
        task: load(w.tx(), id, units(w.tx())?)?,
        group: w.group_if_changed(),
        warnings,
    })
}

// ---------------------------------------------------------------------
// Values

fn day(field: &str, text: &str) -> Result<Day, Failure> {
    text.trim().parse::<Day>().map_err(|e| invalid(field, e))
}

fn duration(field: &str, text: &str, units: DurationUnits) -> Result<i64, Failure> {
    gk_core::parse_minutes(text, units).map_err(|e| invalid(field, e))
}

fn priority_value(p: Priority) -> Option<Value> {
    Some(Value::String(enum_text(p)))
}

fn category_of(tx: &Transaction<'_>, status: Id) -> Result<StatusCategory, Failure> {
    let text: String = tx.query_row(
        "SELECT category FROM statuses WHERE id = ?1",
        [b(status)],
        |r| r.get(0),
    )?;
    Ok(text.parse().unwrap_or(StatusCategory::Todo))
}

/// The project's first to-do status, else its first status.
fn first_status(tx: &Transaction<'_>, project: &ProjectRow) -> Result<Id, Failure> {
    let statuses = lookup::all(tx, List::Statuses, project.id)?;
    statuses
        .iter()
        .find(|s| s.kind == "todo")
        .or_else(|| statuses.first())
        .map(|s| s.id)
        .ok_or_else(|| {
            Failure::Op(OpError::invalid(format!(
                "Project {} has no statuses; add one first.",
                project.key
            )))
        })
}

fn default_type(tx: &Transaction<'_>, project: &ProjectRow) -> Result<Id, Failure> {
    let found: Option<Id> = tx
        .query_row(
            "SELECT id FROM task_types WHERE project_id = ?1
              ORDER BY is_default DESC, rank, id LIMIT 1",
            [b(project.id)],
            |r| id_at(r, 0),
        )
        .optional()?;
    found.ok_or_else(|| {
        Failure::Op(OpError::invalid(format!(
            "Project {} has no task types; add one first.",
            project.key
        )))
    })
}

fn resolve_labels(
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
fn level_of(tx: &Transaction<'_>, id: Id) -> Result<usize, Failure> {
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
fn descendants(tx: &Transaction<'_>, id: Id) -> Result<Vec<Id>, Failure> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let mut stmt = tx.prepare_cached(
            "SELECT id FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL ORDER BY rank, id",
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

/// Checks that `task` (with its subtasks) may go under `parent`.
fn check_parent(
    tx: &Transaction<'_>,
    task: Option<Id>,
    project: Id,
    parent: &TaskRow,
    key: &str,
) -> Result<(), Failure> {
    if parent.project != project {
        return Err(OpError::invalid_at(
            "parent",
            format!("{key}'s parent must be in the same project."),
        )
        .into());
    }
    let below = match task {
        Some(t) => {
            if descendants(tx, t)?.contains(&parent.id) {
                return Err(OpError::invalid_at(
                    "parent",
                    format!("{key} can't be a subtask of itself or of one of its own subtasks."),
                )
                .into());
            }
            height(tx, t)?
        }
        None => 0,
    };
    let levels = level_of(tx, parent.id)? + 1 + below;
    if levels > MAX_LEVELS {
        return Err(OpError::invalid_at(
            "parent",
            format!("Subtasks go at most {MAX_LEVELS} levels deep; this would make {levels}."),
        )
        .into());
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
            return Err(
                OpError::invalid_at(field, format!("{text} is not in the target column.")).into(),
            );
        }
        Ok(t.id)
    };
    let before = before.map(|t| in_column("before", t)).transpose()?;
    let after = after.map(|t| in_column("after", t)).transpose()?;
    Target::from_parts(before, after, at)
}

/// New ranks for placing `task` in the column of `status`; ranks of other
/// tasks that had to be spread out are recorded here.
fn rank_in_column(
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

fn create(w: &mut WriteCtx<'_>, input: TaskCreateInput) -> Result<TaskChange, Failure> {
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
    let parent = match &input.parent {
        Some(p) => {
            let parent = lookup::task(w.tx(), "parent", p)?;
            check_parent(w.tx(), None, project.id, &parent, "The new task")?;
            Some(parent.id)
        }
        None => None,
    };
    let start = input
        .start_date
        .as_deref()
        .map(|d| day("start_date", d))
        .transpose()?;
    let due = input
        .due_date
        .as_deref()
        .map(|d| day("due_date", d))
        .transpose()?;
    gk_core::check_date_range(start, due).map_err(|e| invalid("start_date", e))?;
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

    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let rank = rank_in_column(w, project.id, status, id, target)?;
    let number: i64 = w.tx().query_row(
        "UPDATE projects SET next_number = next_number + 1, updated_at = ?2 WHERE id = ?1
         RETURNING next_number - 1",
        params![b(project.id), w.now()],
        |r| r.get(0),
    )?;
    let category = category_of(w.tx(), status)?;
    let closed_at = category.is_closed().then(|| w.now());
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO tasks (id, project_id, number, title, title_fold, description, status_id,
                            status_entered_at, type_id, priority, parent_id, rank, start_date,
                            due_date, estimate_min, created_at, created_by_kind, created_by_name,
                            edited_at, updated_at, closed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?8, ?16, ?17,
                 ?8, ?8, ?18)",
        params![
            b(id),
            b(project.id),
            number,
            title,
            fold(&title),
            description,
            b(status),
            w.now(),
            b(task_type),
            priority_index(input.priority.unwrap_or_default()),
            parent.map(b),
            rank.as_str(),
            start.map(|d| d.to_string()),
            due.map(|d| d.to_string()),
            estimate,
            caller.actor.kind.as_str(),
            caller.actor.name,
            closed_at
        ],
    )?;
    write_labels(w, id, &labels)?;
    w.tx()
        .execute("UPDATE tasks SET version = 1 WHERE id = ?1", [b(id)])?;
    refresh_text(w, id)?;
    let key = format!("{}-{number}", project.key);
    let pid = Some(project.id);
    w.record_field(TASK, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(TASK, id, pid, "key", None, text(key));
    w.record_field(TASK, id, pid, "title", None, text(title));
    w.record_field(TASK, id, pid, "status", None, id_value(status));
    changed(w, id, Vec::new())
}

fn priority_index(p: Priority) -> i64 {
    match p {
        Priority::None => 0,
        Priority::Low => 1,
        Priority::Normal => 2,
        Priority::High => 3,
        Priority::Urgent => 4,
    }
}

fn update(w: &mut WriteCtx<'_>, input: TaskUpdateInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(OpError::not_found(
            format!("There is no task \"{}\".", input.key),
            &[],
        ))
    })?;
    lookup::ensure_active(&project)?;
    lookup::check_version(&task, input.expected_version, &input.key)?;
    let clears = |f: TaskField| input.clear.contains(&f);
    for (given, field, name) in [
        (
            input.description.is_some(),
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
    ] {
        if given && clears(field) {
            return Err(
                OpError::invalid_at("clear", format!("{name} is both given and cleared.")).into(),
            );
        }
    }
    let (id, pid) = (task.id, Some(project.id));
    let mut text_changed = false;
    if let Some(title) = &input.title {
        let title = gk_core::check_title(title).map_err(|e| invalid("title", e))?;
        text_changed |= fields::set(w, TASK, id, pid, "title", text(title))?;
    }
    if let Some(description) = &input.description {
        gk_core::check_markdown("description", description)
            .map_err(|e| invalid("description", e))?;
        text_changed |= fields::set(w, TASK, id, pid, "description", text(description.as_str()))?;
    }
    if clears(TaskField::Description) {
        text_changed |= fields::set(w, TASK, id, pid, "description", text(""))?;
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
    if let Some(p) = &input.parent {
        let parent = lookup::task(w.tx(), "parent", p)?;
        check_parent(w.tx(), Some(id), project.id, &parent, &input.key)?;
        fields::set(w, TASK, id, pid, "parent", id_value(parent.id))?;
    }
    if clears(TaskField::Parent) {
        fields::set(w, TASK, id, pid, "parent", None)?;
    }
    let start = match (&input.start_date, clears(TaskField::StartDate)) {
        (Some(d), _) => Some(text(day("start_date", d)?.to_string())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    let due = match (&input.due_date, clears(TaskField::DueDate)) {
        (Some(d), _) => Some(text(day("due_date", d)?.to_string())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    if start.is_some() || due.is_some() {
        let start_after = match &start {
            Some(v) => v.clone(),
            None => fields::read(w.tx(), TASK, id, "start_date")?,
        };
        let due_after = match &due {
            Some(v) => v.clone(),
            None => fields::read(w.tx(), TASK, id, "due_date")?,
        };
        check_dates(start_after.as_ref(), due_after.as_ref())?;
        if let Some(v) = start {
            fields::set(w, TASK, id, pid, "start_date", v)?;
        }
        if let Some(v) = due {
            fields::set(w, TASK, id, pid, "due_date", v)?;
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
            return Err(OpError::invalid_at("progress", "Progress is 0 to 100.").into());
        }
        fields::set(w, TASK, id, pid, "progress", Some(Value::from(p)))?;
    }
    if clears(TaskField::Progress) {
        fields::set(w, TASK, id, pid, "progress", None)?;
    }
    if text_changed {
        refresh_text(w, id)?;
    }
    changed(w, id, Vec::new())
}

fn move_task(w: &mut WriteCtx<'_>, input: TaskMoveInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let from = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(OpError::not_found(
            format!("There is no task \"{}\".", input.key),
            &[],
        ))
    })?;
    lookup::ensure_active(&from)?;
    lookup::check_version(&task, input.expected_version, &input.key)?;
    let mut warnings = Vec::new();
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
            &from,
            &to,
            input.status.as_deref(),
        )?);
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
                warnings.push(format!("{} still has {open} open subtasks.", input.key));
            }
        }
    }
    if let Some(target) = target {
        let rank = rank_in_column(w, to.id, status, task.id, target)?;
        fields::set(w, TASK, task.id, pid, "rank", text(rank.as_str()))?;
    }
    changed(w, task.id, warnings)
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
        .ok_or_else(|| Failure::Op(OpError::storage("A task needs a project.")))?;
    let n = number
        .as_ref()
        .and_then(Value::as_i64)
        .ok_or_else(|| Failure::Op(OpError::storage("A task needs a number.")))?;
    w.tx().execute(
        "UPDATE tasks SET project_id = ?2, number = ?3 WHERE id = ?1",
        params![b(id), b(project_id), n],
    )?;
    fields::touch(w, TASK, id, "project")?;
    w.record_field(TASK, id, record_under, "project", before_project, project);
    w.record_field(TASK, id, record_under, "number", before_number, number);
    Ok(())
}

/// Moves `task` and its subtasks from project `from` to `to`: new numbers,
/// the old keys kept as aliases, statuses, types and labels matched by name.
fn move_to_project(
    w: &mut WriteCtx<'_>,
    task: &TaskRow,
    from: &ProjectRow,
    to: &ProjectRow,
    status: Option<&str>,
) -> Result<Vec<String>, Failure> {
    let mut warnings = Vec::new();
    let to_statuses = lookup::all(w.tx(), List::Statuses, to.id)?;
    let to_types = lookup::all(w.tx(), List::Types, to.id)?;
    let to_labels = lookup::all(w.tx(), List::Labels, to.id)?;
    let fallback_status = first_status(w.tx(), to)?;
    let fallback_type = default_type(w.tx(), to)?;
    let root_status = status
        .map(|s| lookup::named(w.tx(), List::Statuses, to, "status", s).map(|n| n.id))
        .transpose()?;
    let pid = Some(to.id);
    for id in descendants(w.tx(), task.id)? {
        let (number, status_name, status_cat, type_name, closed): (
            i64,
            String,
            String,
            String,
            Option<i64>,
        ) = w.tx().query_row(
            "SELECT t.number, s.name, s.category, y.name, t.closed_at
                   FROM tasks t JOIN statuses s ON s.id = t.status_id
                   JOIN task_types y ON y.id = t.type_id WHERE t.id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        let new_number: i64 = w.tx().query_row(
            "UPDATE projects SET next_number = next_number + 1, updated_at = ?2 WHERE id = ?1
             RETURNING next_number - 1",
            params![b(to.id), w.now()],
            |r| r.get(0),
        )?;
        w.tx().execute(
            "INSERT INTO task_key_aliases (project_key, number, task_id) VALUES (?1, ?2, ?3)
             ON CONFLICT (project_key, number) DO UPDATE SET task_id = excluded.task_id",
            params![from.key, number, b(id)],
        )?;
        set_home(w, id, pid, id_value(to.id), Some(Value::from(new_number)))?;
        let new_status = match (id == task.id, root_status) {
            (true, Some(s)) => s,
            _ => to_statuses
                .iter()
                .find(|s| fold(&s.name) == fold(&status_name))
                .map_or(fallback_status, |s| s.id),
        };
        fields::set(w, TASK, id, pid, "status", id_value(new_status))?;
        let before: StatusCategory = status_cat.parse().unwrap_or(StatusCategory::Todo);
        let after = category_of(w.tx(), new_status)?;
        let closed_after = gk_core::closed_at_after_move(before, after, closed, w.now());
        fields::set(w, TASK, id, pid, "closed_at", closed_after.map(Value::from))?;
        let rank = rank_in_column(w, to.id, new_status, id, Target::Bottom)?;
        fields::set(w, TASK, id, pid, "rank", text(rank.as_str()))?;
        let new_type = to_types
            .iter()
            .find(|t| fold(&t.name) == fold(&type_name))
            .map_or(fallback_type, |t| t.id);
        fields::set(w, TASK, id, pid, "type", id_value(new_type))?;
        let mut kept = Vec::new();
        for label in task_labels(w.tx(), id)? {
            match to_labels
                .iter()
                .find(|l| fold(&l.name) == fold(&label.name))
            {
                Some(l) => kept.push(l.id),
                None => warnings.push(format!(
                    "Label \"{}\" doesn't exist in {} and was left off.",
                    label.name, to.key
                )),
            }
        }
        set_labels(w, id, to.id, &kept)?;
        if id == task.id && task.parent.is_some() {
            fields::set(w, TASK, id, pid, "parent", None)?;
        }
        refresh_text(w, id)?;
    }
    warnings.dedup();
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklists_count_task_list_items_outside_code() {
        let text = "- [ ] one\n- [x] two\n* [X] three\n1. [ ] four\n\
                    ```\n- [ ] not this\n```\n- not an item\n  + [ ] nested";
        assert_eq!(checklist(text), (2, 5));
        assert_eq!(checklist(""), (0, 0));
    }

    #[test]
    fn labels_are_recorded_sorted_or_not_at_all() {
        assert_eq!(labels_value(&[]), None);
        let (a, b) = (Id::generate(), Id::generate());
        assert_eq!(labels_value(&[b, a]), labels_value(&[a, b, a]));
    }
}
