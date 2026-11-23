// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent creates a project and a task and moves it on the board, and the
//! person sees the result with the agent named in history.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

#[tokio::test]
async fn an_agent_creates_and_moves_a_task() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Round Trip").await;

    let p = output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    assert_eq!(p["project"]["key"], "AG");
    let request = json!({ "project": "AG", "title": "Plan the work", "client_request_id": "c-1" });
    let created = output(&c.call("create_task", request.clone()).await);
    assert_eq!(created["task"]["key"], "AG-1");
    assert_eq!(created["task"]["created_by"]["kind"], "agent");
    // A retry with the same request ID answers the same and creates nothing.
    assert_eq!(output(&c.call("create_task", request).await), created);

    let version = created["task"]["version"].as_i64().unwrap();
    let moved = output(
        &c.call(
            "move_task",
            json!({ "key": "ag-1", "status": "In progress", "expected_version": version }),
        )
        .await,
    );
    assert_eq!(moved["task"]["status"]["name"], "In progress");
    let stale = c
        .call(
            "move_task",
            json!({ "key": "AG-1", "status": "Done", "expected_version": version }),
        )
        .await;
    assert_eq!(stale.is_error, Some(true));

    let board = output(&c.call("get_board", json!({ "project": "AG" })).await);
    let doing = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|col| col["name"] == "In progress")
        .unwrap();
    assert_eq!(doing["cards"][0]["key"], "AG-1");

    // Removing a status is permanent: the standard profile can't.
    let removal = c
        .call(
            "update_project",
            json!({ "project": "AG", "statuses": [{ "status": "Review", "remove": true }] }),
        )
        .await;
    assert_eq!(removal.is_error, Some(true));
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let feed = engine
        .call(&person, "history.feed", json!({ "actor": "Round Trip" }))
        .unwrap();
    let ops: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["op"].as_str().unwrap())
        .collect();
    assert!(ops.contains(&"task.move"), "{ops:?}");
    assert_eq!(ops.iter().filter(|o| **o == "task.create").count(), 1);
}

#[tokio::test]
async fn an_agent_edits_a_description_and_ticks_its_checklist() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Checklist Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call(
            "create_task",
            json!({ "project": "AG", "title": "Ship", "description": "Plan\n\n- [ ] build\n- [ ] test\n" }),
        )
        .await,
    );
    let task = output(
        &c.call(
            "get_task",
            json!({ "keys": ["AG-1"], "include": ["checklist", "revisions"] }),
        )
        .await,
    );
    let item = &task["items"][0]["checklist_items"][1];
    assert_eq!(
        (item["number"].clone(), item["text"].clone()),
        (json!(2), json!("test"))
    );
    let base = task["items"][0]["description_revision"].clone();

    let ticked = output(
        &c.call(
            "update_checklist",
            json!({ "key": "AG-1", "toggle": [{ "number": 2, "text": "test", "checked": true }] }),
        )
        .await,
    );
    assert_eq!(ticked["checklist"], json!({ "done": 1, "total": 2 }));

    // A whole description from the old base merges with the tick.
    let saved = output(
        &c.call(
            "update_task",
            json!({ "key": "AG-1", "description": "Plan, revised\n\n- [ ] build\n- [ ] test\n",
                    "base_revision": base }),
        )
        .await,
    );
    assert_eq!(saved["description_save"]["outcome"], "merged");
    assert_eq!(
        saved["task"]["description"],
        "Plan, revised\n\n- [ ] build\n- [x] test\n"
    );
    let revision = saved["description_save"]["revision"].clone();
    let detail = output(
        &c.call(
            "get_description_revision",
            json!({ "revision": revision, "diff_from": base }),
        )
        .await,
    );
    assert!(detail["diff"].as_str().unwrap().contains("+Plan, revised"));
    c.close().await;
}

#[tokio::test]
async fn an_agent_reorders_subtasks_and_reads_one_tasks_history() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Order Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call("create_task", json!({ "project": "AG", "title": "Parent" }))
            .await,
    );
    for title in ["First", "Second"] {
        output(
            &c.call(
                "create_task",
                json!({ "project": "AG", "title": title, "parent": "AG-1" }),
            )
            .await,
        );
    }
    let reordered = output(
        &c.call(
            "reorder_subtask",
            json!({ "key": "AG-3", "before": "AG-2" }),
        )
        .await,
    );
    assert_eq!(reordered["task"]["key"], "AG-1");
    assert_eq!(reordered["task"]["subtasks"][0]["key"], "AG-3");
    assert_eq!(reordered["task"]["subtasks"][1]["key"], "AG-2");

    let feed = output(&c.call("get_activity", json!({ "task": "AG-3" })).await);
    let ops: Vec<&str> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["task.reorder_subtask", "task.create"]);
    c.close().await;
}

#[tokio::test]
async fn an_agent_logs_time_runs_the_timer_and_sets_a_reminder() {
    let (_dir, options) = workspace();
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Time Agent").await;
    output(
        &c.call("create_project", json!({ "name": "Agents", "key": "AG" }))
            .await,
    );
    output(
        &c.call("create_task", json!({ "project": "AG", "title": "Parser" }))
            .await,
    );
    let request = json!({ "key": "AG-1", "duration": "1h 30m", "date": "2026-11-23",
                          "note": "Wrote it", "client_request_id": "t-1" });
    let logged = output(&c.call("log_time", request.clone()).await);
    assert_eq!(logged["spent_minutes"], 90);
    assert_eq!(logged["entry"]["author"]["kind"], "agent");
    // A retry logs nothing more.
    assert_eq!(output(&c.call("log_time", request).await), logged);

    let report = output(
        &c.call(
            "get_time_report",
            json!({ "from": "2026-11-23", "to": "2026-11-23" }),
        )
        .await,
    );
    assert_eq!(report["items"][0]["key"], "AG-1");
    assert_eq!(report["total_duration"], "1h 30m");

    let started = output(&c.call("start_timer", json!({ "key": "AG-1" })).await);
    assert_eq!(started["timer"]["started_by"]["name"], "Time Agent");
    let overview = output(&c.call("get_workspace", json!({})).await);
    assert_eq!(overview["timer"]["task"]["key"], "AG-1");
    let stopped = output(&c.call("stop_timer", json!({ "discard": true })).await);
    assert!(stopped.get("timer").is_none());

    let set = output(
        &c.call(
            "set_reminder",
            json!({ "key": "AG-1", "at": "2020-01-01T09:00:00Z" }),
        )
        .await,
    );
    assert_eq!(set["reminder"]["at"], "2020-01-01T09:00:00.000Z");
    let due = output(&c.call("get_reminders", json!({})).await);
    assert_eq!(due["items"][0]["kind"], "reminder");
    let id = due["items"][0]["id"].clone();
    let ack = output(
        &c.call("acknowledge_reminders", json!({ "ids": [id] }))
            .await,
    );
    assert_eq!(ack["acknowledged"].as_array().unwrap().len(), 1);
    c.close().await;
}
