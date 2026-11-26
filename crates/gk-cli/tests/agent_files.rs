// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Files named by path: the person's commands take any path, while the
//! agent server (`glyphkeel mcp`, its own process) keeps agents to the
//! workspace's `exports` folder and the folders in the personal setting
//! `agent_folders`, read again while it runs.

#![allow(clippy::unwrap_used)]

mod common;

use std::fs;

use common::{Agent, Workspace};
use serde_json::json;

#[test]
fn the_person_attaches_any_file_and_agents_only_from_their_folders() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["task", "create", "--project", "GK", "--title", "Ship"]);
    let elsewhere = ws.home().join("Documents");
    fs::create_dir_all(&elsewhere).unwrap();
    let elsewhere = fs::canonicalize(&elsewhere).unwrap();
    let file = elsewhere.join("report.txt");
    fs::write(&file, "Report.\n").unwrap();
    let file_arg = file.to_string_lossy().into_owned();

    // The person's own command: any path.
    let attached = ws.json(&["file", "attach", "--task", "GK-1", "--path", &file_arg]);
    assert_eq!(attached["file"]["name"], "report.txt", "{attached}");

    // An agent: refused, with the folders it may use named.
    let mut agent = Agent::start(ws.home(), &ws.path(), "Attacher");
    let exports = ws.path().join("exports");
    let info = agent.ok("get_workspace", json!({}));
    let folders = info["agent_folders"].as_array().unwrap().clone();
    assert_eq!(folders.len(), 1, "{info}");
    assert!(folders[0].as_str().unwrap().ends_with("exports"), "{info}");
    let refused = agent.call("attach_file", json!({ "task": "GK-1", "path": file_arg }));
    assert_eq!(refused["isError"], true, "{refused}");
    let text = refused["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Error (forbidden)"), "{text}");
    assert!(text.contains(folders[0].as_str().unwrap()), "{text}");
    assert!(text.contains("Preferences › Agents & CLI"), "{text}");

    // A plain name is a file in the exports folder.
    fs::create_dir_all(&exports).unwrap();
    fs::write(exports.join("notes.txt"), "Notes.\n").unwrap();
    let ok = agent.ok(
        "attach_file",
        json!({ "task": "GK-1", "path": "notes.txt" }),
    );
    assert_eq!(ok["file"]["name"], "notes.txt");

    // The person adds the folder in their personal settings; the running
    // server follows at once.
    let config = ws.home().join("config").join("glyphkeel");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("settings.json"),
        serde_json::to_vec(&json!({ "agent_folders": [elsewhere.to_string_lossy()] })).unwrap(),
    )
    .unwrap();
    let ok = agent.ok(
        "attach_file",
        json!({ "task": "GK-1", "path": file.to_string_lossy() }),
    );
    assert_eq!(ok["file"]["name"], "report.txt");
    let info = agent.ok("get_workspace", json!({}));
    assert_eq!(
        info["agent_folders"][1],
        json!(elsewhere.to_string_lossy()),
        "{info}"
    );
    // Its hidden files stay off limits, and `..` never works.
    fs::write(elsewhere.join(".secret"), "x").unwrap();
    for path in [
        elsewhere.join(".secret").to_string_lossy().into_owned(),
        format!("{}/../Documents/report.txt", elsewhere.to_string_lossy()),
    ] {
        let refused = agent.call("attach_file", json!({ "task": "GK-1", "path": path }));
        assert_eq!(refused["isError"], true, "{path}: {refused}");
    }
    agent.finish();
}
