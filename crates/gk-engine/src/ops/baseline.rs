// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Schedule baselines: named snapshots of a project's dated tasks, for the
//! Gantt chart's ghost bars and the query field `slip`.
//!
//! History keeps a baseline as two fields: `record` (its project, name and
//! who saved it) and `tasks` (the snapshot). Revert writes `record` before
//! `tasks` (history applies a row's fields in name order), so a deleted
//! baseline comes back before its tasks do, and going away takes its
//! tasks with it.

use std::collections::HashMap;

use gk_api::OpError;
use gk_api::ops::baseline::{
    Baseline, BaselineChange, BaselineCreate, BaselineCreateInput, BaselineDelete, BaselineList,
    BaselineListInput, BaselineListOp, BaselineRef, BaselineRename, BaselineRenameInput,
};
use gk_api::ops::common::{Actor, ActorKind};
use gk_core::{Id, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::lookup::{self, b, id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for baselines.
pub(crate) const ENTITY: &str = "baseline";
/// The recorded field with the baseline's name and who saved it.
pub(crate) const RECORD: &str = "record";
/// The recorded field with its tasks.
pub(crate) const TASKS: &str = "tasks";
/// The longest name, in characters.
const NAME_MAX: usize = 100;

pub(super) fn register(r: &mut Registry) {
    r.query::<BaselineListOp, _>(desc::<BaselineListOp>(), |q, input: BaselineListInput| {
        list(q.tx, &input)
    });
    r.command::<BaselineCreate, _>(desc::<BaselineCreate>(), create);
    r.command::<BaselineRename, _>(desc::<BaselineRename>(), rename);
    r.command::<BaselineDelete, _>(desc::<BaselineDelete>(), delete);
}

// ---------------------------------------------------------------------
// Stored baselines

/// A baseline as history records it, without its tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) project: String,
    pub(crate) name: String,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

impl Record {
    fn project_id(&self) -> Option<Id> {
        self.project.parse().ok()
    }
}

/// One task of a baseline, as history and archives keep it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) task: String,
    pub(crate) key: String,
    pub(crate) title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) due: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) milestone: bool,
}

const COLUMNS: &str =
    "id, project_id, name, created_at, created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    Ok((
        id_at(r, 0)?,
        Record {
            project: id_at(r, 1)?.to_string(),
            name: r.get(2)?,
            created_at: r.get(3)?,
            created_by_kind: r.get(4)?,
            created_by_name: r.get(5)?,
            updated_at: r.get(6)?,
        },
    ))
}

fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Record>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM baselines WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The tasks of baseline `id`, by key order.
pub(crate) fn entries(tx: &Transaction<'_>, id: Id) -> Result<Vec<Entry>, StoreError> {
    let mut stmt = tx.prepare_cached(
        "SELECT task_id, task_key, title, start_date, due_date, milestone
           FROM baseline_tasks WHERE baseline_id = ?1 ORDER BY task_key, task_id",
    )?;
    let rows = stmt
        .query_map([b(id)], |r| {
            Ok(Entry {
                task: id_at(r, 0)?.to_string(),
                key: r.get(1)?,
                title: r.get(2)?,
                start: r.get(3)?,
                due: r.get(4)?,
                milestone: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn to_value<T: Serialize>(v: &T) -> Result<Value, StoreError> {
    serde_json::to_value(v).map_err(|e| StoreError::Corrupt(e.to_string()))
}

/// Baseline `id`'s `record` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?.as_ref().map(to_value).transpose()
}

/// Baseline `id`'s `tasks` as history records them; `None` when the
/// baseline is gone.
pub(crate) fn raw_tasks(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    if load(tx, id)?.is_none() {
        return Ok(None);
    }
    Ok(Some(to_value(&entries(tx, id)?)?))
}

fn insert(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<(), Failure> {
    let project = r
        .project_id()
        .ok_or_else(|| Failure::Store(StoreError::Corrupt("baseline project".to_owned())))?;
    tx.execute(
        "INSERT INTO baselines (id, project_id, name, name_fold, created_at, created_by_kind,
                                created_by_name, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (id) DO UPDATE SET project_id = excluded.project_id, name = excluded.name,
                name_fold = excluded.name_fold, created_at = excluded.created_at,
                created_by_kind = excluded.created_by_kind,
                created_by_name = excluded.created_by_name, updated_at = excluded.updated_at",
        params![
            b(id),
            b(project),
            r.name,
            fold(&r.name),
            r.created_at,
            r.created_by_kind,
            r.created_by_name,
            r.updated_at,
        ],
    )?;
    Ok(())
}

fn insert_entries(tx: &Transaction<'_>, id: Id, list: &[Entry]) -> Result<(), Failure> {
    tx.execute("DELETE FROM baseline_tasks WHERE baseline_id = ?1", [b(id)])?;
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO baseline_tasks (baseline_id, task_id, task_key, title,
                                                start_date, due_date, milestone)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for e in list {
        let task: Id = e
            .task
            .parse()
            .map_err(|_| Failure::Store(StoreError::Corrupt("baseline task".to_owned())))?;
        if e.start.is_none() && e.due.is_none() {
            continue;
        }
        stmt.execute(params![
            b(id),
            b(task),
            e.key,
            e.title,
            e.start,
            e.due,
            e.milestone
        ])?;
    }
    Ok(())
}

/// Writes baseline `id`'s `record` as history recorded it (`None` deletes
/// the baseline and its tasks), without recording.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        w.tx()
            .execute("DELETE FROM baselines WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    insert(w.tx(), id, &r)
}

/// Writes baseline `id`'s `tasks` as history recorded them, without
/// recording. The baseline itself must be there; `None` leaves only it.
pub(crate) fn write_raw_tasks(
    w: &WriteCtx<'_>,
    id: Id,
    value: Option<&Value>,
) -> Result<(), Failure> {
    let list: Vec<Entry> = match value {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?,
        None => Vec::new(),
    };
    if value.is_some() && load(w.tx(), id)?.is_none() {
        return Err(Failure::Store(StoreError::Corrupt(
            "baseline tasks without their baseline".to_owned(),
        )));
    }
    insert_entries(w.tx(), id, &list)
}

/// Writes back what history recorded of a baseline and records the change.
pub(crate) fn apply(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    if field == TASKS {
        let before = raw_tasks(w.tx(), id)?;
        write_raw_tasks(w, id, value.as_ref())?;
        w.record_field(ENTITY, id, project, TASKS, before, value);
    } else {
        // Going away takes the tasks along: recorded too, so that taking
        // this change back brings them back.
        if value.is_none() {
            let tasks = raw_tasks(w.tx(), id)?;
            w.record_field(ENTITY, id, project, TASKS, tasks, None);
        }
        let before = raw(w.tx(), id)?;
        write_raw(w, id, value.as_ref())?;
        w.record_field(ENTITY, id, project, RECORD, before, value);
    }
    Ok(())
}

/// Stores a baseline from an archive, outside history (an import is one
/// change of its own).
pub(crate) fn insert_imported(
    tx: &Transaction<'_>,
    id: Id,
    r: &Record,
    list: &[Entry],
) -> Result<(), Failure> {
    insert(tx, id, r)?;
    insert_entries(tx, id, list)
}

/// The baselines of project `project`, newest first.
pub(crate) fn rows(tx: &Transaction<'_>, project: Id) -> Result<Vec<(Id, Record)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM baselines WHERE project_id = ?1
          ORDER BY created_at DESC, id DESC"
    ))?;
    Ok(stmt
        .query_map([b(project)], read_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

/// The baseline of `project` that `text` names: its ID, or its name
/// (ignoring case).
pub(crate) fn find(
    tx: &Transaction<'_>,
    project: Id,
    field: &str,
    text: &str,
) -> Result<(Id, Record), Failure> {
    let text = text.trim();
    let all = rows(tx, project)?;
    if let Ok(id) = text.parse::<Id>()
        && let Some(found) = all.iter().find(|(i, _)| *i == id)
    {
        return Ok(found.clone());
    }
    let wanted = fold(text);
    if let Some(found) = all.iter().find(|(_, r)| fold(&r.name) == wanted) {
        return Ok(found.clone());
    }
    let names: Vec<&str> = all.iter().map(|(_, r)| r.name.as_str()).collect();
    let args = [("baseline", text.to_owned())];
    let error = messages::with("baseline-not-found", &args, |m| {
        OpError::not_found(m, &names)
    })
    .with_detail("field", field);
    Err(messages::hint(error, "baseline-not-found-hint", &[]).into())
}

fn output(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<Baseline, Failure> {
    let project = r
        .project_id()
        .map(|p| {
            tx.query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |row| {
                row.get::<_, String>(0)
            })
            .optional()
        })
        .transpose()?
        .flatten()
        .unwrap_or_default();
    let tasks: i64 = tx.query_row(
        "SELECT count(*) FROM baseline_tasks WHERE baseline_id = ?1",
        [b(id)],
        |row| row.get(0),
    )?;
    Ok(Baseline {
        id: id.to_string(),
        project,
        name: r.name.clone(),
        tasks: u32::try_from(tasks).unwrap_or(u32::MAX),
        created_by: Actor {
            kind: ActorKind::from_stored(&r.created_by_kind),
            name: r.created_by_name.clone(),
        },
        created_at: rfc3339(r.created_at),
        updated_at: rfc3339(r.updated_at),
    })
}

fn list(tx: &Transaction<'_>, input: &BaselineListInput) -> Result<BaselineList, Failure> {
    let project = lookup::project(tx, "project", &input.project)?;
    let items = rows(tx, project.id)?
        .iter()
        .map(|(id, r)| output(tx, *id, r))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BaselineList {
        total: items.len() as u64,
        items,
    })
}

// ---------------------------------------------------------------------
// Changing

fn check_name(name: &str) -> Result<String, Failure> {
    gk_core::check_line("name", name, NAME_MAX).map_err(|e| super::task::invalid("name", e))
}

fn check_unique(tx: &Transaction<'_>, project: Id, id: Id, name: &str) -> Result<(), Failure> {
    let wanted = fold(name);
    if rows(tx, project)?
        .iter()
        .any(|(other, r)| *other != id && fold(&r.name) == wanted)
    {
        let error = messages::conflict("baseline-name-taken", &[("baseline", name.to_owned())])
            .with_detail("field", "name");
        return Err(messages::hint(error, "baseline-name-taken-hint", &[]).into());
    }
    Ok(())
}

/// The project's dated tasks now, as a baseline keeps them.
fn snapshot(tx: &Transaction<'_>, project: Id, key: &str) -> Result<Vec<Entry>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT t.id, t.number, t.title, t.start_date, t.due_date, y.glyph = 'milestone'
           FROM tasks t JOIN task_types y ON y.id = t.type_id
          WHERE t.project_id = ?1 AND t.deleted_at IS NULL
            AND (t.start_date IS NOT NULL OR t.due_date IS NOT NULL)
          ORDER BY t.number",
    )?;
    let rows = stmt
        .query_map([b(project)], |r| {
            Ok(Entry {
                task: id_at(r, 0)?.to_string(),
                key: format!("{key}-{}", r.get::<_, i64>(1)?),
                title: r.get(2)?,
                start: r.get(3)?,
                due: r.get(4)?,
                milestone: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn record_value(r: &Record) -> Result<Value, Failure> {
    to_value(r).map_err(Failure::Store)
}

fn create(w: &mut WriteCtx<'_>, input: BaselineCreateInput) -> Result<BaselineChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let name = check_name(&input.name)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    check_unique(w.tx(), project.id, id, &name)?;
    let caller = w.caller().actor.clone();
    let record = Record {
        project: project.id.to_string(),
        name,
        created_at: w.now(),
        created_by_kind: caller.kind.as_str().to_owned(),
        created_by_name: caller.name,
        updated_at: w.now(),
    };
    let tasks = snapshot(w.tx(), project.id, &project.key)?;
    let after = record_value(&record)?;
    write_raw(w, id, Some(&after))?;
    w.record_field(ENTITY, id, Some(project.id), RECORD, None, Some(after));
    let list = to_value(&tasks).map_err(Failure::Store)?;
    write_raw_tasks(w, id, Some(&list))?;
    w.record_field(ENTITY, id, Some(project.id), TASKS, None, Some(list));
    Ok(BaselineChange {
        baseline: output(w.tx(), id, &record)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn rename(w: &mut WriteCtx<'_>, input: BaselineRenameInput) -> Result<BaselineChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let (id, old) = find(w.tx(), project.id, "baseline", &input.baseline)?;
    let name = check_name(&input.name)?;
    check_unique(w.tx(), project.id, id, &name)?;
    let mut r = old.clone();
    r.name = name;
    if r != old {
        r.updated_at = w.now();
        let before = record_value(&old)?;
        let after = record_value(&r)?;
        write_raw(w, id, Some(&after))?;
        w.record_field(
            ENTITY,
            id,
            Some(project.id),
            RECORD,
            Some(before),
            Some(after),
        );
    }
    Ok(BaselineChange {
        baseline: output(w.tx(), id, &r)?,
        deleted: false,
        group: w.group_if_changed(),
    })
}

fn delete(w: &mut WriteCtx<'_>, input: BaselineRef) -> Result<BaselineChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    let (id, old) = find(w.tx(), project.id, "baseline", &input.baseline)?;
    let shown = output(w.tx(), id, &old)?;
    let tasks = raw_tasks(w.tx(), id)?;
    let before = raw(w.tx(), id)?;
    write_raw(w, id, None)?;
    w.record_field(ENTITY, id, Some(project.id), TASKS, tasks, None);
    w.record_field(ENTITY, id, Some(project.id), RECORD, before, None);
    Ok(BaselineChange {
        baseline: shown,
        deleted: true,
        group: w.group_if_changed(),
    })
}

// ---------------------------------------------------------------------
// For the schedule

/// The tasks of baseline `id` by task.
pub(crate) fn by_task(tx: &Transaction<'_>, id: Id) -> Result<HashMap<Id, Entry>, Failure> {
    Ok(entries(tx, id)?
        .into_iter()
        .filter_map(|e| Some((e.task.parse().ok()?, e)))
        .collect())
}
