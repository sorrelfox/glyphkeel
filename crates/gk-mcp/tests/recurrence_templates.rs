// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An agent makes a task recurring, previews and skips occurrences, and
//! uses templates. A read-only agent can look at templates and rules but
//! change nothing.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, set_agent_cap, workspace};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

#[tokio::test]
async fn an_agent_sets_a_rule_skips_once_and_uses_a_template() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
        call("project.create", json!({ "name": "Ops", "key": "OP" }));
        call(
            "task.create",
            json!({ "project": "OP", "title": "Rotate keys", "due_date": "2030-03-04" }),
        );
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Scheduler").await;
    let preview = output(
        &c.call(
            "preview_recurrence",
            json!({ "text": "the 1st of each month", "starts": "2030-03-04", "limit": 2 }),
        )
        .await,
    );
    assert_eq!(
        preview["dates"],
        json!(["2030-04-01", "2030-05-01"]),
        "{preview}"
    );
    let set = output(
        &c.call(
            "set_recurrence",
            json!({ "key": "OP-1", "text": "monthly on the 4th" }),
        )
        .await,
    );
    assert_eq!(set["recurrence"]["next_date"], "2030-04-04", "{set}");
    let skipped = output(&c.call("skip_occurrence", json!({ "key": "OP-1" })).await);
    assert_eq!(skipped["due_date"], "2030-04-04");
    let made = output(
        &c.call(
            "create_template",
            json!({ "name": "Key rotation", "from_task": "OP-1" }),
        )
        .await,
    );
    assert_eq!(made["template"]["task"]["title"], "Rotate keys", "{made}");
    let used = output(
        &c.call(
            "use_template",
            json!({ "template": "Key rotation", "project": "OP", "date": "2030-06-01",
                    "client_request_id": "rotate-june" }),
        )
        .await,
    );
    assert_eq!(used["task"]["key"], "OP-2");
    let again = output(
        &c.call(
            "use_template",
            json!({ "template": "Key rotation", "project": "OP", "date": "2030-06-01",
                    "client_request_id": "rotate-june" }),
        )
        .await,
    );
    assert_eq!(again["task"]["key"], "OP-2", "a retry makes nothing new");
    let listed = output(
        &c.call("list_templates", json!({ "project": "OP", "kind": "task" }))
            .await,
    );
    assert_eq!(listed["total"], 1);
    output(&c.call("stop_recurrence", json!({ "key": "OP-1" })).await);
    c.close().await;
}

#[tokio::test]
async fn a_read_only_agent_reads_templates_but_changes_nothing() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "project.create",
                json!({ "name": "Ops", "key": "OP" }),
            )
            .unwrap();
        engine
            .call(
                &person,
                "template.create",
                json!({ "name": "Check", "project": "OP", "task": { "title": "Check" } }),
            )
            .unwrap();
        set_agent_cap(&engine, "read-only");
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Reader").await;
    let names = c.tool_names().await;
    assert!(names.contains(&"list_templates".to_owned()));
    assert!(names.contains(&"preview_recurrence".to_owned()));
    assert!(!names.contains(&"use_template".to_owned()));
    assert!(!names.contains(&"set_recurrence".to_owned()));
    let t = output(
        &c.call(
            "get_template",
            json!({ "template": "Check", "project": "OP" }),
        )
        .await,
    );
    assert_eq!(t["task"]["title"], "Check");
    c.close().await;
}
