// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project folders from the command line, against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;
use serde_json::json;

#[test]
fn folders_are_made_filled_renamed_and_deleted_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["project", "create", "--name", "Garden", "--key", "GA"]);
    let made = ws.json(&["folder", "create", "--name", "Clients", "--hue", "teal"]);
    assert_eq!(made["folder"]["name"], "Clients");
    ws.ok(&["-q", "folder", "move", "GK", "--folder", "Clients"]);
    ws.ok(&[
        "-q", "folder", "move", "GA", "--folder", "clients", "--at", "top",
    ]);
    let listed = ws.json(&["folder", "list"]);
    assert_eq!(listed["items"][0]["projects"], json!(["GA", "GK"]));

    let renamed = ws.json(&["folder", "rename", "Clients", "Customers"]);
    assert_eq!(renamed["folder"]["name"], "Customers");
    let out = ws.json(&["folder", "move", "GA", "--none", "true"]);
    assert!(out.get("folder").is_none(), "{out}");
    let text = ws.ok(&["folder", "list"]);
    assert!(text.contains("Customers"), "{text}");

    let gone = ws.json(&["folder", "delete", "Customers"]);
    assert_eq!(gone["released"], json!(["GK"]));
    let projects = ws.json(&["project", "list"]);
    assert!(
        projects["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p.get("folder").is_none()),
        "{projects}"
    );
}

#[test]
fn a_project_is_created_in_a_folder_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["-q", "folder", "create", "--name", "Clients"]);
    let made = ws.json(&[
        "project", "create", "--name", "Garden", "--key", "GA", "--folder", "Clients",
    ]);
    assert!(made["project"]["folder"].is_string(), "{made}");
    let listed = ws.json(&["folder", "list"]);
    assert_eq!(listed["items"][0]["projects"], json!(["GA"]));
    let refused = ws.fails(
        1,
        &[
            "project", "create", "--name", "Kid", "--key", "KD", "--parent", "GA", "--folder",
            "Clients",
        ],
    );
    assert!(refused.contains("sub-project"), "{refused}");
}
