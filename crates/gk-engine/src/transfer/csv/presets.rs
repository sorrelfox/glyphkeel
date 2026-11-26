// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Presets of the CSV import: the built-in ones for the CSV exports of
//! Jira and Redmine, and the workspace's own, kept as one setting of the
//! workspace so that history records and reverts their changes.
//!
//! Column names are those the two tools write in English:
//! - Jira's "Export CSV (all fields)": `Summary`, `Issue key`, `Issue id`,
//!   `Parent id` or `Parent`, `Issue Type`, `Status`, `Priority`, one
//!   `Labels` and one `Comment` column per value, `Due date`, estimates and
//!   time spent in seconds, and `Inward issue link (…)` and `Outward issue
//!   link (…)` per kind of link.
//! - Redmine's issue list as CSV: `#`, `Tracker`, `Parent task` (an issue
//!   number), `Status`, `Priority`, `Subject`, `Category`, `Start date`,
//!   `Due date`, `Estimated time` and `Spent time` in hours, `Related
//!   issues` and `Description`.

use gk_api::ops::csv_import::{
    CsvPreset, CsvPresetChange, CsvPresetColumn, CsvPresetDelete, CsvPresetDeleteInput,
    CsvPresetList, CsvPresetListOp, CsvPresetSave, CsvPresetSaveInput, CsvUnit,
};
use gk_core::fold;
use gk_store::rusqlite::Transaction;

use crate::failure::Failure;
use crate::messages;
use crate::ops::{desc, settings};
use crate::registry::Registry;
use crate::write::WriteCtx;

/// The setting the workspace's presets are kept in.
pub(crate) const SETTING: &str = "csv_import_presets";
/// The built-in presets' names.
pub(crate) const JIRA: &str = "jira";
pub(crate) const REDMINE: &str = "redmine";
/// The most presets a workspace keeps.
const MAX_PRESETS: usize = 100;
/// The longest preset name, in characters.
const MAX_NAME: usize = 60;
/// The most columns a preset maps.
const MAX_COLUMNS: usize = 500;

fn map(column: &str, to: &str) -> CsvPresetColumn {
    CsvPresetColumn {
        column: column.to_owned(),
        to: to.to_owned(),
        unit: None,
    }
}

fn timed(column: &str, to: &str, unit: CsvUnit) -> CsvPresetColumn {
    CsvPresetColumn {
        unit: Some(unit),
        ..map(column, to)
    }
}

/// The preset for Jira's CSV export.
fn jira() -> CsvPreset {
    let s = CsvUnit::Seconds;
    CsvPreset {
        name: JIRA.to_owned(),
        built_in: true,
        format: None,
        columns: vec![
            map("Summary", "title"),
            map("Issue key", "key"),
            map("Issue id", "key"),
            map("Parent id", "parent"),
            map("Parent", "parent"),
            map("Issue Type", "type"),
            map("Status", "status"),
            map("Priority", "priority"),
            map("Labels", "labels"),
            map("Description", "description"),
            map("Custom field (Start date)", "start_date"),
            map("Due date", "due_date"),
            timed("Original Estimate", "estimate", s),
            timed("Time Spent", "spent", s),
            map("Comment", "comment"),
            map("Outward issue link (Blocks)", "blocks"),
            map("Inward issue link (Blocks)", "blocked_by"),
            map("Outward issue link (Duplicate)", "duplicates"),
            map("Inward issue link (Duplicate)", "duplicated_by"),
            map("Outward issue link (Cloners)", "clones"),
            map("Inward issue link (Cloners)", "cloned_by"),
            map("Outward issue link (Relates)", "relates"),
            map("Inward issue link (Relates)", "relates"),
        ],
        values: None,
    }
}

/// The preset for Redmine's CSV export of issues.
fn redmine() -> CsvPreset {
    let h = CsvUnit::Hours;
    CsvPreset {
        name: REDMINE.to_owned(),
        built_in: true,
        format: None,
        columns: vec![
            map("#", "key"),
            map("Subject", "title"),
            map("Tracker", "type"),
            map("Status", "status"),
            map("Priority", "priority"),
            map("Parent task", "parent"),
            map("Category", "labels"),
            map("Start date", "start_date"),
            map("Due date", "due_date"),
            timed("Estimated time", "estimate", h),
            timed("Spent time", "spent", h),
            map("Related issues", "relations"),
            map("Description", "description"),
            map("Last notes", "comment"),
        ],
        values: None,
    }
}

/// Column names matched when no preset applies, by target.
const SYNONYMS: &[(&str, &[&str])] = &[
    ("title", &["title", "summary", "subject", "name", "task"]),
    ("description", &["description", "details", "body"]),
    ("status", &["status", "state"]),
    ("type", &["type", "issue type", "tracker", "kind"]),
    ("priority", &["priority"]),
    ("labels", &["labels", "label", "tags", "tag"]),
    ("start_date", &["start date", "start", "starts"]),
    ("due_date", &["due date", "due", "deadline"]),
    (
        "estimate",
        &[
            "estimate",
            "original estimate",
            "estimated time",
            "estimated hours",
        ],
    ),
    (
        "spent",
        &["time spent", "spent time", "spent", "spent hours"],
    ),
    ("key", &["key", "issue key", "id", "#", "issue id"]),
    (
        "parent",
        &["parent", "parent id", "parent task", "parent key"],
    ),
    ("relations", &["related issues"]),
    ("comment", &["comment", "comments"]),
];

/// The target of column `name` when no preset applies.
pub(crate) fn by_name(name: &str) -> Option<&'static str> {
    let n = fold(name.trim());
    SYNONYMS
        .iter()
        .find(|(_, names)| names.contains(&n.as_str()))
        .map(|(to, _)| *to)
}

/// The built-in preset that `headers` look like, if any.
pub(crate) fn detect(headers: &[String]) -> Option<&'static str> {
    let has = |n: &str| headers.iter().any(|h| fold(h.trim()) == fold(n));
    if has("Issue key") && has("Summary") {
        Some(JIRA)
    } else if has("#") && has("Subject") && has("Tracker") {
        Some(REDMINE)
    } else {
        None
    }
}

/// The workspace's own presets, by name.
pub(crate) fn saved(tx: &Transaction<'_>) -> Result<Vec<CsvPreset>, Failure> {
    let stored = settings::raw(tx, SETTING)?;
    let mut list: Vec<CsvPreset> = stored
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    for p in &mut list {
        p.built_in = false;
    }
    list.sort_by_key(|p| fold(&p.name));
    Ok(list)
}

/// Every preset: built-in ones first.
pub(crate) fn all(tx: &Transaction<'_>) -> Result<Vec<CsvPreset>, Failure> {
    let mut out = vec![jira(), redmine()];
    out.extend(saved(tx)?);
    Ok(out)
}

/// The preset named `name` (ignoring case).
pub(crate) fn find(tx: &Transaction<'_>, name: &str) -> Result<CsvPreset, Failure> {
    let wanted = fold(name.trim());
    let presets = all(tx)?;
    if let Some(p) = presets.iter().find(|p| fold(&p.name) == wanted) {
        return Ok(p.clone());
    }
    let names: Vec<String> = presets.into_iter().map(|p| p.name).collect();
    Err(messages::with(
        "csv-preset-not-found",
        &[("name", name.trim().to_owned())],
        |m| gk_api::OpError::not_found(m, &names.iter().map(String::as_str).collect::<Vec<_>>()),
    )
    .with_detail("field", "preset")
    .into())
}

fn store(w: &mut WriteCtx<'_>, list: &[CsvPreset]) -> Result<CsvPresetChange, Failure> {
    let value = if list.is_empty() {
        None
    } else {
        Some(settings::to_value(list)?)
    };
    settings::write_raw(w, SETTING, value)?;
    Ok(CsvPresetChange {
        items: all(w.tx())?,
        group: w.group_if_changed(),
    })
}

fn save(w: &mut WriteCtx<'_>, input: CsvPresetSaveInput) -> Result<CsvPresetChange, Failure> {
    let name = input.name.trim().to_owned();
    if name.is_empty()
        || name.chars().count() > MAX_NAME
        || name.chars().any(super::parse::is_forbidden_control)
        || name.contains(['\n', '\r', '\t'])
    {
        return Err(messages::invalid(
            "name",
            "csv-preset-name",
            &[("max", MAX_NAME.to_string())],
        ));
    }
    if [JIRA, REDMINE].contains(&fold(&name).as_str()) {
        return Err(messages::invalid(
            "name",
            "csv-preset-built-in",
            &[("name", name)],
        ));
    }
    if input.columns.len() > MAX_COLUMNS {
        return Err(messages::invalid(
            "columns",
            "csv-preset-too-many-columns",
            &[("max", MAX_COLUMNS.to_string())],
        ));
    }
    for (i, c) in input.columns.iter().enumerate() {
        super::target::Target::parse(&c.to).map_err(|_| {
            messages::invalid(
                &format!("columns[{i}].to"),
                "csv-unknown-target",
                &[("target", c.to.clone())],
            )
        })?;
    }
    if let Some(f) = &input.format {
        super::check_format(f)?;
    }
    let mut list = saved(w.tx())?;
    list.retain(|p| fold(&p.name) != fold(&name));
    if list.len() >= MAX_PRESETS {
        return Err(messages::invalid(
            "name",
            "csv-preset-too-many",
            &[("max", MAX_PRESETS.to_string())],
        ));
    }
    list.push(CsvPreset {
        name,
        built_in: false,
        format: input.format.map(Into::into),
        columns: input.columns.into_iter().map(Into::into).collect(),
        values: input.values.map(Into::into),
    });
    list.sort_by_key(|p| fold(&p.name));
    store(w, &list)
}

fn delete(w: &mut WriteCtx<'_>, input: CsvPresetDeleteInput) -> Result<CsvPresetChange, Failure> {
    let wanted = fold(input.name.trim());
    if [JIRA, REDMINE].contains(&wanted.as_str()) {
        return Err(messages::invalid(
            "name",
            "csv-preset-built-in",
            &[("name", input.name.trim().to_owned())],
        ));
    }
    let mut list = saved(w.tx())?;
    let before = list.len();
    list.retain(|p| fold(&p.name) != wanted);
    if list.len() == before {
        find(w.tx(), &input.name)?;
    }
    store(w, &list)
}

pub(crate) fn register(r: &mut Registry) {
    r.query::<CsvPresetListOp, _>(desc::<CsvPresetListOp>(), |q, _input| {
        Ok(CsvPresetList { items: all(q.tx)? })
    });
    r.command::<CsvPresetSave, _>(desc::<CsvPresetSave>(), save);
    r.command::<CsvPresetDelete, _>(desc::<CsvPresetDelete>(), delete);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_presets_are_detected_from_the_header_row() {
        let h = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(detect(&h(&["Summary", "Issue key", "Status"])), Some(JIRA));
        assert_eq!(detect(&h(&["#", "Tracker", "Subject"])), Some(REDMINE));
        assert_eq!(detect(&h(&["Title", "Status"])), None);
        assert_eq!(by_name(" Due Date "), Some("due_date"));
        assert_eq!(by_name("Assignee"), None);
        for p in [jira(), redmine()] {
            for c in &p.columns {
                assert!(
                    super::super::target::Target::parse(&c.to).is_ok(),
                    "{}",
                    c.to
                );
            }
        }
    }
}
