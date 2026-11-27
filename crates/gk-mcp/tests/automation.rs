// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Automation rules through the agent server: an agent tries a rule, makes
//! it, sees it act and reads its log; a rule an agent made is held to the
//! rules for agents.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use serde_json::json;

#[tokio::test]
async fn an_agent_tries_makes_and_reads_a_rule() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        for (op, input) in [
            ("project.create", json!({ "name": "Rules", "key": "ZZ" })),
            ("label.create", json!({ "project": "ZZ", "name": "triage" })),
            (
                "rules.update",
                json!({ "project": "ZZ", "agents": { "read_only": ["priority"] } }),
            ),
        ] {
            engine.call(&person, op, input).unwrap();
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Rule Maker").await;
    let names = c.tool_names().await;
    for tool in [
        "list_automations",
        "get_automation",
        "create_automation",
        "test_automation",
        "get_automation_runs",
    ] {
        assert!(names.iter().any(|n| n == tool), "{tool} is listed");
    }
    c.call("create_task", json!({ "project": "ZZ", "title": "First" }))
        .await;
    let tried = c
        .call(
            "test_automation",
            json!({ "task": "ZZ-1", "draft": {
                "trigger": { "on": "task_created" },
                "actions": [{ "kind": "add_label", "label": "triage" }],
            } }),
        )
        .await;
    let result = tried.structured_content.clone().unwrap();
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["changes"][0]["field"], "labels");

    let made = c
        .call(
            "create_automation",
            json!({
                "project": "ZZ",
                "name": "Triage",
                "trigger": { "on": "task_created" },
                "actions": [
                    { "kind": "add_label", "label": "triage" },
                    { "kind": "set_fields", "fields": { "priority": "high" } },
                ],
            }),
        )
        .await;
    assert_ne!(made.is_error, Some(true));
    let rule = made.structured_content.clone().unwrap();
    assert_eq!(rule["rule"]["held_as_agent"], true);
    assert_eq!(rule["rule"]["created_by"]["kind"], "agent");

    c.call("create_task", json!({ "project": "ZZ", "title": "Second" }))
        .await;
    let task = c.call("get_task", json!({ "keys": ["ZZ-2"] })).await;
    let task = task.structured_content.clone().unwrap();
    assert_eq!(
        task["items"][0]["priority"], "none",
        "the rule may not change what agents may not"
    );
    let runs = c
        .call(
            "get_automation_runs",
            json!({ "rule": "Triage", "project": "ZZ" }),
        )
        .await;
    let runs = runs.structured_content.clone().unwrap();
    assert_eq!(runs["items"][0]["result"], "failed");
    assert_eq!(
        runs["items"][0]["error"]["details"]["message_id"],
        "rule-agent-field"
    );

    let fixed = c
        .call(
            "update_automation",
            json!({ "rule": "Triage", "project": "ZZ",
                    "actions": [{ "kind": "add_label", "label": "triage" }] }),
        )
        .await;
    assert_ne!(fixed.is_error, Some(true));
    c.call("create_task", json!({ "project": "ZZ", "title": "Third" }))
        .await;
    let task = c.call("get_task", json!({ "keys": ["ZZ-3"] })).await;
    let task = task.structured_content.clone().unwrap();
    assert_eq!(task["items"][0]["labels"][0]["name"], "triage");
    let listed = c.call("list_automations", json!({ "project": "ZZ" })).await;
    let listed = listed.structured_content.clone().unwrap();
    assert_eq!(listed["items"][0]["last_run"]["result"], "done");
    let gone = c
        .call(
            "delete_automation",
            json!({ "rule": "Triage", "project": "ZZ" }),
        )
        .await;
    assert_ne!(gone.is_error, Some(true));
    c.close().await;
}
