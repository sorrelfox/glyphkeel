// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project folders through MCP: a read-only agent lists them but can't
//! change them; a standard agent makes one and puts a project in it, and
//! its changes are its own in history, and it creates a project straight
//! into a folder.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, lifecycles, open_as_person, text, workspace};
use serde_json::json;

#[tokio::test]
async fn read_only_agents_list_folders_and_standard_agents_arrange_them() {
    let (_dir, mut options) = workspace();
    {
        let (engine, person) = open_as_person(&options);
        for (op, input) in [
            ("project.create", json!({ "name": "Team", "key": "TM" })),
            ("project.create", json!({ "name": "Yard", "key": "ZZ" })),
            ("task.create", json!({ "project": "TM", "title": "One" })),
            ("folder.create", json!({ "name": "Clients" })),
        ] {
            engine.call(&person, op, input).unwrap();
        }
        engine.close();
    }
    let (_, mode, version, _) = lifecycles().remove(0);

    options.profile = gk_api::Profile::ReadOnly;
    let c = connect(&options, mode.clone(), version.clone(), "Reader").await;
    let listed = c.call("list_folders", json!({})).await;
    let listed = listed.structured_content.clone().unwrap();
    assert_eq!(listed["items"][0]["name"], "Clients");
    let refused = c
        .call(
            "move_project_to_folder",
            json!({ "project": "TM", "folder": "Clients" }),
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
    let moved = c
        .call(
            "move_project_to_folder",
            json!({ "project": "TM", "folder": "Clients" }),
        )
        .await;
    assert_eq!(moved.is_error, Some(false), "{}", text(&moved));
    let moved = moved.structured_content.unwrap();
    assert_eq!(moved["folder"]["projects"], json!(["TM"]));
    assert_eq!(moved["folder"]["open_tasks"], 1);
    let activity = c
        .call("get_activity", json!({ "limit": 1 }))
        .await
        .structured_content
        .unwrap();
    assert_eq!(activity["items"][0]["entity"], "project");
    assert_eq!(activity["items"][0]["actor"]["name"], "Arranger");
    let renamed = c
        .call(
            "update_folder",
            json!({ "folder": "Clients", "name": "Customers" }),
        )
        .await;
    assert_eq!(renamed.is_error, Some(false), "{}", text(&renamed));
    let made = c
        .call(
            "create_project",
            json!({ "name": "Orchard", "key": "OR", "folder": "Customers" }),
        )
        .await;
    assert_eq!(made.is_error, Some(false), "{}", text(&made));
    let listed = c
        .call("list_folders", json!({}))
        .await
        .structured_content
        .unwrap();
    assert_eq!(listed["items"][0]["projects"], json!(["TM", "OR"]));
    let unknown = c.call("delete_folder", json!({ "folder": "Nope" })).await;
    assert!(
        text(&unknown).starts_with("Error (not_found)"),
        "{}",
        text(&unknown)
    );
    c.close().await;
}
