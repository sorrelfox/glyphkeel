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
