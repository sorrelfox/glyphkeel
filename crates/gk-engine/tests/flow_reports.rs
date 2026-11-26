// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The flow reports over a real workspace: history replay, reopened tasks,
//! status renames, the trash, people against agents, time zones and the
//! filters.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// 2026-11-02T06:00:00Z, a Monday: 07:00 in Berlin.
const MONDAY: i64 = 1_793_599_200_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
    agent: Context,
}

fn open(dir: &TempDir) -> Ws {
    let clock = Arc::new(AtomicI64::new(MONDAY));
    let tick = Arc::clone(&clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.load(Ordering::SeqCst));
    config.time_zone = Some("Europe/Berlin".into());
    let engine = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
    let session = engine
        .start_session(&SessionStart {
            name: "Robo".into(),
            client_id: Some("robo".into()),
            client_version: Some("1.0".into()),
            profile: Profile::Full,
        })
        .unwrap();
    Ws {
        engine,
        clock,
        agent: Context::agent("Robo", session, Profile::Full),
    }
}

impl Ws {
    fn at(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    fn call(&self, op: &str, input: Value) -> Value {
        self.call_as(&Context::user(Via::App), op, input)
    }

    fn call_as(&self, who: &Context, op: &str, input: Value) -> Value {
        self.engine
            .call(who, op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn fails(&self, op: &str, input: Value) -> gk_api::OpError {
        self.engine
            .call(&Context::user(Via::App), op, input)
            .expect_err(op)
    }

    fn create(&self, who: &Context, title: &str, extra: Value) -> String {
        let mut input = json!({ "project": "TM", "title": title, "status": "To do" });
        if let Value::Object(more) = extra {
            input.as_object_mut().unwrap().extend(more);
        }
        self.call_as(who, "task.create", input)["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn move_to(&self, who: &Context, key: &str, status: &str) {
        self.call_as(who, "task.move", json!({ "key": key, "status": status }));
    }
}

fn column(report: &Value, field: &str) -> Vec<u64> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[field].as_u64().unwrap())
        .collect()
}

/// The band names and each period's counts.
fn flow(report: &Value) -> (Vec<String>, Vec<Vec<u64>>) {
    let names = report["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap().to_owned())
        .collect();
    let counts = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap())
                .collect()
        })
        .collect();
    (names, counts)
}

/// Two weeks of work: A done in the first week; B done, reopened and done
/// again in the second; C still to do; D created and closed by an agent.
fn two_weeks(ws: &Ws) {
    let me = Context::user(Via::App);
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    ws.call("label.create", json!({ "project": "TM", "name": "ui" }));
    let a = ws.create(&me, "A", json!({ "labels": ["ui"] }));
    let b = ws.create(&me, "B", json!({ "type": "Bug" }));
    let c = ws.create(&me, "C", json!({}));
    ws.at(MONDAY + DAY);
    ws.move_to(&me, &a, "In progress");
    ws.at(MONDAY + 2 * DAY);
    ws.move_to(&me, &b, "In progress");
    ws.at(MONDAY + 3 * DAY);
    ws.move_to(&me, &a, "Done");
    ws.at(MONDAY + 8 * DAY);
    let d = ws.create(&ws.agent.clone(), "D", json!({}));
    ws.at(MONDAY + 8 * DAY + HOUR);
    ws.move_to(&ws.agent.clone(), &d, "Done");
    ws.at(MONDAY + 9 * DAY);
    ws.move_to(&me, &b, "Done");
    ws.at(MONDAY + 10 * DAY);
    ws.move_to(&me, &b, "To do");
    ws.at(MONDAY + 11 * DAY);
    ws.move_to(&me, &b, "In progress");
    ws.at(MONDAY + 12 * DAY);
    ws.move_to(&me, &b, "Done");
    assert_eq!((a, c), ("TM-1".to_owned(), "TM-3".to_owned()));
    ws.at(MONDAY + 20 * DAY);
}

const RANGE: (&str, &str) = ("2026-11-02", "2026-11-15");

#[test]
fn created_against_closed_counts_reopened_tasks_and_actors() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    two_weeks(&ws);
    let r = ws.call(
        "report.created_closed",
        json!({ "from": RANGE.0, "to": RANGE.1, "bucket": "week" }),
    );
    assert_eq!(column(&r, "created"), [3, 1]);
    assert_eq!(column(&r, "closed"), [1, 3], "B closes twice");
    assert_eq!(column(&r, "reopened"), [0, 1]);
    assert_eq!(column(&r, "open"), [2, 1]);
    assert_eq!(
        (&r["created"], &r["closed"], &r["open_before"]),
        (&json!(4), &json!(4), &json!(0))
    );
    assert_eq!(r["items"][1]["start"], "2026-11-09");
    assert_eq!(r["items"][1]["end"], "2026-11-15");

    let agents = ws.call(
        "report.created_closed",
        json!({ "from": RANGE.0, "to": RANGE.1, "actor": "agent" }),
    );
    assert_eq!(column(&agents, "created"), [0, 1]);
    assert_eq!(column(&agents, "closed"), [0, 1]);
    let me = ws.call(
        "report.created_closed",
        json!({ "from": RANGE.0, "to": RANGE.1, "actor": "me" }),
    );
    assert_eq!(column(&me, "closed"), [1, 2]);

    // Days: the first week's Thursday has A's close.
    let days = ws.call(
        "report.created_closed",
        json!({ "from": RANGE.0, "to": "2026-11-08", "bucket": "day", "csv": true }),
    );
    assert_eq!(column(&days, "closed"), [0, 0, 0, 1, 0, 0, 0]);
    let csv = days["csv"].as_str().unwrap();
    assert!(
        csv.starts_with(
            "start,end,created,closed,reopened,open\r\n2026-11-02,2026-11-02,3,0,0,3\r\n"
        ),
        "{csv}"
    );

    let err = ws.fails("report.created_closed", json!({ "actor": " " }));
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn cumulative_flow_follows_the_history_and_status_renames() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    two_weeks(&ws);
    let input = json!({ "from": RANGE.0, "to": RANGE.1, "project": "TM" });
    let r = ws.call("report.cumulative_flow", input.clone());
    let (names, counts) = flow(&r);
    assert_eq!(names, ["Backlog", "To do", "In progress", "Review", "Done"]);
    assert_eq!(counts, [vec![0, 1, 1, 0, 1], vec![0, 1, 0, 0, 3]]);

    ws.call(
        "status.update",
        json!({ "project": "TM", "status": "In progress", "name": "Doing" }),
    );
    let renamed = ws.call("report.cumulative_flow", input.clone());
    let (names, again) = flow(&renamed);
    assert_eq!(names[2], "Doing");
    assert_eq!(again, counts, "a rename changes no count");

    let by_category = ws.call(
        "report.cumulative_flow",
        json!({ "from": RANGE.0, "to": RANGE.1, "group_by": "category", "csv": true }),
    );
    let (names, counts) = flow(&by_category);
    assert_eq!(names, ["todo", "in_progress", "done", "cancelled"]);
    assert_eq!(counts, [vec![1, 1, 1, 0], vec![1, 0, 3, 0]]);
    let csv = by_category["csv"].as_str().unwrap();
    assert!(
        csv.starts_with("start,end,todo,in_progress,done,cancelled\r\n"),
        "{csv}"
    );
}

#[test]
fn a_task_in_the_trash_is_not_counted_meanwhile() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    two_weeks(&ws);
    ws.at(MONDAY + 12 * DAY + HOUR);
    ws.call("task.delete", json!({ "keys": ["TM-3"] }));
    let input = json!({ "from": "2026-11-14", "to": "2026-11-15", "bucket": "day" });
    // While it is in the trash, it is left out altogether.
    assert_eq!(
        column(&ws.call("report.created_closed", input.clone()), "open"),
        [0, 0]
    );
    let entry = ws.call("trash.list", json!({}))["items"][0]["id"].clone();
    ws.at(MONDAY + 13 * DAY + HOUR);
    ws.call("trash.restore", json!({ "items": [entry] }));
    let r = ws.call("report.created_closed", input.clone());
    assert_eq!(column(&r, "open"), [0, 1], "back on Sunday");
    assert_eq!(column(&r, "closed"), [1, 0], "the trash is not a close");
}

#[test]
fn cycle_and_lead_time_use_the_first_start_and_the_last_close() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    two_weeks(&ws);
    let r = ws.call(
        "report.cycle_time",
        json!({ "from": RANGE.0, "to": RANGE.1, "tasks": true, "csv": true }),
    );
    // A: in progress day 1, done day 3. B: in progress day 2, last done
    // day 12. D: never in progress, done an hour after its creation.
    assert_eq!(r["cycle"]["count"], 2);
    assert_eq!(r["cycle"]["p50"], 6.0);
    assert_eq!(r["cycle"]["min"], 2.0);
    assert_eq!(r["cycle"]["max"], 10.0);
    assert_eq!(r["lead"]["count"], 3);
    assert_eq!(r["lead"]["p50"], 3.0);
    assert_eq!(r["lead"]["min"], 0.04);
    assert_eq!(r["items"][0]["cycle"]["count"], 1);
    assert_eq!(r["items"][1]["lead"]["count"], 2);
    let tasks = r["tasks"].as_array().unwrap();
    let keys: Vec<&str> = tasks.iter().map(|t| t["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["TM-1", "TM-4", "TM-2"]);
    assert_eq!(tasks[2]["reopened"], 1);
    assert_eq!(tasks[2]["closed_day"], "2026-11-14");
    assert!(tasks[1].get("cycle_days").is_none());
    let csv = r["csv"].as_str().unwrap();
    assert!(csv.contains("\r\nTM-4,D,"), "{csv}");

    // Only the agent's closes.
    let agent = ws.call(
        "report.cycle_time",
        json!({ "from": RANGE.0, "to": RANGE.1, "actor": "Robo" }),
    );
    assert_eq!(agent["lead"]["count"], 1);
    assert_eq!(agent["cycle"]["count"], 0);
    assert!(agent.get("tasks").is_none());
}

#[test]
fn periods_follow_the_personal_time_zone() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    // Sunday 23:30 UTC is Monday 00:30 in Berlin.
    ws.at(MONDAY + 6 * DAY + 17 * HOUR + 30 * 60_000);
    ws.create(&Context::user(Via::App), "Late", json!({}));
    ws.at(MONDAY + 20 * DAY);
    let input = json!({ "from": RANGE.0, "to": RANGE.1 });
    assert_eq!(
        column(&ws.call("report.created_closed", input.clone()), "created"),
        [0, 1]
    );
    ws.engine.set_time_zone("UTC").unwrap();
    assert_eq!(
        column(&ws.call("report.created_closed", input), "created"),
        [1, 0]
    );
}

#[test]
fn filters_narrow_the_tasks_as_they_are_now() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    two_weeks(&ws);
    ws.call(
        "project.create",
        json!({ "name": "Child", "key": "CH", "parent": "TM" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "CH", "title": "Sub", "status": "To do" }),
    );
    let created = |input: Value| -> u64 {
        let mut input = input;
        input["from"] = json!(RANGE.0);
        input["to"] = json!("2026-11-22");
        ws.call("report.created_closed", input)["created"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(created(json!({})), 5);
    assert_eq!(created(json!({ "project": "TM" })), 5, "with sub-projects");
    assert_eq!(
        created(json!({ "project": "TM", "include_subprojects": false })),
        4
    );
    assert_eq!(created(json!({ "project": "CH" })), 1);
    assert_eq!(created(json!({ "type": "bug" })), 1);
    assert_eq!(created(json!({ "label": "UI" })), 1);
    assert_eq!(created(json!({ "query": "is:open" })), 2);

    assert_eq!(
        ws.fails("report.created_closed", json!({ "type": "Nope" }))
            .details["path"],
        "type"
    );
    assert_eq!(
        ws.fails("report.created_closed", json!({ "label": "nope" }))
            .details["path"],
        "label"
    );
    assert_eq!(
        ws.fails("report.cycle_time", json!({ "query": "(" }))
            .details["path"],
        "query"
    );
    let too_long = ws.fails(
        "report.cumulative_flow",
        json!({ "from": "2020-01-01", "to": "2026-01-01", "bucket": "day" }),
    );
    assert_eq!(too_long.details["path"], "from");
    let backwards = ws.fails(
        "report.cycle_time",
        json!({ "from": "2026-11-10", "to": "2026-11-01" }),
    );
    assert_eq!(backwards.details["path"], "to");

    // The default range: the twelve weeks up to today (Sunday 2026-11-22).
    let r = ws.call("report.cumulative_flow", json!({}));
    assert_eq!(r["items"].as_array().unwrap().len(), 12);
    assert_eq!(r["to"], "2026-11-22");
}

#[test]
fn a_removed_status_keeps_its_band_in_earlier_periods() {
    let dir = TempDir::new().unwrap();
    let ws = open(&dir);
    let me = Context::user(Via::App);
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    let a = ws.create(&me, "A", json!({}));
    ws.at(MONDAY + DAY);
    ws.move_to(&me, &a, "Review");
    ws.at(MONDAY + 9 * DAY);
    let sure = Context::user(Via::App).with_confirmation(gk_engine::Confirmation::Given);
    ws.call_as(
        &sure,
        "status.delete",
        json!({ "project": "TM", "status": "Review", "move_tasks_to": "Done" }),
    );
    ws.at(MONDAY + 20 * DAY);
    let r = ws.call(
        "report.cumulative_flow",
        json!({ "from": RANGE.0, "to": RANGE.1 }),
    );
    let (names, counts) = flow(&r);
    let review = names
        .iter()
        .position(|n| n == "Review")
        .expect("Review band");
    let done = names.iter().position(|n| n == "Done").unwrap();
    assert_eq!((counts[0][review], counts[1][review]), (1, 0));
    assert_eq!((counts[0][done], counts[1][done]), (0, 1));
    // The move by the removal is a close.
    let closed = ws.call(
        "report.created_closed",
        json!({ "from": RANGE.0, "to": RANGE.1 }),
    );
    assert_eq!(column(&closed, "closed"), [0, 1]);
}
