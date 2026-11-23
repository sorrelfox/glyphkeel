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
