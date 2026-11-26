// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields from the command line: defining them, setting values on
//! tasks and querying by them, each command its own process.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Workspace, keys};
use serde_json::json;

#[test]
fn fields_are_defined_set_and_queried() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "-q",
        "field",
        "add",
        "--project",
        "GK",
        "--name",
        "Story points",
        "--kind",
        "integer",
    ]);
    ws.ok(&[
        "-q",
        "field",
        "add",
        "--project",
        "GK",
        "--name",
        "Area",
        "--kind",
        "select",
        "--options",
        "Front end",
        "--options",
        "Back end",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Small",
        "--fields",
        r#"{"Story points": "2", "Area": "back end"}"#,
    ]);
    ws.ok(&["-q", "task", "create", "--project", "GK", "--title", "Big"]);
    ws.ok(&[
        "-q",
        "task",
        "edit",
        "GK-2",
        "--fields",
        r#"{"cf.story-points": 8}"#,
    ]);
    let fields = ws.json(&["field", "list", "--project", "GK"]);
    assert_eq!(
        fields["fields"][0]["query_name"], "cf.story-points",
        "{fields}"
    );
    assert_eq!(
        fields["fields"][1]["options"],
        json!(["Front end", "Back end"])
    );
    let found = ws.json(&[
        "task",
        "list",
        "--query",
        "cf.story-points>1 sort:-cf.story-points",
    ]);
    assert_eq!(keys(&found["items"]), ["GK-2", "GK-1"], "{found}");
    let task = ws.json(&["task", "show", "GK-1"]);
    assert_eq!(task["items"][0]["fields"][1]["value"], "Back end", "{task}");
}
