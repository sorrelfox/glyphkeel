// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Sprints of a project, and the sprint each task is in.
//!
//! A sprint is kept whole in history (one `record` field), like a version,
//! so creating, changing, starting, completing and deleting one can all be
//! undone. A task's sprint is an ordinary task field (`sprint`), so the
//! burndown can replay it from the history.
//!
//! A task can only be in a sprint of its own project. When it moves to
//! another project it leaves its sprint in the same change.

use gk_api::OpError;
use gk_api::ops::common::{Category, Priority};
use gk_api::ops::sprint::{
    BurndownDay, Sprint, SprintAddTasks, SprintAddTasksInput, SprintBurndown, SprintBurndownInput,
    SprintBurndownOp, SprintChange, SprintComplete, SprintCompleteInput, SprintCreate,
    SprintCreateInput, SprintDelete, SprintDeleteInput, SprintField, SprintGet, SprintGetInput,
    SprintList, SprintListInput, SprintListOp, SprintProgress, SprintRef, SprintRemoveTasks,
    SprintRemoveTasksInput, SprintStart, SprintStartInput, SprintState, SprintTask, SprintUpdate,
    SprintUpdateInput,
};
use gk_api::ops::task::StatusRef;
use gk_core::{Day, Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::types::ValueRef;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK};
use super::lookup::{self, ProjectRow, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for sprints.
pub(crate) const ENTITY: &str = "sprint";
/// The one recorded field: the whole sprint.
pub(crate) const RECORD: &str = "record";
/// The task field holding its sprint.
pub(crate) const TASK_FIELD: &str = "sprint";
/// The longest sprint name, in characters.
const NAME_MAX: usize = 100;
/// The longest goal, in characters.
const GOAL_MAX: usize = 4000;
/// The most tasks one call puts in or takes out of a sprint.
const TASKS_MAX: usize = 500;
/// The most days a burndown shows.
const BURNDOWN_DAYS_MAX: i64 = 366;
/// How long a sprint started without an end date runs, in days.
const DEFAULT_LENGTH_DAYS: i64 = 14;
/// The words that mean something else where a sprint is named.
const RESERVED: [&str; 3] = ["active", "none", "backlog"];

pub(super) fn register(r: &mut Registry) {
    r.query::<SprintListOp, _>(desc::<SprintListOp>(), |q, input: SprintListInput| {
        list(q.tx, &input)
    });
    r.query::<SprintGet, _>(desc::<SprintGet>(), |q, input: SprintGetInput| {
        get(q.tx, &input)
    });
    r.query::<SprintBurndownOp, _>(
        desc::<SprintBurndownOp>(),
        |q, input: SprintBurndownInput| burndown(q, &input),
    );
    r.command::<SprintCreate, _>(desc::<SprintCreate>(), create);
    r.command::<SprintUpdate, _>(desc::<SprintUpdate>(), update);
    r.command::<SprintDelete, _>(desc::<SprintDelete>(), delete);
    r.command::<SprintStart, _>(desc::<SprintStart>(), start);
    r.command::<SprintComplete, _>(desc::<SprintComplete>(), complete);
    r.command::<SprintAddTasks, _>(desc::<SprintAddTasks>(), add_tasks);
    r.command::<SprintRemoveTasks, _>(desc::<SprintRemoveTasks>(), remove_tasks);
}

/// A sprint as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) project: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) goal: String,
    pub(crate) state: SprintState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) completed_at: Option<i64>,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.parse().ok()
    }
}

const COLUMNS: &str = "id, project_id, name, goal, state, start_date, end_date, started_at, \
                       completed_at, rank, created_at, updated_at";

/// The order sprints are listed in: closed ones by when they were
/// completed, then the active one, then planned ones by rank.
const ORDER: &str = "CASE state WHEN 'closed' THEN 0 WHEN 'active' THEN 1 ELSE 2 END, \
                     completed_at, rank, id";

pub(crate) fn state_of(text: &str) -> SprintState {
    match text {
        "active" => SprintState::Active,
        "closed" => SprintState::Closed,
        _ => SprintState::Planned,
    }
}

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    Ok((
        id_at(r, 0)?,
        Record {
            project: id_at(r, 1)?.to_string(),
            name: r.get(2)?,
            goal: r.get(3)?,
            state: state_of(&r.get::<_, String>(4)?),
            start_date: r.get(5)?,
            end_date: r.get(6)?,
            started_at: r.get(7)?,
            completed_at: r.get(8)?,
            rank: r.get(9)?,
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM sprints WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored sprint `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// The project's active sprint other than `except`, if any.
fn other_active(tx: &Transaction<'_>, project: Id, except: Id) -> Result<Option<String>, Failure> {
    Ok(tx
        .query_row(
            "SELECT name FROM sprints WHERE project_id = ?1 AND state = 'active' AND id <> ?2",
            params![b(project), b(except)],
            |r| r.get(0),
        )
        .optional()?)
}

fn already_active(name: String) -> Failure {
    let error = messages::conflict("sprint-already-active", &[("sprint", name)]);
    messages::hint(error, "sprint-already-active-hint", &[]).into()
}

/// Writes sprint `id` as history recorded it (`None` deletes it), without
/// recording. A sprint that tasks are still in is not deleted, and a
/// second active sprint in a project is refused.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        let used: i64 = w.tx().query_row(
            "SELECT count(*) FROM tasks WHERE sprint_id = ?1",
            [b(id)],
            |r| r.get(0),
        )?;
        if used > 0 {
            return Err(messages::conflict(
                "history-in-use",
                &[("entity", ENTITY.to_owned()), ("count", used.to_string())],
            )
            .into());
        }
        w.tx()
            .execute("DELETE FROM sprints WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    let project = r
        .project_id()
        .ok_or_else(|| Failure::Store(StoreError::Corrupt("sprint project".to_owned())))?;
    if r.state == SprintState::Active
        && let Some(name) = other_active(w.tx(), project, id)?
    {
        return Err(already_active(name));
    }
    w.tx().execute(
        &format!(
            "INSERT INTO sprints ({COLUMNS}, name_fold)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT (id) DO UPDATE SET
               project_id = excluded.project_id, name = excluded.name,
               name_fold = excluded.name_fold, goal = excluded.goal,
               state = excluded.state, start_date = excluded.start_date,
               end_date = excluded.end_date, started_at = excluded.started_at,
               completed_at = excluded.completed_at, rank = excluded.rank,
               created_at = excluded.created_at, updated_at = excluded.updated_at"
        ),
        params![
            b(id),
            b(project),
            r.name,
            r.goal,
            r.state.as_str(),
            r.start_date,
            r.end_date,
            r.started_at,
            r.completed_at,
            r.rank,
            r.created_at,
            r.updated_at,
            fold(&r.name),
        ],
    )?;
    Ok(())
}

/// Writes sprint `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    let project = record.and_then(Record::project_id).or_else(|| {
        before
            .as_ref()
            .and_then(|v| v.get("project"))
            .and_then(Value::as_str)
            .and_then(|p| p.parse().ok())
    });
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, project, RECORD, before, after);
    Ok(())
}

fn project_key(tx: &Transaction<'_>, project: Option<Id>) -> Result<String, Failure> {
    Ok(match project {
        Some(p) => tx
            .query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_default(),
        None => String::new(),
    })
}

/// The columns a sprint task is read with; `t` is the task.
const TASK_SELECT: &str = "SELECT t.id, p.key, t.number, t.title, s.id, s.name, s.category, \
     y.name, t.priority, t.estimate_min, t.closed_at IS NOT NULL, \
     (SELECT k.number FROM tasks k WHERE k.id = t.parent_id) \
     FROM tasks t JOIN projects p ON p.id = t.project_id \
     JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id";

/// The board order tasks are listed in.
const TASK_ORDER: &str = "ORDER BY t.parent_id IS NOT NULL, t.rank, t.id";

fn task_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<SprintTask> {
    let key: String = r.get(1)?;
    let number: i64 = r.get(2)?;
    let category: String = r.get(6)?;
    let parent: Option<i64> = r.get(11)?;
    Ok(SprintTask {
        key: format!("{key}-{number}"),
        title: r.get(3)?,
        status: StatusRef {
            id: id_at(r, 4)?.to_string(),
            name: r.get(5)?,
            category: serde_json::from_value(Value::String(category)).unwrap_or(Category::Todo),
        },
        closed: r.get(10)?,
        task_type: r.get(7)?,
        priority: super::project::parse_enum(fields::priority_name(r.get(8)?))
            .unwrap_or(Priority::None),
        estimate_minutes: r.get(9)?,
        parent: parent.map(|n| format!("{key}-{n}")),
        id: id_at(r, 0)?.to_string(),
    })
}

/// The live tasks in sprint `id`, in board order.
fn tasks_of(tx: &Transaction<'_>, id: Id) -> Result<Vec<SprintTask>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "{TASK_SELECT} WHERE t.sprint_id = ?1 AND t.deleted_at IS NULL AND p.deleted_at IS NULL \
         {TASK_ORDER}"
    ))?;
    Ok(stmt
        .query_map([b(id)], task_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

/// The project's open tasks in no sprint, in board order.
fn backlog(tx: &Transaction<'_>, project: Id) -> Result<Vec<SprintTask>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "{TASK_SELECT} WHERE t.project_id = ?1 AND t.sprint_id IS NULL AND t.deleted_at IS NULL \
         AND t.closed_at IS NULL {TASK_ORDER}"
    ))?;
    Ok(stmt
        .query_map([b(project)], task_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

fn progress_of(tasks: &[SprintTask]) -> SprintProgress {
    let mut out = SprintProgress::default();
    for t in tasks {
        out.total += 1;
        let estimate = t.estimate_minutes.unwrap_or(0);
        out.estimate_minutes += estimate;
        if t.closed {
            out.closed += 1;
            out.closed_estimate_minutes += estimate;
        }
    }
    out
}

fn output(tx: &Transaction<'_>, id: Id, r: &Record, with_tasks: bool) -> Result<Sprint, Failure> {
    let tasks = tasks_of(tx, id)?;
    Ok(Sprint {
        name: r.name.clone(),
        project: project_key(tx, r.project_id())?,
        goal: r.goal.clone(),
        state: r.state,
        start_date: r.start_date.clone(),
        end_date: r.end_date.clone(),
        started_at: r.started_at.map(rfc3339),
        completed_at: r.completed_at.map(rfc3339),
        progress: progress_of(&tasks),
        tasks: with_tasks.then_some(tasks),
        rank: r.rank.clone(),
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        id: id.to_string(),
    })
}

pub(crate) fn reference(id: Id, r: &Record) -> SprintRef {
    SprintRef {
        name: r.name.clone(),
        state: r.state,
        start_date: r.start_date.clone(),
        end_date: r.end_date.clone(),
        id: id.to_string(),
    }
}

/// The sprint task `task` is in, as tasks show it.
pub(crate) fn of_task(tx: &Transaction<'_>, task: Id) -> Result<Option<SprintRef>, Failure> {
    let row = tx
        .prepare_cached(
            "SELECT sp.id, sp.name, sp.state, sp.start_date, sp.end_date
               FROM tasks t JOIN sprints sp ON sp.id = t.sprint_id
              WHERE t.id = ?1",
        )?
        .query_row([b(task)], |r| {
            Ok(SprintRef {
                id: id_at(r, 0)?.to_string(),
                name: r.get(1)?,
                state: state_of(&r.get::<_, String>(2)?),
                start_date: r.get(3)?,
                end_date: r.get(4)?,
            })
        })
        .optional()?;
    Ok(row)
}

/// The sprints of `project`, in list order.
pub(crate) fn of_project(tx: &Transaction<'_>, project: Id) -> Result<Vec<(Id, Record)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM sprints WHERE project_id = ?1 ORDER BY {ORDER}"
    ))?;
    Ok(stmt
        .query_map([b(project)], read_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

fn list(tx: &Transaction<'_>, input: &SprintListInput) -> Result<SprintList, Failure> {
    let project = lookup::project(tx, "project", &input.project)?;
    let mut items = Vec::new();
    for (id, r) in of_project(tx, project.id)? {
        if !input.state.is_empty() && !input.state.contains(&r.state) {
            continue;
        }
        items.push(output(tx, id, &r, input.include_tasks)?);
    }
    let backlog = if input.include_tasks {
        Some(backlog(tx, project.id)?)
    } else {
        None
    };
    Ok(SprintList {
        total: items.len() as u64,
        items,
        backlog,
    })
}

fn not_found(text: &str, names: &[&str]) -> Failure {
    let args = [("sprint", text.trim().to_owned())];
    let error = messages::with("sprint-not-found", &args, |m| OpError::not_found(m, names))
        .with_detail("field", "sprint");
    messages::hint(error, "sprint-not-found-hint", &[]).into()
}

/// The sprint `text` names: its ID; `active` or its name among the sprints
/// of `project`; or its name among all sprints without one.
pub(crate) fn find(
    tx: &Transaction<'_>,
    text: &str,
    project: Option<&String>,
) -> Result<(Id, Record), Failure> {
    let trimmed = text.trim();
    if let Ok(id) = trimmed.parse::<Id>()
        && let Some(r) = load(tx, id)?
        && r.project_id().is_some_and(crate::scope::sees)
    {
        return Ok((id, r));
    }
    let candidates: Vec<(Id, Record)> = match project {
        Some(p) => {
            let p = lookup::project(tx, "project", p)?;
            of_project(tx, p.id)?
        }
        None => {
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM sprints v
                  WHERE EXISTS (SELECT 1 FROM projects p
                                 WHERE p.id = v.project_id AND p.deleted_at IS NULL)
                  ORDER BY {ORDER}"
            ))?;
            stmt.query_map([], read_row)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|(_, r)| r.project_id().is_some_and(crate::scope::sees))
                .collect()
        }
    };
    let found: Vec<&(Id, Record)> = if trimmed.eq_ignore_ascii_case("active") {
        if project.is_none() {
            return Err(messages::invalid(
                "project",
                "sprint-active-needs-project",
                &[],
            ));
        }
        candidates
            .iter()
            .filter(|(_, r)| r.state == SprintState::Active)
            .collect()
    } else {
        let wanted = fold(trimmed);
        candidates
            .iter()
            .filter(|(_, r)| fold(&r.name) == wanted)
            .collect()
    };
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => {
            if trimmed.eq_ignore_ascii_case("active") {
                let error =
                    messages::not_found("sprint-none-active", &[]).with_detail("field", "sprint");
                return Err(messages::hint(error, "sprint-none-active-hint", &[]).into());
            }
            let names: Vec<&str> = candidates.iter().map(|(_, r)| r.name.as_str()).collect();
            Err(not_found(text, &names))
        }
        _ => Err(messages::hint(
            messages::invalid_at(
                "sprint",
                "sprint-name-ambiguous",
                &[("sprint", trimmed.to_owned())],
            ),
            "sprint-name-ambiguous-hint",
            &[],
        )
        .into()),
    }
}

fn get(tx: &Transaction<'_>, input: &SprintGetInput) -> Result<Sprint, Failure> {
    let (id, r) = find(tx, &input.sprint, input.project.as_ref())?;
    output(tx, id, &r, true)
}

fn not_open(r: &Record, field: &str) -> Failure {
    let error = messages::conflict("sprint-closed", &[("sprint", r.name.clone())])
        .with_detail("field", field);
    messages::hint(error, "sprint-closed-hint", &[]).into()
}

/// The sprint a task of `project` is to go in: `text` names it (or is
/// `active`) among the project's sprints, and it must not be closed.
pub(crate) fn resolve_for_task(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    field: &str,
    text: &str,
) -> Result<Id, Failure> {
    let (id, r) = find(tx, text, Some(&project.key)).map_err(|e| match e {
        Failure::Op(op) => Failure::Op(op.with_detail("field", field)),
        other => other,
    })?;
    if r.project_id() != Some(project.id) {
        let args = [("sprint", r.name.clone()), ("project", project.key.clone())];
        return Err(messages::invalid(field, "sprint-other-project", &args));
    }
    if r.state == SprintState::Closed {
        return Err(not_open(&r, field));
    }
    Ok(id)
}

/// The sprint running in `project` now, if any.
pub(crate) fn active_in(tx: &Transaction<'_>, project: Id) -> Result<Option<Id>, Failure> {
    Ok(tx
        .query_row(
            "SELECT id FROM sprints WHERE project_id = ?1 AND state = 'active'",
            [b(project)],
            |r| id_at(r, 0),
        )
        .optional()?)
}

/// Puts the copy `new` of task `old` in the sprint `old` is in, while that
/// sprint is of `project` and not closed (a closed sprint takes no tasks).
pub(crate) fn copy_to(w: &mut WriteCtx<'_>, old: Id, new: Id, project: Id) -> Result<(), Failure> {
    let row: Option<(Id, Id, String)> = w
        .tx()
        .query_row(
            "SELECT sp.id, sp.project_id, sp.state FROM tasks t JOIN sprints sp ON sp.id = t.sprint_id
              WHERE t.id = ?1",
            [b(old)],
            |r| Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((sprint, owner, state)) = row
        && owner == project
        && state != "closed"
    {
        set_task(w, new, project, Some(sprint))?;
    }
    Ok(())
}

/// Puts task `task` (in `project`) in `sprint`, or in none; recorded.
pub(crate) fn set_task(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    sprint: Option<Id>,
) -> Result<bool, Failure> {
    fields::set(
        w,
        TASK,
        task,
        Some(project),
        TASK_FIELD,
        sprint.and_then(fields::id_value),
    )
}

/// Task `task` is moving to project `to`: it leaves a sprint of another
/// project. Returns the name of the sprint it left.
pub(crate) fn leave_on_move(
    w: &mut WriteCtx<'_>,
    task: Id,
    to: Id,
) -> Result<Option<String>, Failure> {
    let row: Option<(Id, String)> = w
        .tx()
        .query_row(
            "SELECT sp.project_id, sp.name FROM tasks t JOIN sprints sp ON sp.id = t.sprint_id
              WHERE t.id = ?1",
            [b(task)],
            |r| Ok((id_at(r, 0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((project, name)) if project != to => {
            set_task(w, task, to, None)?;
            Ok(Some(name))
        }
        _ => Ok(None),
    }
}

fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    super::task::invalid(field, e)
}

fn clean_name(name: &str) -> Result<String, Failure> {
    let name = gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))?;
    if RESERVED.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
        return Err(messages::invalid(
            "name",
            "sprint-name-reserved",
            &[("name", name)],
        ));
    }
    Ok(name)
}

fn clean_goal(text: &str) -> Result<String, Failure> {
    let text = text.trim();
    if text.chars().count() > GOAL_MAX {
        return Err(messages::invalid(
            "goal",
            "sprint-goal-long",
            &[("max", GOAL_MAX.to_string())],
        ));
    }
    Ok(text.to_owned())
}

fn date(w: &WriteCtx<'_>, field: &str, text: &str) -> Result<String, Failure> {
    Ok(super::task::date_input(w, field, text)?.to_string())
}

fn optional_date(
    w: &WriteCtx<'_>,
    field: &str,
    text: Option<&str>,
) -> Result<Option<String>, Failure> {
    text.filter(|d| !d.trim().is_empty())
        .map(|d| date(w, field, d))
        .transpose()
}

fn check_dates(r: &Record) -> Result<(), Failure> {
    if let (Some(start), Some(end)) = (&r.start_date, &r.end_date)
        && start > end
    {
        return Err(messages::invalid("end_date", "sprint-dates-order", &[]));
    }
    Ok(())
}

/// Refuses a name another sprint of the same project has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let Some(project) = r.project_id() else {
        return Ok(());
    };
    let taken: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM sprints WHERE project_id = ?1 AND name_fold = ?2 AND id <> ?3",
            params![b(project), fold(&r.name), b(id)],
            |r| r.get(0),
        )
        .optional()?;
    if taken.is_some() {
        let error = messages::conflict("sprint-name-taken", &[("sprint", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "sprint-name-taken-hint", &[]).into());
    }
    Ok(())
}

fn owner(tx: &Transaction<'_>, r: &Record) -> Result<ProjectRow, Failure> {
    let id = r
        .project_id()
        .ok_or_else(|| Failure::Store(StoreError::Corrupt("sprint project".to_owned())))?;
    lookup::project_by_id(tx, id)?.ok_or_else(|| {
        messages::not_found("sprint-not-found", &[("sprint", r.name.clone())]).into()
    })
}

/// Puts the tasks `texts` names in sprint `id` of `project`. Returns the
/// keys of the tasks that moved.
fn put_tasks(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: &ProjectRow,
    texts: &[String],
) -> Result<Vec<String>, Failure> {
    if texts.len() > TASKS_MAX {
        return Err(messages::invalid(
            "tasks",
            "sprint-too-many-tasks",
            &[("max", TASKS_MAX.to_string())],
        ));
    }
    let mut changed = Vec::new();
    for text in texts {
        let task = lookup::task(w.tx(), "tasks", text)?;
        if task.project != project.id {
            let key = lookup::task_key(w.tx(), task.project, task.number)?;
            return Err(messages::invalid(
                "tasks",
                "sprint-task-other-project",
                &[("task", key), ("project", project.key.clone())],
            ));
        }
        if set_task(w, task.id, project.id, Some(id))? {
            changed.push(lookup::task_key(w.tx(), task.project, task.number)?);
        }
    }
    Ok(changed)
}

fn change(
    w: &WriteCtx<'_>,
    id: Id,
    r: &Record,
    tasks_changed: Vec<String>,
) -> Result<SprintChange, Failure> {
    Ok(SprintChange {
        sprint: Some(output(w.tx(), id, r, false)?),
        deleted: false,
        tasks_changed,
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        group: w.group_if_changed(),
    })
}

fn create(w: &mut WriteCtx<'_>, input: SprintCreateInput) -> Result<SprintChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let last: Option<String> = w
        .tx()
        .query_row(
            "SELECT max(rank) FROM sprints WHERE project_id = ?1",
            [b(project.id)],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let rank = last
        .and_then(|r| Rank::parse(&r).ok())
        .map_or_else(Rank::first, |r| r.after());
    let record = Record {
        project: project.id.to_string(),
        name: clean_name(&input.name)?,
        goal: input
            .goal
            .as_deref()
            .map(clean_goal)
            .transpose()?
            .unwrap_or_default(),
        state: SprintState::Planned,
        start_date: optional_date(w, "start_date", input.start_date.as_deref())?,
        end_date: optional_date(w, "end_date", input.end_date.as_deref())?,
        started_at: None,
        completed_at: None,
        rank: rank.as_str().to_owned(),
        created_at: w.now(),
        updated_at: w.now(),
    };
    check_dates(&record)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    let tasks_changed = put_tasks(w, id, &project, &input.tasks)?;
    change(w, id, &record, tasks_changed)
}

fn update(w: &mut WriteCtx<'_>, input: SprintUpdateInput) -> Result<SprintChange, Failure> {
    let (id, old) = find(w.tx(), &input.sprint, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    let clears = |f: SprintField| input.clear.contains(&f);
    for (given, field, name) in [
        (input.goal.is_some(), SprintField::Goal, "goal"),
        (
            input.start_date.is_some(),
            SprintField::StartDate,
            "start_date",
        ),
        (input.end_date.is_some(), SprintField::EndDate, "end_date"),
    ] {
        if given && clears(field) {
            return Err(messages::invalid(
                "clear",
                "field-given-and-cleared",
                &[("field", name.to_owned())],
            ));
        }
    }
    let mut r = old.clone();
    if let Some(name) = &input.name {
        r.name = clean_name(name)?;
    }
    if let Some(g) = &input.goal {
        r.goal = clean_goal(g)?;
    }
    if clears(SprintField::Goal) {
        r.goal.clear();
    }
    if let Some(d) = &input.start_date {
        r.start_date = Some(date(w, "start_date", d)?);
    }
    if clears(SprintField::StartDate) {
        r.start_date = None;
    }
    if let Some(d) = &input.end_date {
        r.end_date = Some(date(w, "end_date", d)?);
    }
    if clears(SprintField::EndDate) {
        r.end_date = None;
    }
    check_dates(&r)?;
    if r != old {
        r.updated_at = w.now();
        check_unique(w.tx(), id, &r)?;
        set(w, id, Some(&r))?;
        if r.name != old.name {
            super::view::rewrite_queries(w, Some(project.id), |q| {
                gk_query::rename::rename(q, gk_query::NameKind::Sprint, &old.name, &r.name)
            })?;
        }
    }
    change(w, id, &r, Vec::new())
}

fn delete(w: &mut WriteCtx<'_>, input: SprintDeleteInput) -> Result<SprintChange, Failure> {
    let (id, old) = find(w.tx(), &input.sprint, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    if old.state == SprintState::Active {
        let error = messages::conflict("sprint-delete-active", &[("sprint", old.name.clone())]);
        return Err(messages::hint(error, "sprint-delete-active-hint", &[]).into());
    }
    let sprint = output(w.tx(), id, &old, false)?;
    let tasks: Vec<(Id, Id, i64, bool)> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT id, project_id, number, deleted_at IS NULL FROM tasks
              WHERE sprint_id = ?1 ORDER BY number",
        )?;
        stmt.query_map([b(id)], |r| {
            Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let mut tasks_changed = Vec::new();
    for (task, task_project, number, live) in tasks {
        set_task(w, task, task_project, None)?;
        if live {
            tasks_changed.push(lookup::task_key(w.tx(), task_project, number)?);
        }
    }
    super::view::rewrite_queries(w, Some(project.id), |q| {
        gk_query::rename::remove_sprint(q, &old.name)
    })?;
    set(w, id, None)?;
    Ok(SprintChange {
        sprint: Some(sprint),
        deleted: true,
        tasks_changed,
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        group: w.group_if_changed(),
    })
}

fn wrong_state(r: &Record, wanted: &str) -> Failure {
    let args = [
        ("sprint", r.name.clone()),
        ("state", r.state.as_str().to_owned()),
    ];
    messages::conflict(wanted, &args)
        .with_detail("field", "sprint")
        .into()
}

fn start(w: &mut WriteCtx<'_>, input: SprintStartInput) -> Result<SprintChange, Failure> {
    let (id, old) = find(w.tx(), &input.sprint, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    if old.state != SprintState::Planned {
        return Err(wrong_state(&old, "sprint-start-not-planned"));
    }
    if let Some(name) = other_active(w.tx(), project.id, id)? {
        return Err(already_active(name));
    }
    let mut r = old.clone();
    if let Some(g) = &input.goal {
        r.goal = clean_goal(g)?;
    }
    let today = w.zone().day_of(w.now());
    let first: Day = match input.start_date.as_deref().filter(|d| !d.trim().is_empty()) {
        Some(d) => super::task::date_input(w, "start_date", d)?,
        None => r
            .start_date
            .as_deref()
            .and_then(|d| d.parse().ok())
            .unwrap_or(today),
    };
    let last: Day = match input.end_date.as_deref().filter(|d| !d.trim().is_empty()) {
        Some(d) => super::task::date_input(w, "end_date", d)?,
        None => r
            .end_date
            .as_deref()
            .and_then(|d| d.parse().ok())
            .filter(|end| *end >= first)
            .unwrap_or_else(|| first.add_days(DEFAULT_LENGTH_DAYS - 1).unwrap_or(first)),
    };
    r.start_date = Some(first.to_string());
    r.end_date = Some(last.to_string());
    check_dates(&r)?;
    r.state = SprintState::Active;
    r.started_at = Some(w.now());
    r.completed_at = None;
    r.updated_at = w.now();
    set(w, id, Some(&r))?;
    change(w, id, &r, Vec::new())
}

fn complete(w: &mut WriteCtx<'_>, input: SprintCompleteInput) -> Result<SprintChange, Failure> {
    let (id, old) = find(w.tx(), &input.sprint, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    if old.state != SprintState::Active {
        return Err(wrong_state(&old, "sprint-complete-not-active"));
    }
    let target = match input.move_open_tasks_to.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(t) if t.eq_ignore_ascii_case("backlog") || t.eq_ignore_ascii_case("none") => None,
        Some(t) => {
            let (target, tr) = find(w.tx(), t, Some(&project.key)).map_err(|e| match e {
                Failure::Op(op) => Failure::Op(op.with_detail("field", "move_open_tasks_to")),
                other => other,
            })?;
            if target == id {
                return Err(messages::invalid(
                    "move_open_tasks_to",
                    "sprint-move-to-itself",
                    &[],
                ));
            }
            if tr.state != SprintState::Planned {
                let args = [
                    ("sprint", tr.name.clone()),
                    ("state", tr.state.as_str().to_owned()),
                ];
                let error = messages::conflict("sprint-move-not-planned", &args)
                    .with_detail("field", "move_open_tasks_to");
                return Err(error.into());
            }
            Some(target)
        }
    };
    let open: Vec<(Id, Id, i64)> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT id, project_id, number FROM tasks
              WHERE sprint_id = ?1 AND closed_at IS NULL AND deleted_at IS NULL
              ORDER BY number",
        )?;
        stmt.query_map([b(id)], |r| Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut tasks_changed = Vec::new();
    for (task, task_project, number) in open {
        if set_task(w, task, task_project, target)? {
            tasks_changed.push(lookup::task_key(w.tx(), task_project, number)?);
        }
    }
    let mut r = old.clone();
    r.state = SprintState::Closed;
    r.completed_at = Some(w.now());
    r.updated_at = w.now();
    set(w, id, Some(&r))?;
    change(w, id, &r, tasks_changed)
}

fn add_tasks(w: &mut WriteCtx<'_>, input: SprintAddTasksInput) -> Result<SprintChange, Failure> {
    let (id, r) = find(w.tx(), &input.sprint, input.project.as_ref())?;
    let project = owner(w.tx(), &r)?;
    lookup::ensure_active(&project)?;
    if r.state == SprintState::Closed {
        return Err(not_open(&r, "sprint"));
    }
    if input.tasks.is_empty() {
        return Err(messages::invalid("tasks", "sprint-no-tasks", &[]));
    }
    let tasks_changed = put_tasks(w, id, &project, &input.tasks)?;
    change(w, id, &r, tasks_changed)
}

fn remove_tasks(
    w: &mut WriteCtx<'_>,
    input: SprintRemoveTasksInput,
) -> Result<SprintChange, Failure> {
    if input.tasks.is_empty() {
        return Err(messages::invalid("tasks", "sprint-no-tasks", &[]));
    }
    if input.tasks.len() > TASKS_MAX {
        return Err(messages::invalid(
            "tasks",
            "sprint-too-many-tasks",
            &[("max", TASKS_MAX.to_string())],
        ));
    }
    let mut tasks_changed = Vec::new();
    for text in &input.tasks {
        let task = lookup::task(w.tx(), "tasks", text)?;
        if set_task(w, task.id, task.project, None)? {
            tasks_changed.push(lookup::task_key(w.tx(), task.project, task.number)?);
        }
    }
    Ok(SprintChange {
        sprint: None,
        deleted: false,
        tasks_changed,
        views_rewritten: Vec::new(),
        group: w.group_if_changed(),
    })
}

/// One field of a task through time: its value before the first recorded
/// change, and after each change.
#[derive(Debug, Default)]
struct Track {
    /// The value now, when nothing was recorded.
    now: Option<Value>,
    /// `(at, before, after)` in the order they happened.
    changes: Vec<(i64, Option<Value>, Option<Value>)>,
}

impl Track {
    /// The value at moment `at`.
    fn at(&self, at: i64) -> Option<&Value> {
        match self.changes.iter().rposition(|(when, _, _)| *when <= at) {
            Some(i) => self.changes[i].2.as_ref(),
            None => match self.changes.first() {
                Some((_, before, _)) => before.as_ref(),
                None => self.now.as_ref(),
            },
        }
    }
}

/// A task's past, as far as the burndown needs it.
#[derive(Debug, Default)]
struct Past {
    created: i64,
    sprint: Track,
    closed: Track,
    trashed: Track,
    estimate: Track,
}

fn json_at(r: &gk_store::rusqlite::Row<'_>, i: usize) -> gk_store::rusqlite::Result<Option<Value>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Null => None,
        ValueRef::Integer(n) => Some(Value::from(n)),
        ValueRef::Real(f) => Some(Value::from(f)),
        ValueRef::Text(t) => Some(Value::String(String::from_utf8_lossy(t).into_owned())),
        ValueRef::Blob(_) => None,
    })
}

/// A task as it is now: created, sprint, closed, in the trash, estimate.
type TaskNow = (i64, Option<Id>, Option<i64>, bool, Option<i64>);

/// The pasts of every task that was ever in sprint `id`.
fn pasts(tx: &Transaction<'_>, id: Id) -> Result<Vec<Past>, Failure> {
    let wanted = id.to_string();
    let mut ids: Vec<Id> = {
        let mut stmt = tx.prepare_cached(
            "SELECT DISTINCT entity_id FROM activity
              WHERE entity_kind = 'task'
                AND (json_extract(changes, '$.fields.sprint.after') = ?1
                     OR json_extract(changes, '$.fields.sprint.before') = ?1)",
        )?;
        stmt.query_map([&wanted], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    {
        let mut stmt = tx.prepare_cached("SELECT id FROM tasks WHERE sprint_id = ?1")?;
        for t in stmt.query_map([b(id)], |r| id_at(r, 0))? {
            let t = t?;
            if !ids.contains(&t) {
                ids.push(t);
            }
        }
    }
    let mut out = Vec::new();
    for task in ids {
        let row: Option<TaskNow> = tx
            .query_row(
                "SELECT created_at, sprint_id, closed_at, deleted_at IS NOT NULL, estimate_min
                   FROM tasks WHERE id = ?1",
                [b(task)],
                |r| Ok((r.get(0)?, opt_id_at(r, 1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((created, sprint, closed, trashed, estimate)) = row else {
            continue;
        };
        let mut past = Past {
            created,
            sprint: Track {
                now: sprint.map(|s| Value::String(s.to_string())),
                changes: Vec::new(),
            },
            closed: Track {
                now: closed.map(Value::from),
                changes: Vec::new(),
            },
            trashed: Track {
                now: trashed.then(|| Value::from(1)),
                changes: Vec::new(),
            },
            estimate: Track {
                now: estimate.map(Value::from),
                changes: Vec::new(),
            },
        };
        let mut stmt = tx.prepare_cached(
            "SELECT at,
                    json_type(changes, '$.fields.sprint'),
                    json_extract(changes, '$.fields.sprint.before'),
                    json_extract(changes, '$.fields.sprint.after'),
                    json_type(changes, '$.fields.closed_at'),
                    json_extract(changes, '$.fields.closed_at.before'),
                    json_extract(changes, '$.fields.closed_at.after'),
                    json_type(changes, '$.fields.trashed'),
                    json_extract(changes, '$.fields.trashed.before'),
                    json_extract(changes, '$.fields.trashed.after'),
                    json_type(changes, '$.fields.estimate'),
                    json_extract(changes, '$.fields.estimate.before'),
                    json_extract(changes, '$.fields.estimate.after')
               FROM activity
              WHERE entity_kind = 'task' AND entity_id = ?1
              ORDER BY seq",
        )?;
        let mut rows = stmt.query([b(task)])?;
        while let Some(r) = rows.next()? {
            let at: i64 = r.get(0)?;
            for (track, i) in [
                (&mut past.sprint, 1),
                (&mut past.closed, 4),
                (&mut past.trashed, 7),
                (&mut past.estimate, 10),
            ] {
                if r.get::<_, Option<String>>(i)?.is_some() {
                    track
                        .changes
                        .push((at, json_at(r, i + 1)?, json_at(r, i + 2)?));
                }
            }
        }
        out.push(past);
    }
    Ok(out)
}

/// Whether a recorded value says yes: set, and not `false` or `0`.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64() != Some(0),
        Some(_) => true,
    }
}

pub(crate) fn burndown(
    q: &QueryCtx<'_>,
    input: &SprintBurndownInput,
) -> Result<SprintBurndown, Failure> {
    let tx = q.tx;
    let (id, r) = find(tx, &input.sprint, input.project.as_ref())?;
    if r.state == SprintState::Planned {
        let error =
            messages::conflict("sprint-burndown-not-started", &[("sprint", r.name.clone())])
                .with_detail("field", "sprint");
        return Err(error.into());
    }
    let zone = &q.env.zone;
    let today = zone.day_of(q.now);
    let first: Day = r
        .start_date
        .as_deref()
        .and_then(|d| d.parse().ok())
        .or_else(|| r.started_at.map(|at| zone.day_of(at)))
        .unwrap_or(today);
    // Up to its completion, or to today while it runs.
    let (last, cutoff) = match (r.state, r.completed_at) {
        (SprintState::Closed, Some(at)) => (zone.day_of(at), Some(at - 1)),
        _ => (today, None),
    };
    let pasts = pasts(tx, id)?;
    let wanted = Value::String(id.to_string());
    let mut days = Vec::new();
    let mut day = first;
    while day <= last && (days.len() as i64) < BURNDOWN_DAYS_MAX {
        let end_of_day = zone.moment(day.add_days(1).unwrap_or(day), 0) - 1;
        let at = cutoff.map_or(end_of_day, |c| c.min(end_of_day));
        let mut entry = BurndownDay {
            date: day.to_string(),
            open_tasks: 0,
            total_tasks: 0,
            open_estimate_minutes: 0,
            total_estimate_minutes: 0,
        };
        for p in &pasts {
            if p.created > at || p.sprint.at(at) != Some(&wanted) || truthy(p.trashed.at(at)) {
                continue;
            }
            let estimate = p.estimate.at(at).and_then(Value::as_i64).unwrap_or(0);
            entry.total_tasks += 1;
            entry.total_estimate_minutes += estimate;
            if !truthy(p.closed.at(at)) {
                entry.open_tasks += 1;
                entry.open_estimate_minutes += estimate;
            }
        }
        days.push(entry);
        match day.add_days(1) {
            Ok(next) => day = next,
            Err(_) => break,
        }
    }
    let (start_tasks, start_estimate_minutes) = days
        .first()
        .map_or((0, 0), |d| (d.total_tasks, d.total_estimate_minutes));
    Ok(SprintBurndown {
        sprint: reference(id, &r),
        days,
        start_tasks,
        start_estimate_minutes,
    })
}
