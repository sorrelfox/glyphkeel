// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Sprints: planning tasks into them, starting and completing them with
//! their unfinished tasks moving on, one active sprint per project, queries
//! and saved views, undo, and the burndown replayed from the history.

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
const DAY: i64 = 86_400_000;

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

fn code(e: &Engine, op: &str, input: Value) -> ErrorCode {
    e.call(&Context::user(Via::App), op, input)
        .expect_err(op)
        .code
}

fn sprint_of(e: &Engine, key: &str) -> Option<String> {
    call(e, "task.get", json!({ "keys": [key] }))["items"][0]["sprint"]["name"]
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

fn new_task(e: &Engine, title: &str, extra: Value) -> String {
    let mut input = json!({ "project": "TM", "title": title });
    for (k, v) in extra.as_object().unwrap() {
        input[k] = v.clone();
    }
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn setup() -> (TempDir, Arc<AtomicI64>, Engine) {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(NOW));
    let e = open(&dir, &clock);
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(&e, "project.create", json!({ "name": "Yard", "key": "ZZ" }));
    (dir, clock, e)
}

fn done_status(e: &Engine) -> String {
    let project = call(e, "project.get", json!({ "project": "TM" }));
    project["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["category"] == "done")
        .unwrap()["name"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn sprints_are_planned_started_and_completed_with_open_tasks_moving_on() {
    let (_dir, _clock, e) = setup();
    let a = new_task(&e, "A", json!({ "estimate": "2h" }));
    let b = new_task(&e, "B", json!({ "estimate": "1h" }));
    let c = new_task(&e, "C", json!({}));
    let made = call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 1", "goal": "Ship it", "tasks": [a, b] }),
    );
    assert_eq!(made["sprint"]["state"], "planned");
    assert_eq!(made["tasks_changed"], json!([a, b]));
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 2" }),
    );
    // Names are unique per project and some words are kept for queries.
    assert_eq!(
        code(
            &e,
            "sprint.create",
            json!({ "project": "TM", "name": "sprint 1" })
        ),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(
            &e,
            "sprint.create",
            json!({ "project": "TM", "name": "Active" })
        ),
        ErrorCode::Invalid
    );
    // A task of another project can't join.
    let other = call(
        &e,
        "task.create",
        json!({ "project": "ZZ", "title": "Elsewhere" }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        code(
            &e,
            "sprint.add_tasks",
            json!({ "sprint": "Sprint 1", "project": "TM", "tasks": [other] })
        ),
        ErrorCode::Invalid
    );

    // The sprints page: sprints with their tasks, and the backlog.
    let page = call(
        &e,
        "sprint.list",
        json!({ "project": "TM", "include_tasks": true }),
    );
    assert_eq!(page["total"], 2);
    assert_eq!(page["items"][0]["progress"]["total"], 2);
    assert_eq!(page["items"][0]["progress"]["estimate_minutes"], 180);
    assert_eq!(page["backlog"][0]["key"], c);

    // Start: dates default to today and two weeks; one active at a time.
    let started = call(
        &e,
        "sprint.start",
        json!({ "sprint": "Sprint 1", "project": "TM" }),
    );
    assert_eq!(started["sprint"]["state"], "active");
    assert_eq!(started["sprint"]["start_date"], "2026-11-23");
    assert_eq!(started["sprint"]["end_date"], "2026-12-06");
    assert_eq!(
        code(
            &e,
            "sprint.start",
            json!({ "sprint": "Sprint 2", "project": "TM" })
        ),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(
            &e,
            "sprint.delete",
            json!({ "sprint": "active", "project": "TM" })
        ),
        ErrorCode::Conflict
    );
    // Tasks join the active sprint by `active`.
    call(&e, "task.update", json!({ "key": c, "sprint": "active" }));
    assert_eq!(sprint_of(&e, &c).as_deref(), Some("Sprint 1"));
    let done = done_status(&e);
    call(&e, "task.move", json!({ "key": a, "status": done }));

    // Complete: the open ones go to the next sprint, the closed one stays.
    let completed = call(
        &e,
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM", "move_open_tasks_to": "Sprint 2" }),
    );
    assert_eq!(completed["sprint"]["state"], "closed");
    assert_eq!(completed["tasks_changed"], json!([b, c]));
    assert_eq!(sprint_of(&e, &a).as_deref(), Some("Sprint 1"));
    assert_eq!(sprint_of(&e, &b).as_deref(), Some("Sprint 2"));
    // A completed sprint takes no tasks.
    assert_eq!(
        code(&e, "task.update", json!({ "key": c, "sprint": "Sprint 1" })),
        ErrorCode::Conflict
    );
    // Undo brings the sprint back to active with its tasks, in one step.
    call(&e, "history.undo", json!({}));
    assert_eq!(sprint_of(&e, &b).as_deref(), Some("Sprint 1"));
    let back = call(
        &e,
        "sprint.get",
        json!({ "sprint": "active", "project": "TM" }),
    );
    assert_eq!(back["name"], "Sprint 1");
    assert_eq!(back["tasks"].as_array().unwrap().len(), 3);

    // Completing to the backlog leaves them in no sprint.
    call(
        &e,
        "sprint.complete",
        json!({ "sprint": "Sprint 1", "project": "TM", "move_open_tasks_to": "backlog" }),
    );
    assert_eq!(sprint_of(&e, &b), None);
    assert_eq!(sprint_of(&e, &c), None);

    // Bulk add and remove, and moving a task to another project.
    let added = call(
        &e,
        "sprint.add_tasks",
        json!({ "sprint": "Sprint 2", "project": "TM", "tasks": [b, c] }),
    );
    assert_eq!(added["tasks_changed"], json!([b, c]));
    let removed = call(&e, "sprint.remove_tasks", json!({ "tasks": [c] }));
    assert_eq!(removed["tasks_changed"], json!([c]));
    assert_eq!(sprint_of(&e, &c), None);
    let moved = call(&e, "task.move", json!({ "key": b, "project": "ZZ" }));
    assert!(
        moved["warnings"]
            .as_array()
            .is_some_and(|w| w.iter().any(|w| w.as_str().unwrap().contains("Sprint 2"))),
        "{moved}"
    );
    let b_now = moved["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(sprint_of(&e, &b_now), None);

    // Bulk update can set a sprint too.
    call(
        &e,
        "task.bulk_update",
        json!({ "keys": [c], "changes": { "sprint": "Sprint 2" }, "dry_run": false,
                "expected_count": 1 }),
    );
    assert_eq!(sprint_of(&e, &c).as_deref(), Some("Sprint 2"));
}

#[test]
fn queries_find_sprints_and_saved_views_follow_renames_and_deletions() {
    let (_dir, _clock, e) = setup();
    let a = new_task(&e, "A", json!({}));
    let b = new_task(&e, "B", json!({}));
    let c = new_task(&e, "C", json!({}));
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 1", "tasks": [a] }),
    );
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 2", "tasks": [b] }),
    );
    call(
        &e,
        "sprint.start",
        json!({ "sprint": "Sprint 1", "project": "TM" }),
    );
    let search = |q: &str| keys(&call(&e, "task.search", json!({ "query": q })));
    assert_eq!(search("sprint:active"), std::slice::from_ref(&a));
    assert_eq!(search("sprint:\"Sprint 2\""), std::slice::from_ref(&b));
    assert_eq!(search("sprint:none project:TM"), std::slice::from_ref(&c));
    assert_eq!(search("has:sprint sort:key"), [a.clone(), b.clone()]);
    assert_eq!(
        search("sprint:active,\"Sprint 2\" sort:key"),
        [a.clone(), b.clone()]
    );
    assert_eq!(
        code(&e, "task.search", json!({ "query": "sprint:Nope" })),
        ErrorCode::Invalid
    );
    // The history: a task that was in a sprint before.
    call(&e, "sprint.remove_tasks", json!({ "tasks": [b] }));
    assert_eq!(search("was:sprint:\"Sprint 2\""), std::slice::from_ref(&b));
    assert_eq!(search("changed:sprint sort:key"), [a.clone(), b.clone()]);

    call(
        &e,
        "view.create",
        json!({ "name": "Now", "project": "TM", "query": "sprint:\"Sprint 1\" is:open" }),
    );
    let renamed = call(
        &e,
        "sprint.update",
        json!({ "sprint": "Sprint 1", "project": "TM", "name": "Sprint 1b" }),
    );
    assert_eq!(renamed["views_rewritten"], json!(["Now"]));
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "sprint:\"Sprint 1b\" is:open");

    call(
        &e,
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM" }),
    );
    let deleted = call(
        &e,
        "sprint.delete",
        json!({ "sprint": "Sprint 1b", "project": "TM" }),
    );
    assert_eq!(deleted["deleted"], true);
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "is:open");
    call(&e, "history.undo", json!({}));
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "sprint:\"Sprint 1b\" is:open");
}

#[test]
fn the_burndown_replays_the_sprint_day_by_day() {
    let (_dir, clock, e) = setup();
    let a = new_task(&e, "A", json!({ "estimate": "4h" }));
    let b = new_task(&e, "B", json!({ "estimate": "2h" }));
    let c = new_task(&e, "C", json!({ "estimate": "1h" }));
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 1", "tasks": [a, b] }),
    );
    assert_eq!(
        code(
            &e,
            "sprint.burndown",
            json!({ "sprint": "Sprint 1", "project": "TM" })
        ),
        ErrorCode::Conflict
    );
    call(
        &e,
        "sprint.start",
        json!({ "sprint": "Sprint 1", "project": "TM", "end_date": "2026-11-27" }),
    );
    let done = done_status(&e);
    // Day 2: A is done. Day 3: C joins. Day 4: the sprint is completed.
    clock.store(NOW + DAY, Ordering::SeqCst);
    call(&e, "task.move", json!({ "key": a, "status": done }));
    clock.store(NOW + 2 * DAY, Ordering::SeqCst);
    call(&e, "task.update", json!({ "key": c, "sprint": "active" }));
    let running = call(
        &e,
        "sprint.burndown",
        json!({ "sprint": "active", "project": "TM" }),
    );
    let open: Vec<u64> = running["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["open_tasks"].as_u64().unwrap())
        .collect();
    assert_eq!(open, [2, 1, 2], "days to come are left out");
    assert_eq!(running["start_tasks"], 2);
    assert_eq!(running["start_estimate_minutes"], 360);
    assert_eq!(running["days"][2]["open_estimate_minutes"], 180);
    assert_eq!(running["days"][2]["total_tasks"], 3);

    clock.store(NOW + 3 * DAY, Ordering::SeqCst);
    call(
        &e,
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM" }),
    );
    let closed = call(
        &e,
        "sprint.burndown",
        json!({ "sprint": "Sprint 1", "project": "TM" }),
    );
    let days = closed["days"].as_array().unwrap();
    assert_eq!(days.len(), 4);
    assert_eq!(days[0]["date"], "2026-11-23");
    // On the day it was completed, the tasks moved out still count: it
    // stands as it was just before.
    assert_eq!(days[3]["open_tasks"], 2);
    assert_eq!(days[3]["total_tasks"], 3);
}

#[test]
fn sprints_and_their_tasks_survive_export_and_import() {
    let (dir, clock, a) = setup();
    let k = new_task(&a, "In the sprint", json!({}));
    call(
        &a,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 1", "goal": "Line one\nline two",
                "tasks": [k] }),
    );
    call(
        &a,
        "sprint.start",
        json!({ "sprint": "Sprint 1", "project": "TM" }),
    );
    call(
        &a,
        "sprint.create",
        json!({ "project": "TM", "name": "Sprint 2", "start_date": "2026-12-07" }),
    );
    let zip = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let other = TempDir::new().unwrap();
    let b = open(&other, &clock);
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    let list = |e: &Engine| {
        let mut out = call(e, "sprint.list", json!({ "project": "TM" }));
        for v in out["items"].as_array_mut().unwrap() {
            v.as_object_mut().unwrap().remove("id");
        }
        out
    };
    assert_eq!(list(&a), list(&b));
    assert_eq!(sprint_of(&b, &k).as_deref(), Some("Sprint 1"));
}
