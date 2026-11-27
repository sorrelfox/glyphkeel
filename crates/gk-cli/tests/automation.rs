// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Automation rules from the command line, each command its own process
//! against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;

#[test]
fn a_rule_is_made_tried_and_acts_on_new_tasks() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Home", "--key", "HM"]);
    ws.ok(&["label", "add", "--project", "HM", "--name", "chores"]);
    let made = ws.json(&[
        "automation",
        "create",
        "--project",
        "HM",
        "--name",
        "Chores",
        "--trigger",
        r#"{"on": "task_created"}"#,
        "--condition",
        "title:clean",
        "--actions",
        r#"[{"kind": "add_label", "label": "chores"},
            {"kind": "add_comment", "text": "Filed by {rule}"}]"#,
    ]);
    assert_eq!(made["rule"]["name"], "Chores", "{made}");
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "HM",
        "--title",
        "Clean the bath",
    ]);
    let task = ws.json(&["task", "show", "HM-1", "--include", "comments"]);
    let task = &task["items"][0];
    assert_eq!(task["labels"][0]["name"], "chores", "{task}");
    assert_eq!(task["comments"][0]["author"]["kind"], "automation");
    let tried = ws.json(&["automation", "test", "HM-1", "--rule", "Chores"]);
    assert_eq!(tried["matches"], true, "{tried}");
    let listed = ws.ok(&["automation", "list", "--project", "HM"]);
    assert!(listed.contains("Chores"), "{listed}");
    let runs = ws.json(&["automation", "runs", "--project", "HM"]);
    assert_eq!(runs["items"][0]["result"], "done", "{runs}");
    ws.ok(&[
        "-q",
        "automation",
        "edit",
        "Chores",
        "--project",
        "HM",
        "--enabled",
        "false",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "HM",
        "--title",
        "Clean the car",
    ]);
    let task = ws.json(&["task", "show", "HM-2"]);
    assert!(
        task["items"][0]["labels"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "{task}"
    );
    let err = ws.fails(
        1,
        &[
            "automation",
            "create",
            "--name",
            "Broken",
            "--trigger",
            r#"{"on": "schedule", "every": "daily", "time": "08:00"}"#,
            "--actions",
            r#"[{"kind": "notify", "text": "hi"}]"#,
        ],
    );
    assert!(err.contains("needs a condition"), "{err}");
}
