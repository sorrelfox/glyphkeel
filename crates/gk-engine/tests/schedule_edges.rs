// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Edge cases of the schedule over a real workspace: a loop of `blocks`
//! links that got into the data from outside, moves by a caller limited to
//! one project, and which tasks `during:` finds at the edges of a range.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use gk_store::rusqlite::{Connection, params};
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn layout(dir: &TempDir) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.path().join("Personal"))
}

fn open(dir: &TempDir) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(|| NOW);
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&layout(dir), config).unwrap()
}

fn me() -> Context {
    Context::user(Via::App)
}

fn call_as(e: &Engine, who: &Context, op: &str, input: Value) -> Value {
    e.call(who, op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    call_as(e, &me(), op, input)
}

fn task(e: &Engine, project: &str, title: &str, start: Option<&str>, due: Option<&str>) -> Value {
    let mut input = json!({ "project": project, "title": title });
    if let Some(s) = start {
        input["start_date"] = json!(s);
    }
    if let Some(d) = due {
        input["due_date"] = json!(d);
    }
    call(e, "task.create", input)["task"].clone()
}

fn blocks(e: &Engine, from: &str, to: &str) {
    call(
        e,
        "relation.add",
        json!({ "key": from, "kind": "blocks", "other": to }),
    );
}

fn dates(e: &Engine, key: &str) -> (Value, Value) {
    let t = &call(e, "task.get", json!({ "keys": [key] }))["items"][0];
    (t["start_date"].clone(), t["due_date"].clone())
}

fn moved(change: &Value) -> Vec<(String, i64)> {
    change
        .get("moved")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|m| {
                    (
                        m["key"].as_str().unwrap().to_owned(),
                        m["days"].as_i64().unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A UUID as the 16 bytes the store keeps.
fn id_bytes(id: &str) -> Vec<u8> {
    let hex: String = id.chars().filter(|c| *c != '-').collect();
    (0..16)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

#[test]
fn a_loop_of_links_from_outside_is_reported_and_stops_moving_dependents() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    let a = task(&e, "TM", "A", Some("2026-12-01"), Some("2026-12-02"));
    let b = task(&e, "TM", "B", Some("2026-12-03"), Some("2026-12-04"));
    let c = task(&e, "TM", "C", Some("2026-12-05"), Some("2026-12-06"));
    let after = task(&e, "TM", "After", Some("2026-12-10"), Some("2026-12-11"));
    blocks(&e, "TM-1", "TM-2");
    blocks(&e, "TM-2", "TM-3");
    blocks(&e, "TM-1", "TM-4");
    // The app never makes a loop itself.
    let err = e
        .call(
            &me(),
            "relation.add",
            json!({ "key": "TM-3", "kind": "blocks", "other": "TM-1" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    e.close();

    // One written straight into the data, as an old import or another
    // program might leave it.
    {
        let db = Connection::open(layout(&dir).database()).unwrap();
        let link_id = id_bytes("01a21890-0000-7000-8000-000000000001");
        db.execute(
            "INSERT INTO task_relations (id, from_task, to_task, kind, created_at, created_by_kind, created_by_name)
             VALUES (?1, ?2, ?3, 'blocks', ?4, 'user', 'You')",
            params![
                link_id,
                id_bytes(c["id"].as_str().unwrap()),
                id_bytes(a["id"].as_str().unwrap()),
                NOW
            ],
        )
        .unwrap();
    }
    let e = open(&dir);
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    let loops: Vec<&Value> = s["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["kind"] == "dependency_loop")
        .collect();
    assert_eq!(loops.len(), 1, "one problem for the whole loop: {s}");
    assert_eq!(loops[0]["task"], "TM-1");
    assert_eq!(loops[0]["others"], json!(["TM-2", "TM-3"]));
    assert!(!loops[0]["message"].as_str().unwrap().is_empty());
    // The task outside the loop is no part of it.
    assert!(!loops[0].to_string().contains("TM-4"), "{s}");
    let _ = (b, after);

    // Moving a task of the loop with its dependents is refused and
    // changes nothing; the preview moves nothing either.
    let err = e
        .call(
            &me(),
            "task.update",
            json!({ "key": "TM-1", "due_date": "2026-12-08", "move_dependents": true }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict, "{err:?}");
    assert_eq!(err.details["message_id"], "schedule-move-loop");
    assert_eq!(
        dates(&e, "TM-1"),
        (json!("2026-12-01"), json!("2026-12-02"))
    );
    assert_eq!(
        dates(&e, "TM-4"),
        (json!("2026-12-10"), json!("2026-12-11"))
    );
    let preview = call(
        &e,
        "schedule.preview_move",
        json!({ "key": "TM-1", "start_date": "2026-12-01", "due_date": "2026-12-08" }),
    );
    assert_eq!(moved(&preview), []);
    // Without the option the date changes on its own.
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-08" }),
    );
    assert_eq!(
        dates(&e, "TM-1"),
        (json!("2026-12-01"), json!("2026-12-08"))
    );

    // Removing a link of the loop clears the problem.
    call(
        &e,
        "relation.remove",
        json!({ "key": "TM-3", "kind": "blocks", "other": "TM-1" }),
    );
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert!(
        !s["problems"].to_string().contains("dependency_loop"),
        "{s}"
    );
}

#[test]
fn a_caller_limited_to_a_project_moves_only_the_dependents_it_may_see() {
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
        json!({ "name": "Private", "key": "PRV" }),
    );
    task(&e, "TM", "Mine", Some("2026-12-01"), Some("2026-12-04"));
    task(
        &e,
        "TM",
        "Next of mine",
        Some("2026-12-05"),
        Some("2026-12-06"),
    );
    task(&e, "PRV", "Theirs", Some("2026-12-05"), Some("2026-12-06"));
    task(
        &e,
        "TM",
        "After theirs",
        Some("2026-12-07"),
        Some("2026-12-08"),
    );
    blocks(&e, "TM-1", "TM-2");
    blocks(&e, "TM-1", "PRV-1");
    blocks(&e, "PRV-1", "TM-3");

    let limited = me().within_project("TM");
    let preview = call_as(
        &e,
        &limited,
        "schedule.preview_move",
        json!({ "key": "TM-1", "start_date": "2026-12-01", "due_date": "2026-12-06" }),
    );
    assert_eq!(moved(&preview), [("TM-2".to_owned(), 2)]);
    let change = call_as(
        &e,
        &limited,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-06", "move_dependents": true }),
    );
    assert_eq!(moved(&change), moved(&preview));
    assert!(!change.to_string().contains("PRV"), "{change}");
    // The other project's task, and the one behind it, stay.
    assert_eq!(
        dates(&e, "PRV-1"),
        (json!("2026-12-05"), json!("2026-12-06"))
    );
    assert_eq!(
        dates(&e, "TM-3"),
        (json!("2026-12-07"), json!("2026-12-08"))
    );
    // The limited schedule shows no problem it can't name.
    let s = call_as(&e, &limited, "schedule.get", json!({ "project": "TM" }));
    assert!(!s.to_string().contains("PRV"), "{s}");

    // The person, without a limit, sees the problem and moves them all.
    let s = call(&e, "schedule.get", json!({ "query": "is:open" }));
    assert!(
        s["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["kind"] == "starts_before_blocker_ends" && p["task"] == "PRV-1"),
        "{s}"
    );
    let change = call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-06", "start_date": "2026-12-02",
                "move_dependents": true }),
    );
    assert_eq!(
        moved(&change),
        [("PRV-1".to_owned(), 2), ("TM-3".to_owned(), 2)]
    );
}

#[test]
fn during_finds_tasks_by_the_days_they_take_up() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    call(
        &e,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    task(&e, "TM", "Both", Some("2026-12-01"), Some("2026-12-05"));
    task(&e, "TM", "Start only", Some("2026-12-07"), None);
    task(&e, "TM", "Due only", None, Some("2026-12-09"));
    task(&e, "TM", "Neither", None, None);
    task(&e, "TM", "One day", Some("2026-12-11"), Some("2026-12-11"));
    task(&e, "TM", "Long", Some("2026-11-01"), Some("2027-01-31"));
    let done = task(&e, "TM", "Done", Some("2026-12-01"), Some("2026-12-02"));
    call(
        &e,
        "task.move",
        json!({ "key": done["key"], "status": "Done" }),
    );
    let found = |query: &str| -> Vec<String> {
        let list = call(
            &e,
            "task.search",
            json!({ "query": format!("({query}) sort:key"), "limit": 50 }),
        );
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["key"].as_str().unwrap().to_owned())
            .collect()
    };
    let cases: &[(&str, &[&str])] = &[
        // The first and the last day of a range both count.
        ("during:2026-12-05", &["TM-1", "TM-6"]),
        ("during:2026-11-30", &["TM-6"]),
        ("during:2026-12-06..2026-12-07", &["TM-2", "TM-6"]),
        // A single date stands for one day.
        ("during:2026-12-08", &["TM-6"]),
        ("during:2026-12-09", &["TM-3", "TM-6"]),
        ("during:2026-12-11", &["TM-5", "TM-6"]),
        // Closed tasks are found too, unless the query says otherwise.
        ("during:2026-12-02", &["TM-1", "TM-6", "TM-7"]),
        ("during:2026-12-02 is:open", &["TM-1", "TM-6"]),
        // A range around everything finds every dated task.
        (
            "during:2026-01-01..2027-12-31",
            &["TM-1", "TM-2", "TM-3", "TM-5", "TM-6", "TM-7"],
        ),
        // Combined with OR, as the calendar asks for two windows.
        (
            "during:2026-12-07 OR during:2026-12-11",
            &["TM-2", "TM-5", "TM-6"],
        ),
        ("during:2027-02-01..2027-02-28", &[]),
    ];
    for (query, expected) in cases {
        assert_eq!(found(query), *expected, "{query}");
    }
}
