// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Versions from the command line and through the agent server, each its
//! own process against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Agent, Workspace};
use serde_json::json;

#[test]
fn versions_are_made_planned_and_closed_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "-q",
        "version",
        "create",
        "--project",
        "GK",
        "--name",
        "1.0",
        "--due-date",
        "2027-01-29",
    ]);
    ws.ok(&[
        "-q",
        "version",
        "create",
        "--project",
        "GK",
        "--name",
        "1.1",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Engine",
        "--fix-version",
        "1.0",
    ]);
    let listed = ws.json(&["version", "list", "GK"]);
    assert_eq!(listed["total"], 2);
    assert_eq!(listed["items"][0]["name"], "1.0");
    assert_eq!(listed["items"][0]["progress"]["total"], 1);
    let found = ws.json(&["task", "list", "--query", "version:1.0"]);
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["key"], "GK-1");
    let closed = ws.json(&[
        "version",
        "edit",
        "1.0",
        "--project",
        "GK",
        "--status",
        "closed",
        "--move-open-tasks-to",
        "1.1",
    ]);
    assert_eq!(closed["tasks_changed"], json!(["GK-1"]));
    let shown = ws.json(&["version", "show", "1.1", "--project", "GK"]);
    assert_eq!(shown["tasks"][0]["key"], "GK-1");
    // For people: one row per version.
    let text = ws.ok(&["version", "list", "GK"]);
    assert!(text.contains("1.0") && text.contains("1.1"), "{text}");
}

#[test]
fn agents_list_create_and_plan_versions() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    let mut agent = Agent::start(ws.home(), &ws.path(), "tester");
    let made = agent.ok(
        "create_version",
        json!({ "project": "GK", "name": "1.0", "due_date": "2027-01-29" }),
    );
    assert_eq!(made["version"]["status"], "open");
    let task = agent.ok(
        "create_task",
        json!({ "project": "GK", "title": "Engine", "fix_version": "1.0" }),
    );
    assert_eq!(task["task"]["fix_version"]["name"], "1.0");
    let list = agent.ok(
        "list_versions",
        json!({ "project": "GK", "include_tasks": true }),
    );
    assert_eq!(list["items"][0]["tasks"][0]["key"], "GK-1");
    let one = agent.ok("get_version", json!({ "version": "1.0", "project": "GK" }));
    assert_eq!(one["progress"]["total"], 1);
    let deleted = agent.ok(
        "delete_version",
        json!({ "version": "1.0", "project": "GK" }),
    );
    assert_eq!(deleted["tasks_changed"], json!(["GK-1"]));
    agent.finish();
}
