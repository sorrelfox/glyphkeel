// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The schedule for the Gantt chart, and moving the tasks that depend on a
//! task whose dates change, over a real workspace.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Instant;

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

fn me() -> Context {
    Context::user(Via::App)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

/// A task in `project` with the given dates; returns its key.
fn task(e: &Engine, project: &str, title: &str, start: Option<&str>, due: Option<&str>) -> String {
    let mut input = json!({ "project": project, "title": title });
    if let Some(s) = start {
        input["start_date"] = json!(s);
    }
    if let Some(d) = due {
        input["due_date"] = json!(d);
    }
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn blocks(e: &Engine, from: &str, to: &str) {
    call(
        e,
        "relation.add",
        json!({ "key": from, "kind": "blocks", "other": to }),
    );
}

fn row<'a>(schedule: &'a Value, key: &str) -> &'a Value {
    schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .unwrap_or_else(|| panic!("{key} is not in the schedule"))
}

fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn problems(schedule: &Value) -> Vec<(String, String, String)> {
    schedule["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["kind"].as_str().unwrap().to_owned(),
                p["task"].as_str().unwrap().to_owned(),
                p["other"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

fn dates(e: &Engine, key: &str) -> (Value, Value) {
    let t = &call(e, "task.get", json!({ "keys": [key] }))["items"][0];
    (t["start_date"].clone(), t["due_date"].clone())
}

#[test]
fn a_project_schedule_has_bars_summaries_dependencies_and_problems() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    let design = task(&e, "TM", "Design", Some("2026-12-01"), Some("2026-12-04"));
    let build = task(&e, "TM", "Build", Some("2026-12-04"), Some("2026-12-10"));
    blocks(&e, &design, &build);
    let parent = task(&e, "TM", "Release", Some("2026-12-02"), Some("2026-12-20"));
    let child = task(&e, "TM", "Notes", Some("2026-12-01"), Some("2026-12-03"));
    let grandchild = task(&e, "TM", "Proofread", None, Some("2026-12-22"));
    call(&e, "task.update", json!({ "key": child, "parent": parent }));
    call(
        &e,
        "task.update",
        json!({ "key": grandchild, "parent": child }),
    );
    let undated = task(&e, "TM", "Someday", None, None);
    let open_end = task(&e, "TM", "Research", Some("2026-12-05"), None);
    let late = task(&e, "TM", "Late", Some("2026-11-01"), Some("2026-11-20"));
    let ship = call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Ship", "type": "Milestone", "due_date": "2026-12-21" }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned();

    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["today"], "2026-11-23");
    assert_eq!(
        s["range"],
        json!({ "start": "2026-11-01", "end": "2026-12-22" })
    );
    // Top-level rows by start date; subtasks under their parents.
    assert_eq!(
        keys(&s["tasks"]),
        [
            late.as_str(),
            design.as_str(),
            parent.as_str(),
            child.as_str(),
            grandchild.as_str(),
            build.as_str(),
            open_end.as_str(),
            ship.as_str()
        ]
    );
    assert_eq!(keys(&s["unscheduled"]), [undated.as_str()]);

    let p = row(&s, &parent);
    assert_eq!(p["shape"], "summary");
    assert_eq!(p["depth"], 0);
    assert_eq!(
        p["summary"],
        json!({ "start": "2026-12-01", "end": "2026-12-22" })
    );
    assert_eq!(row(&s, &child)["depth"], 1);
    assert_eq!(row(&s, &child)["parent"], json!(parent));
    assert_eq!(
        row(&s, &child)["summary"],
        json!({ "start": "2026-12-01", "end": "2026-12-22" })
    );
    assert_eq!(row(&s, &grandchild)["depth"], 2);
    assert_eq!(row(&s, &grandchild)["shape"], "deadline");
    assert_eq!(row(&s, &design)["shape"], "bar");
    assert_eq!(row(&s, &open_end)["shape"], "no_deadline");
    assert_eq!(row(&s, &ship)["shape"], "milestone");
    assert_eq!(row(&s, &late)["overdue"], true);
    assert_eq!(row(&s, &build)["blocked"], true);
    assert_eq!(row(&s, &design)["dependents"], 1);
    assert_eq!(row(&s, &build)["dependents"], 0);

    let deps = s["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["from"], json!(design));
    assert_eq!(deps[0]["to"], json!(build));
    assert_eq!(deps[0]["violated"], true);

    assert_eq!(
        problems(&s),
        [
            ("overdue".into(), late.clone(), String::new()),
            // Notes starts before Release; Proofread ends after Notes.
            ("outside_parent".into(), child.clone(), parent.clone()),
            ("outside_parent".into(), grandchild.clone(), child.clone()),
            (
                "starts_before_blocker_ends".into(),
                build.clone(),
                design.clone()
            ),
            ("no_due_date".into(), open_end.clone(), String::new()),
        ]
    );
    let first = &s["problems"][3];
    assert_eq!(first["message_id"], "schedule-starts-before-blocker");
    assert_eq!(
        first["message"],
        format!("{build} starts on 2026-12-04, before {design} ends on 2026-12-04.")
    );
    assert_eq!(first["args"]["start"], "2026-12-04");

    // Starting the day after the blocker's due date is fine.
    call(
        &e,
        "task.update",
        json!({ "key": build, "start_date": "2026-12-05" }),
    );
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["dependencies"][0]["violated"], false);
    assert!(
        !problems(&s)
            .iter()
            .any(|p| p.0 == "starts_before_blocker_ends")
    );
    // A closed blocker leaves the task unblocked.
    call(&e, "task.move", json!({ "key": design, "status": "Done" }));
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(row(&s, &build)["blocked"], false);
    assert_eq!(row(&s, &design)["progress_percent"], 100);
}

#[test]
fn sub_projects_queries_and_windows_narrow_the_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(
        &e,
        "project.create",
        json!({ "name": "Docs", "key": "DOC", "parent": "TM" }),
    );
    let early = task(&e, "TM", "Early", Some("2026-12-01"), Some("2026-12-02"));
    let later = task(&e, "TM", "Later", Some("2027-01-10"), Some("2027-01-12"));
    let docs = task(
        &e,
        "DOC",
        "Write docs",
        Some("2026-12-02"),
        Some("2026-12-05"),
    );
    blocks(&e, &early, &docs);

    // Sub-projects are left out unless asked for.
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(keys(&s["tasks"]), [early.as_str(), later.as_str()]);
    // The link to the sub-project's task isn't drawn, but still counts.
    assert_eq!(s["dependencies"], json!([]));
    assert_eq!(row(&s, &early)["dependents"], 1);
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "include_subprojects": true }),
    );
    assert_eq!(
        keys(&s["tasks"]),
        [early.as_str(), docs.as_str(), later.as_str()]
    );
    assert_eq!(s["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(s["problems"][0]["kind"], "starts_before_blocker_ends");

    // A query alone, across projects.
    let s = call(&e, "schedule.get", json!({ "query": "title:docs" }));
    assert_eq!(keys(&s["tasks"]), [docs.as_str()]);
    // The blocker outside the schedule is still named in the problem.
    assert_eq!(s["problems"][0]["other"], json!(early));

    // A window by natural dates.
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "include_subprojects": true, "from": "2026-12-04", "to": "+3m" }),
    );
    assert_eq!(keys(&s["tasks"]), [docs.as_str(), later.as_str()]);

    let err = e.call(&me(), "schedule.get", json!({})).unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = e
        .call(
            &me(),
            "schedule.get",
            json!({ "project": "TM", "from": "2027-01-01", "to": "2026-01-01" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);

    // A caller limited to the sub-project sees only it, and no link out.
    let limited = me().within_project("DOC");
    let s = e.call(&limited, "schedule.get", json!({})).unwrap();
    assert_eq!(keys(&s["tasks"]), [docs.as_str()]);
    assert_eq!(s["problems"], json!([]));
    assert_eq!(row(&s, &docs)["blocked"], false);
}

#[test]
fn moving_a_task_can_move_its_dependents_as_one_change() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    let a = task(&e, "TM", "A", Some("2026-12-01"), Some("2026-12-04"));
    let b = task(&e, "TM", "B", Some("2026-12-05"), Some("2026-12-07"));
    let c = task(&e, "TM", "C", Some("2026-12-08"), Some("2026-12-08"));
    let roomy = task(&e, "TM", "Roomy", Some("2026-12-20"), Some("2026-12-21"));
    let done = task(&e, "TM", "Done", Some("2026-12-05"), Some("2026-12-06"));
    blocks(&e, &a, &b);
    blocks(&e, &b, &c);
    blocks(&e, &a, &roomy);
    blocks(&e, &a, &done);
    call(&e, "task.move", json!({ "key": done, "status": "Done" }));

    // Without the option only the task moves.
    call(
        &e,
        "task.update",
        json!({ "key": a, "due_date": "2026-12-05" }),
    );
    assert_eq!(dates(&e, &b), (json!("2026-12-05"), json!("2026-12-07")));
    call(
        &e,
        "task.update",
        json!({ "key": a, "due_date": "2026-12-04" }),
    );

    let change = call(
        &e,
        "task.update",
        json!({ "key": a, "start_date": "2026-12-03", "due_date": "2026-12-09", "move_dependents": true }),
    );
    let moved: Vec<(String, i64)> = change["moved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["key"].as_str().unwrap().to_owned(),
                m["days"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(moved, [(b.clone(), 5), (c.clone(), 5)]);
    assert_eq!(dates(&e, &b), (json!("2026-12-10"), json!("2026-12-12")));
    assert_eq!(dates(&e, &c), (json!("2026-12-13"), json!("2026-12-13")));
    assert_eq!(
        dates(&e, &roomy),
        (json!("2026-12-20"), json!("2026-12-21"))
    );
    assert_eq!(dates(&e, &done), (json!("2026-12-05"), json!("2026-12-06")));
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "query": "is:open" }),
    );
    assert!(problems(&s).is_empty(), "{:?}", problems(&s));

    // One Undo takes back the task and its dependents.
    call(&e, "history.undo", json!({}));
    assert_eq!(dates(&e, &a), (json!("2026-12-01"), json!("2026-12-04")));
    assert_eq!(dates(&e, &b), (json!("2026-12-05"), json!("2026-12-07")));
    assert_eq!(dates(&e, &c), (json!("2026-12-08"), json!("2026-12-08")));

    // Moving earlier pulls nothing along.
    let change = call(
        &e,
        "task.update",
        json!({ "key": a, "due_date": "2026-12-02", "move_dependents": true }),
    );
    assert!(change.get("moved").is_none());
    assert_eq!(dates(&e, &b), (json!("2026-12-05"), json!("2026-12-07")));
}

#[test]
fn dependents_in_archived_projects_stay_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(&e, "project.create", json!({ "name": "Old", "key": "OLD" }));
    let a = task(&e, "TM", "A", None, Some("2026-12-04"));
    let old = task(&e, "OLD", "Old", Some("2026-12-05"), Some("2026-12-06"));
    blocks(&e, &a, &old);
    call(&e, "project.archive", json!({ "project": "OLD" }));
    let change = call(
        &e,
        "task.update",
        json!({ "key": a, "due_date": "2026-12-10", "move_dependents": true }),
    );
    assert!(change.get("moved").is_none());
    assert_eq!(change["warnings"].as_array().unwrap().len(), 1, "{change}");
    assert_eq!(dates(&e, &old), (json!("2026-12-05"), json!("2026-12-06")));
}

#[test]
fn five_hundred_tasks_with_three_hundred_dependencies_are_scheduled_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(&e, "project.create", json!({ "name": "Big", "key": "BIG" }));
    let mut keys_made = Vec::with_capacity(500);
    for i in 0..500u32 {
        let start = 1 + (i % 28);
        let input = json!({
            "project": "BIG",
            "title": format!("Task {i}"),
            "start_date": format!("2027-02-{start:02}"),
            "due_date": format!("2027-02-{:02}", (start + 1).min(28)),
        });
        keys_made.push(
            call(&e, "task.create", input)["task"]["key"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    // Ten parents with ten subtasks each.
    for p in 0..10 {
        for c in 0..10 {
            let child = &keys_made[100 + p * 10 + c];
            call(
                &e,
                "task.update",
                json!({ "key": child, "parent": keys_made[p] }),
            );
        }
    }
    // 300 links, never backwards, so there are no loops.
    let mut made = 0;
    'outer: for gap in [7usize, 13, 29] {
        for i in (200..500).step_by(2) {
            let j = i + gap;
            if j >= 500 {
                continue;
            }
            blocks(&e, &keys_made[i], &keys_made[j]);
            made += 1;
            if made == 300 {
                break 'outer;
            }
        }
    }
    assert_eq!(made, 300);

    let at = Instant::now();
    let s = call(&e, "schedule.get", json!({ "project": "BIG" }));
    let took = at.elapsed();
    assert_eq!(s["tasks"].as_array().unwrap().len(), 500);
    assert_eq!(s["dependencies"].as_array().unwrap().len(), 300);
    eprintln!("schedule.get over 500 tasks and 300 dependencies: {took:?}");
    // Generous, for unoptimized test builds on a busy machine.
    assert!(took.as_millis() < 3_000, "{took:?}");

    let at = Instant::now();
    call(
        &e,
        "task.update",
        json!({ "key": keys_made[200], "due_date": "2027-03-31", "move_dependents": true }),
    );
    eprintln!("moving one task and its dependents: {:?}", at.elapsed());
}

#[test]
fn a_whole_range_moves_later_or_earlier_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    let a = task(&e, "TM", "A", Some("2026-12-01"), Some("2026-12-04"));
    // Dragging a bar: both dates move past the old ones.
    call(
        &e,
        "task.update",
        json!({ "key": a, "start_date": "2026-12-10", "due_date": "2026-12-12" }),
    );
    assert_eq!(dates(&e, &a), (json!("2026-12-10"), json!("2026-12-12")));
    call(&e, "history.undo", json!({}));
    assert_eq!(dates(&e, &a), (json!("2026-12-01"), json!("2026-12-04")));
    call(&e, "history.redo", json!({}));
    assert_eq!(dates(&e, &a), (json!("2026-12-10"), json!("2026-12-12")));
    call(
        &e,
        "task.update",
        json!({ "key": a, "start_date": "2026-11-01", "due_date": "2026-11-03" }),
    );
    call(&e, "history.undo", json!({}));
    assert_eq!(dates(&e, &a), (json!("2026-12-10"), json!("2026-12-12")));
}

#[test]
fn rows_carry_labels_and_tasks_of_archived_projects_are_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(&e, "label.create", json!({ "project": "TM", "name": "ui" }));
    call(
        &e,
        "label.create",
        json!({ "project": "TM", "name": "api" }),
    );
    let a = task(&e, "TM", "A", Some("2026-12-01"), Some("2026-12-02"));
    call(
        &e,
        "task.update",
        json!({ "key": a, "labels": ["api", "ui"] }),
    );
    let b = task(&e, "TM", "B", None, None);
    call(&e, "task.update", json!({ "key": b, "labels": ["api"] }));

    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    let names: Vec<&str> = row(&s, &a)["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    // The project's label order: the order they were made in.
    assert_eq!(names, ["ui", "api"]);
    assert!(row(&s, &a).get("read_only").is_none());
    assert_eq!(s["unscheduled"][0]["labels"][0]["name"], "api");

    call(&e, "project.archive", json!({ "project": "TM" }));
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(row(&s, &a)["read_only"], "archived_project");
    assert_eq!(s["unscheduled"][0]["read_only"], "archived_project");
}

#[test]
fn a_move_can_be_tried_out_without_changing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    let a = task(&e, "TM", "A", Some("2026-12-01"), Some("2026-12-04"));
    let b = task(&e, "TM", "B", Some("2026-12-05"), Some("2026-12-06"));
    let c = task(&e, "TM", "C", Some("2026-12-20"), Some("2026-12-21"));
    let d = task(&e, "TM", "D", Some("2026-12-08"), Some("2026-12-09"));
    blocks(&e, &a, &b);
    blocks(&e, &b, &c);
    blocks(&e, &b, &d);

    let preview = call(
        &e,
        "schedule.preview_move",
        json!({ "key": a, "start_date": "2026-12-03", "due_date": "2026-12-06" }),
    );
    // B moves two days; D after it moves one; C has room enough.
    assert_eq!(keys(&preview["moved"]), [b.clone(), d.clone()]);
    assert_eq!(preview["moved"][0]["days"], 2);
    assert_eq!(preview["moved"][0]["start_date"], "2026-12-07");
    assert_eq!(preview["moved"][1]["days"], 1);
    assert_eq!(dates(&e, &b), (json!("2026-12-05"), json!("2026-12-06")));

    // The same as the change itself then does.
    let change = call(
        &e,
        "task.update",
        json!({ "key": a, "start_date": "2026-12-03", "due_date": "2026-12-06", "move_dependents": true }),
    );
    assert_eq!(change["moved"], preview["moved"]);

    // Earlier moves nothing; bad dates are refused.
    let none = call(
        &e,
        "schedule.preview_move",
        json!({ "key": a, "due_date": "2026-12-01" }),
    );
    assert_eq!(none["moved"], json!([]));
    let err = e
        .call(
            &me(),
            "schedule.preview_move",
            json!({ "key": a, "start_date": "2026-12-09", "due_date": "2026-12-01" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = e
        .call(
            &me(),
            "schedule.preview_move",
            json!({ "key": a, "start_date": "next week" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn the_gantt_layout_is_kept_per_project_and_can_be_undone() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(
        &e,
        "project.create",
        json!({ "name": "Yard", "key": "YARD" }),
    );

    let empty = call(&e, "gantt.layout.get", json!({ "project": "TM" }));
    assert_eq!(
        empty["layout"],
        json!({ "zoom": "day", "widths": {}, "group_by": "parent", "move_dependents": false })
    );
    let changed = call(
        &e,
        "gantt.layout.update",
        json!({ "project": "TM", "zoom": "month", "table": 520, "widths": { "key": 90 } }),
    );
    assert_eq!(changed["project"], "TM");
    assert!(changed["group"].is_string());
    let more = call(
        &e,
        "gantt.layout.update",
        json!({ "project": "TM", "group_by": "label", "move_dependents": true }),
    );
    assert_eq!(
        more["layout"],
        json!({
            "zoom": "month", "table": 520, "widths": { "key": 90 },
            "group_by": "label", "move_dependents": true
        })
    );
    // Nothing changed: no history group.
    let same = call(
        &e,
        "gantt.layout.update",
        json!({ "project": "TM", "zoom": "month" }),
    );
    assert!(same.get("group").is_none());
    // Per project.
    assert_eq!(
        call(&e, "gantt.layout.get", json!({ "project": "YARD" }))["layout"]["zoom"],
        "day"
    );
    // Undo takes back the last change only.
    call(&e, "history.undo", json!({}));
    let back = call(&e, "gantt.layout.get", json!({ "project": "TM" }));
    assert_eq!(back["layout"]["group_by"], "parent");
    assert_eq!(back["layout"]["zoom"], "month");
    // Reset goes back to the default, which isn't stored.
    let reset = call(
        &e,
        "gantt.layout.update",
        json!({ "project": "TM", "reset": true }),
    );
    assert_eq!(reset["layout"], empty["layout"]);
    // Widths, columns and the table are checked.
    for bad in [
        json!({ "project": "TM", "widths": { "key": 5 } }),
        json!({ "project": "TM", "widths": { "colour": 80 } }),
        json!({ "project": "TM", "table": 50 }),
        json!({ "project": "TM", "zoom": "year" }),
    ] {
        let err = e.call(&me(), "gantt.layout.update", bad).unwrap_err();
        assert_eq!(err.code, ErrorCode::Invalid);
    }
}
