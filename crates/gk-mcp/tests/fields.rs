// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields through MCP: an agent defines, changes and lists fields,
//! sets values on tasks, finds tasks by them, and changes them in bulk; a
//! read-only agent sees them; deleting one needs the full profile.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, set_agent_cap, text, workspace};
use gk_api::Profile;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn refused(r: &CallToolResult) -> String {
    assert_eq!(r.is_error, Some(true), "{r:?}");
    text(r)
}

fn keys(list: &Value) -> Vec<String> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

/// The value of the field named `name` in a task as `get_task` shows it.
fn value_of(task: &Value, name: &str) -> Value {
    task["fields"]
        .as_array()
        .and_then(|a| a.iter().find(|f| f["name"] == name))
        .map_or(Value::Null, |f| f["value"].clone())
}

#[tokio::test]
async fn a_standard_agent_defines_fields_and_works_with_their_values() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "project.create",
                json!({ "name": "Task manager", "key": "TM" }),
            )
            .unwrap();
        engine.close();
    }
    for (label, mode, version, _) in lifecycles() {
        let c = connect(&options, mode, version, "Field Keeper").await;
        let names = c.tool_names().await;
        for tool in [
            "list_custom_fields",
            "create_custom_field",
            "update_custom_field",
        ] {
            assert!(names.iter().any(|n| n == tool), "{label}: {tool}");
        }
        assert!(
            !names.iter().any(|n| n == "delete_custom_field"),
            "{label}: deleting is for the full profile"
        );
        if label.starts_with("discover") {
            // The second lifecycle runs on what the first one made.
            let listed = output(
                &c.call("list_custom_fields", json!({ "project": "TM" }))
                    .await,
            );
            assert_eq!(listed["fields"].as_array().unwrap().len(), 2, "{label}");
            c.close().await;
            continue;
        }

        let made = output(
            &c.call(
                "create_custom_field",
                json!({ "project": "TM", "name": "Story points", "kind": "integer" }),
            )
            .await,
        );
        assert_eq!(made["field"]["query_name"], "cf.story-points", "{made}");
        output(
            &c.call(
                "create_custom_field",
                json!({ "project": "TM", "name": "Area", "kind": "select",
                        "options": ["Front end", "Back end"] }),
            )
            .await,
        );
        let renamed = output(
            &c.call(
                "update_custom_field",
                json!({ "project": "TM", "field": "Story points", "name": "Points" }),
            )
            .await,
        );
        assert_eq!(renamed["field"]["name"], "Points");
        let bad = refused(
            &c.call(
                "create_custom_field",
                json!({ "project": "TM", "name": "points", "kind": "text" }),
            )
            .await,
        );
        assert!(bad.starts_with("Error ("), "{bad}");
        let listed = output(
            &c.call("list_custom_fields", json!({ "project": "TM" }))
                .await,
        );
        let names: Vec<&str> = listed["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Points", "Area"]);

        // Values on create and update, read back and searched for.
        output(
            &c.call(
                "create_task",
                json!({ "project": "TM", "title": "Front page",
                        "fields": { "Points": 3, "Area": "front end" } }),
            )
            .await,
        );
        output(
            &c.call(
                "create_task",
                json!({ "project": "TM", "title": "Database" }),
            )
            .await,
        );
        output(
            &c.call(
                "update_task",
                json!({ "key": "TM-2", "fields": { "cf.points": 8, "Area": "Back end" } }),
            )
            .await,
        );
        let wrong = refused(
            &c.call(
                "update_task",
                json!({ "key": "TM-2", "fields": { "Area": "Middle" } }),
            )
            .await,
        );
        assert!(wrong.contains("Middle"), "{wrong}");
        let task = output(&c.call("get_task", json!({ "keys": ["TM-1"] })).await);
        let task = if task.get("items").is_some() {
            task["items"][0].clone()
        } else {
            task
        };
        assert_eq!(value_of(&task, "Points"), json!(3), "{task}");
        assert_eq!(value_of(&task, "Area"), json!("Front end"));
        let found = output(
            &c.call("search_tasks", json!({ "query": "cf.points>4" }))
                .await,
        );
        assert_eq!(keys(&found), ["TM-2"]);
        assert_eq!(found["items"][0]["fields"]["points"], json!(8), "{found}");
        let found = output(
            &c.call(
                "search_tasks",
                json!({ "query": "was:cf.area:\"back end\" changed:cf.points" }),
            )
            .await,
        );
        assert_eq!(keys(&found), ["TM-2"]);

        // A bulk change sets one field on both, previewed first.
        let preview = output(
            &c.call(
                "bulk_update_tasks",
                json!({ "query": "project:TM", "changes": { "fields": { "Points": 5 } } }),
            )
            .await,
        );
        assert_eq!(preview["count"], 2);
        let done = output(
            &c.call(
                "bulk_update_tasks",
                json!({ "query": "project:TM", "changes": { "fields": { "Points": 5 } },
                        "dry_run": false, "expected_count": 2 }),
            )
            .await,
        );
        assert_eq!(done["changed"], 2, "{done}");
        let found = output(
            &c.call("search_tasks", json!({ "query": "cf.points:5" }))
                .await,
        );
        assert_eq!(keys(&found), ["TM-1", "TM-2"]);
        c.close().await;
    }
}

#[tokio::test]
async fn a_read_only_agent_sees_fields_and_a_full_one_may_delete_them() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call(
            "field.create",
            json!({ "project": "TM", "name": "Customer", "kind": "text" }),
        );
        call(
            "task.create",
            json!({ "project": "TM", "title": "Invoice", "fields": { "Customer": "Zanzibar Freight" } }),
        );
        set_agent_cap(&engine, "full");
        engine.close();
    }
    options.profile = Profile::ReadOnly;
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    let names = c.tool_names().await;
    assert!(names.iter().any(|n| n == "list_custom_fields"));
    assert!(!names.iter().any(|n| n == "create_custom_field"));
    let listed = output(&c.call("list_custom_fields", json!({})).await);
    assert_eq!(listed["fields"][0]["name"], "Customer", "{listed}");
    let found = output(
        &c.call("search_tasks", json!({ "query": "cf.customer:zanzibar" }))
            .await,
    );
    assert_eq!(keys(&found), ["TM-1"]);
    assert_eq!(
        found["items"][0]["fields"]["customer"], "Zanzibar Freight",
        "{found}"
    );
    // Full-text search finds the value too.
    let hits = output(&c.call("search", json!({ "text": "freight" })).await);
    assert_eq!(hits["items"][0]["task"], "TM-1", "{hits}");
    let not_offered = c
        .call(
            "update_task",
            json!({ "key": "TM-1", "fields": { "Customer": "Acme" } }),
        )
        .await;
    refused(&not_offered);
    c.close().await;

    options.profile = Profile::Full;
    let c = connect(&options, mode, version, "Cleaner").await;
    let wrong = refused(
        &c.call(
            "delete_custom_field",
            json!({ "project": "TM", "field": "Customer", "expected_task_count": 3 }),
        )
        .await,
    );
    assert!(wrong.starts_with("Error (conflict)"), "{wrong}");
    output(
        &c.call(
            "delete_custom_field",
            json!({ "project": "TM", "field": "Customer", "expected_task_count": 1 }),
        )
        .await,
    );
    let listed = output(
        &c.call("list_custom_fields", json!({ "project": "TM" }))
            .await,
    );
    assert!(listed["fields"].as_array().unwrap().is_empty(), "{listed}");
    c.close().await;
}
