// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Bulk changes: one change to many tasks, in one command, so one history
//! group (one Undo, one revert). Each task goes through the same code as a
//! single `task.move` and `task.update`, so every rule holds per task.
//!
//! A preview runs that same code and then takes it back (a savepoint), so it
//! shows exactly what the change would do, errors included. The real run
//! needs the preview's count and stops when the tasks now number otherwise.

use std::collections::BTreeMap;

use gk_api::OpError;
use gk_api::Profile;
use gk_api::ops::bulk::{
    BulkChanges, BulkFieldChange, BulkItem, BulkResult, BulkUpdateInput, TaskBulkUpdate,
};
use gk_api::ops::task::{TaskMoveInput, TaskUpdateInput};
use gk_core::{Day, DurationUnits, Id};
use gk_query::{Compiled, SqlEnv};
use gk_store::rusqlite::Transaction;
use serde_json::Value;

use super::desc;
use super::fields::{self, TASK, priority_name};
use super::lookup::{self, b, opt_id_at};
use super::task::{self, task_labels};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

/// The most tasks one bulk change takes, for the standard profile.
const MAX_STANDARD: usize = 100;
/// The most for the full profile and the person.
const MAX_FULL: usize = 500;

pub(super) fn register(r: &mut Registry) {
    r.combined::<TaskBulkUpdate, _>(desc::<TaskBulkUpdate>(), parts, bulk);
}

/// The single-task operations an input runs, each authorized by its own
/// class.
fn parts(input: &Value) -> Vec<&'static str> {
    let changes = input.get("changes");
    let given = |k: &str| {
        changes.and_then(|c| c.get(k)).is_some_and(|v| {
            !v.is_null()
                && v != &Value::Array(Vec::new())
                && v.as_object().is_none_or(|o| !o.is_empty())
        })
    };
    let mut out = Vec::new();
    if given("status") || given("project") {
        out.push("task.move");
    }
    let fields = [
        "type",
        "priority",
        "labels",
        "add_labels",
        "remove_labels",
        "fix_version",
        "parent",
        "start_date",
        "due_date",
        "shift_start",
        "shift_due",
        "estimate",
        "clear",
        "fields",
    ];
    if fields.iter().any(|k| given(k)) {
        out.push("task.update");
    }
    out
}

fn is_empty(c: &BulkChanges) -> bool {
    c == &BulkChanges::default()
}

/// A task's key, title, and the fields a bulk change can touch as text
/// (custom fields as `cf.<query name>`).
type Summary = (String, String, Vec<(String, Option<String>)>);

/// The fields a bulk change can touch, as text, for the preview.
fn summary(
    tx: &Transaction<'_>,
    id: Id,
    units: DurationUnits,
    custom: &[String],
) -> Result<Summary, Failure> {
    let (key, title, project, status, ty, priority, parent, start, due, estimate) = tx.query_row(
        "SELECT p.key || '-' || t.number, t.title, p.key, s.name, y.name, t.priority,
                t.parent_id, t.start_date, t.due_date, t.estimate_min
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
          WHERE t.id = ?1",
        [b(id)],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                opt_id_at(r, 6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<i64>>(9)?,
            ))
        },
    )?;
    let parent = match parent {
        Some(p) => Some(tx.query_row(
            "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
              WHERE t.id = ?1",
            [b(p)],
            |r| r.get::<_, String>(0),
        )?),
        None => None,
    };
    let labels: Vec<String> = task_labels(tx, id)?.into_iter().map(|l| l.name).collect();
    let fix_version = super::version::of_task(tx, id)?.map(|v| v.name);
    let fields: Vec<(&str, Option<String>)> = vec![
        ("project", Some(project)),
        ("status", Some(status)),
        ("type", Some(ty)),
        ("priority", Some(priority_name(priority).to_owned())),
        ("labels", (!labels.is_empty()).then(|| labels.join(", "))),
        ("fix_version", fix_version),
        ("parent", parent),
        ("start_date", start),
        ("due_date", due),
        (
            "estimate",
            estimate.map(|m| gk_core::format_duration(m, units)),
        ),
    ];
    let mut fields: Vec<(String, Option<String>)> =
        fields.into_iter().map(|(f, v)| (f.to_owned(), v)).collect();
    if !custom.is_empty() {
        let values = super::custom_field::task_values(tx, id)?;
        for wanted in custom {
            let written = format!("{}{wanted}", gk_query::fields::CUSTOM_PREFIX);
            let value = values
                .iter()
                .find(|v| v.query_name == written)
                .map(|v| value_text(&v.value));
            fields.push((written, value));
        }
    }
    Ok((key, title, fields))
}

/// A custom field's value as text for the preview: a list's options joined.
fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(value_text).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

/// The query names of the custom fields a change names, for the preview:
/// each of the fields the tasks see whose name, query name or ID is given.
fn custom_names(tx: &Transaction<'_>, c: &BulkChanges) -> Result<Vec<String>, Failure> {
    if c.fields.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for f in super::custom_field::rows(tx, None)? {
        let named = c.fields.keys().any(|k| {
            gk_core::fold(k) == gk_core::fold(&f.name)
                || gk_core::fold(k) == f.query_name
                || k.parse::<Id>().is_ok_and(|id| id == f.id)
        });
        if named && !out.contains(&f.query_name) {
            out.push(f.query_name);
        }
    }
    Ok(out)
}

/// A day moved by a signed offset in calendar units: `+2d`, `-1w`, `+1m`,
/// `+1y`.
fn shifted(day: Day, offset: &str) -> Option<Day> {
    let offset = offset.trim();
    let (sign, rest) = match offset.as_bytes().first()? {
        b'+' => (1, &offset[1..]),
        b'-' => (-1, &offset[1..]),
        _ => (1, offset),
    };
    let unit = rest.chars().last()?;
    let digits = &rest[..rest.len() - unit.len_utf8()];
    let n: i64 = digits.parse().ok().filter(|n: &i64| *n <= 100_000)?;
    match unit.to_ascii_lowercase() {
        'd' => day.add_days(sign * n).ok(),
        'w' => day.add_days(sign * n * 7).ok(),
        'm' => gk_query::dates::add_months(day, sign * n),
        'y' => gk_query::dates::add_months(day, sign * n * 12),
        _ => None,
    }
}

/// The new value of a date field: set (in any friendly form), or the
/// current one shifted; `None` leaves it.
fn new_date(
    w: &WriteCtx<'_>,
    id: Id,
    column: &str,
    set: Option<&String>,
    shift: Option<&String>,
    field: &str,
) -> Result<Option<String>, Failure> {
    let path = format!("changes.{field}");
    match (set, shift) {
        (Some(_), Some(_)) => Err(messages::invalid(
            &path,
            "bulk-set-and-shift",
            &[("field", field.to_owned())],
        )),
        (Some(text), None) => {
            let cal = super::query::calendar(w.tx(), w.zone(), w.now())?;
            gk_query::parse_natural(text, &cal)
                .map(|d| Some(d.to_string()))
                .ok_or_else(|| {
                    messages::invalid(&path, "bulk-date-invalid", &[("text", text.clone())])
                })
        }
        (None, Some(offset)) => {
            let current = fields::read(w.tx(), TASK, id, column)?
                .and_then(|v| v.as_str().and_then(|s| s.parse::<Day>().ok()));
            match current {
                None => Ok(None),
                Some(day) => shifted(day, offset)
                    .map(|d| Some(d.to_string()))
                    .ok_or_else(|| {
                        messages::invalid(
                            &format!("changes.shift_{}", column.trim_end_matches("_date")),
                            "bulk-shift-invalid",
                            &[("text", offset.clone())],
                        )
                    }),
            }
        }
        (None, None) => Ok(None),
    }
}

/// Applies the change to one task.
fn apply(
    w: &mut WriteCtx<'_>,
    id: Id,
    c: &BulkChanges,
    warnings: &mut Vec<String>,
) -> Result<(), Failure> {
    let key = id.to_string();
    if c.status.is_some() || c.project.is_some() {
        let change = task::move_task(
            w,
            TaskMoveInput {
                key: key.clone(),
                status: c.status.clone(),
                project: c.project.clone(),
                mapping: c.mapping.clone(),
                // Values go with a move, to the subtasks too.
                fields: if c.project.is_some() {
                    c.fields.clone()
                } else {
                    BTreeMap::new()
                },
                close_duplicates: c.close_duplicates,
                ..TaskMoveInput::default()
            },
        )?;
        warnings.extend(change.warnings);
    }
    let start_date = new_date(
        w,
        id,
        "start_date",
        c.start_date.as_ref(),
        c.shift_start.as_ref(),
        "start_date",
    )?;
    let due_date = new_date(
        w,
        id,
        "due_date",
        c.due_date.as_ref(),
        c.shift_due.as_ref(),
        "due_date",
    )?;
    let input = TaskUpdateInput {
        key,
        task_type: c.task_type.clone(),
        priority: c.priority,
        labels: c.labels.clone(),
        add_labels: c.add_labels.clone(),
        remove_labels: c.remove_labels.clone(),
        fix_version: c.fix_version.clone(),
        parent: c.parent.clone(),
        start_date,
        due_date,
        estimate: c.estimate.clone(),
        clear: c.clear.clone(),
        fields: if c.project.is_some() {
            BTreeMap::new()
        } else {
            c.fields.clone()
        },
        ..TaskUpdateInput::default()
    };
    if input
        != (TaskUpdateInput {
            key: input.key.clone(),
            ..TaskUpdateInput::default()
        })
    {
        let change = task::update(w, input)?;
        warnings.extend(change.warnings);
    }
    Ok(())
}

/// The most tasks the caller may change at once.
fn most(w: &WriteCtx<'_>) -> Result<usize, Failure> {
    if !w.caller().is_agent() {
        return Ok(MAX_FULL);
    }
    let cap = super::settings::load(w.tx())?.agent_max_profile;
    let profile = w
        .caller()
        .profile
        .unwrap_or(Profile::Standard)
        .capped_by(cap);
    Ok(if profile == Profile::Full {
        MAX_FULL
    } else {
        MAX_STANDARD
    })
}

fn bulk(w: &mut WriteCtx<'_>, input: BulkUpdateInput) -> Result<BulkResult, Failure> {
    let dry_run = input.dry_run.unwrap_or(true);
    let ids: Vec<Id> = match (&input.query, input.keys.is_empty()) {
        (None, true) => return Err(messages::invalid("keys", "bulk-keys-or-query", &[])),
        (Some(_), false) => return Err(messages::invalid("query", "bulk-keys-and-query", &[])),
        (None, false) => {
            let mut ids = Vec::with_capacity(input.keys.len());
            for (i, k) in input.keys.iter().enumerate() {
                let id = lookup::task(w.tx(), &format!("keys[{i}]"), k)?.id;
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            ids
        }
        (Some(q), true) => {
            let (_, checked) = super::query::compile(w.tx(), w.zone(), w.now(), q, "query")?;
            let compiled = Compiled::new(
                &checked,
                &SqlEnv {
                    now: w.now(),
                    scope: None,
                    default_sort: Vec::new(),
                },
            );
            super::query::matching_ids(w.tx(), &compiled)?
        }
    };
    if is_empty(&input.changes) {
        return Err(messages::invalid("changes", "bulk-no-changes", &[]));
    }
    let count = ids.len();
    let max = most(w)?;
    if count > max {
        let args = [("max", max.to_string()), ("count", count.to_string())];
        let error = messages::with("bulk-too-many", &args, |m| OpError::limit(m, max as u64));
        return Err(messages::hint(error, "bulk-too-many-hint", &[]).into());
    }
    if !dry_run {
        match input.expected_count {
            None => {
                return Err(messages::invalid(
                    "expected_count",
                    "bulk-expected-count",
                    &[],
                ));
            }
            Some(n) if n != count as u64 => {
                let error = messages::conflict(
                    "bulk-count-changed",
                    &[("expected", n.to_string()), ("count", count.to_string())],
                )
                .with_detail("count", count as u64);
                return Err(messages::hint(error, "bulk-count-changed-hint", &[]).into());
            }
            Some(_) => {}
        }
    }
    let units = task::units(w.tx())?;
    let custom = custom_names(w.tx(), &input.changes)?;
    let before = ids
        .iter()
        .map(|id| summary(w.tx(), *id, units, &custom))
        .collect::<Result<Vec<_>, _>>()?;
    let changes = input.changes.clone();
    let run = |w: &mut WriteCtx<'_>| -> Result<(Vec<BulkItem>, Vec<String>), Failure> {
        let mut warnings = Vec::new();
        let mut items = Vec::with_capacity(ids.len());
        for (id, (_, _, old)) in ids.iter().zip(&before) {
            apply(w, *id, &changes, &mut warnings)?;
            let (key, title, new) = summary(w.tx(), *id, units, &custom)?;
            let changes = old
                .iter()
                .zip(&new)
                .filter(|(a, b)| a.1 != b.1)
                .map(|(a, b)| BulkFieldChange {
                    field: a.0.clone(),
                    before: a.1.clone(),
                    after: b.1.clone(),
                })
                .collect();
            items.push(BulkItem {
                key,
                title,
                changes,
            });
        }
        let mut seen = std::collections::HashSet::new();
        warnings.retain(|w| seen.insert(w.clone()));
        Ok((items, warnings))
    };
    let (items, warnings) = if dry_run { w.preview(run)? } else { run(w)? };
    Ok(BulkResult {
        dry_run,
        count: count as u64,
        changed: items.iter().filter(|i| !i.changes.is_empty()).count() as u64,
        items,
        group: if dry_run { None } else { w.group_if_changed() },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifts_move_by_calendar_units() {
        let d = |s: &str| s.parse::<Day>().expect("day");
        assert_eq!(shifted(d("2026-01-31"), "+1m"), Some(d("2026-02-28")));
        assert_eq!(shifted(d("2026-11-23"), "+2d"), Some(d("2026-11-25")));
        assert_eq!(shifted(d("2026-11-23"), "-1w"), Some(d("2026-11-16")));
        assert_eq!(shifted(d("2026-11-23"), "3d"), Some(d("2026-11-26")));
        assert_eq!(shifted(d("2026-11-23"), "+x"), None);
    }

    #[test]
    fn parts_follow_the_changes() {
        let p = |v: Value| parts(&serde_json::json!({ "changes": v }));
        assert_eq!(p(serde_json::json!({"status": "Done"})), ["task.move"]);
        assert_eq!(p(serde_json::json!({"add_labels": ["x"]})), ["task.update"]);
        assert!(p(serde_json::json!({"add_labels": []})).is_empty());
        assert_eq!(
            p(serde_json::json!({"fields": {"Points": 3}})),
            ["task.update"]
        );
        assert!(p(serde_json::json!({"fields": {}})).is_empty());
    }
}
