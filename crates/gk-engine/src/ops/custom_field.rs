// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields and their values on tasks.
//!
//! Rules: a field belongs to one project or to every project; its name is
//! unique among the fields a project's tasks have (its own and those of
//! every project), ignoring case, and so is its query name. Fields of the
//! same query name in different projects hold the same kind, so a query on
//! `cf.<name>` means one thing across projects. A new sub-project gets
//! copies of its parent's own fields. A field shown only for some task
//! types takes values only on tasks of those types; a required field needs
//! a value on new tasks (its default counts) and can't be cleared.
//!
//! History: a field is the entity `custom_field` with the settings `name`,
//! `options`, `default`, `types`, `required` and `rank`; a task's value is
//! the task field `cf:<field ID>`, as the API shows the value. Both are
//! written back by revert through this module.

use std::collections::BTreeMap;

use gk_api::ops::common::Edge;
use gk_api::ops::field::{
    CustomField, CustomFieldChange, CustomFieldCreateInput, CustomFieldDeleteInput,
    CustomFieldKind as ApiKind, CustomFieldList, CustomFieldListInput, CustomFieldSetting,
    CustomFieldUpdateInput, CustomFieldValue, FieldCreate, FieldDelete, FieldListOp, FieldUpdate,
};
use gk_core::custom_field::{self as core, FIELD_NAME_MAX_CHARS, OPTIONS_MAX};
use gk_core::{FieldKind, Id, Rank, Stored, fold};
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK, text};
use super::lookup::{self, List, ProjectRow, Target, b, id_at, opt_id_at};
use super::project::{enum_text, parse_enum};
use super::task::invalid;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::write::WriteCtx;

/// History's name for custom fields.
pub(crate) const ENTITY: &str = "custom_field";
/// The start of a task field that holds a custom field's value:
/// `cf:<field ID>`.
pub(crate) const VALUE_PREFIX: &str = "cf:";

pub(super) fn register(r: &mut Registry) {
    r.query::<FieldListOp, _>(desc::<FieldListOp>(), list);
    r.command::<FieldCreate, _>(desc::<FieldCreate>(), create);
    r.command::<FieldUpdate, _>(desc::<FieldUpdate>(), update);
    r.command::<FieldDelete, _>(desc::<FieldDelete>(), delete);
}

// ---------------------------------------------------------------------
// Rows

/// One stored field.
#[derive(Debug, Clone)]
pub(crate) struct FieldRow {
    pub(crate) id: Id,
    pub(crate) project: Option<Id>,
    pub(crate) name: String,
    pub(crate) query_name: String,
    pub(crate) kind: FieldKind,
    pub(crate) options: Vec<String>,
    pub(crate) default: Option<Value>,
    pub(crate) types: Vec<String>,
    pub(crate) required: bool,
    pub(crate) rank: String,
}

impl FieldRow {
    /// Whether the field is shown for tasks of the type named `type_name`.
    fn shown_for(&self, type_name: &str) -> bool {
        self.types.is_empty() || self.types.iter().any(|t| fold(t) == fold(type_name))
    }
}

const COLUMNS: &str = "f.id, f.project_id, f.name, f.query_name, f.kind, f.options, \
                       f.default_value, f.type_names, f.required, f.rank";

fn json_list(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

fn read_row(r: &gk_store::rusqlite::Row<'_>) -> gk_store::rusqlite::Result<FieldRow> {
    let kind: String = r.get(4)?;
    Ok(FieldRow {
        id: id_at(r, 0)?,
        project: opt_id_at(r, 1)?,
        name: r.get(2)?,
        query_name: r.get(3)?,
        kind: kind.parse().unwrap_or(FieldKind::Text),
        options: json_list(&r.get::<_, String>(5)?),
        default: r
            .get::<_, Option<String>>(6)?
            .and_then(|t| serde_json::from_str(&t).ok()),
        types: json_list(&r.get::<_, String>(7)?),
        required: r.get(8)?,
        rank: r.get(9)?,
    })
}

/// The fields of `project` (those of every project first, then its own),
/// or of the whole workspace, in order.
pub(crate) fn rows(tx: &Transaction<'_>, project: Option<Id>) -> Result<Vec<FieldRow>, Failure> {
    let sql = match project {
        Some(_) => format!(
            "SELECT {COLUMNS} FROM custom_fields f
              WHERE f.project_id IS NULL OR f.project_id = ?1
              ORDER BY f.project_id IS NOT NULL, f.rank, f.id"
        ),
        None => format!(
            "SELECT {COLUMNS} FROM custom_fields f LEFT JOIN projects p ON p.id = f.project_id
              WHERE ?1 IS NULL AND (f.project_id IS NULL OR p.deleted_at IS NULL)
              ORDER BY f.project_id IS NOT NULL, p.key, f.rank, f.id"
        ),
    };
    let mut stmt = tx.prepare_cached(&sql)?;
    let rows = stmt
        .query_map([project.map(b)], read_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn row_by_id(tx: &Transaction<'_>, id: Id) -> Result<Option<FieldRow>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM custom_fields f WHERE f.id = ?1"),
            [b(id)],
            read_row,
        )
        .optional()?)
}

/// The field `text` names among `fields`: its name or query name (any
/// case, with or without `cf.`) or its ID.
fn named<'a>(fields: &'a [FieldRow], text: &str) -> Option<&'a FieldRow> {
    let text = text.trim();
    let id = text.parse::<Id>().ok();
    let bare = gk_query::fields::strip_custom_prefix(text).unwrap_or(text);
    let wanted = fold(bare);
    fields
        .iter()
        .find(|f| Some(f.id) == id || fold(&f.name) == wanted || f.query_name == wanted)
}

fn not_found(fields: &[FieldRow], path: &str, text: &str) -> Failure {
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    messages::with(
        "field-not-found",
        &[("name", text.trim().to_owned())],
        |m| gk_api::OpError::not_found(m, &names),
    )
    .with_detail("field", path)
    .into()
}

/// The project an input names, if any, active.
fn project_of(tx: &Transaction<'_>, text: Option<&str>) -> Result<Option<ProjectRow>, Failure> {
    text.map(|t| {
        let p = lookup::project(tx, "project", t)?;
        lookup::ensure_active(&p)?;
        Ok(p)
    })
    .transpose()
}

/// The field an update or deletion names: among the project's fields, or
/// every field of the workspace.
fn find(
    w: &WriteCtx<'_>,
    project: Option<&str>,
    field: &str,
) -> Result<(Option<ProjectRow>, FieldRow), Failure> {
    let p = project_of(w.tx(), project)?;
    let all = rows(w.tx(), p.as_ref().map(|p| p.id))?;
    let row = named(&all, field)
        .cloned()
        .ok_or_else(|| not_found(&all, "field", field))?;
    if let Some(owner) = row.project
        && p.as_ref().is_none_or(|p| p.id != owner)
    {
        let owner = lookup::project_by_id(w.tx(), owner)?;
        if let Some(owner) = owner {
            lookup::ensure_active(&owner)?;
        }
    }
    Ok((p, row))
}

fn api_kind(k: FieldKind) -> ApiKind {
    parse_enum(k.as_str()).unwrap_or(ApiKind::Text)
}

fn core_kind(k: ApiKind) -> FieldKind {
    enum_text(k).parse().unwrap_or(FieldKind::Text)
}

fn task_count(tx: &Transaction<'_>, field: Id) -> Result<u64, Failure> {
    let n: i64 = tx.query_row(
        "SELECT count(DISTINCT v.task_id) FROM custom_values v JOIN tasks t ON t.id = v.task_id
          WHERE v.field_id = ?1 AND t.deleted_at IS NULL",
        [b(field)],
        |r| r.get(0),
    )?;
    Ok(u64::try_from(n).unwrap_or(0))
}

fn to_api(tx: &Transaction<'_>, row: &FieldRow) -> Result<CustomField, Failure> {
    let project = match row.project {
        Some(p) => Some(
            tx.query_row("SELECT key FROM projects WHERE id = ?1", [b(p)], |r| {
                r.get::<_, String>(0)
            })?,
        ),
        None => None,
    };
    Ok(CustomField {
        id: row.id.to_string(),
        name: row.name.clone(),
        query_name: format!("{}{}", gk_query::fields::CUSTOM_PREFIX, row.query_name),
        kind: api_kind(row.kind),
        project,
        options: row.options.clone(),
        default: row.default.clone(),
        types: row.types.clone(),
        required: row.required,
        tasks: task_count(tx, row.id)?,
    })
}

fn list(q: &QueryCtx<'_>, input: CustomFieldListInput) -> Result<CustomFieldList, Failure> {
    let project = match &input.project {
        Some(p) => Some(lookup::project(q.tx, "project", p)?.id),
        None => None,
    };
    let fields = rows(q.tx, project)?
        .iter()
        .map(|r| to_api(q.tx, r))
        .collect::<Result<_, _>>()?;
    Ok(CustomFieldList { fields })
}

/// The workspace's fields as queries see them: one per query name, with
/// the options of every field of that name.
pub(crate) fn query_fields(
    tx: &Transaction<'_>,
) -> Result<Vec<gk_query::fields::CustomField>, Failure> {
    let mut out: Vec<gk_query::fields::CustomField> = Vec::new();
    for row in rows(tx, None)? {
        match out.iter_mut().find(|f| f.query_name == row.query_name) {
            Some(f) => {
                for o in row.options {
                    if !f.options.iter().any(|x| fold(x) == fold(&o)) {
                        f.options.push(o);
                    }
                }
            }
            None => out.push(gk_query::fields::CustomField {
                name: row.name,
                query_name: row.query_name,
                kind: row.kind,
                options: row.options,
            }),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Values

fn stored_sql(s: &Stored) -> Sql {
    match s {
        Stored::Text(t) => Sql::Text(t.clone()),
        Stored::Int(n) => Sql::Integer(*n),
        Stored::Real(r) => Sql::Real(*r),
    }
}

fn sql_json(kind: FieldKind, v: Sql) -> Option<Value> {
    match v {
        Sql::Text(t) => Some(Value::String(t)),
        Sql::Integer(n) if kind == FieldKind::Boolean => Some(Value::Bool(n != 0)),
        Sql::Integer(n) => Some(Value::from(n)),
        Sql::Real(r) => serde_json::Number::from_f64(r).map(Value::Number),
        Sql::Null | Sql::Blob(_) => None,
    }
}

/// The value of `field` on `task` as the API and history show it; `None`
/// when it has none.
fn value_of(tx: &Transaction<'_>, task: Id, field: &FieldRow) -> Result<Option<Value>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT value FROM custom_values WHERE task_id = ?1 AND field_id = ?2 ORDER BY value",
    )?;
    let values: Vec<Sql> = stmt
        .query_map(params![b(task), b(field.id)], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(json_of(field, values))
}

fn json_of(field: &FieldRow, values: Vec<Sql>) -> Option<Value> {
    if field.kind.is_multiple() {
        let mut items: Vec<String> = values
            .into_iter()
            .filter_map(|v| match v {
                Sql::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        if items.is_empty() {
            return None;
        }
        let place = |o: &String| {
            field
                .options
                .iter()
                .position(|x| x == o)
                .unwrap_or(usize::MAX)
        };
        items.sort_by_key(place);
        return Some(Value::Array(items.into_iter().map(Value::String).collect()));
    }
    values
        .into_iter()
        .next()
        .and_then(|v| sql_json(field.kind, v))
}

/// Reads a value given in an input (or a default) for `field`; an empty
/// list means no value. `path` names the input field in errors.
fn parse_input(field: &FieldRow, value: &Value, path: &str) -> Result<Vec<Stored>, Failure> {
    let one = |v: &Value| -> Result<Stored, Failure> {
        let as_text = match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => {
                if field.kind == FieldKind::Integer {
                    return n
                        .as_i64()
                        .map(Stored::Int)
                        .ok_or_else(|| invalid(path, core::FieldError::NotInteger(n.to_string())));
                }
                if field.kind == FieldKind::Decimal {
                    return n
                        .as_f64()
                        .filter(|f| f.is_finite())
                        .map(Stored::Real)
                        .ok_or_else(|| invalid(path, core::FieldError::NotNumber(n.to_string())));
                }
                n.to_string()
            }
            Value::Bool(v) => {
                if field.kind == FieldKind::Boolean {
                    return Ok(Stored::Int(i64::from(*v)));
                }
                v.to_string()
            }
            Value::Null | Value::Array(_) | Value::Object(_) => {
                return Err(messages::invalid(
                    path,
                    "field-value-shape",
                    &[("field", field.name.clone())],
                ));
            }
        };
        core::parse_value(field.kind, &as_text, &field.options, &field.name)
            .map_err(|e| invalid(path, e))
    };
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) if field.kind.is_multiple() => {
            let mut out: Vec<Stored> = Vec::new();
            for item in items {
                let s = one(item)?;
                if !out.contains(&s) {
                    out.push(s);
                }
            }
            Ok(out)
        }
        Value::Array(_) => Err(messages::invalid(
            path,
            "field-one-value",
            &[("field", field.name.clone())],
        )),
        v => Ok(vec![one(v)?]),
    }
}

/// A value as history holds it, read back without checking the options
/// (they may be written back in the same revert after it).
fn stored_from_history(field: &FieldRow, value: Option<&Value>) -> Vec<Stored> {
    let one = |v: &Value| match v {
        Value::String(s) => Some(Stored::Text(s.clone())),
        Value::Bool(v) => Some(Stored::Int(i64::from(*v))),
        Value::Number(n) => match (field.kind, n.as_i64()) {
            (FieldKind::Decimal, _) | (_, None) => n.as_f64().map(Stored::Real),
            (_, Some(i)) => Some(Stored::Int(i)),
        },
        _ => None,
    };
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().filter_map(one).collect(),
        Some(v) => one(v).into_iter().collect(),
    }
}

/// Writes the values of `field` on `task` without recording them.
fn write_values(w: &WriteCtx<'_>, task: Id, field: Id, values: &[Stored]) -> Result<(), Failure> {
    w.tx().execute(
        "DELETE FROM custom_values WHERE task_id = ?1 AND field_id = ?2",
        params![b(task), b(field)],
    )?;
    for v in values {
        w.tx().execute(
            "INSERT OR IGNORE INTO custom_values (task_id, field_id, value) VALUES (?1, ?2, ?3)",
            params![b(task), b(field), stored_sql(v)],
        )?;
    }
    fields::touch(w, TASK, task, "custom_field")
}

/// Gives `task` exactly these values of `field` and records the change.
fn set_values(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    field: &FieldRow,
    values: &[Stored],
) -> Result<bool, Failure> {
    let before = value_of(w.tx(), task, field)?;
    let after = json_of(field, values.iter().map(stored_sql).collect());
    if before == after {
        return Ok(false);
    }
    write_values(w, task, field.id, values)?;
    w.record_field(
        TASK,
        task,
        Some(project),
        &format!("{VALUE_PREFIX}{}", field.id),
        before,
        after,
    );
    Ok(true)
}

/// The type name of `task`.
fn type_name(tx: &Transaction<'_>, task: Id) -> Result<String, Failure> {
    Ok(tx.query_row(
        "SELECT y.name FROM tasks t JOIN task_types y ON y.id = t.type_id WHERE t.id = ?1",
        [b(task)],
        |r| r.get(0),
    )?)
}

/// Sets the values `input` gives on a task of `project` (by field name,
/// query name or ID; `null` clears). For a new task (`creating`), fields
/// with a default get it unless given, and required fields need a value.
pub(crate) fn apply_input(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: &ProjectRow,
    input: &BTreeMap<String, Value>,
    creating: bool,
) -> Result<(), Failure> {
    let all = rows(w.tx(), Some(project.id))?;
    if all.is_empty() && input.is_empty() {
        return Ok(());
    }
    let task_type = type_name(w.tx(), task)?;
    let mut given: Vec<Id> = Vec::new();
    for (key, value) in input {
        let path = format!("fields.{key}");
        let field = named(&all, key).ok_or_else(|| not_found(&all, &path, key))?;
        if given.contains(&field.id) {
            return Err(messages::invalid(
                &path,
                "field-given-twice",
                &[("field", field.name.clone())],
            ));
        }
        given.push(field.id);
        let values = parse_input(field, value, &path)?;
        if !values.is_empty() && !field.shown_for(&task_type) {
            return Err(messages::invalid(
                &path,
                "field-not-for-type",
                &[
                    ("field", field.name.clone()),
                    ("type", task_type.clone()),
                    ("types", field.types.join(", ")),
                ],
            ));
        }
        if values.is_empty() && field.required && field.shown_for(&task_type) {
            return Err(messages::invalid(
                &path,
                "field-required",
                &[("field", field.name.clone())],
            ));
        }
        set_values(w, task, project.id, field, &values)?;
    }
    if creating {
        for field in all.iter().filter(|f| !given.contains(&f.id)) {
            if !field.shown_for(&task_type) {
                continue;
            }
            match &field.default {
                Some(d) => {
                    let values = parse_input(field, d, "fields")?;
                    set_values(w, task, project.id, field, &values)?;
                }
                None if field.required => {
                    return Err(messages::invalid(
                        &format!("fields.{}", field.name),
                        "field-required",
                        &[("field", field.name.clone())],
                    ));
                }
                None => {}
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Templates and recurring tasks

/// The task's values by field name, as a template or a recurring task's
/// series keeps them.
pub(crate) fn blueprint_values(
    tx: &Transaction<'_>,
    task: Id,
) -> Result<BTreeMap<String, Value>, Failure> {
    Ok(task_values(tx, task)?
        .into_iter()
        .map(|v| (v.name, v.value))
        .collect())
}

/// Gives a task made from a template or a series (`task`, in `project`)
/// the values `values` names by field name. A field the project lacks, one
/// not shown for the task's type, or a value the field can't hold is left
/// out, and its name goes to `left_out`. Fields not given get their
/// default. With `strict`, a required field left without a value is
/// refused, as when a task is created; otherwise the task is made anyway.
pub(crate) fn apply_carried(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: &ProjectRow,
    values: &BTreeMap<String, Value>,
    strict: bool,
    left_out: &mut Vec<String>,
) -> Result<(), Failure> {
    let all = rows(w.tx(), Some(project.id))?;
    if all.is_empty() {
        for name in values.keys() {
            if !left_out.contains(name) {
                left_out.push(name.clone());
            }
        }
        return Ok(());
    }
    let task_type = type_name(w.tx(), task)?;
    let mut given: Vec<Id> = Vec::new();
    for (name, value) in values {
        let field = named(&all, name).filter(|f| !given.contains(&f.id));
        let parsed = field
            .filter(|f| f.shown_for(&task_type))
            .and_then(|f| parse_input(f, value, "fields").ok().map(|v| (f, v)));
        match parsed {
            Some((field, stored)) => {
                given.push(field.id);
                if !stored.is_empty() {
                    set_values(w, task, project.id, field, &stored)?;
                }
            }
            None => {
                if !left_out.contains(name) {
                    left_out.push(name.clone());
                }
            }
        }
    }
    for field in all.iter().filter(|f| !given.contains(&f.id)) {
        if !field.shown_for(&task_type) {
            continue;
        }
        match &field.default {
            Some(d) => {
                let stored = parse_input(field, d, "fields")?;
                set_values(w, task, project.id, field, &stored)?;
            }
            None if field.required && strict => {
                return Err(messages::invalid(
                    &format!("fields.{}", field.name),
                    "field-required",
                    &[("field", field.name.clone())],
                ));
            }
            None => {}
        }
    }
    Ok(())
}

/// The own custom fields of `project`, in order, as a project template
/// keeps them.
pub(crate) fn own_blueprints(
    tx: &Transaction<'_>,
    project: Id,
) -> Result<Vec<gk_api::ops::template::FieldBlueprint>, Failure> {
    Ok(rows(tx, Some(project))?
        .into_iter()
        .filter(|f| f.project == Some(project))
        .map(|f| gk_api::ops::template::FieldBlueprint {
            id: f.id.to_string(),
            name: f.name,
            kind: api_kind(f.kind),
            options: f.options,
            default: f.default,
            types: f.types,
            required: f.required,
        })
        .collect())
}

/// Checks a project template's custom fields as `field.create` would:
/// names (each once), options and defaults. `path` is where the list sits
/// in the input.
pub(crate) fn check_blueprints(
    fields: &[gk_api::ops::template::FieldBlueprint],
    path: &str,
) -> Result<(), Failure> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let at = |e: Failure| at_index(e, path, i);
        let (name, query_name) = clean_name(&f.name).map_err(at)?;
        if seen
            .iter()
            .any(|(n, q)| fold(n) == fold(&name) || *q == query_name)
        {
            return Err(messages::invalid(
                &format!("{path}[{i}].name"),
                "template-name-twice",
                &[("name", name)],
            ));
        }
        let kind = core_kind(f.kind);
        let options = clean_options(kind, &f.options).map_err(at)?;
        let row = FieldRow {
            id: Id::from_bytes([0; 16]),
            project: None,
            name: name.clone(),
            query_name: query_name.clone(),
            kind,
            options,
            default: None,
            types: f.types.clone(),
            required: f.required,
            rank: String::new(),
        };
        if let Some(d) = &f.default {
            clean_default(&row, d).map_err(at)?;
        }
        seen.push((name, query_name));
    }
    Ok(())
}

/// Puts `path[i]` in front of an error's field path.
fn at_index(e: Failure, path: &str, i: usize) -> Failure {
    match e {
        Failure::Op(mut op) => {
            let inner = op
                .details
                .get("path")
                .or_else(|| op.details.get("field"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default();
            let full = if inner.is_empty() {
                format!("{path}[{i}]")
            } else {
                format!("{path}[{i}].{inner}")
            };
            op.details.insert("path".to_owned(), Value::String(full));
            Failure::Op(op)
        }
        other => other,
    }
}

/// Gives the new project `project` the custom fields of a project
/// template, checked, unrecorded beyond the project's creation (as a
/// sub-project's copies are). A field the project has already by the same
/// name and kind (a field of every project, or a copy of its parent's) is
/// used instead of a new one. Types the project lacks are dropped from a
/// field's list. Returns the project's field for each field by its ID in
/// the template.
pub(crate) fn insert_blueprints(
    w: &WriteCtx<'_>,
    project: &ProjectRow,
    fields: &[gk_api::ops::template::FieldBlueprint],
) -> Result<BTreeMap<String, Id>, Failure> {
    check_blueprints(fields, "setup.fields")?;
    let known = lookup::all(w.tx(), List::Types, project.id)?;
    let existing = rows(w.tx(), Some(project.id))?;
    let ranks = Rank::spread(fields.len());
    let mut copies = BTreeMap::new();
    for (f, rank) in fields.iter().zip(&ranks) {
        let (name, query_name) = clean_name(&f.name)?;
        let kind = core_kind(f.kind);
        if let Some(same) = existing
            .iter()
            .find(|e| fold(&e.name) == fold(&name) && e.kind == kind)
        {
            copies.insert(f.id.clone(), same.id);
            continue;
        }
        check_unique(w.tx(), Some(project.id), &name, &query_name, kind, None)?;
        let types = f
            .types
            .iter()
            .filter_map(|t| known.iter().find(|k| fold(&k.name) == fold(t)))
            .map(|k| k.name.clone())
            .collect();
        let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
        let mut row = FieldRow {
            id,
            project: Some(project.id),
            name,
            query_name,
            kind,
            options: clean_options(kind, &f.options)?,
            default: None,
            types,
            required: f.required,
            rank: rank.as_str().to_owned(),
        };
        if let Some(d) = &f.default {
            row.default = clean_default(&row, d)?;
        }
        insert(w, &row, Some(project.id), id)?;
        copies.insert(f.id.clone(), id);
    }
    Ok(copies)
}

// ---------------------------------------------------------------------
// Status rules

/// Custom field `id` (as a rule writes it, after `cf:`) when tasks of
/// `project` have it: one of the project's own fields or of every project.
pub(crate) fn of_project(
    tx: &Transaction<'_>,
    project: Id,
    id: &str,
) -> Result<Option<FieldRow>, Failure> {
    let Ok(id) = id.parse::<Id>() else {
        return Ok(None);
    };
    Ok(rows(tx, Some(project))?.into_iter().find(|f| f.id == id))
}

/// Checks a value a status rule sets in `field`: what people could type in
/// the field, or `today` for a date. `path` names the value in errors.
pub(crate) fn check_rule_value(field: &FieldRow, value: &str, path: &str) -> Result<(), Failure> {
    if field.kind == FieldKind::Date && value == "today" {
        return Ok(());
    }
    core::parse_value(field.kind, value, &field.options, &field.name)
        .map(|_| ())
        .map_err(|e| invalid(path, e))
}

/// What a status rule does to `field` of a task entering the status:
/// `Some(value)` sets it (`today` is the day `today` for a date), `None`
/// clears it. Nothing happens to a task of a type the field isn't shown
/// for, to a required field being cleared, to a value that no longer fits
/// the field (its options changed), or, with `only_if_empty`, to a field
/// that has a value.
pub(crate) fn apply_rule(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Id,
    field: &FieldRow,
    value: Option<&str>,
    only_if_empty: bool,
    today: &str,
) -> Result<(), Failure> {
    if !field.shown_for(&type_name(w.tx(), task)?) {
        return Ok(());
    }
    let values = match value {
        None if field.required => return Ok(()),
        None => Vec::new(),
        Some(v) => {
            if only_if_empty && value_of(w.tx(), task, field)?.is_some() {
                return Ok(());
            }
            let text = if field.kind == FieldKind::Date && v == "today" {
                today
            } else {
                v
            };
            match core::parse_value(field.kind, text, &field.options, &field.name) {
                Ok(stored) => vec![stored],
                Err(_) => return Ok(()),
            }
        }
    };
    set_values(w, task, project, field, &values)?;
    Ok(())
}

/// The task's values, in the fields' order.
pub(crate) fn task_values(
    tx: &Transaction<'_>,
    task: Id,
) -> Result<Vec<CustomFieldValue>, Failure> {
    let project: Option<Vec<u8>> = tx
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1",
            [b(task)],
            |r| r.get(0),
        )
        .optional()?;
    let Some(project) = project.and_then(|p| Id::from_slice(&p).ok()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for field in rows(tx, Some(project))? {
        if let Some(value) = value_of(tx, task, &field)? {
            out.push(CustomFieldValue {
                id: field.id.to_string(),
                name: field.name.clone(),
                query_name: format!("{}{}", gk_query::fields::CUSTOM_PREFIX, field.query_name),
                kind: api_kind(field.kind),
                value,
            });
        }
    }
    Ok(out)
}

/// The task's values by query name, as lists show them.
pub(crate) fn listed_values(
    tx: &Transaction<'_>,
    task: Id,
) -> Result<BTreeMap<String, Value>, Failure> {
    let has: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM custom_values WHERE task_id = ?1 LIMIT 1",
            [b(task)],
            |r| r.get(0),
        )
        .optional()?;
    if has.is_none() {
        return Ok(BTreeMap::new());
    }
    Ok(task_values(tx, task)?
        .into_iter()
        .map(|v| {
            let name = v
                .query_name
                .strip_prefix(gk_query::fields::CUSTOM_PREFIX)
                .unwrap_or(&v.query_name)
                .to_owned();
            (name, v.value)
        })
        .collect())
}

/// Gives the copy `new` of task `old` the same values, unrecorded beyond
/// the copy's creation.
pub(crate) fn copy_values(w: &WriteCtx<'_>, old: Id, new: Id) -> Result<(), Failure> {
    w.tx().execute(
        "INSERT OR IGNORE INTO custom_values (task_id, field_id, value)
         SELECT ?2, field_id, value FROM custom_values WHERE task_id = ?1",
        params![b(old), b(new)],
    )?;
    Ok(())
}

/// A field a moving task has a value of, the value (read only when it
/// goes to another field) and the target's field that takes it.
type Carried<'a> = (FieldRow, Option<Value>, Option<&'a FieldRow>);

/// What happens to the values of `task` when it moves to the project whose
/// fields are `target`: for each field it has a value of, the field's name
/// and the target's field that would hold the value (the field itself when
/// the target has it too; else one of the same query name and kind that
/// takes the value, options by name), or none when it would be left off.
fn carry_plan<'a>(
    tx: &Transaction<'_>,
    task: Id,
    target: &'a [FieldRow],
) -> Result<Vec<Carried<'a>>, Failure> {
    let held: Vec<Id> = {
        // In the order the task panel shows them: fields of every project
        // first, then by rank.
        let mut stmt = tx.prepare(
            "SELECT f.id FROM custom_fields f
              WHERE f.id IN (SELECT field_id FROM custom_values WHERE task_id = ?1)
              ORDER BY f.project_id IS NOT NULL, f.rank, f.id",
        )?;
        stmt.query_map([b(task)], |r| id_at(r, 0))?
            .collect::<Result<_, _>>()?
    };
    let mut plan = Vec::new();
    for id in held {
        let Some(old) = row_by_id(tx, id)? else {
            continue;
        };
        if let Some(same) = target.iter().find(|f| f.id == id) {
            plan.push((old, None, Some(same)));
            continue;
        }
        let value = value_of(tx, task, &old)?;
        let new = value.as_ref().and_then(|v| {
            target
                .iter()
                .filter(|f| f.query_name == old.query_name && f.kind == old.kind)
                .find(|f| parse_input(f, v, "fields").is_ok())
        });
        plan.push((old, value, new));
    }
    Ok(plan)
}

/// After `task` moved to project `to`: its values of fields `to` doesn't
/// have go to `to`'s field of the same query name and kind where it has
/// one that takes them (options by name), and are dropped otherwise.
/// Returns warnings for the values left behind.
pub(crate) fn follow_move(
    w: &mut WriteCtx<'_>,
    task: Id,
    to: &ProjectRow,
) -> Result<Vec<String>, Failure> {
    let target = rows(w.tx(), Some(to.id))?;
    let mut warnings = Vec::new();
    for (old, value, new) in carry_plan(w.tx(), task, &target)? {
        if new.is_some_and(|f| f.id == old.id) {
            continue;
        }
        set_values(w, task, to.id, &old, &[])?;
        match (new, &value) {
            (Some(new), Some(v)) => {
                let values = parse_input(new, v, "fields")?;
                set_values(w, task, to.id, new, &values)?;
            }
            _ => warnings.push(messages::text(
                "field-left-off",
                &[("field", old.name.clone()), ("project", to.key.clone())],
            )),
        }
    }
    Ok(warnings)
}

/// For a move check: the custom fields `task` has values of, and the field
/// of project `to` that would hold each (absent when it would be left off).
pub(crate) fn move_matches(
    tx: &Transaction<'_>,
    task: Id,
    to: &ProjectRow,
) -> Result<Vec<(String, Option<String>)>, Failure> {
    let target = rows(tx, Some(to.id))?;
    Ok(carry_plan(tx, task, &target)?
        .into_iter()
        .map(|(old, _, new)| (old.name, new.map(|f| f.name.clone())))
        .collect())
}

/// Gives the new sub-project `child` copies of `parent`'s own fields, and
/// returns the ID of each copy by the ID of its original.
pub(crate) fn copy_to_subproject(
    w: &WriteCtx<'_>,
    parent: Id,
    child: Id,
) -> Result<BTreeMap<Id, Id>, Failure> {
    let own: Vec<FieldRow> = rows(w.tx(), Some(parent))?
        .into_iter()
        .filter(|f| f.project == Some(parent))
        .collect();
    let mut copies = BTreeMap::new();
    for f in own {
        let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
        insert(w, &f, Some(child), id)?;
        copies.insert(f.id, id);
    }
    Ok(copies)
}

fn insert(w: &WriteCtx<'_>, f: &FieldRow, project: Option<Id>, id: Id) -> Result<(), Failure> {
    w.tx().execute(
        "INSERT INTO custom_fields (id, project_id, name, name_fold, query_name, kind, options,
                                    default_value, type_names, required, rank, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            b(id),
            project.map(b),
            f.name,
            fold(&f.name),
            f.query_name,
            f.kind.as_str(),
            Value::from(f.options.clone()).to_string(),
            f.default.as_ref().map(Value::to_string),
            Value::from(f.types.clone()).to_string(),
            f.required,
            f.rank,
            w.now()
        ],
    )?;
    Ok(())
}

/// After a task type of `project` was renamed from `old` to `new`, the
/// project's own fields shown for it name it by its new name.
pub(crate) fn rename_type(
    w: &mut WriteCtx<'_>,
    project: Id,
    old: &str,
    new: &str,
) -> Result<(), Failure> {
    for f in rows(w.tx(), Some(project))?
        .into_iter()
        .filter(|f| f.project == Some(project))
    {
        if !f.types.iter().any(|t| fold(t) == fold(old)) {
            continue;
        }
        let types: Vec<String> = f
            .types
            .iter()
            .map(|t| {
                if fold(t) == fold(old) {
                    new.to_owned()
                } else {
                    t.clone()
                }
            })
            .collect();
        set_setting(w, &f, "types", list_value(&types))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Settings and history

fn list_value(items: &[String]) -> Option<Value> {
    (!items.is_empty()).then(|| Value::from(items.to_vec()))
}

/// A setting of field `id` as history records it; the outer `None` when
/// the field doesn't exist.
fn setting(row: &FieldRow, field: &str) -> Option<Value> {
    match field {
        "name" => text(row.name.clone()),
        "options" => list_value(&row.options),
        "default" => row.default.clone(),
        "types" => list_value(&row.types),
        "required" => Some(Value::Bool(row.required)),
        "rank" => text(row.rank.clone()),
        _ => None,
    }
}

/// Writes a setting without recording it.
fn write_setting(
    w: &WriteCtx<'_>,
    id: Id,
    field: &str,
    value: Option<&Value>,
) -> Result<(), Failure> {
    let list_text = |v: Option<&Value>| match v {
        Some(Value::Array(a)) => Value::Array(a.clone()).to_string(),
        _ => "[]".to_owned(),
    };
    let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_owned();
    let tx = w.tx();
    match field {
        "name" => {
            let name = s(value);
            let query_name = core::query_name(&name).map_err(|e| invalid("name", e))?;
            tx.execute(
                "UPDATE custom_fields SET name = ?2, name_fold = ?3, query_name = ?4 WHERE id = ?1",
                params![b(id), name, fold(&name), query_name],
            )?;
        }
        "options" | "types" => {
            let column = if field == "options" {
                "options"
            } else {
                "type_names"
            };
            tx.execute(
                &format!("UPDATE custom_fields SET {column} = ?2 WHERE id = ?1"),
                params![b(id), list_text(value)],
            )?;
        }
        "default" => {
            tx.execute(
                "UPDATE custom_fields SET default_value = ?2 WHERE id = ?1",
                params![b(id), value.filter(|v| !v.is_null()).map(Value::to_string)],
            )?;
        }
        "required" => {
            tx.execute(
                "UPDATE custom_fields SET required = ?2 WHERE id = ?1",
                params![b(id), value.and_then(Value::as_bool).unwrap_or(false)],
            )?;
        }
        "rank" => {
            tx.execute(
                "UPDATE custom_fields SET rank = ?2 WHERE id = ?1",
                params![b(id), s(value)],
            )?;
        }
        _ => {
            return Err(Failure::Op(messages::storage(
                "storage-unknown-field",
                &[("entity", ENTITY.to_owned()), ("field", field.to_owned())],
            )));
        }
    }
    Ok(())
}

/// Sets a setting and records the change; nothing happens when it already
/// has that value.
fn set_setting(
    w: &mut WriteCtx<'_>,
    row: &FieldRow,
    field: &str,
    value: Option<Value>,
) -> Result<bool, Failure> {
    let current = row_by_id(w.tx(), row.id)?;
    let before = current.as_ref().and_then(|r| setting(r, field));
    if before == value {
        return Ok(false);
    }
    write_setting(w, row.id, field, value.as_ref())?;
    w.record_field(ENTITY, row.id, row.project, field, before, value);
    Ok(true)
}

/// The current value of a recorded field of a custom field, or of a task's
/// value; `None` (outer) when this kind of change can't be reverted.
pub(crate) fn current(
    tx: &Transaction<'_>,
    entity: &str,
    id: Id,
    field: &str,
) -> Result<Option<Option<Value>>, StoreError> {
    if entity == TASK {
        let Some(field_id) = field
            .strip_prefix(VALUE_PREFIX)
            .and_then(|f| f.parse::<Id>().ok())
        else {
            return Ok(None);
        };
        let Some(row) = row_by_id(tx, field_id)? else {
            return Ok(None);
        };
        return value_of(tx, id, &row).map(Some).map_err(store_error);
    }
    let row = row_by_id(tx, id)?;
    Ok(match field {
        "created" => Some(row.map(|_| Value::Bool(true))),
        "deleted" => None,
        _ => row.map(|r| setting(&r, field)),
    })
}

fn store_error(e: Failure) -> StoreError {
    match e {
        Failure::Store(s) => s,
        Failure::Op(o) => StoreError::Corrupt(o.message),
        Failure::SnapshotFirst { .. } => StoreError::Corrupt(String::new()),
    }
}

/// Writes a recorded value back and records that change.
pub(crate) fn apply(
    w: &mut WriteCtx<'_>,
    entity: &str,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    if entity == TASK {
        let row = field
            .strip_prefix(VALUE_PREFIX)
            .and_then(|f| f.parse::<Id>().ok())
            .map(|f| row_by_id(w.tx(), f))
            .transpose()?
            .flatten()
            .ok_or_else(|| {
                Failure::Op(messages::conflict(
                    "history-deleted-not-recreated",
                    &[("entity", ENTITY.to_owned())],
                ))
            })?;
        let project = project.ok_or_else(|| Failure::Op(gk_api::OpError::storage("no project")))?;
        let values = stored_from_history(&row, value.as_ref());
        return set_values(w, id, project, &row, &values).map(drop);
    }
    let Some(row) = row_by_id(w.tx(), id)? else {
        return Err(messages::conflict(
            "history-deleted-not-recreated",
            &[("entity", ENTITY.to_owned())],
        )
        .into());
    };
    if field == "created" {
        if value.is_some() {
            return Ok(());
        }
        let n = task_count(w.tx(), id)?;
        if n > 0 {
            return Err(messages::conflict(
                "history-in-use",
                &[("entity", ENTITY.to_owned()), ("count", n.to_string())],
            )
            .into());
        }
        w.tx()
            .execute("DELETE FROM custom_fields WHERE id = ?1", [b(id)])?;
        w.record_field(ENTITY, id, project, field, Some(Value::Bool(true)), None);
        return Ok(());
    }
    if field == "name"
        && let Some(Value::String(name)) = &value
    {
        let query_name = core::query_name(name).map_err(|e| invalid("name", e))?;
        check_unique(w.tx(), row.project, name, &query_name, row.kind, Some(id))?;
    }
    set_setting(w, &row, field, value).map(drop)
}

// ---------------------------------------------------------------------
// Commands

/// Refuses a name or query name another field the same tasks have
/// already has, or a query name another field holds with another kind.
fn check_unique(
    tx: &Transaction<'_>,
    project: Option<Id>,
    name: &str,
    query_name: &str,
    kind: FieldKind,
    except: Option<Id>,
) -> Result<(), Failure> {
    let others: Vec<FieldRow> = rows(tx, None)?
        .into_iter()
        .chain(match project {
            // Fields of deleted projects don't count, and `rows(None)`
            // leaves them out; this project's own still count.
            Some(p) => rows(tx, Some(p))?,
            None => Vec::new(),
        })
        .filter(|f| Some(f.id) != except)
        .collect();
    let wanted = fold(name);
    let overlaps = |f: &FieldRow| match project {
        // A field of every project meets every field; a project's field
        // meets those of every project and its own.
        None => true,
        Some(p) => f.project.is_none() || f.project == Some(p),
    };
    if let Some(clash) = others
        .iter()
        .find(|f| overlaps(f) && (fold(&f.name) == wanted || f.query_name == query_name))
    {
        return Err(messages::conflict(
            "field-name-taken",
            &[("name", name.to_owned()), ("other", clash.name.clone())],
        )
        .with_detail("field", "name")
        .into());
    }
    if let Some(clash) = others
        .iter()
        .find(|f| f.query_name == query_name && f.kind != kind)
    {
        return Err(messages::conflict(
            "field-kind-clash",
            &[
                ("name", name.to_owned()),
                ("kind", kind.as_str().to_owned()),
                ("other_kind", clash.kind.as_str().to_owned()),
            ],
        )
        .with_detail("field", "kind")
        .into());
    }
    Ok(())
}

/// Checks a list of options: one short line each, unique ignoring case,
/// at most [`OPTIONS_MAX`].
fn clean_options(kind: FieldKind, options: &[String]) -> Result<Vec<String>, Failure> {
    if !kind.has_options() {
        if options.is_empty() {
            return Ok(Vec::new());
        }
        return Err(messages::invalid(
            "options",
            "field-options-not-allowed",
            &[("kind", kind.as_str().to_owned())],
        ));
    }
    if options.is_empty() {
        return Err(messages::invalid("options", "field-needs-options", &[]));
    }
    if options.len() > OPTIONS_MAX {
        return Err(messages::invalid(
            "options",
            "field-options-max",
            &[("max", OPTIONS_MAX.to_string())],
        ));
    }
    let mut out: Vec<String> = Vec::with_capacity(options.len());
    for (i, o) in options.iter().enumerate() {
        let path = format!("options[{i}]");
        let o = core::check_option(o).map_err(|e| invalid(&path, e))?;
        if out.iter().any(|x| fold(x) == fold(&o)) {
            return Err(messages::invalid(
                &path,
                "field-option-twice",
                &[("option", o)],
            ));
        }
        out.push(o);
    }
    Ok(out)
}

/// The task types a field shows for, by their names: for a project's
/// field, types of that project as it spells them.
fn clean_types(
    tx: &Transaction<'_>,
    project: Option<&ProjectRow>,
    types: &[String],
) -> Result<Vec<String>, Failure> {
    let mut out: Vec<String> = Vec::new();
    for (i, t) in types.iter().enumerate() {
        let path = format!("types[{i}]");
        let name = match project {
            Some(p) => lookup::named(tx, List::Types, p, &path, t)?.name,
            None => gk_core::check_line("name", t, FIELD_NAME_MAX_CHARS)
                .map_err(|e| invalid(&path, e))?,
        };
        if !out.iter().any(|x| fold(x) == fold(&name)) {
            out.push(name);
        }
    }
    Ok(out)
}

/// A default value checked against the field, in the form the API shows.
fn clean_default(field: &FieldRow, value: &Value) -> Result<Option<Value>, Failure> {
    let values = parse_input(field, value, "default")?;
    if values.is_empty() {
        return Ok(None);
    }
    Ok(json_of(field, values.iter().map(stored_sql).collect()))
}

fn siblings(tx: &Transaction<'_>, project: Option<Id>) -> Result<Vec<(Id, String)>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, rank FROM custom_fields WHERE project_id IS ?1 ORDER BY rank, id",
    )?;
    let rows = stmt
        .query_map([project.map(b)], |r| {
            Ok((id_at(r, 0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Where `before`/`after`/`at` put a field among the fields of the same
/// project (or of every project).
fn target(
    tx: &Transaction<'_>,
    project: Option<Id>,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<Edge>,
) -> Result<Option<Target>, Failure> {
    let group: Vec<FieldRow> = rows(tx, project)?
        .into_iter()
        .filter(|f| f.project == project)
        .collect();
    let find = |path: &str, t: &str| {
        named(&group, t)
            .map(|f| f.id)
            .ok_or_else(|| not_found(&group, path, t))
    };
    let before = before.map(|t| find("before", t)).transpose()?;
    let after = after.map(|t| find("after", t)).transpose()?;
    Target::from_parts(before, after, at)
}

/// Puts field `id` at `target`, re-ranking others where needed; returns
/// its rank.
fn place(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    id: Id,
    target: Target,
) -> Result<Rank, Failure> {
    let list = siblings(w.tx(), project)?;
    let mut mine = None;
    for (item, rank) in lookup::place(&list, id, target)? {
        if item == id {
            mine = Some(rank);
        } else if let Some(row) = row_by_id(w.tx(), item)? {
            set_setting(w, &row, "rank", text(rank.as_str()))?;
        }
    }
    Ok(match mine {
        Some(r) => r,
        None => list
            .iter()
            .find(|(i, _)| *i == id)
            .and_then(|(_, r)| Rank::parse(r).ok())
            .unwrap_or_else(Rank::first),
    })
}

fn clean_name(name: &str) -> Result<(String, String), Failure> {
    let name =
        gk_core::check_line("name", name, FIELD_NAME_MAX_CHARS).map_err(|e| invalid("name", e))?;
    let query_name = core::query_name(&name).map_err(|e| invalid("name", e))?;
    Ok((name, query_name))
}

fn changed(w: &WriteCtx<'_>, id: Id, values_removed: u64) -> Result<CustomFieldChange, Failure> {
    let field = match row_by_id(w.tx(), id)? {
        Some(row) => Some(to_api(w.tx(), &row)?),
        None => None,
    };
    Ok(CustomFieldChange {
        field,
        views_rewritten: super::view::names(w.tx(), &w.changed_of(super::view::ENTITY))?,
        values_removed,
        group: w.group_if_changed(),
    })
}

fn create(
    w: &mut WriteCtx<'_>,
    input: CustomFieldCreateInput,
) -> Result<CustomFieldChange, Failure> {
    let project = project_of(w.tx(), input.project.as_deref())?;
    let pid = project.as_ref().map(|p| p.id);
    let (name, query_name) = clean_name(&input.name)?;
    let kind = core_kind(input.kind);
    check_unique(w.tx(), pid, &name, &query_name, kind, None)?;
    let options = clean_options(kind, &input.options)?;
    let types = clean_types(w.tx(), project.as_ref(), &input.types)?;
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let mut row = FieldRow {
        id,
        project: pid,
        name,
        query_name,
        kind,
        options,
        default: None,
        types,
        required: input.required,
        rank: Rank::first().as_str().to_owned(),
    };
    if let Some(d) = &input.default {
        row.default = clean_default(&row, d)?;
    }
    insert(w, &row, pid, id)?;
    let target = target(
        w.tx(),
        pid,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?
    .unwrap_or(Target::Bottom);
    let rank = place(w, pid, id, target)?;
    write_setting(w, id, "rank", text(rank.as_str()).as_ref())?;
    w.record_field(ENTITY, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(ENTITY, id, pid, "name", None, text(row.name.clone()));
    changed(w, id, 0)
}

/// Rewrites the saved views after field `row` changed. `query` rewrites a
/// query (and the sort keys, as `sort:` text); `column` a column name. The
/// views of the field's project are rewritten, and all views when no other
/// field keeps the field's old query name.
fn rewrite_views(
    w: &mut WriteCtx<'_>,
    row: &FieldRow,
    query: &dyn Fn(&str) -> Option<String>,
    column: &dyn Fn(&str) -> Option<Option<String>>,
) -> Result<(), Failure> {
    let others = rows(w.tx(), None)?
        .into_iter()
        .any(|f| f.id != row.id && f.query_name == row.query_name);
    let scope = match (others, row.project) {
        (false, _) => None,
        (true, Some(p)) => Some(p),
        (true, None) => return Ok(()),
    };
    super::view::rewrite_views(w, scope, query, column)?;
    Ok(())
}

/// Whether a column names the field with this query name.
fn is_column_of(column: &str, query_name: &str) -> bool {
    gk_query::fields::strip_custom_prefix(column).is_some_and(|c| fold(c) == query_name)
}

#[allow(clippy::too_many_lines)]
fn update(
    w: &mut WriteCtx<'_>,
    input: CustomFieldUpdateInput,
) -> Result<CustomFieldChange, Failure> {
    let (_, row) = find(w, input.project.as_deref(), &input.field)?;
    let owner = match row.project {
        Some(p) => lookup::project_by_id(w.tx(), p)?,
        None => None,
    };
    for (given, setting, name) in [
        (
            input.default.is_some(),
            CustomFieldSetting::Default,
            "default",
        ),
        (input.types.is_some(), CustomFieldSetting::Types, "types"),
    ] {
        if given && input.clear.contains(&setting) {
            return Err(messages::invalid(
                "clear",
                "field-given-and-cleared",
                &[("field", name.to_owned())],
            ));
        }
    }
    let mut removed_tasks: Vec<Id> = Vec::new();
    // The name.
    if let Some(name) = &input.name {
        let (name, query_name) = clean_name(name)?;
        check_unique(
            w.tx(),
            row.project,
            &name,
            &query_name,
            row.kind,
            Some(row.id),
        )?;
        let old_query = row.query_name.clone();
        if set_setting(w, &row, "name", text(name))? && old_query != query_name {
            let (old, new) = (old_query.clone(), query_name.clone());
            rewrite_views(
                w,
                &row,
                &|q| gk_query::rename::rename_field(q, &old, &new),
                &|c| {
                    is_column_of(c, &old)
                        .then(|| Some(format!("{}{new}", gk_query::fields::CUSTOM_PREFIX)))
                },
            )?;
        }
    }
    let mut row = row_by_id(w.tx(), row.id)?.unwrap_or(row);
    // The options: renames first, then the new list.
    if (!input.rename_options.is_empty() || input.options.is_some()) && !row.kind.has_options() {
        return Err(messages::invalid(
            "options",
            "field-options-not-allowed",
            &[("kind", row.kind.as_str().to_owned())],
        ));
    }
    let mut options = row.options.clone();
    let mut renames: Vec<(String, String)> = Vec::new();
    for (i, r) in input.rename_options.iter().enumerate() {
        let path = format!("rename_options[{i}]");
        let Some(at) = options.iter().position(|o| fold(o) == fold(&r.from)) else {
            return Err(messages::invalid(
                &format!("{path}.from"),
                "field-option-unknown",
                &[("option", r.from.clone()), ("choices", options.join(", "))],
            ));
        };
        let to = core::check_option(&r.to).map_err(|e| invalid(&format!("{path}.to"), e))?;
        renames.push((options[at].clone(), to.clone()));
        options[at] = to;
    }
    let mut dropped: Vec<String> = Vec::new();
    if let Some(list) = &input.options {
        let list = clean_options(row.kind, list)?;
        for o in &options {
            match list.iter().find(|x| fold(x) == fold(o)) {
                Some(x) if x != o => renames.push((o.clone(), x.clone())),
                Some(_) => {}
                None => dropped.push(o.clone()),
            }
        }
        options = list;
    } else {
        clean_options(row.kind, &options)?;
    }
    if row.kind.has_options() && options != row.options {
        // Renamed values first, against the old list, then the new list.
        let tasks: Vec<Id> = {
            let mut stmt = w
                .tx()
                .prepare("SELECT DISTINCT task_id FROM custom_values WHERE field_id = ?1")?;
            stmt.query_map([b(row.id)], |r| id_at(r, 0))?
                .collect::<Result<_, _>>()?
        };
        let pid_of = |tx: &Transaction<'_>, t: Id| -> Result<Id, Failure> {
            let p: Vec<u8> =
                tx.query_row("SELECT project_id FROM tasks WHERE id = ?1", [b(t)], |r| {
                    r.get(0)
                })?;
            Id::from_slice(&p).map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))
        };
        for task in tasks {
            let Some(Value::Array(_) | Value::String(_)) = value_of(w.tx(), task, &row)? else {
                continue;
            };
            let current = stored_from_history(&row, value_of(w.tx(), task, &row)?.as_ref());
            let mut next: Vec<Stored> = Vec::new();
            for v in current {
                let Stored::Text(t) = v else { continue };
                let t = renames
                    .iter()
                    .fold(t, |t, (from, to)| if &t == from { to.clone() } else { t });
                if options.contains(&t) {
                    if !next.contains(&Stored::Text(t.clone())) {
                        next.push(Stored::Text(t));
                    }
                } else if !removed_tasks.contains(&task) {
                    removed_tasks.push(task);
                }
            }
            let project = pid_of(w.tx(), task)?;
            let mut with_new = row.clone();
            with_new.options.clone_from(&options);
            set_values(w, task, project, &with_new, &next)?;
        }
        set_setting(w, &row, "options", list_value(&options))?;
        for (from, to) in &renames {
            let q = row.query_name.clone();
            let (from, to) = (from.clone(), to.clone());
            rewrite_views(
                w,
                &row,
                &|src| gk_query::rename::rename_option(src, &q, &from, &to),
                &|_| None,
            )?;
        }
        for gone in &dropped {
            let q = row.query_name.clone();
            let gone = gone.clone();
            rewrite_views(
                w,
                &row,
                &|src| gk_query::rename::remove_option(src, &q, &gone),
                &|_| None,
            )?;
        }
        // A default that was renamed follows; one that was dropped goes.
        if let Some(d) = row.default.clone() {
            let kept = stored_from_history(&row, Some(&d))
                .into_iter()
                .filter_map(|v| match v {
                    Stored::Text(t) => Some(
                        renames
                            .iter()
                            .fold(t, |t, (from, to)| if &t == from { to.clone() } else { t }),
                    ),
                    _ => None,
                })
                .filter(|t| options.contains(t))
                .map(Stored::Text)
                .collect::<Vec<_>>();
            let mut with_new = row.clone();
            with_new.options.clone_from(&options);
            let value = json_of(&with_new, kept.iter().map(stored_sql).collect());
            set_setting(w, &row, "default", value)?;
        }
        row = row_by_id(w.tx(), row.id)?.unwrap_or(row);
    }
    // The default, the types and whether it is required.
    if let Some(d) = &input.default {
        let value = clean_default(&row, d)?;
        set_setting(w, &row, "default", value)?;
    }
    if input.clear.contains(&CustomFieldSetting::Default) {
        set_setting(w, &row, "default", None)?;
    }
    if let Some(types) = &input.types {
        let types = clean_types(w.tx(), owner.as_ref(), types)?;
        set_setting(w, &row, "types", list_value(&types))?;
    }
    if input.clear.contains(&CustomFieldSetting::Types) {
        set_setting(w, &row, "types", None)?;
    }
    if let Some(required) = input.required {
        set_setting(w, &row, "required", Some(Value::Bool(required)))?;
    }
    if let Some(target) = target(
        w.tx(),
        row.project,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )? {
        let rank = place(w, row.project, row.id, target)?;
        set_setting(w, &row, "rank", text(rank.as_str()))?;
    }
    changed(w, row.id, removed_tasks.len() as u64)
}

fn delete(
    w: &mut WriteCtx<'_>,
    input: CustomFieldDeleteInput,
) -> Result<CustomFieldChange, Failure> {
    let (_, row) = find(w, input.project.as_deref(), &input.field)?;
    let count = task_count(w.tx(), row.id)?;
    if let Some(expected) = input.expected_task_count
        && expected != count
    {
        return Err(messages::conflict(
            "field-count-differs",
            &[
                ("field", row.name.clone()),
                ("expected", expected.to_string()),
                ("count", count.to_string()),
            ],
        )
        .with_detail("field", "expected_task_count")
        .into());
    }
    let tasks: Vec<Id> = {
        let mut stmt = w
            .tx()
            .prepare("SELECT DISTINCT task_id FROM custom_values WHERE field_id = ?1")?;
        stmt.query_map([b(row.id)], |r| id_at(r, 0))?
            .collect::<Result<_, _>>()?
    };
    for task in tasks {
        fields::touch(w, TASK, task, "custom_field")?;
    }
    w.record_field(
        ENTITY,
        row.id,
        row.project,
        "deleted",
        None,
        Some(Value::Bool(true)),
    );
    w.record_field(
        ENTITY,
        row.id,
        row.project,
        "name",
        text(row.name.clone()),
        None,
    );
    let q = row.query_name.clone();
    rewrite_views(
        w,
        &row,
        &|src| gk_query::rename::remove_field(src, &q),
        &|c| is_column_of(c, &q).then_some(None),
    )?;
    w.tx()
        .execute("DELETE FROM custom_fields WHERE id = ?1", [b(row.id)])?;
    let mut change = changed(w, row.id, count)?;
    change.field = None;
    Ok(change)
}
