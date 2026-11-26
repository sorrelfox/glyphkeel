// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields in archives.
//!
//! `project.json` lists the project's own fields and the fields of every
//! project that its tasks have values of (marked `shared`); each task in
//! `tasks.jsonl` lists its stored values. On import, a project's own fields
//! come with it; a shared field becomes this workspace's field of every
//! project of the same query name and kind when there is one, and a new
//! such field otherwise (or the project's own, when another kind has the
//! name here).

use std::collections::{HashMap, HashSet};

use gk_core::custom_field::query_name;
use gk_core::{FieldKind, Id, Rank, fold};
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::Rejected;
use super::model::{CustomField, CustomValue, Task};
use crate::failure::Failure;

fn text_id(bytes: &[u8]) -> String {
    Id::from_slice(bytes).map_or_else(|_| String::new(), |id| id.to_string())
}

/// The fields of project `project` for its archive: its own, then those of
/// every project that the tasks `tasks` have values of.
pub(crate) fn export_fields(
    tx: &Transaction<'_>,
    project: &[u8],
    tasks: &[Vec<u8>],
) -> Result<Vec<CustomField>, Failure> {
    let read = |r: &gk_store::rusqlite::Row<'_>| -> gk_store::rusqlite::Result<CustomField> {
        let list = |t: String| serde_json::from_str::<Vec<String>>(&t).unwrap_or_default();
        Ok(CustomField {
            id: text_id(&r.get::<_, Vec<u8>>(0)?),
            name: r.get(1)?,
            kind: r.get(2)?,
            options: list(r.get(3)?),
            default: r
                .get::<_, Option<String>>(4)?
                .and_then(|t| serde_json::from_str(&t).ok()),
            types: list(r.get(5)?),
            required: r.get(6)?,
            rank: r.get(7)?,
            shared: r.get::<_, Option<Vec<u8>>>(8)?.is_none(),
            created_at: r.get(9)?,
        })
    };
    const COLUMNS: &str = "id, name, kind, options, default_value, type_names, required, rank, \
                           project_id, created_at";
    let mut out: Vec<CustomField> = {
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM custom_fields WHERE project_id = ?1 ORDER BY rank, id"
        ))?;
        stmt.query_map([project], read)?.collect::<Result<_, _>>()?
    };
    let mut shared: HashSet<Vec<u8>> = HashSet::new();
    {
        let mut stmt = tx.prepare_cached(
            "SELECT DISTINCT v.field_id FROM custom_values v JOIN custom_fields f
                 ON f.id = v.field_id
              WHERE v.task_id = ?1 AND f.project_id IS NULL",
        )?;
        for t in tasks {
            for id in stmt.query_map([t], |r| r.get::<_, Vec<u8>>(0))? {
                shared.insert(id?);
            }
        }
    }
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM custom_fields WHERE project_id IS NULL ORDER BY rank, id"
    ))?;
    let all: Vec<(Vec<u8>, CustomField)> = stmt
        .query_map([], |r| Ok((r.get::<_, Vec<u8>>(0)?, read(r)?)))?
        .collect::<Result<_, _>>()?;
    out.extend(
        all.into_iter()
            .filter(|(id, _)| shared.contains(id))
            .map(|(_, f)| f),
    );
    Ok(out)
}

/// The stored values of task `task`.
pub(crate) fn export_values(
    tx: &Transaction<'_>,
    task: &[u8],
) -> Result<Vec<CustomValue>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT field_id, value FROM custom_values WHERE task_id = ?1 ORDER BY field_id, value",
    )?;
    let rows = stmt
        .query_map([task], |r| {
            let value = match r.get::<_, Sql>(1)? {
                Sql::Text(t) => Value::String(t),
                Sql::Integer(n) => Value::from(n),
                Sql::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
                Sql::Null | Sql::Blob(_) => Value::Null,
            };
            Ok(CustomValue {
                field_id: text_id(&r.get::<_, Vec<u8>>(0)?),
                value,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Checks the fields of one project file and its tasks' values: known
/// kinds, names a query can use, no field twice, values of known fields.
pub(crate) fn check(
    pj: &str,
    tp: &str,
    fields: &[CustomField],
    tasks: &[Task],
) -> Result<(), Rejected> {
    let bad = |path: &str, line: usize, id: &'static str, value: &str| {
        Rejected::Invalid(path.to_owned(), line, id, vec![("value", value.to_owned())])
    };
    let mut known: HashMap<&str, FieldKind> = HashMap::new();
    let mut names: HashSet<String> = HashSet::new();
    for f in fields {
        let kind: FieldKind = f
            .kind
            .parse()
            .map_err(|_| bad(pj, 1, "transfer-invalid-field", &f.kind))?;
        if f.id.parse::<Id>().is_err()
            || query_name(&f.name).is_err()
            || !names.insert(fold(&f.name))
            || known.insert(f.id.as_str(), kind).is_some()
        {
            return Err(bad(pj, 1, "transfer-invalid-field", &f.name));
        }
        Rank::parse(&f.rank).map_err(|_| bad(pj, 1, "transfer-invalid-rank", &f.rank))?;
    }
    for (i, t) in tasks.iter().enumerate() {
        for v in &t.custom_values {
            let ok = known.contains_key(v.field_id.as_str())
                && matches!(v.value, Value::String(_) | Value::Number(_));
            if !ok {
                return Err(bad(tp, i + 1, "transfer-invalid-field-value", &v.field_id));
            }
        }
    }
    Ok(())
}

/// Plans the fields' IDs: a project's own field gets a new ID when its ID
/// is taken here; a shared one takes the ID of this workspace's field of
/// every project with its query name and kind, if there is one.
pub(crate) fn plan<'a>(
    tx: &Transaction<'_>,
    fields: impl Iterator<Item = &'a CustomField>,
    ids: &mut HashMap<Id, Id>,
) -> Result<(), Failure> {
    for f in fields {
        let Ok(id) = f.id.parse::<Id>() else { continue };
        if ids.contains_key(&id) {
            continue;
        }
        if f.shared
            && let Ok(q) = query_name(&f.name)
        {
            let existing: Option<Vec<u8>> = tx
                .query_row(
                    "SELECT id FROM custom_fields WHERE project_id IS NULL AND query_name = ?1
                        AND kind = ?2",
                    params![q, f.kind],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(existing) = existing.and_then(|e| Id::from_slice(&e).ok()) {
                ids.insert(id, existing);
                continue;
            }
        }
        let taken: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM custom_fields WHERE id = ?1",
                [id.as_bytes().as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        if taken.is_some() {
            ids.insert(id, Id::generate());
        }
    }
    Ok(())
}

/// Writes the fields of one imported project (new ID `project`). Shared
/// fields are written once, as fields of every project, unless the
/// workspace has one already; when the name is taken here by another
/// kind, the field becomes the project's own.
pub(crate) fn write_fields(
    tx: &Transaction<'_>,
    fields: &[CustomField],
    project: Id,
    id_of: &dyn Fn(&str) -> Id,
    now: i64,
) -> Result<(), Failure> {
    for f in fields {
        let id = id_of(&f.id);
        let exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM custom_fields WHERE id = ?1",
                [id.as_bytes().as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_some() {
            continue;
        }
        let Ok(q) = query_name(&f.name) else { continue };
        let mut owner = (!f.shared).then_some(project);
        if owner.is_none() {
            let clash: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM custom_fields WHERE project_id IS NULL
                        AND (name_fold = ?1 OR query_name = ?2)",
                    params![fold(&f.name), q],
                    |r| r.get(0),
                )
                .optional()?;
            if clash.is_some() {
                owner = Some(project);
            }
        }
        let list = |items: &[String]| Value::from(items.to_vec()).to_string();
        tx.execute(
            "INSERT OR IGNORE INTO custom_fields (id, project_id, name, name_fold, query_name,
                 kind, options, default_value, type_names, required, rank, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id.as_bytes().as_slice(),
                owner.map(|p| p.as_bytes().to_vec()),
                f.name,
                fold(&f.name),
                q,
                f.kind,
                list(&f.options),
                f.default.as_ref().map(Value::to_string),
                list(&f.types),
                f.required,
                f.rank,
                if f.created_at > 0 { f.created_at } else { now },
            ],
        )?;
    }
    Ok(())
}

/// Writes the values of one imported task (new ID `task`).
pub(crate) fn write_values(
    tx: &Transaction<'_>,
    task: Id,
    values: &[CustomValue],
    id_of: &dyn Fn(&str) -> Id,
) -> Result<(), Failure> {
    for v in values {
        let value = match &v.value {
            Value::String(s) => Sql::Text(s.clone()),
            Value::Number(n) => match n.as_i64() {
                Some(i) => Sql::Integer(i),
                None => Sql::Real(n.as_f64().unwrap_or_default()),
            },
            _ => continue,
        };
        tx.prepare_cached(
            "INSERT OR IGNORE INTO custom_values (task_id, field_id, value)
             SELECT ?1, id, ?3 FROM custom_fields WHERE id = ?2",
        )?
        .execute(params![
            task.as_bytes().as_slice(),
            id_of(&v.field_id).as_bytes().as_slice(),
            value
        ])?;
    }
    Ok(())
}
