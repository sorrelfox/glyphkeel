// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards through MCP: a read-only agent reads what the widgets show
//! but can't change them; a standard agent arranges them, and its changes
//! are its own in history.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use serde_json::json;

#[tokio::test]
async fn read_only_agents_read_widgets_and_standard_agents_edit_them() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        for (op, input) in [
            ("project.create", json!({ "name": "Team", "key": "TM" })),
            ("task.create", json!({ "project": "TM", "title": "One" })),
            (
                "dashboard.create",
                json!({ "name": "Team", "starter": true }),
            ),
        ] {
            engine.call(&person, op, input).unwrap();
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);

    options.profile = gk_api::Profile::ReadOnly;
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    let shown = c
        .call("get_dashboard", json!({ "dashboard": "Team" }))
        .await;
    let shown = shown.structured_content.clone().unwrap();
    assert_eq!(shown["widgets"][0]["count"], 1);
    let refused = c
        .call(
            "add_dashboard_widget",
            json!({ "dashboard": "Team", "widget": { "kind": "note" } }),
        )
        .await;
    assert_eq!(refused.is_error, Some(true));
    assert!(
        text(&refused).starts_with("Error (forbidden)"),
        "{}",
        text(&refused)
    );
    c.close().await;

    options.profile = gk_api::Profile::Standard;
    let c = connect(&options, mode, version, "Arranger").await;
    let added = c
        .call(
            "add_dashboard_widget",
            json!({ "dashboard": "Team", "widget": { "kind": "note",
                    "settings": { "text": "Hello" } }, "position": 1 }),
        )
        .await;
    assert_eq!(added.is_error, Some(false), "{}", text(&added));
    let activity = c
        .call("get_activity", json!({ "limit": 1 }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(activity["items"][0]["entity"], "dashboard");
    assert_eq!(activity["items"][0]["actor"]["name"], "Arranger");
    let unknown = c
        .call(
            "update_dashboard_widget",
            json!({ "dashboard": "Team", "widget": "99", "width": 2 }),
        )
        .await;
    assert!(
        text(&unknown).starts_with("Error (not_found)"),
        "{}",
        text(&unknown)
    );
    c.close().await;
}

#[tokio::test]
async fn agents_limited_to_a_project_see_no_other_projects_dashboards() {
    let (_dir, mut options) = workspace();
    let other_id = {
        let (engine, person) = open_as_person(&options);
        for (op, input) in [
            ("project.create", json!({ "name": "Team", "key": "TM" })),
            (
                "project.create",
                json!({ "name": "Elsewhere", "key": "ZZ" }),
            ),
            ("task.create", json!({ "project": "TM", "title": "One" })),
            ("task.create", json!({ "project": "ZZ", "title": "Two" })),
            ("task.create", json!({ "project": "ZZ", "title": "Three" })),
            (
                "dashboard.create",
                json!({ "name": "Everything", "starter": true }),
            ),
            (
                "dashboard.create",
                json!({ "name": "Team board", "project": "TM", "starter": true }),
            ),
        ] {
            engine.call(&person, op, input).unwrap();
        }
        // A widget of the workspace's dashboard that looks at the other
        // project.
        engine
            .call(
                &person,
                "dashboard.add_widget",
                json!({ "dashboard": "Everything", "widget": { "kind": "query_list",
                        "settings": { "project": "ZZ", "query": "is:open" } } }),
            )
            .unwrap();
        let other = engine
            .call(
                &person,
                "dashboard.create",
                json!({ "name": "Secret", "project": "ZZ", "starter": true }),
            )
            .unwrap();
        engine.close();
        other["dashboard"]["id"]
            .as_str()
            .or_else(|| other["id"].as_str())
            .unwrap()
            .to_owned()
    };
    let (_, mode, version, _) = lifecycles().remove(0);
    options.project = Some("TM".to_owned());
    let c = connect(&options, mode, version, "Scoped").await;

    let listed = c
        .call("list_dashboards", json!({ "all": true }))
        .await
        .structured_content
        .unwrap();
    let names: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Everything", "Team board"], "{listed}");

    // By ID or by name, the other project's dashboard isn't there.
    for args in [
        json!({ "dashboard": other_id }),
        json!({ "dashboard": "Secret", "project": "ZZ" }),
    ] {
        let r = c.call("get_dashboard", args.clone()).await;
        assert_eq!(r.is_error, Some(true), "{args}: {}", text(&r));
        assert!(!text(&r).contains("Two"), "{}", text(&r));
    }
    let r = c
        .call(
            "update_dashboard",
            json!({ "dashboard": other_id, "name": "Mine now" }),
        )
        .await;
    assert_eq!(r.is_error, Some(true), "{}", text(&r));
    let r = c
        .call("delete_dashboard", json!({ "dashboard": other_id }))
        .await;
    assert_eq!(r.is_error, Some(true), "{}", text(&r));

    // The workspace's dashboard counts only what the agent may see.
    let everything = c
        .call("get_dashboard", json!({ "dashboard": "Everything" }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(everything["widgets"][0]["count"], 1, "{everything}");
    let shown = everything["widgets"].to_string();
    for hidden in ["Two", "Three", "Secret", "ZZ-"] {
        assert!(!shown.contains(hidden), "{hidden}: {shown}");
    }
    let elsewhere = everything["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["widget"]["kind"] == "query_list")
        .unwrap();
    assert!(elsewhere["error"].is_object(), "{elsewhere}");
    assert!(elsewhere["count"].is_null(), "{elsewhere}");
    let by_status = everything["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["widget"]["kind"] == "tasks_by_field")
        .unwrap();
    assert_eq!(by_status["count"], 1, "{by_status}");
    c.close().await;

    let (engine, person) = open_as_person(&options);
    let all = engine
        .call(&person, "dashboard.list", json!({ "all": true }))
        .unwrap();
    assert_eq!(all["total"], 3, "{all}");
    assert!(all.to_string().contains("Secret"));
    engine.close();
}
