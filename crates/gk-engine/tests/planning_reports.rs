// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The planning reports over a real workspace: sprint velocity replayed
//! from the history (carried-over tasks, work added mid-sprint, each
//! measure), the burn-up of a version and of a sprint with its forecast,
//! estimate accuracy with its groups, and the dashboard widgets that show
//! them.

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
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

struct Ws {
    _dir: TempDir,
    clock: Arc<AtomicI64>,
    e: Engine,
}

impl Ws {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(NOW));
        let tick = Arc::clone(&clock);
        let mut config = EngineConfig::new(Role::App);
        config.clock = Arc::new(move || tick.load(Ordering::SeqCst));
        config.time_zone = Some("UTC".to_owned());
        let e = Engine::open(&WorkspaceLayout::new(dir.path().join("W")), config).unwrap();
        let ws = Self {
            _dir: dir,
            clock,
            e,
        };
        ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
        ws.call(
            "field.create",
            json!({ "project": "TM", "name": "Points", "kind": "integer" }),
        );
        ws
    }

    fn at(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    fn call(&self, op: &str, input: Value) -> Value {
        self.e
            .call(&Context::user(Via::App), op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn code(&self, op: &str, input: Value) -> ErrorCode {
        self.e
            .call(&Context::user(Via::App), op, input)
            .expect_err(op)
            .code
    }

    fn task(&self, title: &str, extra: Value) -> String {
        let mut input = json!({ "project": "TM", "title": title });
        for (k, v) in extra.as_object().unwrap() {
            input[k] = v.clone();
        }
        self.call("task.create", input)["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn sized(&self, title: &str, estimate: &str, points: i64) -> String {
        self.task(
            title,
            json!({ "estimate": estimate, "fields": { "Points": points } }),
        )
    }

    fn done(&self, key: &str) {
        let project = self.call("project.get", json!({ "project": "TM" }));
        let done = project["statuses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["category"] == "done")
            .unwrap()["name"]
            .clone();
        self.call("task.move", json!({ "key": key, "status": done }));
    }
}

fn numbers(report: &Value, list: &str, field: &str) -> Vec<f64> {
    report[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[field].as_f64().unwrap())
        .collect()
}

/// Four sprints: one carries a task over, one gets work added mid-way,
/// one gets a task that was closed before it began.
fn four_sprints(ws: &Ws) {
    let a = ws.sized("A", "2h", 3);
    let b = ws.sized("B", "1h", 2);
    let c = ws.sized("C", "4h", 5);
    let d = ws.sized("D", "1h", 1);
    let f = ws.sized("F", "3h", 2);
    let g = ws.sized("G", "1h", 1);
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S1", "tasks": [a, b, c] }),
    );
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S2", "tasks": [d] }),
    );
    // Sprint 1: A and B are done, C carries over to sprint 2.
    ws.call("sprint.start", json!({ "sprint": "S1", "project": "TM" }));
    ws.at(NOW + HOUR);
    ws.done(&a);
    ws.done(&b);
    ws.at(NOW + DAY);
    ws.call(
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM", "move_open_tasks_to": "S2" }),
    );
    // Sprint 2: C and D at the start; E added mid-sprint; C and E done.
    ws.at(NOW + DAY + HOUR);
    ws.call("sprint.start", json!({ "sprint": "S2", "project": "TM" }));
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S3", "tasks": [f] }),
    );
    ws.at(NOW + DAY + 2 * HOUR);
    let e = ws.sized("E", "2h", 8);
    ws.call("task.update", json!({ "key": e, "sprint": "active" }));
    ws.done(&c);
    ws.done(&e);
    ws.at(NOW + 2 * DAY);
    ws.call(
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM", "move_open_tasks_to": "S3" }),
    );
    // Sprint 3: D and F; F done; D back to the backlog.
    ws.at(NOW + 2 * DAY + HOUR);
    ws.call("sprint.start", json!({ "sprint": "S3", "project": "TM" }));
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S4", "tasks": [g] }),
    );
    ws.at(NOW + 2 * DAY + 2 * HOUR);
    ws.done(&f);
    ws.at(NOW + 3 * DAY);
    ws.call(
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM" }),
    );
    // H was done before sprint 4 began; adding it doesn't make it sprint
    // work.
    let h = ws.sized("H", "5h", 5);
    ws.done(&h);
    // Sprint 4: G done.
    ws.at(NOW + 3 * DAY + HOUR);
    ws.call("sprint.start", json!({ "sprint": "S4", "project": "TM" }));
    ws.call("task.update", json!({ "key": h, "sprint": "active" }));
    ws.at(NOW + 3 * DAY + 2 * HOUR);
    ws.done(&g);
    ws.at(NOW + 4 * DAY);
    ws.call(
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM" }),
    );
    // Sprint 5 is planned with D.
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S5", "tasks": [d] }),
    );
}

#[test]
fn velocity_replays_committed_and_completed_work_in_each_measure() {
    let ws = Ws::new();
    four_sprints(&ws);

    let tasks = ws.call("report.velocity", json!({ "project": "TM" }));
    let names: Vec<&str> = tasks["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["sprint"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["S1", "S2", "S3", "S4"]);
    assert_eq!(tasks["measure"], "tasks");
    assert_eq!(
        numbers(&tasks, "sprints", "committed"),
        [3.0, 2.0, 2.0, 1.0]
    );
    assert_eq!(
        numbers(&tasks, "sprints", "completed"),
        [2.0, 2.0, 1.0, 1.0]
    );
    assert_eq!(
        numbers(&tasks, "sprints", "average"),
        [2.0, 2.0, 1.67, 1.33]
    );
    assert_eq!(tasks["average"], 1.33);
    assert_eq!(tasks["average_of"], 3);
    assert_eq!(tasks["sprints"][0]["completed_on"], "2026-11-24");
    let upcoming = &tasks["upcoming"][0];
    assert_eq!(upcoming["sprint"]["name"], "S5");
    assert_eq!(upcoming["planned"], 1.0);

    let hours = ws.call(
        "report.velocity",
        json!({ "project": "TM", "measure": "estimate" }),
    );
    assert_eq!(
        numbers(&hours, "sprints", "committed"),
        [7.0, 5.0, 4.0, 1.0]
    );
    assert_eq!(
        numbers(&hours, "sprints", "completed"),
        [3.0, 6.0, 3.0, 1.0]
    );
    assert_eq!(hours["average"], 3.33);
    assert_eq!(hours["sprints"][1]["completed_tasks"], 2);

    let points = ws.call(
        "report.velocity",
        json!({ "project": "TM", "measure": "field", "field": "cf.points", "csv": true }),
    );
    assert_eq!(points["field"], "Points");
    assert_eq!(
        numbers(&points, "sprints", "committed"),
        [10.0, 6.0, 3.0, 1.0]
    );
    assert_eq!(
        numbers(&points, "sprints", "completed"),
        [5.0, 13.0, 2.0, 1.0]
    );
    assert_eq!(points["average"], 5.33);
    let csv = points["csv"].as_str().unwrap();
    assert!(csv.starts_with("sprint,start_date,end_date,completed_on,committed,"));
    assert!(csv.contains("S2,2026-11-24,"), "{csv}");

    // The last two only; the average still covers the last three.
    let last = ws.call("report.velocity", json!({ "project": "TM", "last": 2 }));
    assert_eq!(numbers(&last, "sprints", "completed"), [1.0, 1.0]);
    assert_eq!(last["average"], 1.33);

    // A field is named only with that measure, and must hold numbers.
    assert_eq!(
        ws.code(
            "report.velocity",
            json!({ "project": "TM", "measure": "field" })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        ws.code(
            "report.velocity",
            json!({ "project": "TM", "field": "cf.points" })
        ),
        ErrorCode::Invalid
    );
    ws.call(
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "text" }),
    );
    assert_eq!(
        ws.code(
            "report.velocity",
            json!({ "project": "TM", "measure": "field", "field": "Area" })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        ws.code("report.velocity", json!({ "project": "TM", "last": 0 })),
        ErrorCode::Invalid
    );
}

#[test]
fn velocity_averages_fewer_than_three_sprints() {
    let ws = Ws::new();
    let none = ws.call("report.velocity", json!({ "project": "TM" }));
    assert_eq!(none["sprints"], json!([]));
    assert!(none.get("average").is_none());
    assert_eq!(none["average_of"], 0);

    let a = ws.sized("A", "2h", 3);
    let b = ws.sized("B", "1h", 2);
    ws.call(
        "sprint.create",
        json!({ "project": "TM", "name": "S1", "tasks": [a, b] }),
    );
    ws.call("sprint.start", json!({ "sprint": "S1", "project": "TM" }));
    ws.at(NOW + HOUR);
    ws.done(&a);
    ws.done(&b);
    ws.at(NOW + DAY);
    ws.call(
        "sprint.complete",
        json!({ "sprint": "active", "project": "TM" }),
    );
    let one = ws.call("report.velocity", json!({ "project": "TM" }));
    assert_eq!(one["average"], 2.0);
    assert_eq!(one["average_of"], 1);
    assert_eq!(one["upcoming"], json!([]));
}

#[test]
fn burnup_shows_scope_growth_completed_work_and_a_forecast() {
    let ws = Ws::new();
    ws.call(
        "version.create",
        json!({ "project": "TM", "name": "1.0", "start_date": "2026-11-23",
                "due_date": "2026-12-04" }),
    );
    let a = ws.task("A", json!({ "fix_version": "1.0", "estimate": "2h" }));
    let b = ws.task("B", json!({ "fix_version": "1.0", "estimate": "1h" }));
    let c = ws.task("C", json!({ "estimate": "3h" }));
    // Day 1: A done. Day 2: C planned for it (the scope steps up), B done.
    ws.at(NOW + DAY);
    ws.done(&a);
    let early = ws.call(
        "report.burnup",
        json!({ "version": "1.0", "project": "TM" }),
    );
    assert_eq!(early["days"].as_array().unwrap().len(), 2);
    assert!(early.get("forecast").is_none(), "fewer than three days");
    ws.at(NOW + 2 * DAY);
    ws.call("task.update", json!({ "key": c, "fix_version": "1.0" }));
    ws.done(&b);
    ws.at(NOW + 3 * DAY);

    let report = ws.call(
        "report.burnup",
        json!({ "version": "1.0", "project": "TM", "csv": true }),
    );
    assert_eq!(report["from"], "2026-11-23");
    assert_eq!(report["to"], "2026-11-26");
    assert_eq!(report["end"], "2026-12-04");
    assert_eq!(report["version"]["name"], "1.0");
    assert_eq!(numbers(&report, "days", "scope"), [2.0, 2.0, 3.0, 3.0]);
    assert_eq!(numbers(&report, "days", "completed"), [0.0, 1.0, 2.0, 2.0]);
    // Least squares through (0, 0), (1, 1), (2, 2), (3, 2): 0.2 + 0.7 x,
    // which reaches 3 at x = 4.
    assert_eq!(report["forecast"]["per_day"], 0.7);
    assert_eq!(report["forecast"]["start"], 0.2);
    assert_eq!(report["forecast"]["date"], "2026-11-27");
    assert!(
        report["csv"]
            .as_str()
            .unwrap()
            .contains("2026-11-25,3,2,3,2")
    );

    let hours = ws.call(
        "report.burnup",
        json!({ "version": "1.0", "project": "TM", "measure": "estimate", "forecast": false }),
    );
    assert_eq!(numbers(&hours, "days", "scope"), [3.0, 3.0, 6.0, 6.0]);
    assert_eq!(numbers(&hours, "days", "completed"), [0.0, 2.0, 3.0, 3.0]);
    assert!(hours.get("forecast").is_none());

    // Everything done: no forecast.
    ws.done(&c);
    let finished = ws.call(
        "report.burnup",
        json!({ "version": "1.0", "project": "TM" }),
    );
    assert!(finished.get("forecast").is_none());

    assert_eq!(
        ws.code("report.burnup", json!({ "project": "TM" })),
        ErrorCode::Invalid
    );
}

#[test]
fn burnup_of_a_sprint_starts_with_it_and_ends_when_it_is_completed() {
    let ws = Ws::new();
    four_sprints(&ws);
    let s2 = ws.call("report.burnup", json!({ "sprint": "S2", "project": "TM" }));
    assert_eq!(s2["sprint"]["name"], "S2");
    assert_eq!(s2["from"], "2026-11-24");
    assert_eq!(s2["to"], "2026-11-25");
    // Day 1: C, D and E (added that day), C and E done. Day 2: as it stood
    // just before it was completed.
    assert_eq!(numbers(&s2, "days", "scope"), [3.0, 3.0]);
    assert_eq!(numbers(&s2, "days", "completed"), [2.0, 2.0]);
    assert!(s2.get("forecast").is_none());
    assert_eq!(
        ws.code("report.burnup", json!({ "sprint": "S5", "project": "TM" })),
        ErrorCode::Conflict
    );
}

#[test]
fn estimate_accuracy_compares_spent_with_estimated_time() {
    let ws = Ws::new();
    ws.call("work_kind.create", json!({ "name": "Review" }));
    for name in ["ui", "api"] {
        ws.call("label.create", json!({ "project": "TM", "name": name }));
    }
    let closed = |title: &str, extra: Value, logged: &[(&str, Option<&str>)]| {
        let key = ws.task(title, extra);
        for (duration, kind) in logged {
            let mut input = json!({ "key": key, "duration": duration });
            if let Some(kind) = kind {
                input["work_kind"] = json!(kind);
            }
            ws.call("time.log", input);
        }
        ws.done(&key);
        key
    };
    let t1 = closed(
        "One",
        json!({ "estimate": "1h", "labels": ["ui"] }),
        &[("60m", None)],
    );
    let t2 = closed(
        "Two",
        json!({ "estimate": "1h", "labels": ["ui", "api"] }),
        &[("60m", Some("Review")), ("30m", None)],
    );
    let t3 = closed("Three", json!({ "estimate": "2h" }), &[("96m", None)]);
    let t4 = closed(
        "Four",
        json!({ "estimate": "1h", "type": "Bug", "labels": ["api"] }),
        &[("30m", None)],
    );
    let t5 = closed(
        "Five",
        json!({ "estimate": "30m", "type": "Bug" }),
        &[("36m", None)],
    );
    closed("No estimate", json!({}), &[("1h", None)]);
    closed("No time", json!({ "estimate": "1h" }), &[]);
    // Open tasks are not compared.
    let open = ws.task("Open", json!({ "estimate": "1h" }));
    ws.call("time.log", json!({ "key": open, "duration": "5h" }));

    let all = ws.call(
        "report.estimates",
        json!({ "project": "TM", "tasks": true }),
    );
    let total = &all["total"];
    assert_eq!(total["count"], 5);
    assert_eq!(total["estimate_minutes"], 330);
    assert_eq!(total["spent_minutes"], 312);
    // Ratios 1.0, 1.5, 0.8, 0.5, 1.2: median 1.0; 0.8, 1.0 and 1.2 are
    // within 20 %.
    assert_eq!(total["median_ratio"], 1.0);
    assert_eq!(total["within_share"], 0.6);
    assert_eq!(total["over"], 1);
    assert_eq!(total["under"], 1);
    let outliers: Vec<&str> = total["outliers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap())
        .collect();
    assert_eq!(outliers, [&t4, &t2, &t3, &t5, &t1]);
    assert_eq!(total["outliers"][1]["ratio"], 1.5);
    assert_eq!(all["without_estimate"], 1);
    assert_eq!(all["without_time"], 1);
    assert_eq!(all["groups"], json!([]));
    assert_eq!(all["tasks"].as_array().unwrap().len(), 5);
    assert_eq!(all["from"], "2026-08-26");

    let by_type = ws.call(
        "report.estimates",
        json!({ "project": "TM", "group_by": "type" }),
    );
    let groups = by_type["groups"].as_array().unwrap();
    assert_eq!(groups[0]["name"], "Task");
    assert_eq!(groups[0]["count"], 3);
    assert_eq!(groups[0]["median_ratio"], 1.0);
    assert_eq!(groups[0]["within_share"], 0.6667);
    assert_eq!(groups[1]["name"], "Bug");
    assert_eq!(groups[1]["median_ratio"], 0.85);
    assert_eq!(groups[1]["within_share"], 0.5);
    assert_eq!(groups[1]["estimate_minutes"], 90);
    assert_eq!(groups[1]["spent_minutes"], 66);

    let by_label = ws.call(
        "report.estimates",
        json!({ "project": "TM", "group_by": "label" }),
    );
    let labels: Vec<(String, u64, f64)> = by_label["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["name"].as_str().unwrap().to_owned(),
                g["count"].as_u64().unwrap(),
                g["median_ratio"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        labels,
        [
            ("api".to_owned(), 2, 1.0),
            ("ui".to_owned(), 2, 1.25),
            (String::new(), 2, 1.0)
        ]
    );

    let by_kind = ws.call(
        "report.estimates",
        json!({ "project": "TM", "group_by": "work_kind" }),
    );
    let kinds: Vec<(String, u64)> = by_kind["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["name"].as_str().unwrap().to_owned(),
                g["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(kinds, [(String::new(), 4), ("Review".to_owned(), 1)]);

    let by_month = ws.call(
        "report.estimates",
        json!({ "project": "TM", "group_by": "month" }),
    );
    assert_eq!(by_month["groups"][0]["key"], "2026-11");

    // A query narrows it; a range before the closings has none.
    let bugs = ws.call(
        "report.estimates",
        json!({ "project": "TM", "query": "type:Bug" }),
    );
    assert_eq!(bugs["total"]["count"], 2);
    let before = ws.call(
        "report.estimates",
        json!({ "from": "2026-11-01", "to": "2026-11-22" }),
    );
    assert_eq!(before["total"]["count"], 0);
    assert!(before["total"].get("median_ratio").is_none());
    assert_eq!(
        ws.code(
            "report.estimates",
            json!({ "from": "2026-11-23", "to": "2026-11-22" })
        ),
        ErrorCode::Invalid
    );
}

#[test]
fn widgets_show_the_same_figures_as_the_reports() {
    let ws = Ws::new();
    four_sprints(&ws);
    ws.call(
        "version.create",
        json!({ "project": "TM", "name": "1.0", "start_date": "2026-11-23" }),
    );
    let made = ws.call(
        "dashboard.create",
        json!({ "name": "Plan", "project": "TM", "widgets": [
            { "kind": "velocity", "settings": { "measure": "field", "field": "cf.points" } },
            { "kind": "burnup", "settings": { "version": "1.0" } },
            { "kind": "estimate_accuracy", "settings": { "days": 30 } }
        ] }),
    );
    assert_eq!(made["dashboard"]["widgets"][0]["width"], 2);
    let shown = ws.call(
        "dashboard.show",
        json!({ "dashboard": "Plan", "project": "TM" }),
    );
    let velocity = ws.call(
        "report.velocity",
        json!({ "project": "TM", "measure": "field", "field": "cf.points" }),
    );
    assert_eq!(shown["widgets"][0]["velocity"], velocity);
    let burnup = ws.call(
        "report.burnup",
        json!({ "version": "1.0", "project": "TM" }),
    );
    assert_eq!(shown["widgets"][1]["burnup"], burnup);
    let estimates = ws.call(
        "report.estimates",
        json!({ "project": "TM", "from": "2026-10-29", "group_by": "type" }),
    );
    assert_eq!(shown["widgets"][2]["estimates"], estimates);

    // Settings are checked as the report checks its input.
    assert_eq!(
        ws.code(
            "dashboard.add_widget",
            json!({ "dashboard": "Plan", "project": "TM",
                    "widget": { "kind": "velocity", "settings": { "measure": "field" } } })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        ws.code(
            "dashboard.add_widget",
            json!({ "dashboard": "Plan", "project": "TM",
                    "widget": { "kind": "velocity", "settings": { "version": "1.0" } } })
        ),
        ErrorCode::Invalid
    );
}
