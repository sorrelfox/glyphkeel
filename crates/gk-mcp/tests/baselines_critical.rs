// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The critical path and schedule baselines through MCP: get_schedule
//! returns each task's slack and whether it is critical; a read-only agent
//! lists baselines but can't save one; a standard agent saves, renames and
//! deletes one, and reads the slip against it.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use serde_json::json;

#[tokio::test]
async fn agents_read_slack_and_keep_baselines() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        for (op, input) in [
            ("project.create", json!({ "name": "Team", "key": "TM" })),
            (
                "task.create",
                json!({ "project": "TM", "title": "Design", "start_date": "2027-01-04", "due_date": "2027-01-06" }),
            ),
            (
                "task.create",
                json!({ "project": "TM", "title": "Build", "start_date": "2027-01-11", "due_date": "2027-01-15" }),
            ),
            (
                "relation.add",
                json!({ "key": "TM-1", "kind": "blocks", "other": "TM-2" }),
            ),
            (
                "baseline.create",
                json!({ "project": "TM", "name": "Kick-off" }),
            ),
        ] {
            engine.call(&person, op, input).unwrap();
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);

    options.profile = gk_api::Profile::ReadOnly;
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    let s = c
        .call("get_schedule", json!({ "project": "TM" }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(s["tasks"][0]["timing"]["slack_days"], 2, "{s}");
    assert_eq!(s["tasks"][1]["timing"]["critical"], true);
    assert_eq!(s["critical_path"]["tasks"], json!(["TM-2"]));
    let listed = c
        .call("list_baselines", json!({ "project": "TM" }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(listed["items"][0]["name"], "Kick-off");
    let refused = c
        .call(
            "create_baseline",
            json!({ "project": "TM", "name": "Mine" }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true));
    c.close().await;

    options.profile = gk_api::Profile::Standard;
    let c = connect(&options, mode, version, "Planner").await;
    let moved = c
        .call(
            "update_task",
            json!({ "key": "TM-2", "due_date": "2027-01-18" }),
        )
        .await;
    assert_eq!(moved.is_error, Some(false), "{}", text(&moved));
    let s = c
        .call(
            "get_schedule",
            json!({ "project": "TM", "baseline": "Kick-off" }),
        )
        .await
        .structured_content
        .unwrap();
    assert_eq!(s["tasks"][1]["baseline"]["slip_days"], 3, "{s}");
    let made = c
        .call(
            "create_baseline",
            json!({ "project": "TM", "name": "Second" }),
        )
        .await;
    assert_eq!(made.is_error, Some(false), "{}", text(&made));
    let made = made.structured_content.unwrap();
    assert_eq!(made["baseline"]["created_by"]["name"], "Planner");
    let renamed = c
        .call(
            "update_baseline",
            json!({ "project": "TM", "baseline": "Second", "name": "Later" }),
        )
        .await;
    assert_eq!(renamed.is_error, Some(false), "{}", text(&renamed));
    let gone = c
        .call(
            "delete_baseline",
            json!({ "project": "TM", "baseline": "Later" }),
        )
        .await;
    assert_eq!(gone.is_error, Some(false), "{}", text(&gone));
    let found = c
        .call("search_tasks", json!({ "query": "slip>2d" }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(found["items"][0]["key"], "TM-2", "{found}");
    c.close().await;
}
