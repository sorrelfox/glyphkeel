// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The workspace's kinds of work (Development, Design, Meeting…), which
//! time entries and the timer may name.
//!
//! A kind is kept whole in history (one `record` field), like a version, so
//! creating, renaming, archiving and deleting one can all be undone. An
//! entry's kind is the entry's `work_kind` field. A kind that live entries
//! or the timer use can't be deleted; it is archived instead, which keeps
//! it on those entries and out of the pickers. Deleting one takes it off
//! entries in the trash, in the same change.

use gk_api::OpError;
use gk_api::ops::time::{
    WorkKind, WorkKindChange, WorkKindCreate, WorkKindCreateInput, WorkKindDelete,
    WorkKindDeleteInput, WorkKindList, WorkKindListInput, WorkKindListOp, WorkKindRef,
    WorkKindUpdate, WorkKindUpdateInput,
};
use gk_core::{Id, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::desc;
use super::lookup::{b, id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

/// History's name for kinds of work.
pub(crate) const ENTITY: &str = "work_kind";
/// The one recorded field: the whole kind.
pub(crate) const RECORD: &str = "record";
/// The longest name, in characters.
const NAME_MAX: usize = 60;

pub(super) fn register(r: &mut Registry) {
    r.query::<WorkKindListOp, _>(desc::<WorkKindListOp>(), |q, input: WorkKindListInput| {
        list(q.tx, input.include_archived)
    });
    r.command::<WorkKindCreate, _>(desc::<WorkKindCreate>(), create);
    r.command::<WorkKindUpdate, _>(desc::<WorkKindUpdate>(), update);
    r.command::<WorkKindDelete, _>(desc::<WorkKindDelete>(), delete);
}

/// A kind as history records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) archived_at: Option<i64>,
    pub(crate) created_at: i64,
    pub(crate) created_by_kind: String,
    pub(crate) created_by_name: String,
    pub(crate) updated_at: i64,
}

const COLUMNS: &str =
    "id, name, archived_at, created_at, created_by_kind, created_by_name, updated_at";

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<(Id, Record)> {
    Ok((
        id_at(r, 0)?,
        Record {
            name: r.get(1)?,
            archived_at: r.get(2)?,
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
            &format!("SELECT {COLUMNS} FROM work_kinds WHERE id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?
        .map(|(_, r)| r))
}

/// The stored kind `id` as history records it, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    load(tx, id)?
        .map(|r| serde_json::to_value(r).map_err(|e| StoreError::Corrupt(e.to_string())))
        .transpose()
}

/// Live entries and the timer that name kind `id`.
fn uses(tx: &Transaction<'_>, id: Id) -> Result<i64, StoreError> {
    Ok(tx.query_row(
        "SELECT (SELECT count(*) FROM time_entries WHERE work_kind_id = ?1 AND deleted_at IS NULL)
              + (SELECT count(*) FROM timer WHERE work_kind_id = ?1)",
        [b(id)],
        |r| r.get(0),
    )?)
}

/// Writes kind `id` as history recorded it (`None` deletes it), without
/// recording. A kind that live entries or the timer name is not deleted.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let Some(value) = value else {
        let used = uses(w.tx(), id)?;
        if used > 0 {
            let name = load(w.tx(), id)?.map(|r| r.name).unwrap_or_default();
            let error = messages::conflict(
                "work-kind-in-use",
                &[("kind", name), ("count", used.to_string())],
            );
            return Err(messages::hint(error, "work-kind-in-use-hint", &[]).into());
        }
        w.tx()
            .execute("DELETE FROM work_kinds WHERE id = ?1", [b(id)])?;
        return Ok(());
    };
    let r: Record = serde_json::from_value(value.clone())
        .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    w.tx()
        .execute(
            &format!(
            "INSERT INTO work_kinds ({COLUMNS}, name_fold) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (id) DO UPDATE SET
               name = excluded.name, name_fold = excluded.name_fold,
               archived_at = excluded.archived_at, created_at = excluded.created_at,
               created_by_kind = excluded.created_by_kind,
               created_by_name = excluded.created_by_name, updated_at = excluded.updated_at"
        ),
            params![
                b(id),
                r.name,
                r.archived_at,
                r.created_at,
                r.created_by_kind,
                r.created_by_name,
                r.updated_at,
                fold(&r.name),
            ],
        )
        .map_err(|e| match e {
            gk_store::rusqlite::Error::SqliteFailure(f, _)
                if f.code == gk_store::rusqlite::ErrorCode::ConstraintViolation =>
            {
                taken(&r.name)
            }
            other => Failure::Store(other.into()),
        })?;
    Ok(())
}

/// Writes kind `id` and records the change.
fn set(w: &mut WriteCtx<'_>, id: Id, record: Option<&Record>) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    let after = record
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?;
    if before == after {
        return Ok(());
    }
    write_raw(w, id, after.as_ref())?;
    w.record_field(ENTITY, id, None, RECORD, before, after);
    Ok(())
}

fn taken(name: &str) -> Failure {
    let error = messages::conflict("work-kind-name-taken", &[("kind", name.to_owned())])
        .with_detail("field", "name");
    messages::hint(error, "work-kind-name-taken-hint", &[]).into()
}

fn clean_name(name: &str) -> Result<String, Failure> {
    gk_core::check_line("name", name, NAME_MAX).map_err(|e| super::task::invalid("name", e))
}

/// Refuses a name another kind has.
fn check_unique(tx: &Transaction<'_>, id: Id, name: &str) -> Result<(), Failure> {
    let other: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM work_kinds WHERE name_fold = ?1 AND id <> ?2",
            params![fold(name), b(id)],
            |r| r.get(0),
        )
        .optional()?;
    if other.is_some() {
        return Err(taken(name));
    }
    Ok(())
}

/// The kind `text` names: its ID, or its name in any case.
pub(crate) fn find(tx: &Transaction<'_>, field: &str, text: &str) -> Result<(Id, Record), Failure> {
    let trimmed = text.trim();
    let by_id = trimmed.parse::<Id>().ok();
    let found = tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM work_kinds WHERE id = ?1 OR name_fold = ?2"),
            params![by_id.map(b), fold(trimmed)],
            read_row,
        )
        .optional()?;
    found.ok_or_else(|| {
        let names = names(tx).unwrap_or_default();
        let error = messages::not_found("work-kind-not-found", &[("kind", trimmed.to_owned())])
            .with_detail("field", field);
        if names.is_empty() {
            messages::hint(error, "work-kind-not-found-none", &[]).into()
        } else {
            messages::hint(
                error,
                "work-kind-not-found-hint",
                &[("choices", names.join(", "))],
            )
            .into()
        }
    })
}

/// The names of the kinds not archived, in name order.
fn names(tx: &Transaction<'_>) -> Result<Vec<String>, StoreError> {
    let mut stmt = tx.prepare_cached(
        "SELECT name FROM work_kinds WHERE archived_at IS NULL ORDER BY name_fold, id",
    )?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The kind a time input names for new time: `None` for an empty text.
/// Archived kinds are not offered for new time.
pub(crate) fn for_entry(
    tx: &Transaction<'_>,
    field: &str,
    text: &str,
    current: Option<Id>,
) -> Result<Option<Id>, Failure> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let (id, record) = find(tx, field, text)?;
    if record.archived_at.is_some() && current != Some(id) {
        let error = messages::conflict("work-kind-archived", &[("kind", record.name)])
            .with_detail("field", field);
        return Err(messages::hint(error, "work-kind-archived-hint", &[]).into());
    }
    Ok(Some(id))
}

/// The kind `id` as entries name it, if it exists.
pub(crate) fn reference(
    tx: &Transaction<'_>,
    id: Option<Id>,
) -> Result<Option<WorkKindRef>, StoreError> {
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(load(tx, id)?.map(|r| WorkKindRef {
        id: id.to_string(),
        name: r.name,
    }))
}

fn output(tx: &Transaction<'_>, id: Id, r: &Record) -> Result<WorkKind, StoreError> {
    let entries: i64 = tx.query_row(
        "SELECT count(*) FROM time_entries WHERE work_kind_id = ?1 AND deleted_at IS NULL",
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(WorkKind {
        id: id.to_string(),
        name: r.name.clone(),
        archived: r.archived_at.is_some(),
        entries: entries.unsigned_abs(),
    })
}

fn list(tx: &Transaction<'_>, include_archived: bool) -> Result<WorkKindList, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM work_kinds WHERE ?1 OR archived_at IS NULL
          ORDER BY name_fold, id"
    ))?;
    let rows = stmt
        .query_map([include_archived], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut items = Vec::with_capacity(rows.len());
    for (id, r) in rows {
        items.push(output(tx, id, &r)?);
    }
    Ok(WorkKindList { items })
}

fn changed(w: &WriteCtx<'_>, id: Id, r: &Record, deleted: bool) -> Result<WorkKindChange, Failure> {
    Ok(WorkKindChange {
        kind: output(w.tx(), id, r)?,
        deleted,
        group: w.group_if_changed(),
    })
}

fn create(w: &mut WriteCtx<'_>, input: WorkKindCreateInput) -> Result<WorkKindChange, Failure> {
    let name = clean_name(&input.name)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    check_unique(w.tx(), id, &name)?;
    let caller = w.caller().clone();
    let record = Record {
        name,
        archived_at: None,
        created_at: w.now(),
        created_by_kind: caller.actor.kind.as_str().to_owned(),
        created_by_name: caller.actor.name,
        updated_at: w.now(),
    };
    set(w, id, Some(&record))?;
    changed(w, id, &record, false)
}

fn update(w: &mut WriteCtx<'_>, input: WorkKindUpdateInput) -> Result<WorkKindChange, Failure> {
    let (id, old) = find(w.tx(), "kind", &input.kind)?;
    let mut record = old.clone();
    if let Some(name) = &input.name {
        record.name = clean_name(name)?;
        check_unique(w.tx(), id, &record.name)?;
    }
    if let Some(archived) = input.archived {
        record.archived_at = match (archived, old.archived_at) {
            (true, Some(at)) => Some(at),
            (true, None) => Some(w.now()),
            (false, _) => None,
        };
    }
    if record != old {
        record.updated_at = w.now();
        set(w, id, Some(&record))?;
    }
    changed(w, id, &record, false)
}

fn delete(w: &mut WriteCtx<'_>, input: WorkKindDeleteInput) -> Result<WorkKindChange, Failure> {
    let (id, old) = find(w.tx(), "kind", &input.kind)?;
    let used = uses(w.tx(), id)?;
    if used > 0 {
        let error = messages::conflict(
            "work-kind-in-use",
            &[("kind", old.name.clone()), ("count", used.to_string())],
        );
        return Err(messages::hint(error, "work-kind-in-use-hint", &[]).into());
    }
    let out = output(w.tx(), id, &old)?;
    // Entries in the trash lose it, recorded, so undo gives it back.
    let trashed: Vec<(Id, Option<Id>)> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT e.id, t.project_id FROM time_entries e JOIN tasks t ON t.id = e.task_id
              WHERE e.work_kind_id = ?1",
        )?;
        stmt.query_map([b(id)], |r| Ok((id_at(r, 0)?, Some(id_at(r, 1)?))))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (entry, project) in trashed {
        super::time::apply_field(w, entry, project, super::time::WORK_KIND_FIELD, None)?;
    }
    set(w, id, None)?;
    Ok(WorkKindChange {
        kind: out,
        deleted: true,
        group: w.group_if_changed(),
    })
}
