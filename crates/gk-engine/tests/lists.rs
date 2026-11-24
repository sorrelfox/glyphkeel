// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The list's groups and the person's own list layout, over a real
//! workspace, and what of the list survives reopening the workspace.

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

fn open(dir: &TempDir) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(|| NOW);
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn keys(out: &Value) -> Vec<String> {
    out["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

/// The groups of a result as `(value, title, count)`.
fn groups(out: &Value) -> Vec<(Option<String>, Option<String>, u64)> {
    out["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["value"].as_str().map(str::to_owned),
                g["title"].as_str().map(str::to_owned),
                g["count"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn s(text: &str) -> Option<String> {
    Some(text.to_owned())
}

/// Project TM with labels and six tasks: two subtasks of TM-1, due dates in
/// three weeks, several statuses and priorities. Project YARD has one task.
fn data_set(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(
        e,
        "project.create",
        json!({ "name": "Yard", "key": "YARD" }),
    );
    for l in ["bug", "ui", "docs"] {
        call(e, "label.create", json!({ "project": "TM", "name": l }));
    }
    let tasks = [
        // TM-1
        json!({ "title": "Login fails", "priority": "high", "labels": ["ui", "bug"],
                "due_date": "2026-11-24" }),
        // TM-2, a subtask of TM-1
        json!({ "title": "Reproduce it", "parent": "TM-1", "priority": "urgent",
                "due_date": "2026-11-29" }),
        // TM-3, a subtask of TM-1
        json!({ "title": "Fix it", "parent": "TM-1", "labels": ["ui"],
                "due_date": "2026-11-30" }),
        // TM-4
        json!({ "title": "Write the guide", "labels": ["docs"], "due_date": "2026-12-09" }),
        // TM-5
        json!({ "title": "Tidy up", "priority": "low" }),
        // TM-6
        json!({ "title": "Release", "priority": "high" }),
    ];
    for t in tasks {
        let mut t = t;
        t["project"] = json!("TM");
        call(e, "task.create", t);
    }
    call(
        e,
        "task.move",
        json!({ "key": "TM-1", "status": "In progress" }),
    );
    call(e, "task.move", json!({ "key": "TM-6", "status": "Done" }));
    call(e, "task.move", json!({ "key": "TM-4", "status": "To do" }));
    call(
        e,
        "task.create",
        json!({ "project": "YARD", "title": "New front page", "priority": "high" }),
    );
}

fn grouped(e: &Engine, by: &str, extra: Value) -> Value {
    let mut input = json!({ "query": "project:TM", "group_by": by, "limit": 200 });
    if let (Value::Object(i), Value::Object(x)) = (&mut input, extra) {
        i.extend(x);
    }
    call(e, "task.search", input)
}

#[test]
fn a_list_is_grouped_by_status_priority_label_parent_due_week_and_project() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    data_set(&e);

    // Status, in board order; each group's rows in the query's order.
    let out = grouped(&e, "status", json!({ "sort": "-key" }));
    assert_eq!(
        groups(&out),
        [
            (s("Backlog"), None, 3),
            (s("To do"), None, 1),
            (s("In progress"), None, 1),
            (s("Done"), None, 1),
        ]
    );
    assert_eq!(keys(&out), ["TM-5", "TM-3", "TM-2", "TM-4", "TM-1", "TM-6"]);
    assert!(out["groups"][0]["hue"].is_string(), "statuses have colours");
    assert_eq!(
        out["sort"],
        json!(["-key"]),
        "the sort keys are the query's"
    );

    // Priority, highest first.
    let out = grouped(&e, "priority", json!({}));
    assert_eq!(
        groups(&out),
        [
            (s("urgent"), None, 1),
            (s("high"), None, 2),
            (s("low"), None, 1),
            (s("none"), None, 2),
        ]
    );
    assert_eq!(&keys(&out)[..3], ["TM-2", "TM-1", "TM-6"]);

    // Label: the first label in alphabetical order; tasks without one last.
    let out = grouped(&e, "label", json!({ "sort": "key" }));
    assert_eq!(
        groups(&out),
        [
            (s("bug"), None, 1),
            (s("docs"), None, 1),
            (s("ui"), None, 1),
            (None, None, 3),
        ]
    );
    assert_eq!(keys(&out), ["TM-1", "TM-4", "TM-3", "TM-2", "TM-5", "TM-6"]);

    // Parent, with its title; tasks without one last.
    let out = grouped(&e, "parent", json!({ "sort": "key" }));
    assert_eq!(
        groups(&out),
        [(s("TM-1"), s("Login fails"), 2), (None, None, 4)]
    );
    assert_eq!(keys(&out), ["TM-2", "TM-3", "TM-1", "TM-4", "TM-5", "TM-6"]);

    // Due week, starting on the workspace's first day (Monday by default).
    let out = grouped(&e, "due_week", json!({ "sort": "key" }));
    assert_eq!(
        groups(&out),
        [
            (s("2026-11-23"), None, 2),
            (s("2026-11-30"), None, 1),
            (s("2026-12-07"), None, 1),
            (None, None, 2),
        ]
    );
    assert_eq!(&keys(&out)[..4], ["TM-1", "TM-2", "TM-3", "TM-4"]);
    call(&e, "settings.update", json!({ "week_start": "sunday" }));
    let out = grouped(&e, "due_week", json!({ "sort": "key" }));
    assert_eq!(
        groups(&out)[..3],
        [
            (s("2026-11-22"), None, 1),
            (s("2026-11-29"), None, 2),
            (s("2026-12-06"), None, 1),
        ]
    );

    // Project, across the workspace; paging keeps the groups' order.
    let out = call(
        &e,
        "task.search",
        json!({ "query": "", "group_by": "project", "limit": 2, "cursor": "5" }),
    );
    assert_eq!(
        groups(&out),
        [(s("TM"), s("Team"), 6), (s("YARD"), s("Yard"), 1)]
    );
    assert_eq!(keys(&out), ["TM-6", "YARD-1"]);

    // Without a grouping there are no groups.
    let out = call(&e, "task.search", json!({ "query": "project:TM" }));
    assert!(out.get("groups").is_none());
    let err = e
        .call(
            &Context::user(Via::App),
            "task.search",
            json!({ "query": "", "group_by": "colour" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn the_list_layout_is_kept_per_project_apart_from_views_and_can_be_undone() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    data_set(&e);

    let empty = call(&e, "list.layout.get", json!({ "project": "TM" }));
    assert_eq!(empty["layout"], json!({ "columns": [], "widths": {} }));
    let changed = call(
        &e,
        "list.layout.update",
        json!({ "project": "TM", "columns": ["title", "key", "due"], "widths": { "title": 320 } }),
    );
    assert_eq!(changed["project"], "TM");
    assert!(changed["group"].is_string());
    let more = call(
        &e,
        "list.layout.update",
        json!({ "project": "TM", "widths": { "due": 140 } }),
    );
    assert_eq!(
        more["layout"],
        json!({ "columns": ["title", "key", "due"], "widths": { "due": 140, "title": 320 } })
    );
    // Per project, and once for the whole workspace's list.
    assert_eq!(
        call(&e, "list.layout.get", json!({ "project": "YARD" }))["layout"]["columns"],
        json!([])
    );
    call(
        &e,
        "list.layout.update",
        json!({ "columns": ["key", "title", "project"] }),
    );
    assert_eq!(
        call(&e, "list.layout.get", json!({}))["layout"]["columns"],
        json!(["key", "title", "project"])
    );
    // Undo takes back the last change only.
    call(&e, "history.undo", json!({}));
    assert_eq!(
        call(&e, "list.layout.get", json!({}))["layout"]["columns"],
        json!([])
    );
    call(&e, "history.undo", json!({}));
    assert_eq!(
        call(&e, "list.layout.get", json!({ "project": "TM" }))["layout"]["widths"],
        json!({ "title": 320 })
    );
    // Widths are checked.
    let err = e
        .call(
            &Context::user(Via::App),
            "list.layout.update",
            json!({ "project": "TM", "widths": { "due": 5 } }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["message_id"], "list-layout-width");
    // A reset goes back to the default.
    let reset = call(
        &e,
        "list.layout.update",
        json!({ "project": "TM", "reset": true }),
    );
    assert_eq!(reset["layout"], json!({ "columns": [], "widths": {} }));
}

#[test]
fn saved_views_and_list_layouts_survive_reopening_the_workspace() {
    let dir = TempDir::new().unwrap();
    {
        let e = open(&dir);
        data_set(&e);
        call(
            &e,
            "view.create",
            json!({ "name": "Open by week", "project": "TM", "query": "is:open",
                    "sort": "due,-priority", "columns": ["key", "title", "due"],
                    "group_by": "due_week", "is_default": true }),
        );
        call(
            &e,
            "list.layout.update",
            json!({ "project": "TM", "columns": ["due", "title", "key"],
                    "widths": { "title": 360, "due": 120 } }),
        );
        e.close();
    }
    let e = open(&dir);
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    let view = &views["items"][0];
    assert_eq!(view["name"], "Open by week");
    assert_eq!(view["is_default"], true);
    assert_eq!(view["group_by"], "due_week");
    assert_eq!(view["columns"], json!(["key", "title", "due"]));
    assert_eq!(view["sort"], "due,-priority");
    let layout = call(&e, "list.layout.get", json!({ "project": "TM" }));
    assert_eq!(
        layout["layout"],
        json!({ "columns": ["due", "title", "key"], "widths": { "due": 120, "title": 360 } })
    );
    assert_eq!(
        call(&e, "list.layout.get", json!({ "project": "YARD" }))["layout"]["widths"],
        json!({})
    );
}

#[test]
fn a_saved_view_reads_its_relative_dates_each_time_it_runs() {
    let dir = TempDir::new().unwrap();
    let now = Arc::new(AtomicI64::new(NOW));
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::clone(&now);
    config.clock = Arc::new(move || clock.load(Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    let e = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    for (title, due) in [("Soon", "2026-11-24"), ("Later", "2026-11-28")] {
        call(
            &e,
            "task.create",
            json!({ "project": "TM", "title": title, "due_date": due }),
        );
    }
    let view = call(
        &e,
        "view.create",
        json!({ "name": "Due in two days", "project": "TM", "query": "due<=+2d" }),
    );
    let query = view["view"]["query"].as_str().unwrap().to_owned();
    assert_eq!(query, "due<=+2d", "the view keeps the words, not a date");
    let run = |e: &Engine| {
        keys(&call(
            e,
            "task.search",
            json!({ "query": query, "project": "TM" }),
        ))
    };
    assert_eq!(run(&e), ["TM-1"]);
    // Four days on, the same view finds the other task.
    now.store(NOW + 4 * 24 * 60 * 60 * 1000, Ordering::SeqCst);
    assert_eq!(run(&e), ["TM-1", "TM-2"]);
    let stored = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(stored["items"][0]["query"], "due<=+2d");
}
