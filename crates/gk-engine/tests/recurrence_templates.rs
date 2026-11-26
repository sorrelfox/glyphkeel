// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Recurring tasks, task and project templates, and default due dates per
//! task type, through the operations: next occurrences on closing and ahead
//! of time, the person's time zone and clock changes, skipping, Undo, and
//! what templates make.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// 2026-11-26T12:00:00Z, a Thursday.
const NOW: i64 = 1_795_694_400_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
    _dir: TempDir,
}

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&Context::user(Via::App), op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn fails(&self, op: &str, input: Value) -> gk_api::OpError {
        self.engine
            .call(&Context::user(Via::App), op, input)
            .unwrap_err()
    }

    fn at(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    fn task(&self, key: &str) -> Value {
        self.call("task.get", json!({ "keys": [key] }))["items"][0].clone()
    }

    fn close(&self, key: &str) -> Value {
        self.call("task.move", json!({ "key": key, "status": "Done" }))
    }
}

fn open(zone: &str, now: i64) -> Ws {
    let ws = open_empty(zone, now);
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    ws
}

fn open_empty(zone: &str, now: i64) -> Ws {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(AtomicI64::new(now));
    let tick = Arc::clone(&clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.load(Ordering::SeqCst));
    config.time_zone = Some(zone.to_owned());
    let engine = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
    Ws {
        engine,
        clock,
        _dir: dir,
    }
}

fn keys(v: &Value) -> Vec<String> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn closing_a_weekly_task_makes_the_next_one_with_its_details_and_undo_takes_it_back() {
    let ws = open("Europe/Berlin", NOW);
    ws.call("label.create", json!({ "project": "TM", "name": "chores" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Water the plants", "labels": ["chores"],
                "description": "- [ ] kitchen\n- [ ] office\n", "estimate": "15m" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Fill the can", "parent": "TM-1" }),
    );
    let set = ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every monday and thursday" }),
    );
    let rec = &set["recurrence"];
    assert_eq!(rec["summary"], "Every week on Monday and Thursday", "{set}");
    assert_eq!(
        set["due_date"], "2026-11-26",
        "a task without dates gets the first date"
    );
    assert_eq!(rec["next_date"], "2026-11-30");
    assert_eq!(ws.task("TM-1")["recurrence"]["made"], 1);

    // The person ticks an item and edits this occurrence only.
    ws.call(
        "task.update",
        json!({ "key": "TM-1", "description": "- [x] kitchen\n- [ ] office\n",
                "title": "Water the plants (balcony too)" }),
    );
    let closed = ws.close("TM-1");
    assert!(closed["task"]["closed_at"].is_string());
    assert!(
        ws.task("TM-1").get("recurrence").is_none(),
        "no longer the current one"
    );
    let next = ws.task("TM-3");
    assert_eq!(next["title"], "Water the plants");
    assert_eq!(next["due_date"], "2026-11-30");
    assert_eq!(next["description"], "- [ ] kitchen\n- [ ] office\n");
    assert_eq!(next["labels"][0]["name"], "chores");
    assert_eq!(next["estimate_minutes"], 15);
    assert_eq!(next["subtask_count"]["total"], 1);
    assert_eq!(next["recurrence"]["made"], 2);
    assert_eq!(next["recurrence"]["next_date"], "2026-12-03");
    let listed = ws.call("task.search", json!({ "query": "is:open" }));
    let row = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "TM-3")
        .unwrap();
    assert_eq!(row["recurring"], true);

    // Undo the close: the occurrence goes, the first one is current again.
    ws.call("history.undo", json!({}));
    assert!(ws.task("TM-1")["closed_at"].is_null());
    assert_eq!(ws.task("TM-1")["recurrence"]["made"], 1);
    let open_now = keys(&ws.call("task.search", json!({ "query": "is:open" })));
    assert!(!open_now.contains(&"TM-3".to_owned()), "{open_now:?}");
}

#[test]
fn details_for_the_series_are_saved_only_when_asked() {
    let ws = open("Europe/Berlin", NOW);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Report", "due_date": "2026-11-27" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "monthly on the last fri" }),
    );
    ws.call(
        "task.update",
        json!({ "key": "TM-1", "title": "Monthly report" }),
    );
    assert_eq!(ws.task("TM-1")["recurrence"]["title"], "Report");
    let changed = ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "update_details": true }),
    );
    assert_eq!(changed["recurrence"]["title"], "Monthly report");
    assert_eq!(
        changed["recurrence"]["summary"], "Every month on the last Friday",
        "the rule stays"
    );
    ws.close("TM-1");
    assert_eq!(ws.task("TM-2")["title"], "Monthly report");
    assert_eq!(ws.task("TM-2")["due_date"], "2026-12-25");
}

#[test]
fn repeating_from_completion_counts_from_the_persons_day() {
    // 23:30 in Berlin on the 26th is already the 27th in Kiritimati.
    let late = NOW + 10 * HOUR + 30 * 60_000;
    for (zone, due) in [
        ("Europe/Berlin", "2026-11-29"),
        ("Pacific/Kiritimati", "2026-11-30"),
    ] {
        let ws = open(zone, NOW);
        ws.call(
            "task.create",
            json!({ "project": "TM", "title": "Descale", "due_date": "2026-11-20" }),
        );
        ws.call(
            "recurrence.set",
            json!({ "key": "TM-1", "every": "day", "interval": 3, "from": "completion" }),
        );
        ws.at(late);
        ws.close("TM-1");
        assert_eq!(ws.task("TM-2")["due_date"], due, "{zone}");
    }
}

#[test]
fn dates_already_past_are_passed_over_and_count() {
    let ws = open("UTC", NOW);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Stand-up", "due_date": "2026-11-02" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every monday, 5 times" }),
    );
    ws.close("TM-1");
    let next = ws.task("TM-2");
    assert_eq!(next["due_date"], "2026-11-30");
    // The 9th, 16th and 23rd were passed over.
    assert_eq!(next["recurrence"]["made"], 5);
    assert!(next["recurrence"].get("next_date").is_none());
    ws.close("TM-2");
    let open_now = keys(&ws.call("task.search", json!({ "query": "is:open" })));
    assert!(open_now.is_empty(), "the rule ran out: {open_now:?}");
}

#[test]
fn skipping_moves_the_dates_and_the_last_one_cannot_be_skipped() {
    let ws = open("UTC", NOW);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Backup", "start_date": "2026-11-28",
                "due_date": "2026-11-30" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "weekly on mon, 2 times" }),
    );
    let skipped = ws.call("recurrence.skip", json!({ "key": "TM-1" }));
    assert_eq!(skipped["due_date"], "2026-12-07");
    assert_eq!(ws.task("TM-1")["start_date"], "2026-12-05");
    let err = ws.fails("recurrence.skip", json!({ "key": "TM-1" }));
    assert_eq!(err.code, ErrorCode::Conflict);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.task("TM-1")["due_date"], "2026-11-30");
}

#[test]
fn stopping_keeps_the_task_and_makes_nothing_more() {
    let ws = open("UTC", NOW);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Water", "due_date": "2026-11-26" }),
    );
    ws.call("recurrence.set", json!({ "key": "TM-1", "text": "daily" }));
    let stopped = ws.call("recurrence.clear", json!({ "key": "TM-1" }));
    assert!(stopped.get("recurrence").is_none());
    assert!(ws.task("TM-1").get("recurrence").is_none());
    ws.close("TM-1");
    assert!(keys(&ws.call("task.search", json!({ "query": "is:open" }))).is_empty());
    let err = ws.fails("recurrence.set", json!({ "key": "TM-1", "text": "daily" }));
    assert_eq!(
        err.code,
        ErrorCode::Invalid,
        "a closed task is not made recurring"
    );
}

#[test]
fn series_made_ahead_get_their_occurrence_on_the_persons_midnight_across_clock_changes() {
    // Chile moves its clocks at midnight: 2026-09-06 starts at 01:00.
    let sat = 1_788_609_600_000; // 2026-09-05T12:00:00Z
    let ws = open("America/Santiago", sat);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Plan the week", "due_date": "2026-08-30" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every sunday", "create": "ahead" }),
    );
    let run = ws.call("recurrence.run", json!({}));
    assert!(run["created"].as_array().unwrap().is_empty(), "{run}");
    assert_eq!(run["next_at"], "2026-09-06T04:00:00.000Z");
    // The previous occurrence is still open; the next is made anyway.
    ws.at(sat + 16 * HOUR + 1); // 2026-09-06T04:00:00.001Z
    let run = ws.call("recurrence.run", json!({}));
    assert_eq!(run["created"][0]["key"], "TM-2", "{run}");
    assert_eq!(ws.task("TM-2")["due_date"], "2026-09-06");
    assert!(ws.task("TM-1")["closed_at"].is_null());
    assert_eq!(run["next_at"], "2026-09-13T03:00:00.000Z");

    // Berlin leaves summer time on 2026-10-25: that midnight is UTC+2, the
    // next one UTC+1.
    let ws = open("Europe/Berlin", 1_792_843_200_000); // 2026-10-24T12:00:00Z
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Daily log", "due_date": "2026-10-24" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "daily", "create": "ahead" }),
    );
    assert_eq!(
        ws.call("recurrence.run", json!({}))["next_at"],
        "2026-10-24T22:00:00.000Z"
    );
    ws.at(1_792_843_200_000 + DAY + 12 * HOUR); // 2026-10-26T00:00:00Z
    let run = ws.call("recurrence.run", json!({}));
    assert_eq!(run["created"][0]["key"], "TM-2", "{run}");
    assert_eq!(
        ws.task("TM-2")["due_date"],
        "2026-10-26",
        "the missed 25th is passed over"
    );
    assert_eq!(run["next_at"], "2026-10-26T23:00:00.000Z");
}

#[test]
fn rules_are_previewed_and_checked() {
    let ws = open("UTC", NOW);
    let p = ws.call(
        "recurrence.preview",
        json!({ "text": "the 31st of each month", "starts": "2027-01-01", "limit": 3 }),
    );
    assert_eq!(
        p["dates"],
        json!(["2027-01-31", "2027-02-28", "2027-03-31"])
    );
    assert_eq!(p["summary"], "Every month on day 31");
    let p = ws.call(
        "recurrence.preview",
        json!({ "every": "week", "interval": 2, "weekdays": ["mon", "fri"], "count": 3 }),
    );
    assert_eq!(
        p["dates"],
        json!(["2026-11-27", "2026-12-07", "2026-12-11"])
    );
    assert_eq!(p["summary"], "Every 2 weeks on Monday and Friday, 3 times");
    let err = ws.fails("recurrence.preview", json!({ "text": "now and then" }));
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["message_id"], "value-recurrence-text");
    let err = ws.fails(
        "recurrence.preview",
        json!({ "text": "3 days after done", "create": "ahead" }),
    );
    assert_eq!(
        err.details["message_id"],
        "recurrence-ahead-from-completion"
    );
}

#[test]
fn a_type_can_give_new_tasks_a_due_date() {
    let ws = open("Pacific/Kiritimati", NOW);
    ws.call(
        "type.update",
        json!({ "project": "TM", "type": "Bug", "due_offset_days": 3 }),
    );
    let project = ws.call("project.get", json!({ "project": "TM" }));
    let bug = project["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "Bug")
        .unwrap()
        .clone();
    assert_eq!(bug["due_offset_days"], 3);
    // It is already the 27th in Kiritimati.
    let t = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Crash", "type": "Bug" }),
    );
    assert_eq!(t["task"]["due_date"], "2026-11-30");
    let t = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Given", "type": "Bug", "due_date": "2026-12-24" }),
    );
    assert_eq!(t["task"]["due_date"], "2026-12-24");
    let t = ws.call("task.create", json!({ "project": "TM", "title": "Plain" }));
    assert!(t["task"].get("due_date").is_none());
    ws.call(
        "type.update",
        json!({ "project": "TM", "type": "Bug", "clear_due_offset": true }),
    );
    let t = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Later", "type": "Bug" }),
    );
    assert!(t["task"].get("due_date").is_none());
    ws.call("history.undo", json!({}));
    ws.call("history.undo", json!({}));
    let project = ws.call("project.get", json!({ "project": "TM" }));
    assert!(
        project["types"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "Bug" && t["due_offset_days"] == 3),
        "Undo brings the offset back"
    );
}

#[test]
fn task_templates_keep_names_dates_subtasks_and_checklists() {
    let ws = open("UTC", NOW);
    ws.call("label.create", json!({ "project": "TM", "name": "crash" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Bug report", "type": "Bug", "labels": ["crash"],
                "priority": "high", "description": "## Steps\n\n- [x] reproduce\n- [ ] fix\n",
                "due_date": "2026-12-01" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Write a test", "parent": "TM-1" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Check CI", "parent": "TM-2" }),
    );
    let made = ws.call(
        "template.create",
        json!({ "name": "Bug", "from_task": "TM-1", "description": "For crashes" }),
    );
    let task = &made["template"]["task"];
    assert_eq!(task["due_in_days"], 5);
    assert_eq!(
        task["description"],
        "## Steps\n\n- [ ] reproduce\n- [ ] fix\n"
    );
    assert_eq!(task["subtasks"][1]["under"], 0);
    let err = ws.fails(
        "template.create",
        json!({ "name": "bug", "from_task": "TM-1" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);

    let used = ws.call(
        "template.apply",
        json!({ "template": "Bug", "project": "TM", "title": "Crash on save",
                "date": "2027-01-10" }),
    );
    assert_eq!(used["tasks"], 3);
    let t = &used["task"];
    assert_eq!(t["title"], "Crash on save");
    assert_eq!(t["type"]["name"], "Bug");
    assert_eq!(t["priority"], "high");
    assert_eq!(t["due_date"], "2027-01-15");
    assert_eq!(t["labels"][0]["name"], "crash");
    assert_eq!(t["subtask_count"]["total"], 1);

    // Another project without the label: it is left out and reported.
    ws.call("project.create", json!({ "name": "Other", "key": "OT" }));
    let used = ws.call(
        "template.apply",
        json!({ "template": made["template"]["id"], "project": "OT" }),
    );
    assert_eq!(used["left_out"], json!(["crash"]));
    assert_eq!(used["task"]["key"], "OT-1");
    ws.call("history.undo", json!({}));
    assert!(keys(&ws.call("task.search", json!({ "query": "project:OT" }))).is_empty());

    let list = ws.call("template.list", json!({ "project": "TM" }));
    assert_eq!(list["items"][0]["items"], 3);
    ws.call(
        "template.delete",
        json!({ "template": "Bug", "project": "TM" }),
    );
    assert_eq!(ws.call("template.list", json!({}))["total"], 0);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.call("template.list", json!({}))["total"], 1);
}

#[test]
fn project_templates_set_up_statuses_types_pages_tasks_and_task_templates() {
    let ws = open("UTC", NOW);
    let made = ws.call(
        "template.create",
        json!({
            "name": "Software project",
            "setup": {
                "description": "Code and releases",
                "statuses": [
                    { "name": "Ideas", "category": "todo", "hue": "grey" },
                    { "name": "Building", "category": "in_progress", "hue": "blue",
                      "wip_limit": 3 },
                    { "name": "Shipped", "category": "done", "hue": "green" }
                ],
                "types": [
                    { "name": "Story", "glyph": "feature", "hue": "blue", "is_default": true },
                    { "name": "Bug", "glyph": "bug", "hue": "red", "due_offset_days": 7 }
                ],
                "labels": [{ "name": "frontend", "hue": "purple" }],
                "wiki": [
                    { "title": "Overview", "body": "# Overview" },
                    { "title": "Releases", "parent": "Overview" }
                ],
                "tasks": [{ "title": "Set up CI", "due_in_days": 2, "labels": ["frontend"],
                            "subtasks": [{ "title": "Pick a runner" }] }],
                "task_templates": [{ "name": "Bug report",
                                     "task": { "title": "Bug", "type": "Bug" } }]
            }
        }),
    );
    assert_eq!(made["template"]["kind"], "project");
    let used = ws.call(
        "template.apply",
        json!({ "template": "Software project", "name": "App", "key": "APP" }),
    );
    let p = &used["project"];
    assert_eq!(p["key"], "APP");
    assert_eq!(p["description"], "Code and releases");
    let statuses: Vec<&str> = p["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["Ideas", "Building", "Shipped"]);
    assert_eq!(p["types"][0]["is_default"], true);
    assert_eq!(p["types"][1]["due_offset_days"], 7);
    assert_eq!(used["tasks"], 2);
    let ci = ws.task("APP-1");
    assert_eq!(ci["status"]["name"], "Ideas");
    assert_eq!(ci["due_date"], "2026-11-28");
    let pages = ws.call("wiki.list", json!({ "project": "APP" }));
    assert!(pages.to_string().contains("Releases"), "{pages}");
    let bug = ws.call(
        "template.apply",
        json!({ "template": "Bug report", "project": "APP" }),
    );
    assert_eq!(
        bug["task"]["due_date"], "2026-12-03",
        "the type's default due date"
    );

    // A project template made from that project carries the same set-up.
    let again = ws.call(
        "template.create",
        json!({ "name": "Copy", "from_project": "APP", "include_tasks": true }),
    );
    let setup = &again["template"]["setup"];
    assert_eq!(setup["statuses"][1]["wip_limit"], 3);
    assert_eq!(setup["wiki"][1]["parent"], "Overview");
    assert_eq!(setup["task_templates"][0]["name"], "Bug report");
    assert_eq!(setup["tasks"].as_array().unwrap().len(), 2);

    // Undo removes the project made from the template.
    ws.call("history.undo", json!({}));
    ws.call("history.undo", json!({}));
    ws.call("history.undo", json!({}));
    let projects = ws.call("project.list", json!({}));
    assert!(!projects.to_string().contains("\"APP\""), "{projects}");
}

#[test]
fn templates_and_series_travel_in_export_archives() {
    let ws = open("UTC", NOW);
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Water", "due_date": "2026-11-26" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every 2 days" }),
    );
    ws.call(
        "template.create",
        json!({ "name": "Watering", "from_task": "TM-1" }),
    );
    ws.call(
        "template.create",
        json!({ "name": "Garden", "setup": { "labels": [{ "name": "green", "hue": "green" }] } }),
    );
    ws.call(
        "type.update",
        json!({ "project": "TM", "type": "Bug", "due_offset_days": 4 }),
    );
    let archive = ws._dir.path().join("out.zip");
    ws.call(
        "transfer.export",
        json!({ "path": archive.to_string_lossy() }),
    );

    let other = open_empty("UTC", NOW);
    let imported = other.call(
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    );
    assert!(imported.to_string().contains("TM"), "{imported}");
    let water = other.task("TM-1");
    assert_eq!(water["recurrence"]["summary"], "Every 2 days");
    let templates = other.call("template.list", json!({}));
    let names: Vec<&str> = templates["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Garden", "Watering"]);
    let p = other.call("project.get", json!({ "project": "TM" }));
    assert!(p.to_string().contains("\"due_offset_days\":4"));
    other.close("TM-1");
    assert_eq!(other.task("TM-2")["due_date"], "2026-11-28");
}

fn field_values(task: &Value) -> Vec<(String, Value)> {
    task.get("fields")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|f| (f["name"].as_str().unwrap().to_owned(), f["value"].clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn version_name(task: &Value) -> Option<String> {
    task.get("fix_version")
        .and_then(|v| v["name"].as_str())
        .map(str::to_owned)
}

/// A project with a number field, a required select field and version 1.0,
/// and a task in it that has values and the version.
fn with_fields_and_version(ws: &Ws) {
    ws.call(
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    ws.call(
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "select",
                "options": ["Front end", "Back end"], "required": true }),
    );
    ws.call("version.create", json!({ "project": "TM", "name": "1.0" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Weekly review", "fix_version": "1.0",
                "fields": { "Points": 3, "Area": "Back end" } }),
    );
}

#[test]
fn occurrences_keep_fields_and_open_versions_and_are_made_despite_status_rules() {
    let ws = open("UTC", NOW);
    with_fields_and_version(&ws);
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every monday" }),
    );
    // New tasks start in Backlog, which now needs an estimate and sets a
    // priority on the way in.
    ws.call(
        "rules.update",
        json!({ "project": "TM", "statuses": [{ "status": "Backlog", "rules": {
            "required": ["estimate"],
            "set": [{ "field": "priority", "value": "high" }],
        } }] }),
    );
    let err = ws.fails(
        "task.create",
        json!({ "project": "TM", "title": "By hand", "fields": { "Area": "Front end" } }),
    );
    assert_eq!(err.details["message_id"], "rule-required", "{err:?}");

    ws.close("TM-1");
    let next = ws.task("TM-2");
    assert_eq!(next["title"], "Weekly review");
    assert_eq!(next["recurrence"]["made"], 2);
    assert_eq!(
        field_values(&next),
        [
            ("Points".to_owned(), json!(3)),
            ("Area".to_owned(), json!("Back end"))
        ]
    );
    assert_eq!(version_name(&next).as_deref(), Some("1.0"));
    assert_eq!(next["priority"], "high", "the status's action applies");
    assert!(next["estimate"].is_null(), "made without the estimate");

    // A version that is no longer open isn't given to the next one.
    ws.call(
        "version.update",
        json!({ "version": "1.0", "project": "TM", "status": "locked" }),
    );
    ws.close("TM-2");
    let third = ws.task("TM-3");
    assert_eq!(third["recurrence"]["made"], 3);
    assert_eq!(version_name(&third), None);
    assert_eq!(field_values(&third).len(), 2);
}

#[test]
fn task_templates_keep_fields_and_versions_and_follow_the_rules_when_used() {
    let ws = open("UTC", NOW);
    with_fields_and_version(&ws);
    let made = ws.call(
        "template.create",
        json!({ "name": "Review", "from_task": "TM-1" }),
    );
    let task = &made["template"]["task"];
    assert_eq!(task["fields"], json!({ "Area": "Back end", "Points": 3 }));
    assert_eq!(task["fix_version"], "1.0");

    let used = ws.call(
        "template.apply",
        json!({ "template": "Review", "project": "TM" }),
    );
    assert_eq!(field_values(&used["task"]).len(), 2);
    assert_eq!(version_name(&used["task"]).as_deref(), Some("1.0"));
    assert!(used.get("left_out").is_none(), "{used}");

    // Using a template is creating a task: the status's rules hold.
    ws.call(
        "rules.update",
        json!({ "project": "TM", "statuses": [{ "status": "Backlog",
                "rules": { "required": ["estimate"] } }] }),
    );
    let err = ws.fails(
        "template.apply",
        json!({ "template": "Review", "project": "TM" }),
    );
    assert_eq!(err.details["message_id"], "rule-required", "{err:?}");
    ws.call(
        "rules.update",
        json!({ "project": "TM", "statuses": [{ "status": "Backlog", "rules": {} }] }),
    );

    // A required field the template has no value for is refused, as by hand.
    ws.call(
        "template.create",
        json!({ "name": "Bare", "project": "TM", "task": { "title": "Bare" } }),
    );
    let err = ws.fails(
        "template.apply",
        json!({ "template": "Bare", "project": "TM" }),
    );
    assert_eq!(err.details["message_id"], "field-required", "{err:?}");

    // A version no longer open, and fields the project lacks, are left out.
    ws.call(
        "version.update",
        json!({ "version": "1.0", "project": "TM", "status": "closed" }),
    );
    ws.call("project.create", json!({ "name": "Other", "key": "OT" }));
    let elsewhere = ws.call(
        "template.apply",
        json!({ "template": made["template"]["id"], "project": "OT" }),
    );
    let left: Vec<&str> = elsewhere["left_out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(left, ["1.0", "Area", "Points"], "{elsewhere}");
    let here = ws.call(
        "template.apply",
        json!({ "template": "Review", "project": "TM" }),
    );
    assert_eq!(version_name(&here["task"]), None);
    assert_eq!(here["left_out"], json!(["1.0"]));
}

#[test]
fn a_moved_occurrence_takes_its_series_along() {
    let ws = open("UTC", NOW);
    ws.call("project.create", json!({ "name": "Other", "key": "OT" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Pay the rent" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "TM-1", "text": "every week" }),
    );
    let moved = ws.call("task.move", json!({ "key": "TM-1", "project": "OT" }));
    let key = moved["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(key, "OT-1");
    assert_eq!(ws.task(&key)["recurrence"]["made"], 1);
    ws.close(&key);
    let next = ws.task("OT-2");
    assert_eq!(next["title"], "Pay the rent");
    assert_eq!(next["recurrence"]["made"], 2);
    let in_tm = keys(&ws.call("task.search", json!({ "query": "project:TM" })));
    assert!(in_tm.is_empty(), "{in_tm:?}");
}

#[test]
fn project_templates_keep_custom_fields_and_rules_but_no_versions() {
    let ws = open("UTC", NOW);
    let points = ws.call(
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer", "default": 1 }),
    )["field"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let cf = format!("cf:{points}");
    ws.call("version.create", json!({ "project": "TM", "name": "1.0" }));
    ws.call(
        "rules.update",
        json!({ "project": "TM",
                "statuses": [{ "status": "In progress", "rules": { "required": [cf] } }],
                "agents": { "read_only": [cf, "title"] } }),
    );
    let made = ws.call(
        "template.create",
        json!({ "name": "Team set-up", "from_project": "TM" }),
    );
    let setup = &made["template"]["setup"];
    assert_eq!(setup["fields"][0]["name"], "Points");
    assert_eq!(setup["fields"][0]["default"], 1);
    assert!(setup.get("versions").is_none());

    let used = ws.call(
        "template.apply",
        json!({ "template": "Team set-up", "name": "App", "key": "APP" }),
    );
    let fields = ws.call("field.list", json!({ "project": "APP" }));
    let copy = fields["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Points")
        .unwrap();
    let new_id = copy["id"].as_str().unwrap();
    assert_ne!(new_id, points, "a field of its own");
    assert_eq!(copy["default"], 1);
    let p = &used["project"];
    let in_progress = p["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "In progress")
        .unwrap();
    assert_eq!(
        in_progress["rules"]["required"],
        json!([format!("cf:{new_id}")])
    );
    assert_eq!(
        p["agent_rules"]["read_only"],
        json!([format!("cf:{new_id}"), "title"])
    );
    let versions = ws.call("version.list", json!({ "project": "APP" }));
    assert_eq!(versions["total"], 0, "{versions}");
    let task = ws.call("task.create", json!({ "project": "APP", "title": "First" }));
    assert_eq!(
        field_values(&task["task"]),
        [("Points".to_owned(), json!(1))]
    );

    // Rules naming a field the template doesn't have are refused when it
    // is used, and nothing is made.
    ws.call(
        "template.create",
        json!({ "name": "Broken", "setup": {
            "statuses": [{ "name": "Open", "category": "todo", "hue": "grey",
                           "rules": { "required": ["cf:0190e5c4a2b37c3d8e9f0a1b2c3d4e5f"] } }]
        } }),
    );
    let err = ws.fails(
        "template.apply",
        json!({ "template": "Broken", "name": "Nope", "key": "NO" }),
    );
    assert_eq!(
        err.details["message_id"], "rule-custom-field-unknown",
        "{err:?}"
    );
    assert_eq!(err.details["path"], "setup.statuses[0].rules.required[0]");
    let projects = ws.call("project.list", json!({}));
    assert!(!projects.to_string().contains("\"NO\""), "{projects}");
}
