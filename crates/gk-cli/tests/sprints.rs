// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Sprints from the command line and through the agent server, each its
//! own process against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Agent, Workspace};
use serde_json::json;

#[test]
fn sprints_are_planned_started_and_completed_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    for title in ["Engine", "Screen"] {
        ws.ok(&["-q", "task", "create", "--project", "GK", "--title", title]);
    }
    ws.ok(&[
        "-q",
        "sprint",
        "create",
        "--project",
        "GK",
        "--name",
        "Sprint 1",
        "--goal",
        "First cut",
    ]);
    ws.ok(&[
        "-q",
        "sprint",
        "create",
        "--project",
        "GK",
        "--name",
        "Sprint 2",
    ]);
    let added = ws.json(&[
        "sprint",
        "add",
        "Sprint 1",
        "--project",
        "GK",
        "--tasks",
        "GK-1",
        "--tasks",
        "GK-2",
    ]);
    assert_eq!(added["tasks_changed"], json!(["GK-1", "GK-2"]));
    let started = ws.json(&[
        "sprint",
        "start",
        "Sprint 1",
        "--project",
        "GK",
        "--end-date",
        "+1w",
    ]);
    assert_eq!(started["sprint"]["state"], "active");
    let found = ws.json(&["task", "list", "--query", "sprint:active"]);
    assert_eq!(found["total"], 2);
    let completed = ws.json(&[
        "sprint",
        "complete",
        "active",
        "--project",
        "GK",
        "--move-open-tasks-to",
        "Sprint 2",
    ]);
    assert_eq!(completed["tasks_changed"], json!(["GK-1", "GK-2"]));
    let shown = ws.json(&["sprint", "show", "Sprint 2", "--project", "GK"]);
    assert_eq!(shown["tasks"][0]["key"], "GK-1");
    let burndown = ws.json(&["sprint", "burndown", "Sprint 1", "--project", "GK"]);
    assert_eq!(burndown["days"][0]["open_tasks"], 2);
    // For people: one row per sprint.
    let text = ws.ok(&["sprint", "list", "GK"]);
    assert!(
        text.contains("Sprint 1") && text.contains("Sprint 2"),
        "{text}"
    );
}

#[test]
fn agents_plan_start_and_complete_sprints() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Engine",
    ]);
    let mut agent = Agent::start(ws.home(), &ws.path(), "tester");
    let made = agent.ok(
        "create_sprint",
        json!({ "project": "GK", "name": "Sprint 1", "tasks": ["GK-1"] }),
    );
    assert_eq!(made["sprint"]["state"], "planned");
    agent.ok(
        "start_sprint",
        json!({ "sprint": "Sprint 1", "project": "GK" }),
    );
    let task = agent.ok(
        "create_task",
        json!({ "project": "GK", "title": "Docs", "sprint": "active" }),
    );
    assert_eq!(task["task"]["sprint"]["name"], "Sprint 1");
    let list = agent.ok(
        "list_sprints",
        json!({ "project": "GK", "include_tasks": true }),
    );
    assert_eq!(list["items"][0]["tasks"][1]["key"], "GK-2");
    let removed = agent.ok("remove_tasks_from_sprint", json!({ "tasks": ["GK-2"] }));
    assert_eq!(removed["tasks_changed"], json!(["GK-2"]));
    let burndown = agent.ok(
        "get_sprint_burndown",
        json!({ "sprint": "active", "project": "GK" }),
    );
    assert_eq!(burndown["sprint"]["name"], "Sprint 1");
    let done = agent.ok(
        "complete_sprint",
        json!({ "sprint": "active", "project": "GK" }),
    );
    assert_eq!(done["tasks_changed"], json!(["GK-1"]));
    agent.finish();
}
