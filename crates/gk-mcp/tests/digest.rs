// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The digest through MCP: an agent catches up on what others changed with
//! `get_digest`; one limited to a project sees only that project's part,
//! and no agent can move the person's marker.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, workspace};
use gk_api::Profile;
use gk_engine::{Context, SessionStart};
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn output(r: &CallToolResult) -> Value {
    assert_eq!(r.is_error, Some(false), "{r:?}");
    r.structured_content.clone().unwrap()
}

fn names(list: &Value) -> Vec<String> {
    let mut names: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn get_digest_keeps_to_the_project_an_agent_is_limited_to() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        let call = |ctx: &Context, op: &str, input: Value| engine.call(ctx, op, input).unwrap();
        call(
            &person,
            "project.create",
            json!({ "name": "Team", "key": "TM" }),
        );
        call(
            &person,
            "project.create",
            json!({ "name": "Its docs", "key": "DOC", "parent": "TM" }),
        );
        call(
            &person,
            "project.create",
            json!({ "name": "Private", "key": "PRV" }),
        );
        for (name, project) in [("Team agent", "DOC"), ("Private agent", "PRV")] {
            let session = engine
                .start_session(&SessionStart {
                    name: name.into(),
                    client_id: Some("tester".into()),
                    client_version: None,
                    profile: Profile::Standard,
                })
                .unwrap();
            let agent = Context::agent(name, session, Profile::Standard);
            call(
                &agent,
                "task.create",
                json!({ "project": project, "title": format!("By {name}") }),
            );
            engine.end_session(session).unwrap();
        }
        call(
            &Context::user(gk_api::ops::common::Via::Cli),
            "task.create",
            json!({ "project": "PRV", "title": "Scripted secret", "due_date": "today" }),
        );
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);

    // Unlimited, an agent sees both sessions and the command line.
    let c = connect(&options, mode.clone(), version.clone(), "Catcher").await;
    let all = output(&c.call("get_digest", json!({ "since": "today" })).await);
    assert_eq!(names(&all["agents"]), ["Private agent", "Team agent"]);
    assert_eq!(all["cli"]["changes"], 1, "{all}");
    assert_eq!(all["became_due"][0]["task"]["key"], "PRV-2");
    // An agent catches up on the person's own changes in the app too.
    assert!(all["app"]["changes"].as_u64().unwrap() > 0, "{all}");
    // The marker is the person's: agents have no tool for it.
    assert!(
        !c.tool_names()
            .await
            .iter()
            .any(|name| name.contains("dismiss")),
    );
    c.close().await;

    options.project = Some("TM".to_owned());
    let c = connect(&options, mode, version, "Scoped catcher").await;
    let scoped = output(&c.call("get_digest", json!({ "since": "today" })).await);
    assert_eq!(names(&scoped["agents"]), ["Team agent"]);
    assert_eq!(scoped["agents"][0]["tasks"][0]["key"], "DOC-1");
    assert_eq!(scoped["cli"]["changes"], 0);
    assert_eq!(scoped["became_due"], json!([]));
    assert!(!scoped.to_string().contains("PRV"), "{scoped}");
    let refused = c.call("get_digest", json!({ "project": "PRV" })).await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    // Left out, `since` is the end of this agent's previous session; this
    // one has none, so the last day.
    let own = output(&c.call("get_digest", json!({})).await);
    assert_eq!(own["since_from"], "last_day");
    c.close().await;
}
