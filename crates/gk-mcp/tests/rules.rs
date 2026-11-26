// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The person's rules for agents reach an agent as `forbidden` with the
//! reason, and the agent sees the rules on the project.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use serde_json::json;

#[tokio::test]
async fn an_agent_is_kept_out_of_a_status_the_person_closed() {
    let (_dir, options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        engine
            .call(
                &person,
                "project.create",
                json!({ "name": "Agents", "key": "AG" }),
            )
            .unwrap();
        engine
            .call(
                &person,
                "rules.update",
                json!({ "project": "AG",
                        "statuses": [{ "status": "Done", "rules": { "agents_may_not_enter": true } }] }),
            )
            .unwrap();
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);
    let c = connect(&options, mode, version, "Rule Taker").await;
    let project = c.call("get_project", json!({ "project": "AG" })).await;
    let shown = project.structured_content.clone().unwrap();
    let done = shown["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Done")
        .unwrap()
        .clone();
    assert_eq!(done["rules"]["agents_may_not_enter"], true);

    c.call("create_task", json!({ "project": "AG", "title": "Ship" }))
        .await;
    let refused = c
        .call("move_task", json!({ "key": "AG-1", "status": "Done" }))
        .await;
    assert_eq!(refused.is_error, Some(true));
    let message = text(&refused);
    assert!(
        message.starts_with("Error (forbidden): Agents may not move tasks into Done"),
        "{message}"
    );
    c.close().await;
}
