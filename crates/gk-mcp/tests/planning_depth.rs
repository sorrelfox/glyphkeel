// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent plans in depth: it checks a move to another project and moves
//! with a mapping, links with a lag, logs time with a kind of work, closes
//! duplicates with the original, sets a WIP minimum and a type's due offset,
//! and finds tasks by their history.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn keys(list: &Value) -> Vec<String> {
    let mut keys: Vec<String> = list["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect();
    keys.sort();
    keys
}

fn named<'a>(list: &'a Value, name: &str) -> &'a Value {
    list.as_array()
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("{name} in {list}"))
}

#[tokio::test]
async fn an_agent_checks_a_move_and_moves_with_a_mapping() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Team", "key": "TM" }));
        call("project.create", json!({ "name": "Other", "key": "OT" }));
        call(
            "status.create",
            json!({ "project": "TM", "name": "QA", "category": "in_progress" }),
        );
        call("label.create", json!({ "project": "TM", "name": "ui" }));
        call(
            "label.create",
            json!({ "project": "OT", "name": "frontend" }),
        );
        call("task.create", json!({ "project": "TM", "title": "Parent" }));
        call("task.create", json!({ "project": "TM", "title": "Child" }));
        call("task.update", json!({ "key": "TM-1", "labels": ["ui"] }));
        call("task.move", json!({ "key": "TM-1", "status": "QA" }));
        call("task.update", json!({ "key": "TM-2", "parent": "TM-1" }));
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Mover").await;

    let check = output(
        &c.call(
            "check_move",
            json!({ "keys": ["TM-1", "TM-2"], "project": "OT" }),
        )
        .await,
    );
    assert_eq!(
        check["keys"],
        json!([{ "from": "TM-1", "to": "OT-1" }, { "from": "TM-2", "to": "OT-2" }]),
        "{check}"
    );
    let qa = named(&check["statuses"], "QA");
    assert!(qa.get("target").is_none(), "{check}");
    assert!(named(&check["labels"], "ui").get("target").is_none());
    // The check changed nothing.
    let still = output(&c.call("get_task", json!({ "keys": ["TM-1"] })).await);
    assert_eq!(still["items"][0]["key"], "TM-1");

    // A mapping that names nothing in the target is refused.
    let refused = c
        .call(
            "move_task",
            json!({ "key": "TM-1", "project": "OT",
                    "mapping": { "statuses": { "QA": "Nope" } } }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");

    let moved = output(
        &c.call(
            "move_task",
            json!({ "key": "TM-1", "project": "OT",
                    "mapping": { "statuses": { "QA": "Review" }, "labels": { "ui": "frontend" } } }),
        )
        .await,
    );
    let t = &moved["task"];
    assert_eq!(t["key"], "OT-1", "{moved}");
    assert_eq!(t["status"]["name"], "Review");
    assert_eq!(t["labels"][0]["name"], "frontend", "{moved}");
    // The old keys still find the task and its subtask.
    let found = output(
        &c.call("get_task", json!({ "keys": ["TM-1", "TM-2"] }))
            .await,
    );
    assert_eq!(found["items"][0]["key"], "OT-1");
    assert_eq!(found["items"][1]["key"], "OT-2");
    c.close().await;
}

#[tokio::test]
async fn an_agent_links_with_a_lag_and_logs_time_by_kind() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Build", "key": "BD" }));
        call(
            "task.create",
            json!({ "project": "BD", "title": "Pour", "start_date": "2026-12-01", "due_date": "2026-12-02" }),
        );
        call(
            "task.create",
            json!({ "project": "BD", "title": "Build on it", "start_date": "2026-12-03", "due_date": "2026-12-04" }),
        );
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(1);
    let c = connect(&options, mode, version, "Site Agent").await;

    let link = output(
        &c.call(
            "link_tasks",
            json!({ "key": "BD-1", "kind": "blocks", "other": "BD-2", "lag_days": 2 }),
        )
        .await,
    );
    assert_eq!(link["task"]["links"][0]["lag_days"], 2, "{link}");
    let schedule = output(&c.call("get_schedule", json!({ "project": "BD" })).await);
    let dep = &schedule["dependencies"][0];
    assert_eq!(dep["lag_days"], 2, "{schedule}");
    assert_eq!(dep["violated"], true, "BD-2 starts the day after BD-1");
    // A lag only goes on a blocking link.
    let refused = c
        .call(
            "link_tasks",
            json!({ "key": "BD-1", "kind": "relates", "other": "BD-2", "lag_days": 1 }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    // Moving the blocker with its dependents keeps the lag.
    output(
        &c.call(
            "update_task",
            json!({ "key": "BD-1", "due_date": "2026-12-03", "move_dependents": true }),
        )
        .await,
    );
    let schedule = output(&c.call("get_schedule", json!({ "project": "BD" })).await);
    let row = schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "BD-2")
        .unwrap();
    assert_eq!(row["start_date"], "2026-12-06", "{schedule}");

    // Kinds of work.
    output(
        &c.call("create_work_kind", json!({ "name": "Development" }))
            .await,
    );
    output(
        &c.call("create_work_kind", json!({ "name": "Meeting" }))
            .await,
    );
    let twice = c
        .call("create_work_kind", json!({ "name": "meeting" }))
        .await;
    assert_eq!(twice.is_error, Some(true), "names are unique in any case");
    let logged = output(
        &c.call(
            "log_time",
            json!({ "key": "BD-1", "duration": "2h", "work_kind": "development" }),
        )
        .await,
    );
    assert_eq!(
        logged["entry"]["work_kind"]["name"], "Development",
        "{logged}"
    );
    let unknown = c
        .call(
            "log_time",
            json!({ "key": "BD-1", "duration": "1h", "work_kind": "Travel" }),
        )
        .await;
    assert_eq!(unknown.is_error, Some(true), "{unknown:?}");

    let timer = output(
        &c.call(
            "start_timer",
            json!({ "key": "BD-2", "work_kind": "Meeting" }),
        )
        .await,
    );
    assert_eq!(timer["timer"]["work_kind"]["name"], "Meeting", "{timer}");
    let stopped = output(
        &c.call(
            "stop_timer",
            json!({ "duration": "10m", "work_kind": "Development" }),
        )
        .await,
    );
    assert_eq!(
        stopped["entry"]["work_kind"]["name"], "Development",
        "{stopped}"
    );

    // A kind in use is archived, not deleted.
    let in_use = c
        .call("delete_work_kind", json!({ "kind": "Development" }))
        .await;
    assert_eq!(in_use.is_error, Some(true), "{in_use:?}");
    output(
        &c.call(
            "update_work_kind",
            json!({ "kind": "Meeting", "name": "Meetings", "archived": true }),
        )
        .await,
    );
    let listed = output(&c.call("list_work_kinds", json!({})).await);
    let names: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Development"], "{listed}");
    let all = output(
        &c.call("list_work_kinds", json!({ "include_archived": true }))
            .await,
    );
    assert_eq!(all["items"].as_array().unwrap().len(), 2, "{all}");
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let report = engine
        .call(&person, "time.report", json!({ "group_by": "work_kind" }))
        .unwrap();
    assert!(report.to_string().contains("Development"), "{report}");
    engine.close();
}

#[tokio::test]
async fn an_agent_closes_duplicates_with_the_original() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Team", "key": "TM" }));
        call("project.create", json!({ "name": "Web", "key": "SHOP" }));
        call(
            "task.create",
            json!({ "project": "TM", "title": "Crash on start" }),
        );
        call(
            "task.create",
            json!({ "project": "TM", "title": "App crashes" }),
        );
        call(
            "task.create",
            json!({ "project": "SHOP", "title": "Start crash" }),
        );
        for d in ["TM-2", "SHOP-1"] {
            call(
                "relation.add",
                json!({ "key": d, "kind": "duplicates", "other": "TM-1" }),
            );
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Triage").await;

    // Left out: a warning names the open duplicates.
    let warned = c
        .call("move_task", json!({ "key": "TM-1", "status": "Done" }))
        .await;
    let shown = output(&warned);
    let all = format!("{shown} {:?}", warned.content);
    assert!(all.contains("TM-2") && all.contains("SHOP-1"), "{all}");
    output(
        &c.call("move_task", json!({ "key": "TM-1", "status": "Backlog" }))
            .await,
    );

    let closed = output(
        &c.call(
            "move_task",
            json!({ "key": "TM-1", "status": "Done", "close_duplicates": true }),
        )
        .await,
    );
    assert_eq!(
        closed["duplicates_closed"],
        json!(["SHOP-1", "TM-2"]),
        "{closed}"
    );
    let got = output(
        &c.call("get_task", json!({ "keys": ["TM-2", "SHOP-1"] }))
            .await,
    );
    for t in got["items"].as_array().unwrap() {
        assert_eq!(t["status"]["name"], "Done", "{got}");
    }
    c.close().await;
}

#[tokio::test]
async fn an_agent_sets_a_wip_minimum_and_a_due_offset() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "project.create",
                json!({ "name": "Team", "key": "TM" }),
            )
            .unwrap();
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Board Keeper").await;

    output(
        &c.call(
            "update_project",
            json!({ "project": "TM",
                    "statuses": [{ "status": "In progress", "wip_limit": 3, "wip_min": 2 }],
                    "types": [{ "type": "Bug", "due_offset_days": 3 }] }),
        )
        .await,
    );
    let project = output(&c.call("get_project", json!({ "project": "TM" })).await);
    assert_eq!(
        named(&project["statuses"], "In progress")["wip_min"],
        2,
        "{project}"
    );
    assert_eq!(
        named(&project["types"], "Bug")["due_offset_days"],
        3,
        "{project}"
    );

    let column = |board: &Value| named(&board["columns"], "In progress").clone();
    let board = output(&c.call("get_board", json!({ "project": "TM" })).await);
    assert_eq!(column(&board)["wip_min"], 2, "{board}");
    assert_eq!(column(&board)["under_minimum"], true, "{board}");

    // A minimum above the limit is refused.
    let refused = c
        .call(
            "update_project",
            json!({ "project": "TM", "statuses": [{ "status": "In progress", "wip_min": 4 }] }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");

    // New bugs get a due date; given dates and other types are left alone.
    let bug = output(
        &c.call(
            "create_task",
            json!({ "project": "TM", "title": "Crash", "type": "Bug" }),
        )
        .await,
    );
    assert!(bug["task"]["due_date"].is_string(), "{bug}");
    let given = output(
        &c.call(
            "create_task",
            json!({ "project": "TM", "title": "Given", "type": "Bug", "due_date": "2027-02-01" }),
        )
        .await,
    );
    assert_eq!(given["task"]["due_date"], "2027-02-01");
    let plain = output(
        &c.call("create_task", json!({ "project": "TM", "title": "Plain" }))
            .await,
    );
    assert!(plain["task"].get("due_date").is_none(), "{plain}");

    for key in ["TM-1", "TM-2"] {
        output(
            &c.call("move_task", json!({ "key": key, "status": "In progress" }))
                .await,
        );
    }
    let board = output(&c.call("get_board", json!({ "project": "TM" })).await);
    assert_eq!(column(&board)["under_minimum"], false, "{board}");
    c.close().await;
}

#[tokio::test]
async fn an_agent_finds_tasks_by_their_history() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Team", "key": "TM" }));
        for title in ["Went through", "Never moved", "Reprioritised"] {
            call("task.create", json!({ "project": "TM", "title": title }));
        }
        call(
            "task.move",
            json!({ "key": "TM-1", "status": "In progress" }),
        );
        call("task.move", json!({ "key": "TM-1", "status": "Backlog" }));
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "History Reader").await;
    output(
        &c.call(
            "update_task",
            json!({ "key": "TM-3", "priority": "urgent" }),
        )
        .await,
    );
    let find = async |query: &str| {
        keys(&output(
            &c.call("search_tasks", json!({ "query": query })).await,
        ))
    };
    assert_eq!(find("was:status:\"In progress\"").await, ["TM-1"]);
    assert_eq!(
        find("project:TM -was:status:\"In progress\"").await,
        ["TM-2", "TM-3"]
    );
    assert_eq!(find("changed:status").await, ["TM-1"]);
    assert_eq!(find("changed:priority").await, ["TM-3"]);
    assert_eq!(find("changed-by:agent").await, ["TM-3"]);
    assert_eq!(find("changed-by:\"history READER\"").await, ["TM-3"]);
    // An unknown status is an error the agent can read.
    let bad = c
        .call("search_tasks", json!({ "query": "was:status:Doing" }))
        .await;
    assert_eq!(bad.is_error, Some(true), "{bad:?}");
    c.close().await;
}
