// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Changes an agent proposed, decided from the command line: the agent
//! server runs as its own process, its changes wait, and
//! `glyphkeel proposals list|show|approve|reject` decides on them.

#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::{Agent, Workspace};
use serde_json::{Value, json};

/// The proposal a `pending_approval` tool result names.
fn proposal_of(result: &Value) -> String {
    assert_eq!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Error (pending_approval)"), "{text}");
    text.lines()
        .find_map(|l| l.strip_prefix("Proposal: "))
        .unwrap()
        .to_owned()
}

#[test]
fn proposals_are_listed_shown_approved_and_rejected_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["-q", "project", "create", "--name", "Garden", "--key", "GA"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GA",
        "--title",
        "Plant",
    ]);
    ws.ok(&[
        "-q",
        "agents",
        "policy",
        "--max-profile",
        "standard",
        "--approvals",
        r#"{"read_only":"off","standard":"all_writes","full":"off","expire_days":14}"#,
    ]);

    let mut agent = Agent::start(ws.home(), &ws.path(), "planner");
    let first = proposal_of(&agent.call(
        "update_task",
        json!({ "key": "GA-1", "title": "Plant tulips" }),
    ));
    let second = proposal_of(&agent.call(
        "add_comment",
        json!({ "key": "GA-1", "body": "Bulbs are in the shed." }),
    ));
    agent.request(
        "resources/subscribe",
        json!({ "uri": "glyphkeel://proposals" }),
    );

    let listed = ws.json(&["proposals", "list", "--status", "pending"]);
    assert_eq!(listed["pending"], 2, "{listed}");
    let text = ws.ok(&["proposals", "list"]);
    assert!(text.contains("task.update"), "{text}");
    let shown = ws.json(&["proposals", "show", &first]);
    assert_eq!(
        shown["changes"][0]["fields"]["title"]["after"],
        "Plant tulips"
    );
    assert_eq!(shown["agent"]["name"], "planner");

    let approved = ws.json(&["proposals", "approve", &first]);
    assert_eq!(approved["approved"], 1, "{approved}");
    let task = ws.json(&["task", "show", "GA-1"]);
    assert_eq!(task["items"][0]["title"], "Plant tulips");
    assert!(
        agent
            .notification("notifications/resources/updated", Duration::from_secs(10))
            .is_some(),
        "the agent hears of the decision"
    );

    let rejected = ws.json(&[
        "proposals",
        "reject",
        &second,
        "--reason",
        "Say where the shed key is.",
    ]);
    assert_eq!(rejected["items"][0]["status"], "rejected");
    let seen = agent.ok("get_proposals", json!({ "ids": [second] }));
    assert_eq!(seen["items"][0]["reason"], "Say where the shed key is.");

    // Nothing waits any more: approving again is refused.
    let refused = ws.fails(3, &["proposals", "reject", &first]);
    assert!(!refused.is_empty());
    agent.finish();
}
