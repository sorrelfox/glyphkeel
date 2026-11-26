// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Versions (releases) of a project, and the version each task is planned
//! for.
//!
//! A version is kept whole in history (one `record` field), like a saved
//! view, so creating, changing and deleting one can all be undone. A task's
//! version is an ordinary task field (`fix_version`).
//!
//! A task can be planned for a version of its own project, or of an
//! ancestor project that shares its versions with its sub-projects. When
//! that stops being true (the task or its project moves, the sharing
//! changes, the version goes), the task's version is cleared in the same
//! change, so it undoes with it.

use gk_api::OpError;
use gk_api::ops::common::Category;
use gk_api::ops::task::StatusRef;
use gk_api::ops::version::{
    Version, VersionChange, VersionCreate, VersionCreateInput, VersionDelete, VersionDeleteInput,
    VersionField, VersionGet, VersionGetInput, VersionList, VersionListInput, VersionListOp,
    VersionProgress, VersionRef, VersionSharing, VersionStatus, VersionTask, VersionUpdate,
    VersionUpdateInput,
};
use gk_core::progress::SubtaskProgress;
use gk_core::{Id, Rank, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK};
use super::lookup::{self, ProjectRow, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for versions.
pub(crate) const ENTITY: &str = "version";
/// The one recorded field: the whole version.
pub(crate) const RECORD: &str = "record";
/// The task field holding its version.
pub(crate) const TASK_FIELD: &str = "fix_version";
/// The longest version name, in characters.
const NAME_MAX: usize = 100;
/// The longest description, in characters.
const DESCRIPTION_MAX: usize = 4000;

pub(super) fn register(r: &mut Registry) {
    r.query::<VersionListOp, _>(desc::<VersionListOp>(), |q, input: VersionListInput| {
        list(q.tx, &input)
    });
    r.query::<VersionGet, _>(desc::<VersionGet>(), |q, input: VersionGetInput| {
        get(q.tx, &input)
    });
    r.command::<VersionCreate, _>(desc::<VersionCreate>(), create);
    r.command::<VersionUpdate, _>(desc::<VersionUpdate>(), update);
    r.command::<VersionDelete, _>(desc::<VersionDelete>(), delete);
}

/// A version as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) project: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) status: VersionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) due_date: Option<String>,
    pub(crate) sharing: VersionSharing,
    pub(crate) rank: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.parse().ok()
    }
}

const COLUMNS: &str = "id, project_id, name, description, status, start_date, due_date, \
                       sharing, rank, created_at, updated_at";

/// The order versions are listed in: by due date, those without one last,
/// then by rank.
const ORDER: &str = "due_date IS NULL, due_date, rank, id";

pub(crate) fn status_of(text: &str) -> VersionStatus {
    match text {
        "locked" => VersionStatus::Locked,
        "closed" => VersionStatus::Closed,
        _ => VersionStatus::Open,
    }
}

pub(crate) fn sharing_of(text: &str) -> VersionSharing {
    match text {
        "descendants" => VersionSharing::Descendants,
        _ => VersionSharing::None,
    }
}

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    Ok((
        id_at(r, 0)?,
        Record {
            project: id_at(r, 1)?.to_string(),
            name: r.get(2)?,
            description: r.get(3)?,
            status: status_of(&r.get::<_, String>(4)?),
            start_date: r.get(5)?,
            due_date: r.get(6)?,
            sharing: sharing_of(&r.get::<_, String>(7)?),
            rank: r.get(8)?,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM versions WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored version `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Writes version `id` as history recorded it (`None` deletes it), without
/// recording. A version that tasks are still planned for is not deleted.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        let used: i64 = w.tx().query_row(
            "SELECT count(*) FROM tasks WHERE fix_version_id = ?1",
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
            .execute("DELETE FROM versions WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    let project = r
        .project_id()
        .ok_or_else(|| Failure::Store(StoreError::Corrupt("version project".to_owned())))?;
    w.tx().execute(
        &format!(
            "INSERT INTO versions ({COLUMNS}, name_fold)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (id) DO UPDATE SET
               project_id = excluded.project_id, name = excluded.name,
               name_fold = excluded.name_fold, description = excluded.description,
               status = excluded.status, start_date = excluded.start_date,
               due_date = excluded.due_date, sharing = excluded.sharing,
               rank = excluded.rank, created_at = excluded.created_at,
               updated_at = excluded.updated_at"
        ),
        params![
            b(id),
            b(project),
            r.name,
            r.description,
            r.status.as_str(),
            r.start_date,
            r.due_date,
            r.sharing.as_str(),
            r.rank,
            r.created_at,
            r.updated_at,
            fold(&r.name),
        ],
    )?;
    Ok(())
}

/// Writes version `id` and records the change.
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

/// The project and its ancestors, nearest first.
fn ancestry(tx: &Transaction<'_>, project: Id) -> Result<Vec<Id>, Failure> {
    let mut out = vec![project];
    let mut at = project;
    // Sub-projects go at most 3 levels deep; the bound guards damaged data.
    for _ in 0..16 {
        let parent: Option<Option<Id>> = tx
            .query_row(
                "SELECT parent_id FROM projects WHERE id = ?1",
                [b(at)],
                |r| opt_id_at(r, 0),
            )
            .optional()?;
        match parent.flatten() {
            Some(p) if !out.contains(&p) => {
                out.push(p);
                at = p;
            }
            _ => break,
        }
    }
    Ok(out)
}

/// The versions tasks of `project` can be planned for: its own, then those
/// its ancestors share, nearest first. Each with whether it is inherited.
fn visible(tx: &Transaction<'_>, project: Id) -> Result<Vec<(Id, Record, bool)>, Failure> {
    let mut out = Vec::new();
    for (depth, p) in ancestry(tx, project)?.into_iter().enumerate() {
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM versions WHERE project_id = ?1 ORDER BY {ORDER}"
        ))?;
        let rows = stmt
            .query_map([b(p)], read_row)?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, r) in rows {
            if depth == 0 || r.sharing == VersionSharing::Descendants {
                out.push((id, r, depth > 0));
            }
        }
    }
    Ok(out)
}

/// Whether tasks of `project` can be planned for version `version`.
fn reaches(tx: &Transaction<'_>, version: Id, project: Id) -> Result<bool, Failure> {
    Ok(visible(tx, project)?
        .iter()
        .any(|(id, _, _)| *id == version))
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

/// The live tasks planned for version `id`, in board order.
fn task_ids(tx: &Transaction<'_>, id: Id) -> Result<Vec<Id>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT t.id FROM tasks t JOIN projects p ON p.id = t.project_id
          WHERE t.fix_version_id = ?1 AND t.deleted_at IS NULL AND p.deleted_at IS NULL
          ORDER BY t.parent_id IS NOT NULL, p.key, t.rank, t.id",
    )?;
    Ok(stmt
        .query_map([b(id)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?)
}

/// How far version `id` has come, and its tasks when `with_tasks`.
fn progress(
    tx: &Transaction<'_>,
    id: Id,
    with_tasks: bool,
) -> Result<(VersionProgress, Option<Vec<VersionTask>>), Failure> {
    let mut out = VersionProgress::default();
    let mut parts = Vec::new();
    let mut tasks = Vec::new();
    for task in task_ids(tx, id)? {
        let row = tx.query_row(
            "SELECT p.key, t.number, t.title, s.id, s.name, s.category, y.name,
                    t.estimate_min, t.spent_min, t.closed_at IS NOT NULL
               FROM tasks t JOIN projects p ON p.id = t.project_id
               JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
              WHERE t.id = ?1",
            [b(task)],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    id_at(r, 3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, bool>(9)?,
                ))
            },
        )?;
        let (key, number, title, status, status_name, category, ty, estimate, spent, closed) = row;
        let percent = super::rollup::progress(tx, task)?;
        out.total += 1;
        if closed {
            out.closed += 1;
        }
        out.estimate_minutes += estimate.unwrap_or(0);
        out.spent_minutes += spent;
        parts.push(SubtaskProgress {
            percent,
            estimate_minutes: estimate,
            closed,
        });
        if with_tasks {
            tasks.push(VersionTask {
                key: format!("{key}-{number}"),
                title,
                project: key,
                status: StatusRef {
                    id: status.to_string(),
                    name: status_name,
                    category: serde_json::from_value(Value::String(category))
                        .unwrap_or(Category::Todo),
                },
                closed,
                task_type: ty,
                progress_percent: percent,
                id: task.to_string(),
            });
        }
    }
    out.percent = gk_core::progress::subtask_progress(&parts).unwrap_or(0);
    Ok((out, with_tasks.then_some(tasks)))
}

fn output(
    tx: &Transaction<'_>,
    id: Id,
    r: &Record,
    inherited: bool,
    with_tasks: bool,
) -> Result<Version, Failure> {
    let (progress, tasks) = progress(tx, id, with_tasks)?;
    Ok(Version {
        name: r.name.clone(),
        project: project_key(tx, r.project_id())?,
        description: r.description.clone(),
        status: r.status,
        start_date: r.start_date.clone(),
        due_date: r.due_date.clone(),
        sharing: r.sharing,
        inherited,
        progress,
        tasks,
        rank: r.rank.clone(),
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
        id: id.to_string(),
    })
}

/// The version task `task` is planned for, as tasks show it.
pub(crate) fn of_task(tx: &Transaction<'_>, task: Id) -> Result<Option<VersionRef>, Failure> {
    let row = tx
        .query_row(
            "SELECT v.id, v.name, p.key, v.status, v.due_date
               FROM tasks t JOIN versions v ON v.id = t.fix_version_id
               JOIN projects p ON p.id = v.project_id
              WHERE t.id = ?1",
            [b(task)],
            |r| {
                Ok(VersionRef {
                    id: id_at(r, 0)?.to_string(),
                    name: r.get(1)?,
                    project: r.get(2)?,
                    status: status_of(&r.get::<_, String>(3)?),
                    due_date: r.get(4)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

fn list(tx: &Transaction<'_>, input: &VersionListInput) -> Result<VersionList, Failure> {
    let project = lookup::project(tx, "project", &input.project)?;
    let mut items = Vec::new();
    for (id, r, inherited) in visible(tx, project.id)? {
        if (inherited && input.own_only)
            || (!input.status.is_empty() && !input.status.contains(&r.status))
        {
            continue;
        }
        items.push(output(tx, id, &r, inherited, input.include_tasks)?);
    }
    // Own and inherited versions together, in one order: by due date, those
    // without one last; on the same day an ancestor's first.
    items.sort_by(|a, c| {
        let key = |v: &Version| {
            (
                v.due_date.is_none(),
                v.due_date.clone(),
                !v.inherited,
                v.rank.clone(),
                fold(&v.name),
            )
        };
        key(a).cmp(&key(c))
    });
    let unplanned = if input.include_tasks {
        let n: i64 = tx.query_row(
            "SELECT count(*) FROM tasks
              WHERE project_id = ?1 AND fix_version_id IS NULL AND deleted_at IS NULL
                AND closed_at IS NULL",
            [b(project.id)],
            |r| r.get(0),
        )?;
        Some(u32::try_from(n).unwrap_or(u32::MAX))
    } else {
        None
    };
    Ok(VersionList {
        total: items.len() as u64,
        items,
        unplanned,
    })
}

fn not_found(text: &str, names: &[&str]) -> Failure {
    let args = [("version", text.trim().to_owned())];
    let error = messages::with("version-not-found", &args, |m| OpError::not_found(m, names))
        .with_detail("field", "version");
    messages::hint(error, "version-not-found-hint", &[]).into()
}

/// The version `text` names: its ID, or its name among the versions
/// `project` sees (its own first), or among all versions without one.
fn find(
    tx: &Transaction<'_>,
    text: &str,
    project: Option<&String>,
) -> Result<(Id, Record, bool), Failure> {
    if let Ok(id) = text.trim().parse::<Id>()
        && let Some(r) = load(tx, id)?
        && r.project_id().is_some_and(crate::scope::sees)
    {
        return Ok((id, r, false));
    }
    let wanted = fold(text.trim());
    let candidates: Vec<(Id, Record, bool)> = match project {
        Some(p) => {
            let p = lookup::project(tx, "project", p)?;
            visible(tx, p.id)?
        }
        None => {
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM versions v
                  WHERE EXISTS (SELECT 1 FROM projects p
                                 WHERE p.id = v.project_id AND p.deleted_at IS NULL)
                  ORDER BY {ORDER}"
            ))?;
            stmt.query_map([], read_row)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|(_, r)| r.project_id().is_some_and(crate::scope::sees))
                .map(|(id, r)| (id, r, false))
                .collect()
        }
    };
    let found: Vec<&(Id, Record, bool)> = candidates
        .iter()
        .filter(|(_, r, _)| fold(&r.name) == wanted)
        .collect();
    // A project's own version comes before one an ancestor shares with it.
    let own: Vec<&&(Id, Record, bool)> = found.iter().filter(|(_, _, i)| !*i).collect();
    match (own.len(), found.len()) {
        (1, _) | (0, 1) => Ok((*own.first().copied().unwrap_or(&found[0])).clone()),
        (_, 0) => {
            let names: Vec<&str> = candidates.iter().map(|(_, r, _)| r.name.as_str()).collect();
            Err(not_found(text, &names))
        }
        _ => Err(messages::hint(
            messages::invalid_at(
                "version",
                "version-name-ambiguous",
                &[("version", text.trim().to_owned())],
            ),
            "version-name-ambiguous-hint",
            &[],
        )
        .into()),
    }
}

fn get(tx: &Transaction<'_>, input: &VersionGetInput) -> Result<Version, Failure> {
    let (id, r, inherited) = find(tx, &input.version, input.project.as_ref())?;
    output(tx, id, &r, inherited, true)
}

/// The version a task of `project` is to be planned for: `text` names it
/// among the versions the project sees, and it must be open.
pub(crate) fn resolve_for_task(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    field: &str,
    text: &str,
) -> Result<Id, Failure> {
    let options = visible(tx, project.id)?;
    let wanted = fold(text.trim());
    let by_id = text.trim().parse::<Id>().ok();
    let mut found: Vec<&(Id, Record, bool)> = options
        .iter()
        .filter(|(id, r, _)| Some(*id) == by_id || fold(&r.name) == wanted)
        .collect();
    // The project's own version first.
    found.sort_by_key(|(_, _, inherited)| *inherited);
    let Some((id, r, _)) = found.first() else {
        let names: Vec<&str> = options
            .iter()
            .filter(|(_, r, _)| r.status == VersionStatus::Open)
            .map(|(_, r, _)| r.name.as_str())
            .collect();
        let args = [
            ("version", text.trim().to_owned()),
            ("project", project.key.clone()),
        ];
        let error = messages::with("version-not-for-project", &args, |m| {
            OpError::not_found(m, &names)
        })
        .with_detail("field", field);
        return Err(messages::hint(error, "version-not-found-hint", &[]).into());
    };
    if r.status != VersionStatus::Open {
        let args = [
            ("version", r.name.clone()),
            ("status", r.status.as_str().to_owned()),
        ];
        let error = messages::conflict("version-not-open", &args).with_detail("field", field);
        return Err(messages::hint(error, "version-not-open-hint", &[]).into());
    }
    Ok(*id)
}

/// Plans task `task` (in `project`) for `version`, or for none; recorded.
pub(crate) fn set_task(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    version: Option<Id>,
) -> Result<bool, Failure> {
    fields::set(
        w,
        TASK,
        task,
        Some(project),
        TASK_FIELD,
        version.and_then(fields::id_value),
    )
}

/// Plans the copy `copy` (in `project`) of task `original` for the same
/// version, while that version is open.
pub(crate) fn copy_to(
    w: &mut WriteCtx<'_>,
    original: Id,
    copy: Id,
    project: Id,
) -> Result<(), Failure> {
    if let Some(v) = of_task(w.tx(), original)?
        && v.status == VersionStatus::Open
        && let Ok(id) = v.id.parse::<Id>()
        && reaches(w.tx(), id, project)?
    {
        set_task(w, copy, project, Some(id))?;
    }
    Ok(())
}

/// Clears the version of every task of `projects` that can no longer be
/// planned for it (after a move or a change of sharing). Returns the keys
/// of the tasks changed.
pub(crate) fn drop_unreachable(
    w: &mut WriteCtx<'_>,
    projects: &[Id],
) -> Result<Vec<String>, Failure> {
    let mut changed = Vec::new();
    for project in projects {
        let reachable: Vec<Id> = visible(w.tx(), *project)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let rows: Vec<(Id, Id, i64)> = {
            let mut stmt = w.tx().prepare_cached(
                "SELECT id, fix_version_id, number FROM tasks
                  WHERE project_id = ?1 AND fix_version_id IS NOT NULL",
            )?;
            stmt.query_map([b(*project)], |r| {
                Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };
        for (task, version, number) in rows {
            if !reachable.contains(&version) {
                set_task(w, task, *project, None)?;
                changed.push(lookup::task_key(w.tx(), *project, number)?);
            }
        }
    }
    Ok(changed)
}

/// Clears the version of task `task`, now in `project`, when that project
/// can't plan tasks for it. Returns the name of the version taken off.
pub(crate) fn drop_unreachable_task(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
) -> Result<Option<String>, Failure> {
    let Some(current) = of_task(w.tx(), task)? else {
        return Ok(None);
    };
    let version: Id = current
        .id
        .parse()
        .map_err(|_| Failure::Store(StoreError::Corrupt("version ID".to_owned())))?;
    if reaches(w.tx(), version, project)? {
        return Ok(None);
    }
    set_task(w, task, project, None)?;
    Ok(Some(current.name))
}

/// The project and every project below it.
fn subtree(tx: &Transaction<'_>, project: Id) -> Result<Vec<Id>, Failure> {
    let mut stmt = tx.prepare_cached(
        "WITH RECURSIVE below (id) AS (
           SELECT ?1 UNION SELECT p.id FROM projects p JOIN below ON p.parent_id = below.id
         )
         SELECT id FROM below",
    )?;
    Ok(stmt
        .query_map([b(project)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?)
}

/// After project `project` moved in the tree: the tasks of it and its
/// sub-projects lose versions they no longer see.
pub(crate) fn project_moved(w: &mut WriteCtx<'_>, project: Id) -> Result<Vec<String>, Failure> {
    let projects = subtree(w.tx(), project)?;
    drop_unreachable(w, &projects)
}

fn invalid(field: &str, e: impl gk_core::Message) -> Failure {
    super::task::invalid(field, e)
}

fn clean_name(name: &str) -> Result<String, Failure> {
    gk_core::check_line("name", name, NAME_MAX).map_err(|e| invalid("name", e))
}

fn clean_description(text: &str) -> Result<String, Failure> {
    let text = text.trim();
    if text.chars().count() > DESCRIPTION_MAX {
        return Err(messages::invalid(
            "description",
            "version-description-long",
            &[("max", DESCRIPTION_MAX.to_string())],
        ));
    }
    Ok(text.to_owned())
}

fn date(w: &WriteCtx<'_>, field: &str, text: &str) -> Result<String, Failure> {
    Ok(super::task::date_input(w, field, text)?.to_string())
}

fn check_dates(r: &Record) -> Result<(), Failure> {
    if let (Some(start), Some(due)) = (&r.start_date, &r.due_date)
        && start > due
    {
        return Err(messages::invalid("due_date", "version-dates-order", &[]));
    }
    Ok(())
}

/// Refuses a name another version of the same project has.
fn check_unique(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let Some(project) = r.project_id() else {
        return Ok(());
    };
    let taken: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM versions WHERE project_id = ?1 AND name_fold = ?2 AND id <> ?3",
            params![b(project), fold(&r.name), b(id)],
            |r| r.get(0),
        )
        .optional()?;
    if taken.is_some() {
        let error = messages::conflict("version-name-taken", &[("version", r.name.clone())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "version-name-taken-hint", &[]).into());
    }
    Ok(())
}

fn owner(tx: &Transaction<'_>, r: &Record) -> Result<ProjectRow, Failure> {
    let id = r
        .project_id()
        .ok_or_else(|| Failure::Store(StoreError::Corrupt("version project".to_owned())))?;
    lookup::project_by_id(tx, id)?.ok_or_else(|| {
        messages::not_found("version-not-found", &[("version", r.name.clone())]).into()
    })
}

fn create(w: &mut WriteCtx<'_>, input: VersionCreateInput) -> Result<VersionChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let last: Option<String> = w
        .tx()
        .query_row(
            "SELECT max(rank) FROM versions WHERE project_id = ?1",
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
        description: input
            .description
            .as_deref()
            .map(clean_description)
            .transpose()?
            .unwrap_or_default(),
        status: input.status.unwrap_or_default(),
        start_date: input
            .start_date
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(|d| date(w, "start_date", d))
            .transpose()?,
        due_date: input
            .due_date
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(|d| date(w, "due_date", d))
            .transpose()?,
        sharing: input.sharing.unwrap_or_default(),
        rank: rank.as_str().to_owned(),
        created_at: w.now(),
        updated_at: w.now(),
    };
    check_dates(&record)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    check_unique(w.tx(), id, &record)?;
    set(w, id, Some(&record))?;
    Ok(VersionChange {
        version: output(w.tx(), id, &record, false, false)?,
        deleted: false,
        tasks_changed: Vec::new(),
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        group: w.group_if_changed(),
    })
}

/// The projects whose saved views can name version `r`: its own, and its
/// sub-projects when it is shared.
fn naming_projects(tx: &Transaction<'_>, r: &Record) -> Result<Vec<Id>, Failure> {
    let Some(project) = r.project_id() else {
        return Ok(Vec::new());
    };
    Ok(if r.sharing == VersionSharing::Descendants {
        subtree(tx, project)?
    } else {
        vec![project]
    })
}

fn update(w: &mut WriteCtx<'_>, input: VersionUpdateInput) -> Result<VersionChange, Failure> {
    let (id, old, inherited) = find(w.tx(), &input.version, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    let clears = |f: VersionField| input.clear.contains(&f);
    for (given, field, name) in [
        (
            input.description.is_some(),
            VersionField::Description,
            "description",
        ),
        (
            input.start_date.is_some(),
            VersionField::StartDate,
            "start_date",
        ),
        (input.due_date.is_some(), VersionField::DueDate, "due_date"),
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
    if let Some(d) = &input.description {
        r.description = clean_description(d)?;
    }
    if clears(VersionField::Description) {
        r.description.clear();
    }
    if let Some(d) = &input.start_date {
        r.start_date = Some(date(w, "start_date", d)?);
    }
    if clears(VersionField::StartDate) {
        r.start_date = None;
    }
    if let Some(d) = &input.due_date {
        r.due_date = Some(date(w, "due_date", d)?);
    }
    if clears(VersionField::DueDate) {
        r.due_date = None;
    }
    if let Some(s) = input.status {
        r.status = s;
    }
    if let Some(s) = input.sharing {
        r.sharing = s;
    }
    check_dates(&r)?;
    // Where the open tasks go when it closes.
    let target = match &input.move_open_tasks_to {
        None => None,
        Some(_) if r.status != VersionStatus::Closed => {
            return Err(messages::invalid(
                "move_open_tasks_to",
                "version-move-needs-close",
                &[],
            ));
        }
        Some(t) if t.trim().eq_ignore_ascii_case("none") => Some(None),
        Some(t) => {
            let (target, tr, _) = find(w.tx(), t, Some(&project.key))?;
            if target == id {
                return Err(messages::invalid(
                    "move_open_tasks_to",
                    "version-move-to-itself",
                    &[],
                ));
            }
            if tr.status != VersionStatus::Open {
                let args = [
                    ("version", tr.name.clone()),
                    ("status", tr.status.as_str().to_owned()),
                ];
                let error = messages::conflict("version-not-open", &args)
                    .with_detail("field", "move_open_tasks_to");
                return Err(messages::hint(error, "version-not-open-hint", &[]).into());
            }
            Some(Some(target))
        }
    };
    let mut tasks_changed = Vec::new();
    if r != old {
        r.updated_at = w.now();
        check_unique(w.tx(), id, &r)?;
        set(w, id, Some(&r))?;
        if r.name != old.name {
            let mut projects = naming_projects(w.tx(), &old)?;
            for p in naming_projects(w.tx(), &r)? {
                if !projects.contains(&p) {
                    projects.push(p);
                }
            }
            for p in projects {
                super::view::rewrite_queries(w, Some(p), |q| {
                    gk_query::rename::rename(q, gk_query::NameKind::Version, &old.name, &r.name)
                })?;
            }
        }
        if r.sharing != old.sharing
            && let Some(p) = r.project_id()
        {
            let projects = subtree(w.tx(), p)?;
            tasks_changed.extend(drop_unreachable(w, &projects)?);
        }
    }
    if let Some(target) = target {
        let open: Vec<(Id, Id, i64)> = {
            let mut stmt = w.tx().prepare_cached(
                "SELECT id, project_id, number FROM tasks
                  WHERE fix_version_id = ?1 AND closed_at IS NULL AND deleted_at IS NULL
                  ORDER BY project_id, number",
            )?;
            stmt.query_map([b(id)], |r| Ok((id_at(r, 0)?, id_at(r, 1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for (task, task_project, number) in open {
            if let Some(t) = target
                && !reaches(w.tx(), t, task_project)?
            {
                let key = lookup::task_key(w.tx(), task_project, number)?;
                return Err(messages::conflict("version-move-not-shared", &[("task", key)]).into());
            }
            if set_task(w, task, task_project, target)? {
                tasks_changed.push(lookup::task_key(w.tx(), task_project, number)?);
            }
        }
    }
    Ok(VersionChange {
        version: output(w.tx(), id, &r, inherited, false)?,
        deleted: false,
        tasks_changed,
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        group: w.group_if_changed(),
    })
}

fn delete(w: &mut WriteCtx<'_>, input: VersionDeleteInput) -> Result<VersionChange, Failure> {
    let (id, old, inherited) = find(w.tx(), &input.version, input.project.as_ref())?;
    let project = owner(w.tx(), &old)?;
    lookup::ensure_active(&project)?;
    let version = output(w.tx(), id, &old, inherited, false)?;
    let tasks: Vec<(Id, Id, i64, bool)> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT id, project_id, number, deleted_at IS NULL FROM tasks
              WHERE fix_version_id = ?1 ORDER BY project_id, number",
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
    for p in naming_projects(w.tx(), &old)? {
        super::view::rewrite_queries(w, Some(p), |q| {
            gk_query::rename::remove_version(q, &old.name)
        })?;
    }
    set(w, id, None)?;
    Ok(VersionChange {
        version,
        deleted: true,
        tasks_changed,
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        group: w.group_if_changed(),
    })
}
