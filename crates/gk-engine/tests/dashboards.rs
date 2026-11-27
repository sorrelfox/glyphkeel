// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards: making them, with the starter widgets or given ones, each
//! widget's value from the existing reports and searches, arranging and
//! changing widgets, names unique per project, undo, and queries that
//! follow a renamed status.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &TempDir, clock: &Arc<AtomicI64>) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::clone(clock);
    config.clock = Arc::new(move || clock.load(Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join("W")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fail(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).expect_err(op)
}

fn setup() -> (TempDir, Engine) {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(NOW));
    let e = open(&dir, &clock);
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(&e, "project.create", json!({ "name": "Yard", "key": "ZZ" }));
    for (project, title, due, priority) in [
        ("TM", "Late", Some("2026-11-20"), "high"),
        ("TM", "Soon", Some("2026-11-25"), "normal"),
        ("TM", "Later", Some("2027-01-10"), "low"),
        ("ZZ", "Elsewhere", Some("2026-11-21"), "urgent"),
    ] {
        let mut input = json!({ "project": project, "title": title, "priority": priority });
        if let Some(due) = due {
            input["due_date"] = json!(due);
        }
        call(&e, "task.create", input);
    }
    (dir, e)
}

fn keys(tasks: &Value) -> Vec<&str> {
    tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap())
        .collect()
}

#[test]
fn the_starter_dashboard_shows_counts_lists_bars_reports_and_activity() {
    let (_dir, e) = setup();
    let made = call(
        &e,
        "dashboard.create",
        json!({ "name": "Overview", "starter": true }),
    );
    let widgets = made["dashboard"]["widgets"].as_array().unwrap();
    let kinds: Vec<&str> = widgets
        .iter()
        .map(|w| w["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "query_count",
            "overdue",
            "due_soon",
            "tasks_by_field",
            "created_closed",
            "activity"
        ]
    );
    // No title is stored, so each program shows its own in its language.
    assert!(widgets.iter().all(|w| w["title"].is_null()), "{widgets:?}");
    assert_eq!(widgets[4]["width"], 2, "charts start two columns wide");
    assert_eq!(made["dashboard"]["created_by"]["kind"], "user");

    let shown = call(&e, "dashboard.show", json!({ "dashboard": "overview" }));
    let values = shown["widgets"].as_array().unwrap();
    assert_eq!(values[0]["count"], 4);
    assert_eq!(values[0]["query"], "is:open");
    assert!(values[0].get("tasks").is_none(), "a count lists no tasks");
    assert_eq!(
        keys(&values[1]["tasks"]),
        ["TM-1", "ZZ-1"],
        "overdue, oldest first"
    );
    assert_eq!(keys(&values[2]["tasks"]), ["TM-2"], "due within a week");
    let bars = values[3]["bars"].as_array().unwrap();
    assert_eq!(bars[0]["value"], "Backlog");
    assert_eq!(bars[0]["count"], 4);
    assert_eq!(bars[0]["query"], "is:open status:Backlog");
    assert_eq!(values[4]["created_closed"]["created"], 4);
    assert!(values[5]["activity"].as_array().unwrap().len() >= 4);

    // One widget alone, by its place.
    let one = call(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Overview", "widget": "3" }),
    );
    assert_eq!(keys(&one["tasks"]), ["TM-2"]);
}

#[test]
fn a_project_dashboard_limits_its_widgets_to_the_project() {
    let (_dir, e) = setup();
    call(
        &e,
        "dashboard.create",
        json!({ "name": "Team", "project": "TM", "widgets": [
            { "kind": "overdue" },
            { "kind": "tasks_by_field", "settings": { "field": "priority" } },
            { "kind": "query_list", "settings": { "query": "is:open sort:title", "limit": 2 } },
            { "kind": "burndown" },
            { "kind": "version_progress" },
            { "kind": "note", "settings": { "text": "# Hello" } },
            { "kind": "time_report", "settings": { "days": 30 } },
            { "kind": "automation_failures" },
            { "kind": "cumulative_flow", "settings": { "bucket": "day" } },
            { "kind": "cycle_time" }
        ] }),
    );
    // Same name, other scope: allowed.
    call(&e, "dashboard.create", json!({ "name": "Team" }));
    let shown = call(
        &e,
        "dashboard.show",
        json!({ "dashboard": "Team", "project": "TM" }),
    );
    let v = shown["widgets"].as_array().unwrap();
    assert_eq!(keys(&v[0]["tasks"]), ["TM-1"]);
    assert_eq!(
        v[0]["query"],
        "project:TM/* is:open due<today sort:due,-priority"
    );
    let bars: Vec<(&str, u64)> = v[1]["bars"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["value"].as_str().unwrap(), b["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(bars, [("high", 1), ("normal", 1), ("low", 1)]);
    assert_eq!(keys(&v[2]["tasks"]), ["TM-1", "TM-3"]);
    assert_eq!(v[2]["count"], 3);
    // No active sprint: the burndown says so, the rest still show.
    assert_eq!(v[3]["error"]["code"], "not_found");
    assert_eq!(v[4]["versions"], json!([]));
    assert_eq!(v[5]["text"], "# Hello");
    assert_eq!(v[6]["time"]["total_minutes"], 0);
    assert_eq!(v[7]["count"], 0);
    assert_eq!(v[8]["cumulative_flow"]["bucket"], "day");
    assert_eq!(v[9]["cycle_time"]["lead"]["count"], 0);

    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "One", "tasks": ["TM-1", "TM-2"] }),
    );
    call(
        &e,
        "sprint.start",
        json!({ "sprint": "One", "project": "TM" }),
    );
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.0" }),
    );
    let burndown = call(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Team", "project": "TM", "widget": "4" }),
    );
    assert_eq!(burndown["burndown"]["sprint"]["name"], "One");
    assert_eq!(burndown["burndown"]["days"][0]["open_tasks"], 2);
    let versions = call(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Team", "project": "TM", "widget": "5" }),
    );
    assert_eq!(versions["versions"][0]["name"], "1.0");

    let listed = call(&e, "dashboard.list", json!({ "project": "TM" }));
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["project"], "TM");
    let all = call(&e, "dashboard.list", json!({ "all": true }));
    assert_eq!(all["total"], 2);
    assert_eq!(
        all["items"][0].get("project"),
        None,
        "the workspace's first"
    );
}

#[test]
fn widgets_are_added_moved_resized_changed_removed_and_undone() {
    let (_dir, e) = setup();
    call(&e, "dashboard.create", json!({ "name": "Mine" }));
    let add = |kind: &str, settings: Value| {
        call(
            &e,
            "dashboard.add_widget",
            json!({ "dashboard": "Mine", "widget": { "kind": kind, "settings": settings } }),
        )
    };
    let first = add("query_count", json!({ "query": "is:open" }));
    let first_id = first["widget"]["id"].as_str().unwrap().to_owned();
    add("note", json!({ "text": "Read me" }));
    let third = add("overdue", json!({}));
    assert_eq!(third["dashboard"]["widgets"].as_array().unwrap().len(), 3);

    // Move the note first, make the count wider, and change its query.
    let moved = call(
        &e,
        "dashboard.update_widget",
        json!({ "dashboard": "Mine", "widget": "2", "position": 1 }),
    );
    assert_eq!(moved["dashboard"]["widgets"][0]["kind"], "note");
    let changed = call(
        &e,
        "dashboard.update_widget",
        json!({ "dashboard": "Mine", "widget": first_id, "width": 3, "title": "Bugs",
                "settings": { "query": "is:open priority:high" } }),
    );
    assert_eq!(changed["widget"]["width"], 3);
    assert_eq!(changed["widget"]["title"], "Bugs");
    let value = call(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Mine", "widget": first_id }),
    );
    assert_eq!(value["count"], 1);
    // Clearing a setting.
    let cleared = call(
        &e,
        "dashboard.update_widget",
        json!({ "dashboard": "Mine", "widget": first_id, "clear": ["query"] }),
    );
    assert!(cleared["widget"]["settings"].get("query").is_none());

    // Removing, then undoing it.
    call(
        &e,
        "dashboard.remove_widget",
        json!({ "dashboard": "Mine", "widget": "3" }),
    );
    let after = call(&e, "dashboard.list", json!({}));
    assert_eq!(after["items"][0]["widgets"].as_array().unwrap().len(), 2);
    call(&e, "history.undo", json!({}));
    let back = call(&e, "dashboard.list", json!({}));
    assert_eq!(back["items"][0]["widgets"].as_array().unwrap().len(), 3);

    // Rename, then delete and undo the deletion.
    call(
        &e,
        "dashboard.update",
        json!({ "dashboard": "Mine", "name": "Ours", "description": "Shared" }),
    );
    let gone = call(&e, "dashboard.delete", json!({ "dashboard": "Ours" }));
    assert_eq!(gone["deleted"], true);
    assert_eq!(call(&e, "dashboard.list", json!({}))["total"], 0);
    call(&e, "history.undo", json!({}));
    let restored = call(&e, "dashboard.list", json!({}));
    assert_eq!(restored["items"][0]["name"], "Ours");
    assert_eq!(restored["items"][0]["description"], "Shared");
}

#[test]
fn bad_input_is_refused_with_its_place() {
    let (_dir, e) = setup();
    call(&e, "dashboard.create", json!({ "name": "Mine" }));
    let taken = fail(&e, "dashboard.create", json!({ "name": "MINE" }));
    assert_eq!(taken.code, ErrorCode::Conflict);
    let unused = fail(
        &e,
        "dashboard.add_widget",
        json!({ "dashboard": "Mine", "widget": { "kind": "note", "settings": { "query": "x" } } }),
    );
    assert_eq!(unused.code, ErrorCode::Invalid);
    assert_eq!(unused.details["path"], "widget.settings.query");
    let bad_query = fail(
        &e,
        "dashboard.create",
        json!({ "name": "Other", "widgets": [
            { "kind": "query_count", "settings": { "query": "status:Nowhere" } }
        ] }),
    );
    assert_eq!(bad_query.code, ErrorCode::Invalid);
    assert_eq!(bad_query.details["path"], "widgets[0].settings.query");
    let no_project = fail(
        &e,
        "dashboard.add_widget",
        json!({ "dashboard": "Mine", "widget": { "kind": "burndown" } }),
    );
    assert_eq!(no_project.details["path"], "widget.settings.project");
    let no_field = fail(
        &e,
        "dashboard.add_widget",
        json!({ "dashboard": "Mine", "widget": { "kind": "tasks_by_field",
                "settings": { "field": "colour" } } }),
    );
    assert_eq!(no_field.code, ErrorCode::Invalid);
    let wide = fail(
        &e,
        "dashboard.add_widget",
        json!({ "dashboard": "Mine", "widget": { "kind": "overdue", "width": 4 } }),
    );
    assert_eq!(wide.details["path"], "widget.width");
    let missing = fail(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Mine", "widget": "1" }),
    );
    assert_eq!(missing.code, ErrorCode::NotFound);
    let nowhere = fail(&e, "dashboard.show", json!({ "dashboard": "Nope" }));
    assert_eq!(nowhere.code, ErrorCode::NotFound);
}

#[test]
fn widget_queries_follow_a_renamed_status_of_their_project() {
    let (_dir, e) = setup();
    call(
        &e,
        "dashboard.create",
        json!({ "name": "Mine", "widgets": [
            { "kind": "query_count", "settings": { "query": "status:Backlog", "project": "ZZ" } }
        ] }),
    );
    call(
        &e,
        "status.update",
        json!({ "project": "ZZ", "status": "Backlog", "name": "Waiting" }),
    );
    let listed = call(&e, "dashboard.list", json!({}));
    assert_eq!(
        listed["items"][0]["widgets"][0]["settings"]["query"],
        "status:Waiting"
    );
    assert_eq!(
        listed["items"][0]["widgets"][0]["settings"]["project"],
        "ZZ"
    );
    let value = call(
        &e,
        "dashboard.widget",
        json!({ "dashboard": "Mine", "widget": "1" }),
    );
    assert_eq!(value["count"], 1);
}
