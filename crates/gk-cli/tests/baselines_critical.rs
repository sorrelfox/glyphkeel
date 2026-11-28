// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The critical path and schedule baselines from the command line: the
//! schedule with only its critical tasks, saving and listing baselines, and
//! the slip against one.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Workspace, keys};

/// Project GK: Design (Monday to Wednesday) blocks Build (the next Monday
/// to Friday); Notes runs one day alongside.
fn plan() -> Workspace {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    for args in [
        &[
            "--title",
            "Design",
            "--start-date",
            "2027-01-04",
            "--due-date",
            "2027-01-06",
        ][..],
        &[
            "--title",
            "Build",
            "--start-date",
            "2027-01-11",
            "--due-date",
            "2027-01-15",
        ],
        &[
            "--title",
            "Notes",
            "--start-date",
            "2027-01-05",
            "--due-date",
            "2027-01-05",
        ],
    ] {
        let mut all = vec!["-q", "task", "create", "--project", "GK"];
        all.extend_from_slice(args);
        ws.ok(&all);
    }
    ws.ok(&[
        "-q", "task", "link", "GK-1", "--kind", "blocks", "--other", "GK-2",
    ]);
    ws
}

#[test]
fn the_schedule_shows_slack_and_only_the_critical_tasks_when_asked() {
    let ws = plan();
    let s = ws.json(&["schedule", "--project", "GK"]);
    let slack: Vec<i64> = s["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["timing"]["slack_days"].as_i64().unwrap())
        .collect();
    assert_eq!(keys(&s["tasks"]), ["GK-1", "GK-3", "GK-2"]);
    assert_eq!(slack, [2, 8, 0]);
    let critical = ws.json(&["schedule", "--project", "GK", "--critical"]);
    assert_eq!(keys(&critical["tasks"]), ["GK-2"]);
    assert_eq!(critical["critical_path"]["tasks"][0], "GK-2");
    assert_eq!(critical["critical_path"]["finish"], "2027-01-15");
    let found = ws.json(&["task", "list", "--query", "is:critical"]);
    assert_eq!(keys(&found["items"]), ["GK-2"]);
    let found = ws.json(&["task", "list", "--query", "slack<3d"]);
    assert_eq!(keys(&found["items"]), ["GK-1", "GK-2"]);
}

#[test]
fn baselines_are_saved_listed_and_compared_with() {
    let ws = plan();
    ws.ok(&["-q", "baseline", "create", "GK", "--name", "Kick-off"]);
    ws.ok(&["-q", "task", "edit", "GK-2", "--due-date", "2027-01-19"]);
    let listed = ws.json(&["baseline", "list", "GK"]);
    assert_eq!(listed["items"][0]["name"], "Kick-off");
    assert_eq!(listed["items"][0]["tasks"], 3);
    let s = ws.json(&["schedule", "--project", "GK", "--baseline", "Kick-off"]);
    let build = s["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "GK-2")
        .unwrap();
    assert_eq!(build["baseline"]["slip_days"], 4);
    assert_eq!(build["baseline"]["due_date"], "2027-01-15");
    let found = ws.json(&["task", "list", "--query", "slip>3d"]);
    assert_eq!(keys(&found["items"]), ["GK-2"]);
    ws.ok(&[
        "-q",
        "baseline",
        "rename",
        "GK",
        "--baseline",
        "Kick-off",
        "--name",
        "First plan",
    ]);
    ws.ok(&["-q", "baseline", "delete", "GK", "--baseline", "First plan"]);
    let listed = ws.json(&["baseline", "list", "GK"]);
    assert_eq!(listed["total"], 0);
    let err = ws.fails(
        2,
        &["schedule", "--project", "GK", "--baseline", "Kick-off"],
    );
    assert!(err.contains("Kick-off"), "{err}");
}
