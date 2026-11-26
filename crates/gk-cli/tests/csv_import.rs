// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel import csv`: a dry run of a Redmine export, the import, and
//! the presets from the command line.

#![allow(clippy::unwrap_used)]

mod common;

use std::path::Path;

use common::Workspace;

#[test]
fn a_redmine_export_is_checked_and_imported() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Web", "--key", "SHOP"]);
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../gk-engine/tests/csv/redmine-1252.csv");
    let path = fixture.to_string_lossy().into_owned();
    let check = ws.json(&["import", "csv", &path, "--project", "SHOP", "--dry-run"]);
    assert_eq!(check["dry_run"], true);
    assert_eq!(check["preset"], "redmine");
    assert_eq!(check["format"]["encoding"], "windows1252");
    assert_eq!(check["counts"]["created"], 2);
    let text = ws.ok(&["import", "csv", &path, "--project", "SHOP"]);
    assert!(!text.is_empty());
    let task = ws.json(&["task", "show", "SHOP-1"]);
    assert_eq!(task["items"][0]["title"], "Menü überarbeiten");

    let presets = ws.json(&["csv-preset", "list"]);
    assert_eq!(presets["items"].as_array().unwrap().len(), 2);
    ws.ok(&[
        "csv-preset",
        "save",
        "Ours",
        "--columns",
        r#"[{"column": "Subject", "to": "title"}]"#,
    ]);
    ws.ok(&["csv-preset", "delete", "Ours"]);
    let err = ws.fails(1, &["csv-preset", "delete", "jira"]);
    assert!(!err.is_empty());
}
