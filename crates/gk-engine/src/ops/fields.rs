// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The recorded fields of projects, statuses, types, labels, tasks and wiki
//! pages: how each maps to its column, how its value reads in history, and
//! how it is written. Handlers change fields through [`set`], which records the
//! change; revert writes the recorded value back through the same path.
//! Descriptions and wiki pages' text are not listed: they change through
//! their revisions (`description.rs`).

use gk_core::{Id, fold};
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::failure::Failure;
use crate::messages;
use crate::write::WriteCtx;

/// History's name for projects.
pub(crate) const PROJECT: &str = "project";
/// History's name for statuses.
pub(crate) const STATUS: &str = "status";
/// History's name for task types.
pub(crate) const TYPE: &str = "task_type";
/// History's name for labels.
pub(crate) const LABEL: &str = "label";
/// History's name for tasks.
pub(crate) const TASK: &str = "task";
/// History's name for wiki pages.
pub(crate) const WIKI: &str = "wiki_page";

/// How a column's value reads in history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Enc {
    /// Text.
    Text,
    /// A whole number.
    Int,
    /// 0 or 1, read as `false` or `true`.
    Bool,
    /// An ID, read as its text.
    Id,
    /// JSON text, read as the JSON value.
    Json,
    /// A priority 0–4, read by its name.
    Priority,
}

/// One recorded field.
#[derive(Debug, Clone, Copy)]
struct Spec {
    entity: &'static str,
    field: &'static str,
    table: &'static str,
    column: &'static str,
    enc: Enc,
    /// The matching-key column kept next to it.
    fold: Option<&'static str>,
}

const fn spec(
    entity: &'static str,
    field: &'static str,
    table: &'static str,
    column: &'static str,
    enc: Enc,
) -> Spec {
    Spec {
        entity,
        field,
        table,
        column,
        enc,
        fold: None,
    }
}

const fn folded(entity: &'static str, field: &'static str, table: &'static str) -> Spec {
    Spec {
        entity,
        field,
        table,
        column: field,
        enc: Enc::Text,
        fold: Some(if matches!(field.as_bytes(), b"title") {
            "title_fold"
        } else {
            "name_fold"
        }),
    }
}

const SPECS: &[Spec] = &[
    folded(PROJECT, "name", "projects"),
    spec(PROJECT, "hue", "projects", "hue", Enc::Text),
    spec(PROJECT, "icon", "projects", "icon", Enc::Text),
    spec(PROJECT, "modules", "projects", "modules", Enc::Json),
    spec(PROJECT, "key", "projects", "key", Enc::Text),
    spec(PROJECT, "parent", "projects", "parent_id", Enc::Id),
    spec(PROJECT, "rank", "projects", "rank", Enc::Text),
    spec(PROJECT, "state", "projects", "state", Enc::Text),
    folded(STATUS, "name", "statuses"),
    spec(STATUS, "category", "statuses", "category", Enc::Text),
    spec(STATUS, "hue", "statuses", "hue", Enc::Text),
    spec(STATUS, "wip_limit", "statuses", "wip_limit", Enc::Int),
    spec(STATUS, "rank", "statuses", "rank", Enc::Text),
    folded(TYPE, "name", "task_types"),
    spec(TYPE, "glyph", "task_types", "glyph", Enc::Text),
    spec(TYPE, "hue", "task_types", "hue", Enc::Text),
    spec(TYPE, "rank", "task_types", "rank", Enc::Text),
    spec(TYPE, "is_default", "task_types", "is_default", Enc::Bool),
    folded(LABEL, "name", "labels"),
    spec(LABEL, "hue", "labels", "hue", Enc::Text),
    spec(LABEL, "rank", "labels", "rank", Enc::Text),
    folded(TASK, "title", "tasks"),
    spec(TASK, "status", "tasks", "status_id", Enc::Id),
    spec(TASK, "type", "tasks", "type_id", Enc::Id),
    spec(TASK, "priority", "tasks", "priority", Enc::Priority),
    spec(TASK, "parent", "tasks", "parent_id", Enc::Id),
    spec(TASK, "rank", "tasks", "rank", Enc::Text),
    spec(TASK, "subtask_rank", "tasks", "subtask_rank", Enc::Text),
    spec(TASK, "start_date", "tasks", "start_date", Enc::Text),
    spec(TASK, "due_date", "tasks", "due_date", Enc::Text),
    spec(TASK, "estimate", "tasks", "estimate_min", Enc::Int),
    spec(TASK, "progress", "tasks", "progress_override", Enc::Int),
    spec(TASK, "closed_at", "tasks", "closed_at", Enc::Int),
    spec(TASK, "project", "tasks", "project_id", Enc::Id),
    spec(TASK, "number", "tasks", "number", Enc::Int),
    folded(WIKI, "title", "wiki_pages"),
    spec(WIKI, "slug", "wiki_pages", "slug", Enc::Text),
    spec(WIKI, "parent", "wiki_pages", "parent_id", Enc::Id),
    spec(WIKI, "rank", "wiki_pages", "rank", Enc::Text),
    spec(
        WIKI,
        "agent_brief",
        "wiki_pages",
        "is_agent_brief",
        Enc::Bool,
    ),
];

const PRIORITIES: [&str; 5] = ["none", "low", "normal", "high", "urgent"];

/// The name of priority `n` (0–4) as history and outputs show it.
pub(crate) fn priority_name(n: i64) -> &'static str {
    usize::try_from(n)
        .ok()
        .and_then(|i| PRIORITIES.get(i))
        .copied()
        .unwrap_or("none")
}

fn find(entity: &str, field: &str) -> Option<&'static Spec> {
    SPECS
        .iter()
        .find(|s| s.entity == entity && s.field == field)
}

/// Whether `entity`/`field` is a plain column field.
pub(crate) fn is_column(entity: &str, field: &str) -> bool {
    find(entity, field).is_some()
}

fn to_json(enc: Enc, v: Sql) -> Option<Value> {
    match (enc, v) {
        (_, Sql::Null) => None,
        (Enc::Id, Sql::Blob(b)) => Id::from_slice(&b).ok().map(|id| id.to_string().into()),
        (Enc::Bool, Sql::Integer(i)) => Some(Value::Bool(i != 0)),
        (Enc::Priority, Sql::Integer(i)) => Some(priority_name(i).into()),
        (Enc::Json, Sql::Text(t)) => serde_json::from_str(&t).ok(),
        (_, Sql::Integer(i)) => Some(i.into()),
        (_, Sql::Text(t)) => Some(t.into()),
        (_, Sql::Real(r)) => serde_json::Number::from_f64(r).map(Value::Number),
        (_, Sql::Blob(_)) => None,
    }
}

fn to_sql(enc: Enc, v: Option<&Value>) -> Result<Sql, Failure> {
    let bad = || Failure::Op(messages::storage("storage-value-type", &[]));
    Ok(match (enc, v) {
        (_, None | Some(Value::Null)) => Sql::Null,
        (Enc::Id, Some(Value::String(s))) => {
            Sql::Blob(s.parse::<Id>().map_err(|_| bad())?.as_bytes().to_vec())
        }
        (Enc::Bool, Some(Value::Bool(b))) => Sql::Integer(i64::from(*b)),
        (Enc::Priority, Some(Value::String(s))) => Sql::Integer(
            PRIORITIES
                .iter()
                .position(|p| p == s)
                .and_then(|i| i64::try_from(i).ok())
                .ok_or_else(bad)?,
        ),
        (Enc::Json, Some(v)) => Sql::Text(v.to_string()),
        (Enc::Int, Some(Value::Number(n))) => Sql::Integer(n.as_i64().ok_or_else(bad)?),
        (Enc::Text, Some(Value::String(s))) => Sql::Text(s.clone()),
        _ => return Err(bad()),
    })
}

/// The current value of `entity`/`field` of item `id`; `None` when it has
/// none (or the item doesn't exist).
pub(crate) fn read(
    tx: &Transaction<'_>,
    entity: &str,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let Some(s) = find(entity, field) else {
        return Ok(None);
    };
    let v: Option<Sql> = tx
        .query_row(
            &format!("SELECT {} FROM {} WHERE id = ?1", s.column, s.table),
            [id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.and_then(|v| to_json(s.enc, v)))
}

/// Writes `value` to `entity`/`field` of item `id` without recording it,
/// keeping its matching key and the item's change times in step.
fn write_raw(w: &WriteCtx<'_>, s: &Spec, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    let sql = to_sql(s.enc, value)?;
    let fold_value = match (&s.fold, value) {
        (Some(_), Some(Value::String(text))) => Some(fold(text)),
        _ => None,
    };
    let tx = w.tx();
    match (s.fold, fold_value) {
        (Some(fold_column), Some(f)) => tx.execute(
            &format!(
                "UPDATE {} SET {} = ?2, {fold_column} = ?3 WHERE id = ?1",
                s.table, s.column
            ),
            params![id.as_bytes().as_slice(), sql, f],
        )?,
        _ => tx.execute(
            &format!("UPDATE {} SET {} = ?2 WHERE id = ?1", s.table, s.column),
            params![id.as_bytes().as_slice(), sql],
        )?,
    };
    touch(w, s.entity, id, s.field)
}

/// Marks item `id` as changed now: projects and wiki pages get a new
/// `updated_at`; tasks a new version and, for their own fields, a new
/// `edited_at`.
pub(crate) fn touch(w: &WriteCtx<'_>, entity: &str, id: Id, field: &str) -> Result<(), Failure> {
    let id = id.as_bytes().as_slice();
    match entity {
        PROJECT => {
            w.tx().execute(
                "UPDATE projects SET updated_at = ?2 WHERE id = ?1",
                params![id, w.now()],
            )?;
        }
        WIKI => {
            w.tx().execute(
                "UPDATE wiki_pages SET updated_at = ?2 WHERE id = ?1",
                params![id, w.now()],
            )?;
        }
        TASK => {
            let entered = if field == "status" {
                ", status_entered_at = ?2"
            } else {
                ""
            };
            w.tx().execute(
                &format!(
                    "UPDATE tasks SET version = version + 1, edited_at = ?2, updated_at = ?2{entered}
                      WHERE id = ?1"
                ),
                params![id, w.now()],
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// Sets `entity`/`field` of item `id` (in `project`) to `value` and records
/// the change; nothing happens when the value is already that. Returns
/// whether it changed.
pub(crate) fn set(
    w: &mut WriteCtx<'_>,
    entity: &str,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<bool, Failure> {
    let s = find(entity, field).ok_or_else(|| {
        Failure::Op(messages::storage(
            "storage-unknown-field",
            &[("entity", entity.to_owned()), ("field", field.to_owned())],
        ))
    })?;
    let before = read(w.tx(), entity, id, field)?;
    if before == value {
        return Ok(false);
    }
    write_raw(w, s, id, value.as_ref())?;
    w.record_field(entity, id, project, field, before, value);
    Ok(true)
}

/// `text` as a JSON string value.
pub(crate) fn text(text: impl Into<String>) -> Option<Value> {
    Some(Value::String(text.into()))
}

/// `id` as a JSON value.
pub(crate) fn id_value(id: Id) -> Option<Value> {
    Some(Value::String(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_is_listed_once() {
        for (i, a) in SPECS.iter().enumerate() {
            for b in &SPECS[i + 1..] {
                assert!(
                    !(a.entity == b.entity && a.field == b.field),
                    "{}.{}",
                    a.entity,
                    a.field
                );
            }
        }
        assert_eq!(find(TASK, "title").and_then(|s| s.fold), Some("title_fold"));
        assert_eq!(find(STATUS, "name").and_then(|s| s.fold), Some("name_fold"));
    }

    #[test]
    fn priorities_read_by_name() {
        assert_eq!(priority_name(3), "high");
        assert_eq!(priority_name(9), "none");
        assert_eq!(
            to_sql(Enc::Priority, Some(&Value::from("urgent"))).ok(),
            Some(Sql::Integer(4))
        );
        assert_eq!(
            to_json(Enc::Priority, Sql::Integer(0)),
            Some(Value::from("none"))
        );
    }
}
