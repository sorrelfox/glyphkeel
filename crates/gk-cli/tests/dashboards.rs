// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards from the command line and through the agent server, each its
//! own process against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Agent, Workspace};
use serde_json::json;

#[test]
fn dashboards_are_made_arranged_and_shown_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    for title in ["Engine", "Screen"] {
        ws.ok(&["-q", "task", "create", "--project", "GK", "--title", title]);
    }
    let made = ws.json(&["dashboard", "create", "--name", "Team", "--starter", "true"]);
    assert_eq!(made["dashboard"]["widgets"].as_array().unwrap().len(), 6);
    let added = ws.json(&[
        "dashboard",
        "add-widget",
        "Team",
        "--widget",
        r##"{"kind": "note", "settings": {"text": "Ship it"}}"##,
        "--position",
        "1",
    ]);
    assert_eq!(added["dashboard"]["widgets"][0]["kind"], "note");
    let resized = ws.json(&["dashboard", "edit-widget", "Team", "1", "--width", "3"]);
    assert_eq!(resized["widget"]["width"], 3);
    ws.ok(&["-q", "dashboard", "remove-widget", "Team", "7"]);

    let shown = ws.json(&["dashboard", "show", "Team"]);
    assert_eq!(shown["widgets"][1]["count"], 2);
    let one = ws.json(&["dashboard", "widget", "Team", "2"]);
    assert_eq!(one["query"], "is:open");

    // For people: a heading per widget and its figures.
    let text = ws.ok(&["dashboard", "show", "Team"]);
    assert!(text.starts_with("Team\n"), "{text}");
    assert!(text.contains("1. Note\n  Ship it"), "{text}");
    assert!(text.contains("2. Open tasks\n  2 tasks"), "{text}");
    let list = ws.ok(&["dashboard", "list"]);
    assert!(list.contains("Team"), "{list}");
}

#[test]
fn agents_read_widget_values_and_edit_dashboards() {
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
        "--priority",
        "high",
    ]);
    let mut agent = Agent::start(ws.home(), &ws.path(), "tester");
    let made = agent.ok(
        "create_dashboard",
        json!({ "name": "Bugs", "project": "GK", "widgets": [
            { "kind": "query_count", "title": "High", "settings": { "query": "priority:high" } }
        ] }),
    );
    assert_eq!(made["dashboard"]["created_by"]["kind"], "agent");
    agent.ok(
        "add_dashboard_widget",
        json!({ "dashboard": "Bugs", "project": "GK",
                "widget": { "kind": "tasks_by_field", "settings": { "field": "type" } } }),
    );
    let shown = agent.ok(
        "get_dashboard",
        json!({ "dashboard": "Bugs", "project": "GK" }),
    );
    assert_eq!(shown["widgets"][0]["count"], 1);
    assert_eq!(shown["widgets"][1]["bars"][0]["count"], 1);
    let widget = agent.ok(
        "get_dashboard_widget",
        json!({ "dashboard": "Bugs", "project": "GK", "widget": "1" }),
    );
    assert_eq!(widget["query"], "project:GK/* priority:high");
    agent.ok(
        "update_dashboard_widget",
        json!({ "dashboard": "Bugs", "project": "GK", "widget": "2", "position": 1 }),
    );
    agent.ok(
        "remove_dashboard_widget",
        json!({ "dashboard": "Bugs", "project": "GK", "widget": "2" }),
    );
    agent.ok(
        "update_dashboard",
        json!({ "dashboard": "Bugs", "project": "GK", "name": "Bug watch" }),
    );
    let listed = agent.ok("list_dashboards", json!({ "all": true }));
    assert_eq!(listed["items"][0]["name"], "Bug watch");
    assert_eq!(listed["items"][0]["widgets"][0]["kind"], "tasks_by_field");
    agent.ok(
        "delete_dashboard",
        json!({ "dashboard": "Bug watch", "project": "GK" }),
    );
    agent.finish();
}
