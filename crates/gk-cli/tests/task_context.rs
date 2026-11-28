// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel task context <key>`: a task's whole context for scripts, with
//! the brief, the parent and subtasks, and the changes agents proposed to
//! it that wait for the person.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Agent, Workspace};
use serde_json::json;

#[test]
fn the_context_of_a_task_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["-q", "project", "create", "--name", "Garden", "--key", "GA"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GA",
        "--title",
        "Spring",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GA",
        "--title",
        "Plant",
        "--parent",
        "GA-1",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GA",
        "--title",
        "Bulbs",
        "--parent",
        "GA-2",
    ]);
    ws.ok(&[
        "-q",
        "project",
        "edit",
        "GA",
        "--description",
        "Water after planting.",
    ]);
    ws.ok(&[
        "-q",
        "agents",
        "policy",
        "--max-profile",
        "standard",
        "--approvals",
        r#"{"read_only":"off","standard":"all_writes","full":"off","expire_days":14}"#,
    ]);
    let mut agent = Agent::start(ws.home(), &ws.path(), "planner");
    let proposed = agent.call(
        "update_task",
        json!({ "key": "GA-2", "title": "Plant tulips" }),
    );
    assert_eq!(proposed["isError"], true, "{proposed}");

    let context = ws.json(&["task", "context", "GA-2"]);
    assert_eq!(context["task"]["key"], "GA-2");
    assert_eq!(context["parents"][0]["key"], "GA-1");
    assert_eq!(context["parents"].as_array().unwrap().len(), 1);
    assert_eq!(context["subtasks"][0]["task"]["key"], "GA-3");
    assert_eq!(context["brief"]["source"], "description");
    assert_eq!(context["brief"]["body"], "Water after planting.");
    assert_eq!(context["proposals"][0]["op"], "task.update", "{context}");
    assert_eq!(context["proposals"][0]["agent"]["name"], "planner");
    assert_eq!(context["trimmed"]["comments"], 0);

    let none = ws.json(&["task", "context", "GA-2", "--depth", "0"]);
    assert!(none["subtasks"].as_array().unwrap().is_empty());

    let text = ws.ok(&["task", "context", "GA-2"]);
    assert!(text.contains("Water after planting."), "{text}");
}
