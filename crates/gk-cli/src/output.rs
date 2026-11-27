// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Printing an operation's output: exactly its JSON with `--json`, otherwise
//! a plain rendering for people that works for every output shape: lists
//! (`items`) as tables, objects as `field: value` lines.

use std::fmt::Write as _;

use gk_api::OpError;
use serde_json::{Map, Value};

use crate::messages::text;

/// Columns that lead a table when present, in this order; the rest follow
/// alphabetically.
const LEADING: &[&str] = &["key", "seq", "name", "title", "id"];

/// Fields that only let the app translate a text shown next to them; people
/// read the text.
const FOR_TRANSLATION: &[&str] = &["reason_id", "reason_args", "message_id", "args"];

/// The JSON text of `value`: keys sorted at every level, so the output is
/// the same whatever order the engine built it in.
#[must_use]
pub(crate) fn json(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&sorted(value))
        .unwrap_or_else(|_| unreachable!("a JSON value always serializes"));
    text.push('\n');
    text
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted(&o[k]));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        v => v.clone(),
    }
}

/// The text for people.
#[must_use]
pub(crate) fn human(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(o) if o.get("items").is_some_and(Value::is_array) => list(&mut out, o),
        Value::Object(o) => object(&mut out, o, 0),
        Value::Array(a) => table(&mut out, a, 0),
        v => {
            out.push_str(&cell(v));
            out.push('\n');
        }
    }
    out
}

/// An error for standard error.
#[must_use]
pub(crate) fn error(e: &OpError, as_json: bool) -> String {
    if as_json {
        return json(&serde_json::to_value(e).unwrap_or(Value::Null));
    }
    let mut out = text("cli-error", &[("message", e.message.clone())]);
    out.push('\n');
    if let Some(hint) = &e.hint {
        let _ = writeln!(out, "{}", text("cli-hint", &[("hint", hint.clone())]));
    }
    if let Some(choices) = e.details.get("choices").and_then(Value::as_array) {
        let words: Vec<String> = choices.iter().map(cell).collect();
        let _ = writeln!(
            out,
            "{}",
            text("cli-choices", &[("choices", words.join(", "))])
        );
    }
    if let Some(path) = e.details.get("path").and_then(Value::as_str)
        && !path.is_empty()
    {
        let _ = writeln!(out, "{}", text("cli-field", &[("field", path.to_owned())]));
    }
    out
}

fn list(out: &mut String, o: &Map<String, Value>) {
    let items = o
        .get("items")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    table(out, items, 0);
    let total = o.get("total").and_then(Value::as_u64);
    if let Some(total) = total
        && total > items.len() as u64
    {
        let _ = writeln!(
            out,
            "{}",
            text(
                "cli-shown-of-total",
                &[
                    ("shown", items.len().to_string()),
                    ("total", total.to_string()),
                ],
            )
        );
    }
    if let Some(cursor) = o.get("next_cursor").and_then(Value::as_str) {
        let _ = writeln!(
            out,
            "{}",
            text("cli-next-page", &[("cursor", cursor.to_owned())])
        );
    }
    let rest: Map<String, Value> = o
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "items" | "total" | "next_cursor"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if !rest.is_empty() {
        out.push('\n');
        object(out, &rest, 0);
    }
}

fn object(out: &mut String, o: &Map<String, Value>, indent: usize) {
    let pad = " ".repeat(indent);
    for (k, v) in ordered(o) {
        match v {
            Value::Object(inner) if !is_compact(inner) => {
                let _ = writeln!(out, "{pad}{k}:");
                object(out, inner, indent + 2);
            }
            Value::Array(a) if a.is_empty() => {
                let _ = writeln!(out, "{pad}{k}: {}", text("cli-none", &[]));
            }
            Value::Array(a) if a.iter().any(Value::is_object) => {
                let _ = writeln!(out, "{pad}{k}:");
                table(out, a, indent + 2);
            }
            Value::String(s) if s.contains('\n') => {
                let _ = writeln!(out, "{pad}{k}:");
                for line in s.lines() {
                    let _ = writeln!(out, "{pad}  {line}");
                }
            }
            v => {
                let _ = writeln!(out, "{pad}{k}: {}", cell(v));
            }
        }
    }
}

/// Objects with an obvious one-line form.
fn is_compact(o: &Map<String, Value>) -> bool {
    o.get("name").is_some_and(Value::is_string) || is_field_changes(o)
}

/// `{"field": {"before": …, "after": …}}`, as history writes it.
fn is_field_changes(o: &Map<String, Value>) -> bool {
    !o.is_empty()
        && o.values().all(|v| {
            v.as_object()
                .is_some_and(|c| !c.is_empty() && c.keys().all(|k| k == "before" || k == "after"))
        })
}

fn table(out: &mut String, rows: &[Value], indent: usize) {
    let pad = " ".repeat(indent);
    if rows.is_empty() {
        let _ = writeln!(out, "{pad}{}", text("cli-none", &[]));
        return;
    }
    if !rows.iter().all(Value::is_object) {
        for row in rows {
            let _ = writeln!(out, "{pad}{}", cell(row));
        }
        return;
    }
    let mut columns: Vec<String> = Vec::new();
    for row in rows {
        if let Value::Object(o) = row {
            for k in o.keys() {
                if !columns.contains(k) && !FOR_TRANSLATION.contains(&k.as_str()) {
                    columns.push(k.clone());
                }
            }
        }
    }
    order(&mut columns);
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|c| row.get(c).map(cell).unwrap_or_default())
                .collect()
        })
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            cells
                .iter()
                .map(|r| r[i].chars().count())
                .chain([c.chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |values: &[String]| {
        let mut s = pad.clone();
        for (i, v) in values.iter().enumerate() {
            if i + 1 == values.len() {
                s.push_str(v);
            } else {
                let _ = write!(s, "{v:<w$}  ", w = widths[i]);
            }
        }
        s.trim_end().to_owned()
    };
    let headers: Vec<String> = columns.iter().map(|c| c.to_uppercase()).collect();
    let _ = writeln!(out, "{}", line(&headers));
    for row in &cells {
        let _ = writeln!(out, "{}", line(row));
    }
}

fn ordered(o: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut keys: Vec<String> = o.keys().cloned().collect();
    order(&mut keys);
    keys.iter()
        .filter(|k| !FOR_TRANSLATION.contains(&k.as_str()))
        .filter_map(|k| o.get_key_value(k.as_str()))
        .collect()
}

fn order(keys: &mut [String]) {
    keys.sort_by_key(|k| {
        (
            LEADING.iter().position(|l| l == k).unwrap_or(LEADING.len()),
            k.clone(),
        )
    });
}

/// One value on one line.
fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.replace('\n', " "),
        Value::Bool(b) => text(if *b { "cli-true" } else { "cli-false" }, &[]),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(cell).collect::<Vec<_>>().join(", "),
        Value::Object(o) if is_field_changes(o) => o
            .iter()
            .map(|(field, change)| {
                let side = |s: &str| {
                    change
                        .get(s)
                        .map_or_else(|| text("cli-no-value", &[]), cell)
                };
                text(
                    "cli-field-change",
                    &[
                        ("field", field.clone()),
                        ("before", side("before")),
                        ("after", side("after")),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
        Value::Object(o) => match o.get("name").and_then(Value::as_str) {
            Some(name) => name.to_owned(),
            None => serde_json::to_string(&sorted(v)).unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_keys_are_sorted() {
        let v = json!({ "b": 1, "a": { "d": 2, "c": [ { "f": 1, "e": 2 } ] } });
        assert_eq!(
            json(&v),
            "{\n  \"a\": {\n    \"c\": [\n      {\n        \"e\": 2,\n        \"f\": 1\n      }\n    ],\n    \"d\": 2\n  },\n  \"b\": 1\n}\n"
        );
    }

    #[test]
    fn lists_become_tables() {
        let v = json!({
            "items": [
                { "seq": 2, "actor": { "kind": "user", "name": "You" }, "fields": { "trash_days": { "before": 30, "after": 7 } } },
                { "seq": 1, "actor": { "kind": "agent", "name": "Claude Code" }, "fields": { "week_start": { "after": "sunday" } } }
            ],
            "total": 3,
            "next_cursor": "abc"
        });
        insta::assert_snapshot!(human(&v), @r"
        SEQ  ACTOR        FIELDS
        2    You          trash_days: 30 → 7
        1    Claude Code  week_start: – → sunday
        2 of 3
        next page: --cursor abc
        ");
    }

    #[test]
    fn objects_become_lines() {
        let v = json!({ "name": "Personal", "settings": { "trash_days": 30, "agent_max_profile": "standard" }, "changed": [], "ok": true });
        insta::assert_snapshot!(human(&v), @r"
        name: Personal
        changed: (none)
        ok: yes
        settings:
          agent_max_profile: standard
          trash_days: 30
        ");
    }

    #[test]
    fn errors_show_hint_and_choices() {
        let e = OpError::not_found("No status 'Doing'.", &["To do", "Done"])
            .with_hint("Use one of the listed statuses.");
        assert_eq!(
            error(&e, false),
            "glyphkeel: No status 'Doing'.\nhint: Use one of the listed statuses.\nchoices: To do, Done\n"
        );
    }
}
