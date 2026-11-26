// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The project templates that come with Glyphkeel (listed in every
//! workspace, used like any project template, never changed or deleted),
//! what closing a recurring task's occurrence answers, and closing
//! duplicates from a bulk change.

#![allow(clippy::unwrap_used)]

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Ws {
    engine: Engine,
    _dir: TempDir,
}

impl Ws {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::new(Role::App);
        config.time_zone = Some("UTC".to_owned());
        let engine =
            Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
        Self { engine, _dir: dir }
    }

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
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_software_project_template_sets_up_epics_stories_and_a_board_with_a_review_step() {
    let ws = Ws::new();
    let list = ws.call("template.list", json!({ "kind": "project" }));
    let first = &list["items"][0];
    assert_eq!(first["name"], "Software project", "{list}");
    assert_eq!(first["built_in"], "software");
    let id = first["id"].as_str().unwrap().to_owned();

    let used = ws.call(
        "template.apply",
        json!({ "template": "software project", "name": "App", "key": "APP" }),
    );
    let p = &used["project"];
    assert_eq!(
        names(&p["statuses"]),
        [
            "Backlog",
            "To do",
            "In progress",
            "In review",
            "Done",
            "Won't do"
        ]
    );
    assert_eq!(p["statuses"][2]["wip_limit"], 5);
    assert_eq!(p["statuses"][3]["wip_limit"], 3);
    assert_eq!(names(&p["types"]), ["Story", "Task", "Bug", "Epic"]);
    let glyphs: Vec<&str> = p["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["glyph"].as_str().unwrap())
        .collect();
    assert_eq!(glyphs, ["story", "task", "bug", "epic"]);
    assert_eq!(p["types"][0]["is_default"], true);
    assert_eq!(
        names(&p["labels"]),
        ["Frontend", "Backend", "Documentation"]
    );
    let done = &p["statuses"][4]["rules"];
    assert_eq!(done["agents_may_not_enter"], true, "{p}");
    assert_eq!(done["stop_timer"], true);
    let pages = ws.call("wiki.list", json!({ "project": "APP" }));
    assert!(pages.to_string().contains("Releases"), "{pages}");

    // Its task templates came along; a story starts as a story.
    let story = ws.call(
        "template.apply",
        json!({ "template": "Story", "project": "APP", "title": "Sign in" }),
    );
    assert_eq!(story["task"]["type"]["glyph"], "story");
    assert!(
        story["task"]["description"]
            .as_str()
            .unwrap()
            .contains("Acceptance criteria")
    );
    let epic = ws.call(
        "task.create",
        json!({ "project": "APP", "title": "Accounts", "type": "Epic" }),
    );
    assert_eq!(epic["task"]["type"]["glyph"], "epic");

    // It can be read by its ID, but not changed or deleted.
    let read = ws.call("template.get", json!({ "template": id }));
    assert_eq!(read["setup"]["types"][3]["name"], "Epic");
    let err = ws.fails(
        "template.update",
        json!({ "template": id, "name": "Mine now" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["message_id"], "template-built-in");
    let err = ws.fails("template.delete", json!({ "template": "Software project" }));
    assert_eq!(err.details["message_id"], "template-built-in");

    // A template of the workspace's own with that name wins.
    ws.call(
        "template.create",
        json!({ "name": "Software project", "setup": { "types": [
            { "name": "Ticket", "glyph": "task", "hue": "blue" }
        ] } }),
    );
    let own = ws.call("template.get", json!({ "template": "Software project" }));
    assert!(own.get("built_in").is_none(), "{own}");
    let both = ws.call("template.list", json!({ "kind": "project" }));
    assert_eq!(both["total"], 2);
}

#[test]
fn closing_an_occurrence_answers_with_the_series_as_it_now_is() {
    let ws = Ws::new();
    ws.call("project.create", json!({ "name": "Home", "key": "HM" }));
    ws.call(
        "task.create",
        json!({ "project": "HM", "title": "Bins", "due_date": "2030-01-07" }),
    );
    ws.call(
        "recurrence.set",
        json!({ "key": "HM-1", "text": "every week on mon" }),
    );
    let closed = ws.call("task.move", json!({ "key": "HM-1", "status": "Done" }));
    let later = ws.call("task.get", json!({ "keys": ["HM-1"] }))["items"][0].clone();
    assert_eq!(
        closed["task"].get("recurrence"),
        later.get("recurrence"),
        "the answer is what a later read shows: {closed}"
    );
    assert!(closed["task"].get("recurrence").is_none());
    assert_eq!(closed["next_occurrence"], "HM-2");
    let next = ws.call("task.get", json!({ "keys": ["HM-2"] }))["items"][0].clone();
    assert_eq!(next["recurrence"]["current"], "HM-2", "{next}");
    assert_eq!(next["recurrence"]["made"], 2);

    // The board marks the open occurrence.
    let board = ws.call("board.get", json!({ "project": "HM" }));
    let cards: Vec<&Value> = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["cards"].as_array().unwrap())
        .collect();
    let mark = |key: &str| {
        cards
            .iter()
            .find(|c| c["key"] == key)
            .and_then(|c| c.get("recurring"))
            .cloned()
    };
    assert_eq!(mark("HM-2"), Some(Value::Bool(true)), "{board}");
    assert_eq!(mark("HM-1"), None);

    // The last occurrence ends the series; the answer says nothing comes next.
    ws.call(
        "recurrence.set",
        json!({ "key": "HM-2", "text": "every week on mon", "count": 2 }),
    );
    let last = ws.call("task.move", json!({ "key": "HM-2", "status": "Done" }));
    assert!(last.get("next_occurrence").is_none(), "{last}");
    assert!(last["task"].get("recurrence").is_none());
}

#[test]
fn a_bulk_close_can_close_the_duplicates_too_in_one_change() {
    let ws = Ws::new();
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    for title in ["Crash", "Freeze", "Crash again", "Freeze again"] {
        ws.call("task.create", json!({ "project": "TM", "title": title }));
    }
    for (copy, original) in [("TM-3", "TM-1"), ("TM-4", "TM-2")] {
        ws.call(
            "relation.add",
            json!({ "key": copy, "kind": "duplicates", "other": original }),
        );
    }
    let open = |key: &str| {
        ws.call("task.get", json!({ "keys": [key] }))["items"][0]["closed_at"].is_null()
    };
    let bulk = |close: Option<bool>| {
        let mut changes = json!({ "status": "Done" });
        if let Some(close) = close {
            changes["close_duplicates"] = Value::Bool(close);
        }
        ws.call(
            "task.bulk_update",
            json!({ "keys": ["TM-1", "TM-2"], "changes": changes, "dry_run": false,
                    "expected_count": 2 }),
        )
    };

    let warned = bulk(None);
    assert!(warned["warnings"].to_string().contains("TM-3"), "{warned}");
    assert!(open("TM-3"));
    ws.call("history.undo", json!({}));

    let quiet = bulk(Some(false));
    assert!(quiet.get("warnings").is_none(), "{quiet}");
    assert!(open("TM-3") && open("TM-4"));
    ws.call("history.undo", json!({}));

    bulk(Some(true));
    assert!(!open("TM-3") && !open("TM-4"), "the duplicates closed too");
    ws.call("history.undo", json!({}));
    assert!(
        open("TM-1") && open("TM-3"),
        "one Undo opens them all again"
    );
}

#[test]
fn a_due_date_and_priority_given_with_a_task_template_win_in_one_change() {
    let ws = Ws::new();
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    ws.call(
        "template.create",
        json!({ "name": "Report", "project": "TM", "task": {
            "title": "Monthly report", "priority": "low", "due_in_days": 10,
            "subtasks": [{ "title": "Collect numbers" }]
        } }),
    );
    let used = ws.call(
        "template.apply",
        json!({ "template": "Report", "project": "TM", "title": "November report",
                "due_date": "2030-01-04", "priority": "urgent" }),
    );
    let task = &used["task"];
    assert_eq!(task["title"], "November report");
    assert_eq!(task["due_date"], "2030-01-04");
    assert_eq!(task["priority"], "urgent");
    assert_eq!(used["tasks"], 2);
    ws.call("history.undo", json!({}));
    let left = ws.call("task.search", json!({ "query": "is:open" }));
    assert_eq!(
        left["items"].as_array().unwrap().len(),
        0,
        "one Undo: {left}"
    );

    let err = ws.fails(
        "template.apply",
        json!({ "template": "Report", "project": "TM", "due_date": "someday soon" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let after = ws.call("task.search", json!({ "query": "is:open" }));
    assert_eq!(
        after["items"].as_array().unwrap().len(),
        0,
        "nothing made: {after}"
    );
}
