// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Options that take several values: repeated, or separated by commas,
//! with `\,` for a comma inside a value.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;
use serde_json::{Value, json};

fn names(labels: &Value) -> Vec<String> {
    labels
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn values_separated_by_commas_are_several_values() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Zed", "--key", "ZZ"]);
    ws.ok(&[
        "-q",
        "field",
        "add",
        "--project",
        "ZZ",
        "--name",
        "Area",
        "--kind",
        "select",
        "--options",
        "A,B,C",
    ]);
    let fields = ws.json(&["field", "list", "--project", "ZZ"]);
    assert_eq!(
        fields["fields"][0]["options"],
        json!(["A", "B", "C"]),
        "{fields}"
    );

    for label in ["bug", "ui", "a,b", "c"] {
        ws.ok(&["-q", "label", "add", "--project", "ZZ", "--name", label]);
    }
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "ZZ",
        "--title",
        "One",
        "--labels",
        "bug, ui",
    ]);
    let task = ws.json(&["task", "show", "ZZ-1"]);
    assert_eq!(names(&task["items"][0]["labels"]), ["bug", "ui"], "{task}");

    // A comma inside a name: escaped, mixed with repeated options.
    ws.ok(&[
        "-q", "task", "edit", "ZZ-1", "--labels", r"a\,b", "--labels", "c,bug",
    ]);
    let task = ws.json(&["task", "show", "ZZ-1"]);
    let mut got = names(&task["items"][0]["labels"]);
    got.sort();
    assert_eq!(got, ["a,b", "bug", "c"], "{task}");

    // Positional lists and choices split too.
    let shown = ws.json(&["task", "show", "ZZ-1,ZZ-1", "--include", "links,comments"]);
    assert!(shown["items"][0]["links"].is_array(), "{shown}");
    assert!(shown["items"][0]["comments"].is_array(), "{shown}");
    // A wrong value among them is named.
    let err = ws.fails(1, &["task", "show", "ZZ-1", "--include", "links,nope"]);
    assert!(err.contains("'nope'"), "{err}");
}

#[test]
fn the_help_says_how_to_give_several_values() {
    let ws = Workspace::new();
    let help = ws.ok(&["task", "create", "--help"]);
    assert!(help.contains("separate values with commas"), "{help}");
    assert!(help.contains(r"\,"), "{help}");
}
