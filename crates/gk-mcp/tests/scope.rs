// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent started with `--project TM` works only in TM and its
//! sub-projects: other projects don't appear in lists or searches, and
//! naming or changing anything in them fails.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn refused(r: &CallToolResult) {
    assert_eq!(r.is_error, Some(true), "{r:?}");
}

fn keys(list: &Value) -> Vec<String> {
    let mut keys: Vec<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn an_agent_limited_to_a_project_sees_and_changes_only_that_project() {
    let (_dir, mut options) = workspace();
    let (other_comment, other_page) = {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task manager", "key": "TM" }),
        );
        call(
            "project.create",
            json!({ "name": "Its docs", "key": "DOC", "parent": "TM" }),
        );
        call("project.create", json!({ "name": "Private", "key": "PRV" }));
        for (project, title) in [
            ("TM", "Shared word in TM"),
            ("DOC", "Shared word in DOC"),
            ("PRV", "Shared word in PRV"),
        ] {
            call(
                "task.create",
                json!({ "project": project, "title": title, "due_date": "today" }),
            );
        }
        let comment = call(
            "comment.add",
            json!({ "key": "PRV-1", "body": "A private remark." }),
        )["comment"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let page = call(
            "wiki.create",
            json!({ "project": "PRV", "title": "Secrets", "body": "Shared word." }),
        )["page"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        engine.close();
        (comment, page)
    };
    options.project = Some("TM".to_owned());
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scoped Helper").await;

    // Lists and searches show TM and its sub-project only.
    let projects = output(&c.call("list_projects", json!({})).await);
    assert_eq!(keys(&projects), ["DOC", "TM"]);
    let tasks = output(&c.call("search_tasks", json!({})).await);
    assert_eq!(keys(&tasks), ["DOC-1", "TM-1"]);
    let named = output(
        &c.call("search_tasks", json!({ "query": "project:PRV" }))
            .await,
    );
    assert_eq!(named["total"], 0);
    let found = output(&c.call("search", json!({ "text": "shared" })).await);
    assert!(
        found["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["task"].as_str().is_some_and(|k| !k.starts_with("PRV"))),
        "{found}"
    );
    assert_eq!(found["total"], 2, "{found}");
    let today = output(&c.call("get_today", json!({})).await);
    assert!(!today.to_string().contains("PRV-1"), "{today}");
    let feed = output(&c.call("get_activity", json!({})).await);
    assert!(
        !feed["items"].as_array().unwrap().is_empty() && !feed.to_string().contains("Private"),
        "{feed}"
    );

    // Anything of another project is missing.
    refused(&c.call("get_task", json!({ "keys": ["PRV-1"] })).await);
    refused(&c.call("get_project", json!({ "project": "PRV" })).await);
    refused(&c.call("get_board", json!({ "project": "PRV" })).await);
    refused(&c.call("search_tasks", json!({ "project": "PRV" })).await);
    refused(&c.call("get_wiki_page", json!({ "page": other_page })).await);
    refused(
        &c.call(
            "update_comment",
            json!({ "comment": other_comment, "body": "Changed." }),
        )
        .await,
    );
    refused(
        &c.call(
            "create_task",
            json!({ "project": "PRV", "title": "Sneaked in" }),
        )
        .await,
    );
    refused(
        &c.call(
            "link_tasks",
            json!({ "key": "TM-1", "kind": "relates", "other": "PRV-1" }),
        )
        .await,
    );
    // A new project outside TM is a change outside the limit.
    refused(
        &c.call(
            "create_project",
            json!({ "name": "Elsewhere", "key": "ELS" }),
        )
        .await,
    );

    // Inside TM the agent works as usual, sub-projects included.
    let made = output(
        &c.call(
            "create_task",
            json!({ "project": "DOC", "title": "Write the guide" }),
        )
        .await,
    );
    assert_eq!(made["task"]["key"], "DOC-2");
    let sub = output(
        &c.call(
            "create_project",
            json!({ "name": "Below", "key": "BLW", "parent": "TM" }),
        )
        .await,
    );
    assert_eq!(sub["project"]["key"], "BLW");
    output(
        &c.call(
            "create_task",
            json!({ "project": "BLW", "title": "In the new sub-project" }),
        )
        .await,
    );
    let bulk = output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "title:shared", "changes": { "priority": "high" } }),
        )
        .await,
    );
    assert_eq!(bulk["count"], 2, "{bulk}");
    output(
        &c.call(
            "bulk_update_tasks",
            json!({ "query": "title:shared", "changes": { "priority": "high" },
                    "dry_run": false, "expected_count": 2 }),
        )
        .await,
    );
    c.close().await;

    // The private project is untouched.
    let (engine, person) = open_as_person(&options);
    let prv = engine
        .call(&person, "task.get", json!({ "keys": ["PRV-1"] }))
        .unwrap();
    assert_eq!(prv["items"][0]["priority"], "none");
    let tm = engine
        .call(&person, "task.get", json!({ "keys": ["TM-1"] }))
        .unwrap();
    assert_eq!(tm["items"][0]["priority"], "high");
    assert_eq!(prv["items"][0]["title"], "Shared word in PRV");
}
