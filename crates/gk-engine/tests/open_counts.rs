// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The open-task counts the sidebar shows: a project's own, its total with
//! its sub-projects, and a folder's total. Tasks in a done or cancelled
//! status and tasks in the trash are left out, and the counts follow a task
//! that is closed or reopened.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &TempDir) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::new(AtomicI64::new(NOW));
    config.clock = Arc::new(move || clock.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join("W")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

/// The ID of the project's first status in `category`.
fn status(e: &Engine, project: &str, category: &str) -> String {
    let p = call(e, "project.get", json!({ "project": project }));
    p["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["category"] == category)
        .unwrap_or_else(|| panic!("{project} has no {category} status"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A new project; a top-level one gets a cancelled status beside the
/// default ones, and sub-projects take their parent's statuses.
fn project(e: &Engine, name: &str, key: &str, parent: Option<&str>) {
    let mut input = json!({ "name": name, "key": key });
    if let Some(parent) = parent {
        input["parent"] = json!(parent);
    }
    call(e, "project.create", input);
    if parent.is_none() {
        call(
            e,
            "status.create",
            json!({ "project": key, "name": "Dropped", "category": "cancelled" }),
        );
    }
}

fn task(e: &Engine, project: &str, title: &str) -> String {
    let made = call(
        e,
        "task.create",
        json!({ "project": project, "title": title }),
    );
    made["task"]["key"].as_str().unwrap().to_owned()
}

fn set_status(e: &Engine, key: &str, status: &str) {
    call(e, "task.move", json!({ "key": key, "status": status }));
}

/// `(open_tasks, open_tasks_with_subprojects)` of a project in the tree.
fn listed(e: &Engine, key: &str) -> (u64, u64) {
    let list = call(e, "project.list", json!({ "include_archived": true }));
    let p = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == key)
        .unwrap_or_else(|| panic!("{key} is not listed"))
        .clone();
    (
        p["open_tasks"].as_u64().unwrap(),
        p["open_tasks_with_subprojects"].as_u64().unwrap(),
    )
}

fn folder_open(e: &Engine, name: &str) -> u64 {
    let list = call(e, "folder.list", json!({}));
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == name)
        .unwrap()["open_tasks"]
        .as_u64()
        .unwrap()
}

#[test]
fn ten_tasks_with_three_done_count_seven_and_cancelled_and_trashed_ones_are_left_out() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "Harbour", "HB", None);
    let keys: Vec<String> = (1..=10)
        .map(|n| task(&e, "HB", &format!("Task {n}")))
        .collect();
    let done = status(&e, "HB", "done");
    let cancelled = status(&e, "HB", "cancelled");
    let todo = status(&e, "HB", "todo");
    let doing = status(&e, "HB", "in_progress");
    for key in &keys[..3] {
        set_status(&e, key, &done);
    }
    // One in progress is still open.
    set_status(&e, &keys[3], &doing);
    assert_eq!(listed(&e, "HB"), (7, 7));
    let project = call(&e, "project.get", json!({ "project": "HB" }));
    assert_eq!(project["open_tasks"], 7);
    assert_eq!(project["total_tasks"], 10);

    // A cancelled task is closed too.
    set_status(&e, &keys[4], &cancelled);
    assert_eq!(listed(&e, "HB"), (6, 6));

    // A task in the trash isn't counted, and comes back when restored.
    call(&e, "task.delete", json!({ "keys": [keys[5]] }));
    assert_eq!(listed(&e, "HB"), (5, 5));
    let project = call(&e, "project.get", json!({ "project": "HB" }));
    assert_eq!(project["open_tasks"], 5);
    assert_eq!(project["total_tasks"], 9);
    call(&e, "history.undo", json!({}));
    assert_eq!(listed(&e, "HB"), (6, 6));

    // Reopening raises the count again, closing lowers it.
    set_status(&e, &keys[0], &todo);
    assert_eq!(listed(&e, "HB"), (7, 7));
    set_status(&e, &keys[0], &done);
    assert_eq!(listed(&e, "HB"), (6, 6));
}

#[test]
fn sub_project_and_folder_totals_add_up_open_tasks_only() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "Harbour", "HB", None);
    project(&e, "Docks", "DK", Some("HB"));
    project(&e, "Old pier", "OP", Some("HB"));
    call(&e, "folder.create", json!({ "name": "Coast" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "HB", "folder": "Coast" }),
    );
    task(&e, "HB", "Paint the hull");
    let shut = task(&e, "HB", "Order rope");
    task(&e, "DK", "Fix crane");
    let gone = task(&e, "DK", "Sweep");
    let dropped = task(&e, "DK", "Move the bollard");
    task(&e, "OP", "Count planks");
    set_status(&e, &shut, &status(&e, "HB", "done"));
    set_status(&e, &dropped, &status(&e, "DK", "cancelled"));
    call(&e, "task.delete", json!({ "keys": [gone] }));

    assert_eq!(listed(&e, "DK"), (1, 1));
    assert_eq!(listed(&e, "OP"), (1, 1));
    assert_eq!(listed(&e, "HB"), (1, 3));
    assert_eq!(folder_open(&e, "Coast"), 3);

    // An archived sub-project is shown apart, so the parent's and the
    // folder's totals leave it out; its own row still counts its tasks.
    call(&e, "project.archive", json!({ "project": "OP" }));
    assert_eq!(listed(&e, "HB"), (1, 2));
    assert_eq!(listed(&e, "OP"), (1, 1));
    assert_eq!(folder_open(&e, "Coast"), 2);
}
