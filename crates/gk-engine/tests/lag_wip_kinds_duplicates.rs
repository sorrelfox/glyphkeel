// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The lag of a dependency, the WIP minimum of a board column, kinds of
//! work on logged time and closing duplicates with their original, over a
//! real workspace.

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

fn fails(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).expect_err(op)
}

fn workspace() -> (TempDir, Engine) {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    (dir, e)
}

fn task(e: &Engine, input: Value) -> String {
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn dates(e: &Engine, key: &str) -> (Value, Value) {
    let t = call(e, "task.get", json!({ "keys": [key] }))["items"][0].clone();
    (t["start_date"].clone(), t["due_date"].clone())
}

#[test]
fn a_lag_delays_the_dependent_task_and_is_undone_with_its_link() {
    let (_dir, e) = workspace();
    let a = task(
        &e,
        json!({ "project": "TM", "title": "Pour", "start_date": "2026-12-01", "due_date": "2026-12-02" }),
    );
    let b = task(
        &e,
        json!({ "project": "TM", "title": "Build on it", "start_date": "2026-12-03", "due_date": "2026-12-04" }),
    );
    let link = call(
        &e,
        "relation.add",
        json!({ "key": a, "kind": "blocks", "other": b, "lag_days": 2 }),
    );
    assert_eq!(link["task"]["links"][0]["lag_days"], 2, "{link}");
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["dependencies"][0]["lag_days"], 2, "{s}");
    assert_eq!(
        s["dependencies"][0]["violated"], true,
        "B starts the day after A"
    );

    // The preview and "also move dependents" keep the lag.
    let preview = call(
        &e,
        "schedule.preview_move",
        json!({ "key": a, "start_date": "2026-12-01", "due_date": "2026-12-03" }),
    );
    assert_eq!(preview["moved"][0]["start_date"], "2026-12-06", "{preview}");
    call(
        &e,
        "task.update",
        json!({ "key": a, "due_date": "2026-12-03", "move_dependents": true }),
    );
    assert_eq!(dates(&e, &b), (json!("2026-12-06"), json!("2026-12-07")));
    call(&e, "history.undo", json!({}));

    // Linking again changes the lag; 0 removes it; Undo puts it back.
    call(
        &e,
        "relation.add",
        json!({ "key": b, "kind": "blocked_by", "other": a, "lag_days": 0 }),
    );
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert!(s["dependencies"][0].get("lag_days").is_none(), "{s}");
    call(&e, "history.undo", json!({}));
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["dependencies"][0]["lag_days"], 2, "{s}");

    // Removing the link and undoing that keeps the lag too.
    call(
        &e,
        "relation.remove",
        json!({ "key": a, "kind": "blocks", "other": b }),
    );
    call(&e, "history.undo", json!({}));
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["dependencies"][0]["lag_days"], 2, "{s}");

    let err = fails(
        &e,
        "relation.add",
        json!({ "key": a, "kind": "relates", "other": b, "lag_days": 1 }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["message_id"], "link-lag-blocks-only");
    let err = fails(
        &e,
        "relation.add",
        json!({ "key": a, "kind": "blocks", "other": b, "lag_days": 366 }),
    );
    assert_eq!(err.details["message_id"], "link-lag-range");
}

#[test]
fn a_column_below_its_wip_minimum_says_so() {
    let (_dir, e) = workspace();
    call(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "In progress", "wip_min": 2, "wip_limit": 3 }),
    );
    let column = |e: &Engine| {
        let board = call(e, "board.get", json!({ "project": "TM" }));
        board["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "In progress")
            .unwrap()
            .clone()
    };
    let c = column(&e);
    assert_eq!(
        (c["wip_min"].clone(), c["under_minimum"].clone()),
        (json!(2), json!(true))
    );
    for title in ["One", "Two"] {
        let key = task(&e, json!({ "project": "TM", "title": title }));
        call(
            &e,
            "task.move",
            json!({ "key": key, "status": "In progress" }),
        );
    }
    assert_eq!(column(&e)["under_minimum"], false);

    let project = call(&e, "project.get", json!({ "project": "TM" }));
    let status = project["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "In progress")
        .unwrap()
        .clone();
    assert_eq!(status["wip_min"], 2, "{status}");

    let err = fails(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "In progress", "wip_min": 4 }),
    );
    assert_eq!(err.details["message_id"], "status-wip-min-above-limit");
    let err = fails(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "In progress", "wip_min": 0 }),
    );
    assert_eq!(err.details["message_id"], "status-wip-min");
    call(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "In progress", "clear": ["wip_min"] }),
    );
    assert!(column(&e).get("wip_min").is_none());
    call(&e, "history.undo", json!({}));
    assert_eq!(column(&e)["wip_min"], 2);
}

#[test]
fn kinds_of_work_mark_time_and_group_the_report() {
    let (_dir, e) = workspace();
    let key = task(&e, json!({ "project": "TM", "title": "Parser" }));
    for name in ["Development", "Meeting"] {
        call(&e, "work_kind.create", json!({ "name": name }));
    }
    let err = fails(&e, "work_kind.create", json!({ "name": "meeting" }));
    assert_eq!(err.details["message_id"], "work-kind-name-taken");

    let dev = call(
        &e,
        "time.log",
        json!({ "key": key, "duration": "2h", "work_kind": "development" }),
    );
    assert_eq!(dev["entry"]["work_kind"]["name"], "Development", "{dev}");
    let meeting = call(
        &e,
        "time.log",
        json!({ "key": key, "duration": "30m", "work_kind": "Meeting" }),
    );
    call(&e, "time.log", json!({ "key": key, "duration": "15m" }));
    let err = fails(
        &e,
        "time.log",
        json!({ "key": key, "duration": "15m", "work_kind": "Travel" }),
    );
    assert_eq!(err.details["message_id"], "work-kind-not-found");

    let report = call(
        &e,
        "time.report",
        json!({ "group_by": "work_kind", "csv": "groups" }),
    );
    let rows: Vec<(String, i64)> = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["title"].as_str().unwrap().to_owned(),
                r["minutes"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Development".to_owned(), 120),
            ("Meeting".to_owned(), 30),
            (String::new(), 15)
        ]
    );
    assert!(
        report["csv"]
            .as_str()
            .unwrap()
            .starts_with("work_kind,title,"),
        "{report}"
    );
    let only = call(&e, "time.report", json!({ "work_kind": "none" }));
    assert_eq!(only["total_minutes"], 15);
    let only = call(
        &e,
        "time.report",
        json!({ "work_kind": "Meeting", "csv": "entries" }),
    );
    assert_eq!(only["total_minutes"], 30);
    assert!(
        only["csv"].as_str().unwrap().contains(",Meeting\r\n"),
        "{only}"
    );

    // An entry changes its kind, and Undo changes it back.
    let entry = meeting["entry"]["id"].as_str().unwrap().to_owned();
    let changed = call(
        &e,
        "time.update",
        json!({ "entry": entry, "work_kind": "Development" }),
    );
    assert_eq!(changed["entry"]["work_kind"]["name"], "Development");
    call(&e, "history.undo", json!({}));
    let cleared = call(
        &e,
        "time.update",
        json!({ "entry": entry, "work_kind": "" }),
    );
    assert!(cleared["entry"].get("work_kind").is_none(), "{cleared}");
    call(&e, "history.undo", json!({}));

    // The timer carries a kind to the entry it logs.
    call(
        &e,
        "timer.start",
        json!({ "key": key, "work_kind": "Meeting" }),
    );
    let timer = call(&e, "timer.get", json!({}));
    assert_eq!(timer["timer"]["work_kind"]["name"], "Meeting", "{timer}");
    let stopped = call(
        &e,
        "timer.stop",
        json!({ "duration": "10m", "work_kind": "Development" }),
    );
    assert_eq!(
        stopped["entry"]["work_kind"]["name"], "Development",
        "{stopped}"
    );

    // A kind in use is archived, not deleted; archived kinds take no new
    // time but keep their entries.
    let err = fails(&e, "work_kind.delete", json!({ "kind": "Meeting" }));
    assert_eq!(err.details["message_id"], "work-kind-in-use");
    call(
        &e,
        "work_kind.update",
        json!({ "kind": "Meeting", "archived": true, "name": "Meetings" }),
    );
    let list = call(&e, "work_kind.list", json!({}));
    assert_eq!(list["items"].as_array().unwrap().len(), 1, "{list}");
    let all = call(&e, "work_kind.list", json!({ "include_archived": true }));
    assert_eq!(all["items"][1]["name"], "Meetings");
    assert_eq!(all["items"][1]["archived"], true);
    assert_eq!(all["items"][1]["entries"], 1);
    let err = fails(
        &e,
        "time.log",
        json!({ "key": key, "duration": "15m", "work_kind": "Meetings" }),
    );
    assert_eq!(err.details["message_id"], "work-kind-archived");

    // An unused kind is deleted, and Undo brings it back.
    let travel = call(&e, "work_kind.create", json!({ "name": "Travel" }));
    let gone = call(&e, "work_kind.delete", json!({ "kind": "travel" }));
    assert_eq!(gone["deleted"], true);
    call(&e, "history.undo", json!({}));
    let back = call(&e, "work_kind.list", json!({}));
    assert!(
        back["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k["id"] == travel["kind"]["id"]),
        "{back}"
    );
}

#[test]
fn closing_a_task_can_close_its_duplicates_as_one_change() {
    let (_dir, e) = workspace();
    call(&e, "project.create", json!({ "name": "Web", "key": "WEB" }));
    let original = task(&e, json!({ "project": "TM", "title": "Crash on start" }));
    let same = task(&e, json!({ "project": "TM", "title": "App crashes" }));
    let other = task(&e, json!({ "project": "WEB", "title": "Start crash" }));
    let closed = task(&e, json!({ "project": "TM", "title": "Old report" }));
    call(&e, "task.move", json!({ "key": closed, "status": "Done" }));
    for d in [&same, &other, &closed] {
        call(
            &e,
            "relation.add",
            json!({ "key": d, "kind": "duplicates", "other": original }),
        );
    }

    // Left out: the duplicates stay open and a warning names them.
    let change = call(
        &e,
        "task.move",
        json!({ "key": original, "status": "Done" }),
    );
    let warnings = change["warnings"].to_string();
    assert!(
        warnings.contains(&same) && warnings.contains(&other),
        "{change}"
    );
    assert!(!warnings.contains(&closed), "{change}");
    call(&e, "history.undo", json!({}));

    // `false`: no warning.
    let change = call(
        &e,
        "task.move",
        json!({ "key": original, "status": "Done", "close_duplicates": false }),
    );
    assert!(change.get("warnings").is_none(), "{change}");
    call(&e, "history.undo", json!({}));

    let change = call(
        &e,
        "task.move",
        json!({ "key": original, "status": "Done", "close_duplicates": true }),
    );
    assert_eq!(
        change["duplicates_closed"],
        json!([same, other]),
        "{change}"
    );
    let status = |key: &str| {
        call(&e, "task.get", json!({ "keys": [key] }))["items"][0]["status"]["name"].clone()
    };
    assert_eq!(status(&same), "Done");
    assert_eq!(status(&other), "Done");

    // One Undo opens all three again.
    call(&e, "history.undo", json!({}));
    assert_eq!(status(&original), "Backlog");
    assert_eq!(status(&same), "Backlog");
    assert_eq!(status(&other), "Backlog");
}

#[test]
fn an_export_keeps_the_kinds_of_work_by_name() {
    let dir = TempDir::new().unwrap();
    let a = open(&dir);
    call(&a, "project.create", json!({ "name": "Team", "key": "TM" }));
    let key = task(&a, json!({ "project": "TM", "title": "Parser" }));
    call(&a, "work_kind.create", json!({ "name": "Code review" }));
    call(
        &a,
        "time.log",
        json!({ "key": key, "duration": "1h", "work_kind": "code review" }),
    );
    let zip = dir.path().join("tm.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let other = TempDir::new().unwrap();
    let b = open(&other);
    call(&b, "work_kind.create", json!({ "name": "CODE REVIEW" }));
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    let report = call(
        &b,
        "time.report",
        json!({ "group_by": "work_kind", "from": "2026-11-01", "to": "2026-11-30" }),
    );
    assert_eq!(report["items"][0]["title"], "CODE REVIEW", "{report}");
    assert_eq!(
        call(&b, "work_kind.list", json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
