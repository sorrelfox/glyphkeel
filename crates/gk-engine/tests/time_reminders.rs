// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Time entries, the timer, the time report and reminders over a real
//! workspace, with a clock the tests move and a fixed time zone.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role, rfc3339};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// 2026-11-23T06:00:00Z, a Monday: 07:00 in Berlin.
const START: i64 = 1_795_413_600_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
}

fn layout(dir: &TempDir) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.path().join("Personal"))
}

fn open_at(dir: &TempDir, role: Role, clock: &Arc<AtomicI64>) -> Ws {
    let tick = Arc::clone(clock);
    let mut config = EngineConfig::new(role);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    config.time_zone = Some("Europe/Berlin".into());
    Ws {
        engine: Engine::open(&layout(dir), config).unwrap(),
        clock: Arc::clone(clock),
    }
}

fn open(dir: &TempDir) -> Ws {
    open_at(dir, Role::App, &Arc::new(AtomicI64::new(START)))
}

fn app() -> Context {
    Context::user(Via::App)
}

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&app(), op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn fails(&self, op: &str, input: Value) -> gk_api::OpError {
        self.engine.call(&app(), op, input).expect_err(op)
    }

    fn task(&self, key: &str, include: &[&str]) -> Value {
        self.call("task.get", json!({ "keys": [key], "include": include }))["items"][0].clone()
    }

    fn new_task(&self, input: Value) -> String {
        self.call("task.create", input)["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn now(&self) -> i64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn due(&self, input: Value) -> Value {
        self.call("reminder.due", input)
    }
}

fn ids(due: &Value) -> Vec<String> {
    due["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

fn kinds(due: &Value) -> Vec<String> {
    due["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn entries_are_logged_edited_and_deleted_with_totals_history_and_undo() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let parent = ws.new_task(json!({ "project": "TM", "title": "Parent" }));
    let child = ws.new_task(json!({ "project": "TM", "title": "Child", "parent": parent }));
    let version = ws.task(&child, &[])["version"].clone();

    let first = ws.call(
        "time.log",
        json!({ "key": child, "duration": "+30m", "note": " Parser " }),
    );
    assert_eq!(first["entry"]["minutes"], 30);
    assert_eq!(first["entry"]["date"], "2026-11-23");
    assert_eq!(first["entry"]["note"], "Parser");
    assert_eq!(first["spent_minutes"], 30);
    let second = ws.call(
        "time.log",
        json!({ "key": child, "duration": "1,5h", "date": "yesterday" }),
    );
    assert_eq!(second["entry"]["date"], "2026-11-22");
    assert_eq!(second["entry"]["duration"], "1h 30m");
    assert_eq!(second["spent_minutes"], 120);

    let task = ws.task(&child, &["time"]);
    assert_eq!(task["spent_minutes"], 120);
    assert_eq!(task["version"], version, "logging time is not a field edit");
    let days: Vec<&str> = task["time_entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["date"].as_str().unwrap())
        .collect();
    assert_eq!(days, ["2026-11-23", "2026-11-22"], "newest day first");
    assert_eq!(ws.task(&parent, &[])["totals"]["spent_minutes"], 120);

    // Rules.
    let err = ws.fails("time.log", json!({ "key": child, "duration": "0m" }));
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = ws.fails("time.log", json!({ "key": child, "duration": "25h" }));
    assert!(err.message.contains("more than a day"), "{}", err.message);

    // Edit with the version; a stale version is refused.
    let id = first["entry"]["id"].as_str().unwrap().to_owned();
    let edited = ws.call(
        "time.update",
        json!({ "entry": id, "duration": "45m", "note": "", "expected_version": 1 }),
    );
    assert_eq!(edited["entry"]["minutes"], 45);
    assert_eq!(edited["entry"]["version"], 2);
    assert!(edited["entry"].get("note").is_none());
    assert_eq!(edited["spent_minutes"], 135);
    let err = ws.fails(
        "time.update",
        json!({ "entry": id, "duration": "1h", "expected_version": 1 }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);

    // Delete, then Undo, Undo the edit, Redo.
    let gone = ws.call("time.delete", json!({ "entry": id }));
    assert_eq!(gone["deleted"], true);
    assert_eq!(gone["spent_minutes"], 90);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.task(&child, &[])["spent_minutes"], 135);
    ws.call("history.undo", json!({}));
    let task = ws.task(&child, &["time"]);
    assert_eq!(task["spent_minutes"], 120);
    assert_eq!(task["time_entries"][0]["note"], "Parser");
    ws.call("history.redo", json!({}));
    assert_eq!(ws.task(&child, &[])["spent_minutes"], 135);

    // The task's history lists its entries.
    let feed = ws.call("history.feed", json!({ "task": child, "limit": 50 }));
    let entities: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["entity"].as_str().unwrap_or_default())
        .collect();
    assert!(
        entities.iter().filter(|e| **e == "time_entry").count() >= 4,
        "{entities:?}"
    );
}

#[test]
fn one_timer_runs_pauses_and_logs_its_time() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    let b = ws.new_task(json!({ "project": "TM", "title": "B" }));

    assert!(ws.call("timer.get", json!({})).get("timer").is_none());
    assert_eq!(ws.fails("timer.stop", json!({})).code, ErrorCode::NotFound);
    let started = ws.call("timer.start", json!({ "key": a, "note": "Reading" }));
    assert_eq!(started["timer"]["state"], "running");
    assert_eq!(started["timer"]["task"]["key"], a);

    ws.advance(20 * MINUTE);
    let paused = ws.call("timer.pause", json!({}));
    assert_eq!(paused["timer"]["state"], "paused");
    ws.advance(3 * HOUR);
    assert_eq!(
        ws.call("timer.get", json!({}))["timer"]["elapsed"],
        "20m",
        "a paused timer doesn't count"
    );
    let resumed = ws.call("timer.start", json!({}));
    assert_eq!(resumed["timer"]["state"], "running");
    ws.advance(25 * MINUTE);

    // Starting on another task stops the first and logs its time.
    let switched = ws.call("timer.start", json!({ "key": b }));
    assert_eq!(switched["entry"]["task"], a);
    assert_eq!(switched["entry"]["minutes"], 45);
    assert_eq!(switched["entry"]["note"], "Reading");
    assert_eq!(switched["timer"]["task"]["key"], b);
    assert_eq!(ws.task(&a, &[])["spent_minutes"], 45);

    // The workspace overview shows the timer.
    assert_eq!(
        ws.call("workspace.get", json!({}))["timer"]["task"]["key"],
        b
    );

    // Under half a minute logs nothing.
    ws.advance(10_000);
    let stopped = ws.call("timer.stop", json!({}));
    assert!(stopped.get("entry").is_none());
    assert_eq!(stopped["warnings"].as_array().unwrap().len(), 1);
    assert!(stopped.get("timer").is_none());

    // Stop with another duration; Undo brings the timer back and takes the
    // entry away.
    ws.call("timer.start", json!({ "key": b }));
    ws.advance(2 * HOUR);
    let stopped = ws.call("timer.stop", json!({ "duration": "1h", "note": "Review" }));
    assert_eq!(stopped["entry"]["minutes"], 60);
    assert_eq!(ws.task(&b, &[])["spent_minutes"], 60);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.task(&b, &[])["spent_minutes"], 0);
    assert_eq!(ws.call("timer.get", json!({}))["timer"]["task"]["key"], b);
}

#[test]
fn a_running_timer_survives_a_restart_and_logs_on_its_first_day() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START + 16 * HOUR)); // 23:00 in Berlin
    let ws = open_at(&dir, Role::App, &clock);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    ws.call("timer.start", json!({ "key": a }));
    drop(ws);

    clock.fetch_add(90 * MINUTE, Ordering::SeqCst); // past midnight
    let ws = open_at(&dir, Role::App, &clock);
    let timer = ws.call("timer.get", json!({}))["timer"].clone();
    assert_eq!(timer["state"], "running");
    assert_eq!(timer["elapsed"], "1h 30m");
    let stopped = ws.call("timer.stop", json!({}));
    assert_eq!(stopped["entry"]["minutes"], 90);
    assert_eq!(stopped["entry"]["date"], "2026-11-23", "the day it started");
}

#[test]
fn the_timer_is_shared_by_programs_on_one_workspace() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let app_side = open_at(&dir, Role::App, &clock);
    let cli_side = open_at(&dir, Role::Cli, &clock);
    app_side.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = app_side.new_task(json!({ "project": "TM", "title": "A" }));
    let events = app_side.engine.subscribe();
    let _watch = app_side
        .engine
        .watch_other_processes(Duration::from_millis(10))
        .unwrap();

    let cli = Context::user(Via::Cli);
    cli_side
        .engine
        .call(&cli, "timer.start", json!({ "key": a }))
        .unwrap();
    let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(event.origin.via, Via::Cli);
    assert_eq!(event.changes[0].entity, "timer");
    assert_eq!(
        app_side.call("timer.get", json!({}))["timer"]["task"]["key"],
        a
    );

    // Starting a second timer from the app stops the one the command line
    // started.
    let b = app_side.new_task(json!({ "project": "TM", "title": "B" }));
    clock.fetch_add(30 * MINUTE, Ordering::SeqCst);
    let switched = app_side.call("timer.start", json!({ "key": b }));
    assert_eq!(switched["entry"]["minutes"], 30);
    let seen = cli_side.engine.call(&cli, "timer.get", json!({})).unwrap();
    assert_eq!(seen["timer"]["task"]["key"], b);
}

#[test]
fn the_report_groups_by_day_task_and_project_in_the_workspace_units() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("settings.update", json!({ "hours_per_day": 6 }));
    ws.call("project.create", json!({ "name": "Tasks", "key": "TM" }));
    ws.call("project.create", json!({ "name": "Web", "key": "WEB" }));
    ws.call(
        "project.create",
        json!({ "name": "Docs", "key": "DOC", "parent": "WEB" }),
    );
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    let w = ws.new_task(json!({ "project": "WEB", "title": "W" }));
    let d = ws.new_task(json!({ "project": "DOC", "title": "D" }));
    let log = |key: &str, duration: &str, date: &str| {
        ws.call(
            "time.log",
            json!({ "key": key, "duration": duration, "date": date }),
        );
    };
    log(&a, "4h", "2026-11-23");
    log(&a, "3h", "2026-11-24");
    log(&w, "1h", "2026-11-24");
    log(&d, "30m", "2026-11-25");
    log(&d, "8h", "2026-11-30"); // next week

    // Default: this week (Monday to Sunday), by task, most time first.
    let report = ws.call("time.report", json!({}));
    assert_eq!(report["from"], "2026-11-23");
    assert_eq!(report["to"], "2026-11-29");
    let rows: Vec<(String, i64)> = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["key"].as_str().unwrap().to_owned(),
                r["minutes"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(rows, [(a.clone(), 420), (w.clone(), 60), (d.clone(), 30)]);
    assert_eq!(report["items"][0]["duration"], "1d 1h", "6-hour days");
    assert_eq!(report["total_minutes"], 510);
    assert_eq!(report["total_duration"], "1d 2h 30m");

    let by_day = ws.call("time.report", json!({ "group_by": "day" }));
    let days: Vec<(&str, &str)> = by_day["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["key"].as_str().unwrap(), r["title"].as_str().unwrap()))
        .collect();
    assert_eq!(
        days,
        [
            ("2026-11-23", "Monday"),
            ("2026-11-24", "Tuesday"),
            ("2026-11-25", "Wednesday")
        ]
    );

    let web = ws.call(
        "time.report",
        json!({ "project": "WEB", "group_by": "project", "from": "2026-11-01",
                "to": "2026-11-30", "entries": true }),
    );
    let rows: Vec<(&str, i64)> = web["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["key"].as_str().unwrap(), r["minutes"].as_i64().unwrap()))
        .collect();
    assert_eq!(rows, [("DOC", 510), ("WEB", 60)], "sub-projects included");
    assert_eq!(web["entries"].as_array().unwrap().len(), 3);

    let err = ws.fails(
        "time.report",
        json!({ "from": "2026-11-30", "to": "2026-11-01" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn a_task_due_tomorrow_alerts_at_the_reminder_time_today() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir); // 07:00 local
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A", "due_date": "2026-11-24" }));
    ws.new_task(json!({ "project": "TM", "title": "No date" }));

    let before = ws.due(json!({}));
    assert_eq!(before["total"], 0);
    assert_eq!(
        before["next_at"], "2026-11-23T08:00:00.000Z",
        "09:00 in Berlin"
    );

    ws.advance(2 * HOUR + MINUTE); // 09:01
    let due = ws.due(json!({}));
    assert_eq!(kinds(&due), ["deadline"]);
    assert_eq!(due["items"][0]["task"]["key"], a);
    assert_eq!(due["items"][0]["due_date"], "2026-11-24");
    assert_eq!(due["items"][0]["missed"], false);

    // Acknowledged, it stays quiet, also after a restart; a new due date
    // alerts again.
    let id = ids(&due).remove(0);
    let ack = ws.call(
        "reminder.acknowledge",
        json!({ "ids": [id, "deadline:nope"] }),
    );
    assert_eq!(ack["acknowledged"], json!([id]));
    assert_eq!(ack["unknown"], json!(["deadline:nope"]));
    assert_eq!(ws.due(json!({}))["total"], 0);
    let clock = Arc::clone(&ws.clock);
    drop(ws);
    let ws = open_at(&dir, Role::App, &clock);
    assert_eq!(ws.due(json!({}))["total"], 0);
    ws.call("task.update", json!({ "key": a, "due_date": "2026-11-23" }));
    assert_eq!(kinds(&ws.due(json!({}))), ["deadline"]);

    // The reminder time is a setting; closing the task ends the alert.
    ws.call("task.update", json!({ "key": a, "due_date": "2026-11-25" }));
    assert_eq!(ws.due(json!({}))["next_at"], "2026-11-24T08:00:00.000Z");
    ws.call("settings.update", json!({ "reminder_time": "18:30" }));
    let due = ws.due(json!({}));
    assert_eq!(due["total"], 0);
    assert_eq!(due["next_at"], "2026-11-24T17:30:00.000Z");
    ws.call("task.move", json!({ "key": a, "status": "Done" }));
    assert!(ws.due(json!({})).get("next_at").is_none());
}

#[test]
fn reminders_missed_while_the_app_was_closed_show_as_missed_at_the_next_start() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let ws = open_at(&dir, Role::App, &clock);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    let b = ws.new_task(json!({ "project": "TM", "title": "B", "due_date": "2026-11-26" }));
    let set = ws.call(
        "reminder.set",
        json!({ "key": a, "at": "2026-11-23 12:00", "note": "Call back" }),
    );
    assert_eq!(set["reminder"]["at"], "2026-11-23T11:00:00.000Z");
    let due = ws.due(json!({}));
    assert_eq!(due["total"], 0);
    assert_eq!(due["next_at"], "2026-11-23T11:00:00.000Z");
    drop(ws);

    // The app was closed for two days; it starts at 10:00 on the 25th.
    clock.store(START + 2 * 24 * HOUR + 3 * HOUR, Ordering::SeqCst);
    let started = clock.load(Ordering::SeqCst);
    let ws = open_at(&dir, Role::App, &clock);
    let due = ws.due(json!({ "since": rfc3339(started) }));
    assert_eq!(kinds(&due), ["reminder", "deadline"]);
    assert!(
        due["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["missed"] == true)
    );
    assert_eq!(due["items"][0]["note"], "Call back");
    assert_eq!(due["items"][1]["task"]["key"], b);

    // What comes due while it runs is not missed.
    ws.call(
        "reminder.set",
        json!({ "key": b, "at": rfc3339(ws.now() + 10 * MINUTE) }),
    );
    ws.advance(11 * MINUTE);
    let due = ws.due(json!({ "since": rfc3339(started) }));
    let fresh: Vec<bool> = due["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["missed"].as_bool().unwrap())
        .collect();
    assert_eq!(fresh, [true, true, false]);
    ws.call("reminder.acknowledge", json!({ "ids": ids(&due) }));
    assert_eq!(ws.due(json!({}))["total"], 0);
}

#[test]
fn a_reminder_before_the_due_date_follows_the_due_date_and_can_be_undone() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    let set = ws.call(
        "reminder.set",
        json!({ "key": a, "days_before_due": 2, "time": "08:15" }),
    );
    assert!(set["reminder"].get("at").is_none(), "no due date yet");
    assert_eq!(set["reminder"]["time"], "08:15");

    ws.call("task.update", json!({ "key": a, "due_date": "2026-11-25" }));
    let reminder = ws.task(&a, &["reminder"])["reminder"].clone();
    assert_eq!(reminder["at"], "2026-11-23T07:15:00.000Z");
    ws.advance(20 * MINUTE); // 07:20 local
    assert_eq!(ws.due(json!({}))["total"], 0);
    ws.advance(HOUR);
    let due = ws.due(json!({}));
    assert_eq!(kinds(&due), ["reminder"]);
    ws.call("reminder.acknowledge", json!({ "ids": ids(&due) }));
    assert_eq!(ws.task(&a, &["reminder"])["reminder"]["acknowledged"], true);

    // A later due date makes it fire again; changing the reminder is
    // undone like any change.
    ws.call("task.update", json!({ "key": a, "due_date": "2026-11-27" }));
    assert_eq!(ws.due(json!({}))["next_at"], "2026-11-25T07:15:00.000Z");
    ws.call("reminder.clear", json!({ "key": a }));
    assert!(ws.task(&a, &["reminder"]).get("reminder").is_none());
    ws.call("history.undo", json!({}));
    assert_eq!(ws.task(&a, &["reminder"])["reminder"]["days_before_due"], 2);

    let err = ws.fails("reminder.set", json!({ "key": a }));
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = ws.fails(
        "reminder.set",
        json!({ "key": a, "at": "2026-11-24 09:00", "days_before_due": 1 }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = ws.fails("reminder.set", json!({ "key": a, "at": "soon" }));
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn a_timer_running_for_hours_reminds_once() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    ws.call("timer.start", json!({ "key": a }));
    ws.advance(HOUR);
    ws.call("timer.pause", json!({}));
    ws.advance(5 * HOUR);
    let due = ws.due(json!({}));
    assert_eq!(due["total"], 0);
    assert!(
        due.get("next_at").is_none(),
        "a paused timer doesn't remind"
    );

    ws.call("timer.start", json!({}));
    let due = ws.due(json!({}));
    // Started at 07:00, an hour run, five hours paused, resumed at 13:00
    // local: seven more hours.
    let next = due["next_at"].as_str().unwrap().to_owned();
    assert!(next.starts_with("2026-11-23T19:00:00"), "{next}");
    ws.advance(7 * HOUR);
    let due = ws.due(json!({}));
    assert_eq!(kinds(&due), ["timer"]);
    ws.call("reminder.acknowledge", json!({ "ids": ids(&due) }));
    ws.advance(5 * HOUR);
    assert_eq!(ws.due(json!({}))["total"], 0);

    // The hours are a setting; 0 switches the reminder off.
    ws.call("timer.stop", json!({ "discard": true }));
    ws.call("settings.update", json!({ "timer_reminder_hours": 1 }));
    ws.call("timer.start", json!({ "key": a }));
    ws.advance(HOUR + MINUTE);
    assert_eq!(kinds(&ws.due(json!({}))), ["timer"]);
    ws.call("settings.update", json!({ "timer_reminder_hours": 0 }));
    assert_eq!(ws.due(json!({}))["total"], 0);
}

#[test]
fn a_timer_whose_task_went_to_the_trash_is_discarded() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task(json!({ "project": "TM", "title": "A" }));
    ws.call("timer.start", json!({ "key": a }));
    ws.advance(HOUR);
    // No operation moves a task to the trash yet; do it in the database.
    let raw = gk_store::open_raw(&layout(&dir).database()).unwrap();
    raw.execute("UPDATE tasks SET deleted_at = 1", []).unwrap();
    drop(raw);
    assert_eq!(
        ws.call("timer.get", json!({}))["timer"]["task_trashed"],
        true
    );
    assert_eq!(ws.fails("timer.stop", json!({})).code, ErrorCode::Conflict);
    let stopped = ws.call("timer.stop", json!({ "discard": true }));
    assert!(stopped.get("entry").is_none());
    assert!(ws.call("timer.get", json!({})).get("timer").is_none());
}
