// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Projects, sub-projects, statuses, types, labels, tasks and the board,
//! over a real workspace.

#![allow(clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_api::ops::history::{HistoryRevert, RevertInput};
use gk_api::ops::settings::{SettingKey, SettingsPatch, SettingsUpdate};
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

fn open(dir: &TempDir) -> Engine {
    Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(Role::App),
    )
    .unwrap()
}

fn open_role(dir: &TempDir, role: Role) -> Engine {
    Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(role),
    )
    .unwrap()
}

fn me() -> Context {
    Context::user(Via::Cli)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, op: &str, input: Value) -> OpError {
    e.call(&me(), op, input).expect_err(op)
}

fn project(e: &Engine, key: &str, parent: Option<&str>) -> Value {
    let mut input = json!({ "name": format!("Project {key}"), "key": key });
    if let Some(p) = parent {
        input["parent"] = json!(p);
    }
    call(e, "project.create", input)["project"].clone()
}

fn task(e: &Engine, project: &str, title: &str) -> Value {
    call(
        e,
        "task.create",
        json!({ "project": project, "title": title }),
    )["task"]
        .clone()
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect()
}

fn keys(column: &Value) -> Vec<String> {
    column["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_new_project_gets_the_default_statuses_and_types() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let p = project(&e, "TM", None);
    assert_eq!(
        names(&p["statuses"]),
        ["Backlog", "To do", "In progress", "Review", "Done"]
    );
    let categories: Vec<&str> = p["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["category"].as_str().unwrap())
        .collect();
    assert_eq!(
        categories,
        ["todo", "todo", "in_progress", "in_progress", "done"]
    );
    assert_eq!(names(&p["types"]), ["Task", "Bug", "Feature", "Milestone"]);
    assert_eq!(p["types"][0]["is_default"], true);
    assert_eq!(p["labels"], json!([]));
    assert_eq!(p["modules"]["board"], true);

    for bad in ["T", "tm1", "1TM", "TOOLONGKEY1", "T-M"] {
        let err = fails(&e, "project.create", json!({ "name": "x", "key": bad }));
        assert_eq!(err.code, ErrorCode::Invalid, "{bad}");
    }
    let err = fails(
        &e,
        "project.create",
        json!({ "name": "Again", "key": "TM" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    let err = fails(&e, "project.create", json!({ "name": " \t ", "key": "EM" }));
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn sub_projects_copy_their_parent_and_stay_three_levels_deep() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    call(
        &e,
        "project.update",
        json!({ "project": "TM", "labels": [{ "name": "bug", "hue": "red" }],
                "statuses": [{ "name": "Blocked", "category": "in_progress", "after": "In progress" }] }),
    );
    let sub = project(&e, "CH", Some("TM"));
    assert_eq!(
        names(&sub["statuses"]),
        [
            "Backlog",
            "To do",
            "In progress",
            "Blocked",
            "Review",
            "Done"
        ]
    );
    assert_eq!(names(&sub["labels"]), ["bug"]);
    assert_eq!(sub["parent"], "TM");
    assert_eq!(sub["path"], "Project TM / Project CH");

    // Later changes to one don't reach the other.
    call(
        &e,
        "status.update",
        json!({ "project": "CH", "status": "blocked", "name": "Waiting" }),
    );
    let parent = call(&e, "project.get", json!({ "project": "TM" }));
    assert!(names(&parent["statuses"]).contains(&"Blocked".to_owned()));

    project(&e, "DEEP", Some("CH"));
    let err = fails(
        &e,
        "project.create",
        json!({ "name": "Too deep", "key": "FOUR", "parent": "DEEP" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = fails(
        &e,
        "project.move",
        json!({ "project": "TM", "parent": "DEEP" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(err.message.contains("own sub-projects"), "{}", err.message);

    // A move that would make a fourth level is refused; a legal one works.
    project(&e, "OTHER", None);
    project(&e, "MID", Some("OTHER"));
    let err = fails(
        &e,
        "project.move",
        json!({ "project": "CH", "parent": "MID" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(err.message.contains("would need 4"), "{}", err.message);
    call(
        &e,
        "project.move",
        json!({ "project": "CH", "parent": "OTHER", "at": "top" }),
    );
    call(
        &e,
        "project.move",
        json!({ "project": "DEEP", "top_level": true }),
    );

    let list = call(&e, "project.list", json!({}));
    let tree: Vec<(String, u64)> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["key"].as_str().unwrap().to_owned(),
                p["depth"].as_u64().unwrap(),
            )
        })
        .collect();
    let expected = [("TM", 0), ("OTHER", 0), ("CH", 1), ("MID", 1), ("DEEP", 0)];
    assert_eq!(tree, expected.map(|(k, d)| (k.to_owned(), d)).to_vec());
    call(
        &e,
        "project.move",
        json!({ "project": "DEEP", "at": "top" }),
    );
    let list = call(&e, "project.list", json!({}));
    assert_eq!(list["items"][0]["key"], "DEEP");
}

#[test]
fn an_archived_subtree_refuses_every_change_until_unarchived() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    project(&e, "CH", Some("TM"));
    task(&e, "CH", "Before");
    call(&e, "project.archive", json!({ "project": "TM" }));
    let sub = call(&e, "project.get", json!({ "project": "CH" }));
    assert_eq!(sub["state"], "archived");
    for (op, input) in [
        ("task.create", json!({ "project": "CH", "title": "x" })),
        ("task.update", json!({ "key": "CH-1", "title": "x" })),
        ("project.update", json!({ "project": "TM", "name": "x" })),
        ("label.create", json!({ "project": "CH", "name": "x" })),
    ] {
        assert_eq!(fails(&e, op, input).code, ErrorCode::Forbidden, "{op}");
    }
    assert_eq!(
        fails(&e, "project.unarchive", json!({ "project": "CH" })).code,
        ErrorCode::Forbidden
    );
    let list = call(&e, "project.list", json!({}));
    assert_eq!(list["total"], 0);
    let list = call(&e, "project.list", json!({ "include_archived": true }));
    assert_eq!(list["total"], 2);
    call(
        &e,
        "project.update",
        json!({ "project": "TM", "archived": false, "name": "Back" }),
    );
    task(&e, "CH", "After");
}

#[test]
fn a_new_key_keeps_the_old_keys_working() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    project(&e, "ZZ", None);
    task(&e, "TM", "One");
    let p = call(
        &e,
        "project.change_key",
        json!({ "project": "TM", "key": "TK" }),
    )["project"]
        .clone();
    assert_eq!(p["key"], "TK");
    assert_eq!(p["old_keys"], json!(["TM"]));
    let t = call(&e, "task.get", json!({ "keys": ["tm-1"] }));
    assert_eq!(t["items"][0]["key"], "TK-1");
    assert_eq!(
        call(&e, "project.get", json!({ "project": "tm" }))["key"],
        "TK"
    );
    // An old key is reserved, and a project can take its own old key back.
    let err = fails(
        &e,
        "project.change_key",
        json!({ "project": "ZZ", "key": "TM" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    let p = call(
        &e,
        "project.change_key",
        json!({ "project": "TK", "key": "TM" }),
    )["project"]
        .clone();
    assert_eq!(p["key"], "TM");
    assert_eq!(p["old_keys"], json!(["TK"]));
}

#[test]
fn tasks_get_the_next_key_and_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    let t = task(&e, "TM", "  Write\tthe\nnotes\u{7}  ");
    assert_eq!(t["key"], "TM-1");
    assert_eq!(t["title"], "Write the notes");
    assert_eq!(t["status"]["name"], "Backlog");
    assert_eq!(t["type"]["name"], "Task");
    assert_eq!(t["priority"], "none");
    assert_eq!(t["version"], 1);
    assert_eq!(t["created_by"]["kind"], "user");

    for bad in ["", "   ", "\u{7}"] {
        let err = fails(&e, "task.create", json!({ "project": "TM", "title": bad }));
        assert_eq!(err.code, ErrorCode::Invalid);
        assert_eq!(err.details["path"], "title");
    }
    let err = fails(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "x".repeat(501) }),
    );
    assert!(err.message.contains("500"), "{}", err.message);

    // A revert trashes the task, and its number never comes back.
    let created = call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Two" }),
    );
    assert_eq!(created["task"]["key"], "TM-2");
    call(
        &e,
        "history.revert",
        json!({ "groups": [created["group"].as_str().unwrap()] }),
    );
    assert_eq!(
        fails(&e, "task.get", json!({ "keys": ["TM-2"] })).code,
        ErrorCode::NotFound
    );
    drop(e);
    let e = open(&dir);
    assert_eq!(task(&e, "TM", "Three")["key"], "TM-3");

    let full = call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Full", "status": "in progress", "type": "bug",
                "priority": "high", "start_date": "2026-11-01", "due_date": "2026-12-01",
                "estimate": "1w 2d 4h", "description": "- [x] a\n- [ ] b" }),
    )["task"]
        .clone();
    assert_eq!(full["status"]["category"], "in_progress");
    assert_eq!(full["estimate_minutes"], 2400 + 960 + 240);
    assert_eq!(full["estimate"], "1w 2d 4h");
    assert_eq!(full["checklist"], json!({ "done": 1, "total": 2 }));
    let err = fails(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "x", "start_date": "2026-12-02", "due_date": "2026-12-01" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn updates_change_only_what_is_given_and_clear_what_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    call(
        &e,
        "label.create",
        json!({ "project": "TM", "name": "bug" }),
    );
    call(&e, "label.create", json!({ "project": "TM", "name": "ui" }));
    task(&e, "TM", "One");
    let t = call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-01", "labels": ["BUG", "ui"], "estimate": "1,5h" }),
    )["task"]
        .clone();
    assert_eq!(t["labels"].as_array().unwrap().len(), 2);
    assert_eq!(t["estimate_minutes"], 90);
    let version = t["version"].as_i64().unwrap();

    let t = call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "expected_version": version, "remove_labels": ["bug"],
                "clear": ["due_date", "estimate"] }),
    )["task"]
        .clone();
    assert_eq!(names(&t["labels"]), ["ui"]);
    assert!(t.get("due_date").is_none());
    assert!(t.get("estimate").is_none());
    assert_eq!(t["title"], "One");

    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "expected_version": version, "title": "late" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-01", "clear": ["due_date"] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-12-01" }),
    );
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "start_date": "2026-12-05" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);

    // Subtasks: same project, no loops, at most five levels.
    for i in 2..=5 {
        task(&e, "TM", &format!("Level {i}"));
        call(
            &e,
            "task.update",
            json!({ "key": format!("TM-{i}"), "parent": format!("TM-{}", i - 1) }),
        );
    }
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "parent": "TM-5" }),
    );
    assert!(err.message.contains("own subtasks"), "{}", err.message);
    let err = fails(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Six", "parent": "TM-5" }),
    );
    assert!(err.message.contains("5 levels"), "{}", err.message);
    project(&e, "OT", None);
    let err = fails(
        &e,
        "task.create",
        json!({ "project": "OT", "title": "x", "parent": "TM-1" }),
    );
    assert!(err.message.contains("same project"), "{}", err.message);
}

#[test]
fn moves_change_status_rank_and_project() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    for t in ["A", "B", "C"] {
        task(&e, "TM", t);
    }
    let board = |e: &Engine| call(e, "board.get", json!({ "project": "TM" }));
    assert_eq!(keys(&board(&e)["columns"][0]), ["TM-1", "TM-2", "TM-3"]);
    call(&e, "task.move", json!({ "key": "TM-3", "at": "top" }));
    call(&e, "task.move", json!({ "key": "TM-1", "before": "TM-2" }));
    assert_eq!(keys(&board(&e)["columns"][0]), ["TM-3", "TM-1", "TM-2"]);

    let moved = call(&e, "task.move", json!({ "key": "TM-1", "status": "Done" }));
    assert!(moved["task"].get("closed_at").is_some());
    let t = call(&e, "task.move", json!({ "key": "TM-1", "status": "To do" }))["task"].clone();
    assert!(t.get("closed_at").is_none());
    assert_eq!(t["status"]["name"], "To do");
    let err = fails(&e, "task.move", json!({ "key": "TM-1", "status": "Nope" }));
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(err.details["choices"].as_array().unwrap().len() == 5);

    // A parent closes with a warning while a subtask is open.
    call(
        &e,
        "task.update",
        json!({ "key": "TM-2", "parent": "TM-1" }),
    );
    let closing = call(&e, "task.move", json!({ "key": "TM-1", "status": "Done" }));
    assert_eq!(closing["warnings"].as_array().unwrap().len(), 1);

    // To another project: new keys, old ones forward, subtasks come along.
    project(&e, "OT", None);
    task(&e, "OT", "Already there");
    let moved = call(&e, "task.move", json!({ "key": "TM-1", "project": "OT" }))["task"].clone();
    assert_eq!(moved["key"], "OT-2");
    assert_eq!(moved["status"]["name"], "Done");
    assert_eq!(moved["old_keys"], json!(["TM-1"]));
    let child = call(&e, "task.get", json!({ "keys": ["TM-2"] }))["items"][0].clone();
    assert_eq!(child["key"], "OT-3");
    assert_eq!(child["parent"], "OT-2");
    assert_eq!(task(&e, "TM", "Next")["key"], "TM-4");
}

#[test]
fn statuses_types_and_labels_are_removed_only_with_care() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    call(
        &e,
        "label.create",
        json!({ "project": "TM", "name": "bug" }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One", "status": "Review", "labels": ["bug"], "type": "Bug" }),
    );
    let sure = Context::user(Via::Cli).with_confirmation(Confirmation::Given);

    // Permanent: refused without a confirmation.
    let err = fails(
        &e,
        "status.delete",
        json!({ "project": "TM", "status": "Review" }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
    let err = e
        .call(
            &sure,
            "status.delete",
            json!({ "project": "TM", "status": "Review" }),
        )
        .unwrap_err();
    assert_eq!(err.details["path"], "move_tasks_to");
    let out = e
        .call(
            &sure,
            "status.delete",
            json!({ "project": "TM", "status": "Review", "move_tasks_to": "Done" }),
        )
        .unwrap();
    assert!(!names(&out["project"]["statuses"]).contains(&"Review".to_owned()));
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["status"]["name"], "Done");
    assert!(t.get("closed_at").is_some());
    let feed = call(&e, "history.feed", json!({ "group": out["group"] }));
    assert!(
        feed["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["entity"] == "task" && i["fields"].get("status").is_some())
    );
    // A removal can't be reverted.
    let r = call(&e, "history.revert", json!({ "groups": [out["group"]] }));
    assert_eq!(r["skipped"].as_array().unwrap().len(), 1);

    let err = e
        .call(
            &sure,
            "type.delete",
            json!({ "project": "TM", "type": "Task" }),
        )
        .unwrap_err();
    assert!(err.message.contains("default"), "{}", err.message);
    e.call(
        &sure,
        "type.delete",
        json!({ "project": "TM", "type": "Bug" }),
    )
    .unwrap();
    e.call(
        &sure,
        "label.delete",
        json!({ "project": "TM", "label": "BUG" }),
    )
    .unwrap();
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["type"]["name"], "Task");
    assert_eq!(t["labels"], json!([]));

    // Names are unique ignoring case.
    let err = fails(
        &e,
        "status.create",
        json!({ "project": "TM", "name": "DONE" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
}

#[test]
fn update_project_checks_each_part_against_its_own_class() {
    let dir = tempfile::tempdir().unwrap();
    let e = open_role(&dir, Role::Mcp);
    project(&e, "TM", None);
    let session = e
        .start_session(&SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, Profile::Standard);
    let added = e
        .call(
            &agent,
            "project.update",
            json!({ "project": "TM", "statuses": [{ "name": "Blocked" }], "hue": "green" }),
        )
        .unwrap();
    assert!(names(&added["project"]["statuses"]).contains(&"Blocked".to_owned()));
    let err = e
        .call(
            &agent,
            "project.update",
            json!({ "project": "TM", "statuses": [{ "status": "Blocked", "remove": true }] }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert!(err.message.contains("status.delete"), "{}", err.message);

    // The person confirms the permanent part.
    let err = fails(
        &e,
        "project.update",
        json!({ "project": "TM", "statuses": [{ "status": "Blocked", "remove": true }] }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
    let sure = Context::user(Via::Cli).with_confirmation(Confirmation::Given);
    e.call(
        &sure,
        "project.update",
        json!({ "project": "TM", "statuses": [{ "status": "Blocked", "remove": true }] }),
    )
    .unwrap();
    let err = fails(
        &e,
        "project.update",
        json!({ "project": "TM", "statuses": [{ "status": "Nope", "name": "x" }] }),
    );
    assert_eq!(err.details["path"], "statuses[0].status");
}

#[test]
fn a_repeated_request_from_another_process_gets_the_first_answer() {
    let dir = tempfile::tempdir().unwrap();
    let first = open_role(&dir, Role::Cli);
    project(&first, "TM", None);
    let input = json!({ "project": "TM", "title": "Once", "client_request_id": "r-1" });
    let a = call(&first, "task.create", input.clone());
    let second = open_role(&dir, Role::Cli);
    let b = call(&second, "task.create", input);
    assert_eq!(a, b);
    let board = call(&second, "board.get", json!({ "project": "TM" }));
    assert_eq!(keys(&board["columns"][0]), ["TM-1"]);
}

#[test]
fn the_board_follows_the_personal_layout() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    call(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "In progress", "wip_limit": 1 }),
    );
    for t in ["A", "B"] {
        call(
            &e,
            "task.create",
            json!({ "project": "TM", "title": t, "status": "In progress" }),
        );
    }
    task(&e, "TM", "Waiting");
    let layout = call(
        &e,
        "layout.update",
        json!({ "project": "TM", "hide": ["backlog"], "order": ["Done", "In progress"],
                "column_sort": { "In progress": "priority" } }),
    );
    assert!(layout["group"].is_string());
    let board = call(&e, "board.get", json!({ "project": "TM" }));
    let order: Vec<&str> = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["Done", "In progress", "Backlog", "To do", "Review"]);
    assert_eq!(board["hidden_columns"], 1);
    assert_eq!(board["hidden_tasks"], 1);
    let backlog = &board["columns"][2];
    assert_eq!(backlog["hidden"], true);
    assert_eq!(backlog["cards"], json!([]));
    let doing = &board["columns"][1];
    assert_eq!(doing["over_limit"], true);
    assert_eq!(doing["sort_mode"], "priority");

    // A new status shows next to the one it follows in the project.
    call(
        &e,
        "status.create",
        json!({ "project": "TM", "name": "Blocked", "after": "In progress" }),
    );
    let board = call(&e, "board.get", json!({ "project": "TM" }));
    assert_eq!(board["columns"][2]["name"], "Blocked");

    // Sub-projects add their tasks, matched by status name.
    project(&e, "CH", Some("TM"));
    call(
        &e,
        "status.create",
        json!({ "project": "CH", "name": "Only here" }),
    );
    task(&e, "CH", "Sub task");
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "include_subprojects": true }),
    );
    let last = board["columns"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["name"], "Only here");
    assert_eq!(board["hidden_tasks"], 2);

    let reset = call(
        &e,
        "layout.update",
        json!({ "project": "TM", "reset": true }),
    );
    assert_eq!(reset["layout"]["hidden"], json!([]));
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "card_fields": ["colour"] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn undo_takes_back_task_changes_and_moves() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let app = Context::user(Via::App);
    e.call(&app, "project.create", json!({ "name": "T", "key": "TM" }))
        .unwrap();
    e.call(
        &app,
        "task.create",
        json!({ "project": "TM", "title": "One" }),
    )
    .unwrap();
    e.call(
        &app,
        "task.create",
        json!({ "project": "TM", "title": "Two" }),
    )
    .unwrap();
    e.call(
        &app,
        "task.update",
        json!({ "key": "TM-1", "title": "Renamed", "priority": "urgent" }),
    )
    .unwrap();
    e.call(
        &app,
        "task.move",
        json!({ "key": "TM-1", "status": "Done", "at": "top" }),
    )
    .unwrap();
    e.call(&app, "history.undo", json!({})).unwrap();
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["status"]["name"], "Backlog");
    assert!(t.get("closed_at").is_none());
    e.call(&app, "history.undo", json!({})).unwrap();
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["title"], "One");
    assert_eq!(t["priority"], "none");
    e.call(&app, "history.redo", json!({})).unwrap();
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["title"], "Renamed");
    // Undoing a creation puts the task in the trash; redo brings it back.
    e.call(&app, "history.undo", json!({})).unwrap();
    e.call(&app, "history.undo", json!({})).unwrap();
    assert_eq!(
        fails(&e, "task.get", json!({ "keys": ["TM-2"] })).code,
        ErrorCode::NotFound
    );
    e.call(&app, "history.redo", json!({})).unwrap();
    assert_eq!(
        call(&e, "task.get", json!({ "keys": ["TM-2"] }))["total"],
        1
    );
}

#[test]
fn settings_can_be_set_back_to_their_default() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let set = e
        .run::<SettingsUpdate>(
            &me(),
            &SettingsPatch {
                hours_per_day: Some(6),
                ..SettingsPatch::default()
            },
        )
        .unwrap();
    assert_eq!(set.settings.hours_per_day, 6);
    project(&e, "TM", None);
    let t = call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "x", "estimate": "1d" }),
    );
    assert_eq!(t["task"]["estimate_minutes"], 360);
    let back = e
        .run::<SettingsUpdate>(
            &me(),
            &SettingsPatch {
                clear: vec![SettingKey::HoursPerDay],
                ..SettingsPatch::default()
            },
        )
        .unwrap();
    assert_eq!(back.settings.hours_per_day, 8);
    assert_eq!(back.changed, ["hours_per_day"]);
    // The estimate is stored in minutes, so only how it reads changes.
    let t = call(&e, "task.get", json!({ "keys": ["TM-1"] }))["items"][0].clone();
    assert_eq!(t["estimate"], "6h");
    let _ = e
        .run::<HistoryRevert>(
            &me(),
            &RevertInput {
                groups: vec![back.group.unwrap()],
                ..RevertInput::default()
            },
        )
        .unwrap();
}

/// Takes a workspace back from schema version 4 to 3: the timer without
/// pausing, reminders as version 3 had them.
const BACK_TO_3: &str = "
    DROP INDEX tasks_fix_version; ALTER TABLE tasks DROP COLUMN fix_version_id;
    DROP TABLE versions;
    ALTER TABLE wiki_pages DROP COLUMN is_template;
    DROP INDEX wiki_pages_one_home; ALTER TABLE wiki_pages DROP COLUMN is_home;
    DROP TABLE text_links;
    ALTER TABLE wiki_revisions DROP COLUMN resolved_at;
    ALTER TABLE timer DROP COLUMN running_since; ALTER TABLE timer DROP COLUMN elapsed_ms;
    ALTER TABLE timer DROP COLUMN note; ALTER TABLE timer DROP COLUMN reminded_at;
    DROP TABLE reminders;
    CREATE TABLE reminders (
      id BLOB PRIMARY KEY CHECK (length(id) = 16), task_id BLOB NOT NULL REFERENCES tasks (id),
      remind_at INTEGER NOT NULL, fired_at INTEGER, dismissed_at INTEGER) STRICT;
    CREATE INDEX reminders_task ON reminders (task_id);
    CREATE INDEX reminders_due ON reminders (remind_at)
      WHERE fired_at IS NULL AND dismissed_at IS NULL;";

#[test]
fn the_app_backs_up_an_older_workspace_before_upgrading_it() {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    open(&dir).close();
    {
        // Back to the first schema version, as an older program left it.
        let conn = gk_store::open_raw(&layout.database()).unwrap();
        conn.execute_batch(BACK_TO_3).unwrap();
        conn.execute_batch(
            "DROP INDEX tasks_subtasks; ALTER TABLE tasks DROP COLUMN subtask_rank;
             DROP TABLE request_log; DROP INDEX wiki_pages_name; PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let err = Engine::open(&layout, EngineConfig::new(Role::Cli)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Storage);
    let e = open(&dir);
    project(&e, "TM", None);
    let backups = e.backups().list();
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert_eq!(
        backups[0].kind,
        gk_api::ops::backup::BackupKind::PreMigration
    );
    assert_eq!(backups[0].schema_version, 1);
    assert!(e.backups().verify(&backups[0].name).unwrap().ok);
}

#[test]
fn there_are_no_members_mentions_or_watchers() {
    // A local, single-person workspace: no operation, tool or table deals
    // with members, mentions or watchers.
    let banned = ["member", "mention", "watch", "invite", "assign"];
    for op in gk_api::CATALOG {
        assert!(
            !banned.iter().any(|b| op.id.contains(b)),
            "operation {}",
            op.id
        );
    }
    for tool in gk_api::TOOLS {
        assert!(
            !banned.iter().any(|b| tool.name.contains(b)),
            "tool {}",
            tool.name
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let tables: Vec<String> = e
        .store()
        .read(|tx| {
            let mut stmt = tx.prepare("SELECT name FROM sqlite_schema WHERE type = 'table'")?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, gk_store::StoreError>(rows)
        })
        .unwrap();
    assert!(tables.iter().any(|t| t == "tasks"));
    for table in &tables {
        assert!(!banned.iter().any(|b| table.contains(b)), "table {table}");
    }
    // A task names no person to assign it to.
    project(&e, "TM", None);
    let t = task(&e, "TM", "Mine");
    for field in ["assignee", "members", "watchers", "mentions"] {
        assert!(t.get(field).is_none(), "{field} in {t}");
    }
}

#[test]
fn a_failing_upgrade_puts_the_workspace_back_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    let e = open(&dir);
    project(&e, "TM", None);
    task(&e, "TM", "Kept as it was");
    e.close();
    {
        // An older workspace (schema version 4) with a project icon that the
        // next upgrade would clear, and a column the last upgrade adds
        // already present, so the last upgrade fails after the one before
        // it has run.
        let conn = gk_store::open_raw(&layout.database()).unwrap();
        conn.execute_batch("UPDATE projects SET icon = 'unicorn'; PRAGMA user_version = 4;")
            .unwrap();
    }
    let err = Engine::open(&layout, EngineConfig::new(Role::App)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Storage);
    assert_eq!(err.details["restored"], true, "{err:?}");
    let backup = err.details["backup"].as_str().unwrap().to_owned();
    assert!(err.message.contains(&backup), "{}", err.message);
    assert!(err.message.contains("resolved_at"), "{}", err.message);
    assert!(err.hint.is_some());
    // The workspace is the older one again, untouched by the step that
    // worked.
    {
        let conn = gk_store::open_raw(&layout.database()).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        let icon: String = conn
            .query_row("SELECT icon FROM projects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(icon, "unicorn");
        let title: String = conn
            .query_row("SELECT title FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Kept as it was");
    }
    // A program that can't upgrade still refuses it as an older workspace.
    let refused = Engine::open(&layout, EngineConfig::new(Role::Cli)).unwrap_err();
    assert_eq!(refused.details["found"], 4, "{refused:?}");
}

#[test]
fn upgrading_keeps_the_subtasks_in_the_order_they_were_shown() {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    let e = open(&dir);
    project(&e, "TM", None);
    let parent = task(&e, "TM", "Parent")["key"].as_str().unwrap().to_owned();
    let mut subs = Vec::new();
    for title in ["One", "Two", "Three"] {
        let t = call(
            &e,
            "task.create",
            json!({ "project": "TM", "title": title, "parent": parent }),
        );
        subs.push(t["task"]["key"].as_str().unwrap().to_owned());
    }
    // Before subtasks had an order of their own, they were shown by status,
    // then by their place in the column: Two, Three, One.
    call(
        &e,
        "task.move",
        json!({ "key": subs[0], "status": "In progress" }),
    );
    e.close();
    {
        let conn = gk_store::open_raw(&layout.database()).unwrap();
        conn.execute_batch(BACK_TO_3).unwrap();
        conn.execute_batch(
            "DROP INDEX tasks_subtasks; ALTER TABLE tasks DROP COLUMN subtask_rank;
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }
    let e = open(&dir);
    let order = |e: &Engine| -> Vec<String> {
        call(
            e,
            "task.get",
            json!({ "keys": [parent], "include": ["subtasks"] }),
        )["items"][0]["subtasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["key"].as_str().unwrap().to_owned())
            .collect()
    };
    let expected = [subs[1].clone(), subs[2].clone(), subs[0].clone()];
    assert_eq!(order(&e), expected);
    // From now on statuses don't reorder them.
    call(&e, "task.move", json!({ "key": subs[2], "status": "Done" }));
    assert_eq!(order(&e), expected);
    call(
        &e,
        "task.reorder_subtask",
        json!({ "key": subs[0], "at": "top" }),
    );
    assert_eq!(
        order(&e),
        [subs[0].clone(), subs[1].clone(), subs[2].clone()]
    );
}

#[test]
fn new_labels_get_a_colour_the_project_does_not_use_yet() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let p = project(&e, "TM", None);
    assert_eq!(p["next_label_hue"], "red");
    let add = |name: &str, hue: Option<&str>| {
        let mut input = json!({ "project": "TM", "name": name });
        if let Some(h) = hue {
            input["hue"] = json!(h);
        }
        let p = call(&e, "label.create", input)["project"].clone();
        let label = p["labels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["name"] == name)
            .unwrap()
            .clone();
        (
            label["hue"].as_str().unwrap().to_owned(),
            p["next_label_hue"].clone(),
        )
    };
    assert_eq!(add("a", None), ("red".to_owned(), json!("orange")));
    assert_eq!(
        add("b", Some("orange")),
        ("orange".to_owned(), json!("yellow"))
    );
    assert_eq!(add("c", None).0, "yellow");
    // Labels added in one project change get different colours too.
    let p = call(
        &e,
        "project.update",
        json!({ "project": "TM", "labels": [{ "name": "d" }, { "name": "e" }] }),
    )["project"]
        .clone();
    let hues: Vec<&str> = p["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["hue"].as_str().unwrap())
        .collect();
    assert_eq!(hues, ["red", "orange", "yellow", "green", "teal"]);
    // Once every colour is taken, the least-used one comes next.
    for name in ["f", "g", "h", "i"] {
        add(name, None);
    }
    assert_eq!(add("j", None).0, "red");
    assert_eq!(add("k", Some("red")).1, json!("orange"));
    // A sub-project copies its parent's labels and continues from them.
    let sub = project(&e, "TS", Some("TM"));
    assert_eq!(sub["next_label_hue"], "orange");
}

#[test]
fn new_projects_get_a_colour_their_siblings_do_not_use_yet() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    assert_eq!(project(&e, "AA", None)["hue"], "red");
    call(
        &e,
        "project.create",
        json!({ "name": "B", "key": "BB", "hue": "orange" }),
    );
    assert_eq!(project(&e, "CC", None)["hue"], "yellow");
    // Sub-projects are compared with their own siblings.
    assert_eq!(project(&e, "AB", Some("AA"))["hue"], "red");
    assert_eq!(project(&e, "AC", Some("AA"))["hue"], "orange");
    assert_eq!(project(&e, "DD", None)["hue"], "green");
}

#[test]
fn a_project_icon_comes_from_the_fixed_set_and_undoes() {
    let dir = tempfile::tempdir().unwrap();
    let e = open(&dir);
    let app = Context::user(Via::App);
    let p = e
        .call(
            &app,
            "project.create",
            json!({ "name": "Web", "key": "WEB", "icon": "rocket" }),
        )
        .unwrap();
    assert_eq!(p["project"]["icon"], "rocket");
    let listed = call(&e, "project.list", json!({}));
    assert_eq!(listed["items"][0]["icon"], "rocket");
    let err = fails(
        &e,
        "project.update",
        json!({ "project": "WEB", "icon": "unicorn" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(
        fails(
            &e,
            "project.create",
            json!({ "name": "X", "key": "XX", "icon": "" })
        )
        .code
            == ErrorCode::Invalid
    );

    let group = e
        .call(
            &app,
            "project.update",
            json!({ "project": "WEB", "icon": "graduation-cap" }),
        )
        .unwrap()["group"]
        .clone();
    e.call(
        &app,
        "project.update",
        json!({ "project": "WEB", "clear": ["icon"] }),
    )
    .unwrap();
    let icon = |e: &Engine| call(e, "project.get", json!({ "project": "WEB" }))["icon"].clone();
    assert_eq!(icon(&e), Value::Null, "no icon: the circle");
    e.call(&app, "history.undo", json!({})).unwrap();
    assert_eq!(icon(&e), "graduation-cap");
    e.call(&app, "history.undo", json!({})).unwrap();
    assert_eq!(icon(&e), "rocket");
    e.call(&app, "history.redo", json!({})).unwrap();
    assert_eq!(icon(&e), "graduation-cap");
    let feed = call(&e, "history.feed", json!({ "group": group }));
    assert!(
        feed.to_string().contains("graduation-cap"),
        "the change shows in the history: {feed}"
    );
}
