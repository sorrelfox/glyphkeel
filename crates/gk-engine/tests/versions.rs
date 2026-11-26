// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Versions (releases): planning tasks for them, sharing with sub-projects,
//! closing one and moving its open tasks on, queries, renames in saved
//! views, undo, and export and import.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &TempDir, name: &str) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(|| NOW);
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join(name)), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn code(e: &Engine, op: &str, input: Value) -> ErrorCode {
    e.call(&Context::user(Via::App), op, input)
        .expect_err(op)
        .code
}

fn task(e: &Engine, key: &str) -> Value {
    call(e, "task.get", json!({ "keys": [key] }))["items"][0].clone()
}

fn version_of(e: &Engine, key: &str) -> Option<String> {
    task(e, key)["fix_version"]["name"]
        .as_str()
        .map(str::to_owned)
}

fn keys(out: &Value) -> Vec<String> {
    out["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn new_task(e: &Engine, project: &str, title: &str, extra: Value) -> String {
    let mut input = json!({ "project": project, "title": title });
    for (k, v) in extra.as_object().unwrap() {
        input[k] = v.clone();
    }
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// TM with sub-project TM-APP below it, and YARD beside them.
fn projects(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(
        e,
        "project.create",
        json!({ "name": "App", "key": "APP", "parent": "TM" }),
    );
    call(
        e,
        "project.create",
        json!({ "name": "Yard", "key": "YARD" }),
    );
}

#[test]
fn tasks_are_planned_for_open_versions_and_progress_is_weighted() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "W");
    projects(&e);
    let made = call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.0", "due_date": "2026-12-15",
                "description": "First release" }),
    );
    assert_eq!(made["version"]["status"], "open");
    assert_eq!(made["version"]["sharing"], "none");
    assert!(made["group"].is_string());
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "2.0", "start_date": "2027-01-04" }),
    );
    // Names are unique per project, ignoring case; dates keep their order.
    assert_eq!(
        code(
            &e,
            "version.create",
            json!({ "project": "TM", "name": "1.0" })
        ),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(
            &e,
            "version.create",
            json!({ "project": "TM", "name": "x", "start_date": "2026-12-02",
                    "due_date": "2026-12-01" })
        ),
        ErrorCode::Invalid
    );

    let a = new_task(
        &e,
        "TM",
        "Big",
        json!({ "fix_version": "1.0", "estimate": "3h" }),
    );
    let b = new_task(&e, "TM", "Small", json!({ "estimate": "1h" }));
    call(&e, "task.update", json!({ "key": b, "fix_version": "1.0" }));
    assert_eq!(version_of(&e, &a).as_deref(), Some("1.0"));
    assert_eq!(task(&e, &b)["fix_version"]["due_date"], "2026-12-15");
    // A version of another project is not offered.
    call(
        &e,
        "version.create",
        json!({ "project": "YARD", "name": "Y1" }),
    );
    assert_eq!(
        code(&e, "task.update", json!({ "key": b, "fix_version": "Y1" })),
        ErrorCode::NotFound
    );

    // The small task done: a quarter of the estimated work.
    call(&e, "task.move", json!({ "key": b, "status": "Done" }));
    let list = call(
        &e,
        "version.list",
        json!({ "project": "TM", "include_tasks": true }),
    );
    assert_eq!(list["total"], 2);
    let first = &list["items"][0];
    assert_eq!(first["name"], "1.0", "due dates first");
    assert_eq!(first["progress"]["total"], 2);
    assert_eq!(first["progress"]["closed"], 1);
    assert_eq!(first["progress"]["percent"], 25);
    assert_eq!(first["progress"]["estimate_minutes"], 240);
    assert_eq!(first["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(list["unplanned"], 0);

    // Locked: the tasks stay, no new ones.
    call(
        &e,
        "version.update",
        json!({ "version": "1.0", "project": "TM", "status": "locked" }),
    );
    assert_eq!(
        code(
            &e,
            "task.create",
            json!({ "project": "TM", "title": "Late", "fix_version": "1.0" })
        ),
        ErrorCode::Conflict
    );
    assert_eq!(version_of(&e, &a).as_deref(), Some("1.0"));
    call(
        &e,
        "task.update",
        json!({ "key": a, "clear": ["fix_version"] }),
    );
    assert_eq!(version_of(&e, &a), None);
}

#[test]
fn shared_versions_reach_sub_projects_until_the_sharing_or_the_tree_changes() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "W");
    projects(&e);
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.0", "sharing": "descendants" }),
    );
    call(
        &e,
        "version.create",
        json!({ "project": "APP", "name": "app-1" }),
    );
    let listed = call(&e, "version.list", json!({ "project": "APP" }));
    let names: Vec<(&str, bool)> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["name"].as_str().unwrap(), v["inherited"] == true))
        .collect();
    assert_eq!(names, [("1.0", true), ("app-1", false)]);
    let own = call(
        &e,
        "version.list",
        json!({ "project": "APP", "own_only": true }),
    );
    assert_eq!(own["total"], 1);

    let a = new_task(&e, "APP", "Shared", json!({ "fix_version": "1.0" }));
    let b = new_task(&e, "APP", "Moved", json!({ "fix_version": "1.0" }));
    assert_eq!(version_of(&e, &a).as_deref(), Some("1.0"));

    // A task moved to a project the version isn't shared with loses it.
    let moved = call(&e, "task.move", json!({ "key": b, "project": "YARD" }));
    let moved_key = moved["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(version_of(&e, &moved_key), None);
    assert!(
        moved["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("1.0")),
        "{moved}"
    );

    // No longer shared: the sub-project's task loses it, in the same change.
    let changed = call(
        &e,
        "version.update",
        json!({ "version": "1.0", "project": "TM", "sharing": "none" }),
    );
    assert_eq!(changed["tasks_changed"], json!([a]));
    assert_eq!(version_of(&e, &a), None);
    call(&e, "history.undo", json!({}));
    assert_eq!(version_of(&e, &a).as_deref(), Some("1.0"));

    // The sub-project moving out of TM loses it too.
    call(
        &e,
        "project.move",
        json!({ "project": "APP", "top_level": true }),
    );
    assert_eq!(version_of(&e, &a), None);
}

#[test]
fn closing_a_version_moves_its_open_tasks_and_undoes_in_one_step() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "W");
    projects(&e);
    for name in ["1.0", "1.1"] {
        call(
            &e,
            "version.create",
            json!({ "project": "TM", "name": name }),
        );
    }
    let done = new_task(&e, "TM", "Done", json!({ "fix_version": "1.0" }));
    let open = new_task(&e, "TM", "Open", json!({ "fix_version": "1.0" }));
    call(&e, "task.move", json!({ "key": done, "status": "Done" }));

    // Moving on needs the version to close.
    assert_eq!(
        code(
            &e,
            "version.update",
            json!({ "version": "1.0", "project": "TM", "move_open_tasks_to": "1.1" })
        ),
        ErrorCode::Invalid
    );
    let closed = call(
        &e,
        "version.update",
        json!({ "version": "1.0", "project": "TM", "status": "closed",
                "move_open_tasks_to": "1.1" }),
    );
    assert_eq!(closed["version"]["status"], "closed");
    assert_eq!(closed["tasks_changed"], json!([open]));
    assert_eq!(version_of(&e, &done).as_deref(), Some("1.0"));
    assert_eq!(version_of(&e, &open).as_deref(), Some("1.1"));
    // A closed version takes no tasks.
    assert_eq!(
        code(
            &e,
            "task.update",
            json!({ "key": open, "fix_version": "1.0" })
        ),
        ErrorCode::Conflict
    );

    call(&e, "history.undo", json!({}));
    let v = call(
        &e,
        "version.get",
        json!({ "version": "1.0", "project": "TM" }),
    );
    assert_eq!(v["status"], "open");
    assert_eq!(v["progress"]["total"], 2);
    assert_eq!(version_of(&e, &open).as_deref(), Some("1.0"));
}

#[test]
fn queries_find_versions_and_saved_views_follow_renames_and_deletions() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "W");
    projects(&e);
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.0", "due_date": "2026-12-01" }),
    );
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.1", "due_date": "2026-12-20" }),
    );
    let a = new_task(&e, "TM", "A", json!({ "fix_version": "1.1" }));
    let b = new_task(&e, "TM", "B", json!({ "fix_version": "1.0" }));
    let c = new_task(&e, "TM", "C", json!({}));

    let search = |q: &str| keys(&call(&e, "task.search", json!({ "query": q })));
    assert_eq!(search("version:1.0"), std::slice::from_ref(&b));
    assert_eq!(search("version:none project:TM"), std::slice::from_ref(&c));
    assert_eq!(search("has:version sort:version"), [b.clone(), a.clone()]);
    assert_eq!(
        search("-version:1.1 project:TM sort:key"),
        [b.clone(), c.clone()]
    );
    assert_eq!(
        code(&e, "task.search", json!({ "query": "version:9.9" })),
        ErrorCode::Invalid
    );

    call(
        &e,
        "view.create",
        json!({ "name": "Next", "project": "TM", "query": "version:1.1 is:open" }),
    );
    let renamed = call(
        &e,
        "version.update",
        json!({ "version": "1.1", "project": "TM", "name": "1.1 beta" }),
    );
    assert_eq!(renamed["views_rewritten"], json!(["Next"]));
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "version:\"1.1 beta\" is:open");
    assert_eq!(search("version:\"1.1 beta\""), std::slice::from_ref(&a));

    // Deleting it plans its tasks for none and takes it out of views; undo
    // brings everything back.
    let deleted = call(
        &e,
        "version.delete",
        json!({ "version": "1.1 beta", "project": "TM" }),
    );
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["tasks_changed"], json!([a]));
    assert_eq!(version_of(&e, &a), None);
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "is:open");
    call(&e, "history.undo", json!({}));
    assert_eq!(version_of(&e, &a).as_deref(), Some("1.1 beta"));
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "version:\"1.1 beta\" is:open");

    // Undoing a version's creation while tasks use it is refused.
    let made = call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "3.0" }),
    );
    call(&e, "task.update", json!({ "key": c, "fix_version": "3.0" }));
    let refused = e.call(
        &Context::user(Via::App),
        "history.revert",
        json!({ "groups": [made["group"]] }),
    );
    let refused = refused.map_or_else(
        |err| err.code == ErrorCode::Conflict,
        |out| out["skipped"].as_array().is_some_and(|s| !s.is_empty()),
    );
    assert!(refused, "the creation of a version in use is not reverted");
    assert_eq!(version_of(&e, &c).as_deref(), Some("3.0"));
}

#[test]
fn versions_and_planned_tasks_survive_export_and_import() {
    let dir = TempDir::new().unwrap();
    let a = open(&dir, "A");
    projects(&a);
    call(
        &a,
        "version.create",
        json!({ "project": "TM", "name": "1.0", "sharing": "descendants",
                "due_date": "2026-12-15", "description": "Line one\nline two" }),
    );
    call(
        &a,
        "version.create",
        json!({ "project": "TM", "name": "0.9", "status": "closed" }),
    );
    let k = new_task(&a, "APP", "Shared", json!({ "fix_version": "1.0" }));
    let zip = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let b = open(&dir, "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    let list = |e: &Engine| {
        let mut out = call(e, "version.list", json!({ "project": "TM" }));
        for v in out["items"].as_array_mut().unwrap() {
            v.as_object_mut().unwrap().remove("id");
        }
        out
    };
    assert_eq!(list(&a), list(&b));
    assert_eq!(version_of(&b, &k).as_deref(), Some("1.0"));
}
