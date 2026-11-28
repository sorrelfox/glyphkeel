// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The board as `glyphkeel board` prints it for people: columns with their
//! cards, each card's coloured edge and cover named; `--json` unchanged.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;

/// A 4 × 3 PNG image.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAADCAYAAAC09K7GAAAAEklEQVR4nGPgEpH7j4wZCAoAAGdSDsWfzwrRAAAAAElFTkSuQmCC";

#[test]
fn the_board_lists_columns_and_cards_with_their_edge_and_cover() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&["-q", "label", "add", "--project", "GK", "--name", "bug"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Crash on start",
        "--labels",
        "bug",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Write notes",
    ]);
    let attached = ws.json(&[
        "file",
        "attach",
        "--task",
        "GK-2",
        "--name",
        "shot.png",
        "--data-base64",
        PNG,
    ]);
    let id = attached["file"]["id"].as_str().unwrap().to_owned();
    ws.ok(&["-q", "file", "cover", "GK-2", "--attachment", &id]);
    ws.ok(&[
        "-q",
        "board",
        "layout",
        "set",
        "GK",
        "--colour-by",
        "query",
        "--colour-rules",
        r#"[{"name": "Bugs", "query": "label:bug", "hue": "red"}]"#,
        "--covers",
        "chosen",
    ]);

    let text = ws.ok(&["board", "GK"]);
    assert!(text.starts_with("Board of GK\n"), "{text}");
    assert!(text.contains("\nBacklog (2 tasks)\n"), "{text}");
    assert!(
        text.contains("  GK-1  Crash on start  edge: Bugs\n"),
        "{text}"
    );
    assert!(
        text.contains("  GK-2  Write notes  cover: shot.png\n"),
        "{text}"
    );
    assert!(text.contains("\nDone (0 tasks)\n  no cards\n"), "{text}");

    // The JSON is the board itself.
    let json = ws.json(&["board", "GK"]);
    assert_eq!(json["columns"][0]["cards"][0]["edge"]["name"], "Bugs");
    assert_eq!(json["columns"][0]["cards"][1]["cover"]["name"], "shot.png");
}

#[test]
fn the_board_shows_swimlanes_by_priority_and_keeps_them_in_the_layout() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    for (title, priority) in [("Crash on start", "urgent"), ("Write notes", "low")] {
        ws.ok(&[
            "-q",
            "task",
            "create",
            "--project",
            "GK",
            "--title",
            title,
            "--priority",
            priority,
        ]);
    }
    ws.ok(&["-q", "task", "create", "--project", "GK", "--title", "Tidy"]);

    let text = ws.ok(&["board", "GK", "--lanes", "priority"]);
    assert!(text.contains("\nColumns\n  Backlog (3 tasks)\n"), "{text}");
    assert!(
        text.contains("\nLane: Urgent (1 task)\n  Backlog (1)\n    GK-1  Crash on start\n"),
        "{text}"
    );
    assert!(
        text.contains("\nLane: High (0 tasks)\n  no cards\n"),
        "{text}"
    );
    assert!(
        text.contains("\nLane: No priority (1 task)\n  Backlog (1)\n    GK-3  Tidy\n"),
        "{text}"
    );
    // The lane without a value comes last.
    assert!(text.trim_end().ends_with("GK-3  Tidy"), "{text}");

    // The JSON carries the lanes, and each card its lane.
    let json = ws.json(&["board", "GK", "--lanes", "priority"]);
    assert_eq!(json["lanes"][0]["id"], "urgent");
    assert_eq!(
        json["lanes"][0]["counts"].as_object().unwrap().len(),
        json["columns"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["hidden"] == false)
            .count()
    );
    assert_eq!(json["columns"][0]["cards"][0]["lane"], "urgent");

    // Kept in the layout; the plain board then has them.
    ws.ok(&["-q", "board", "layout", "set", "GK", "--lanes", "priority"]);
    let text = ws.ok(&["board", "GK"]);
    assert!(
        text.contains("\nLane: Low (1 task)\n  Backlog (1)\n"),
        "{text}"
    );
    let json = ws.json(&["board", "GK", "--lanes", "none"]);
    assert!(json.get("lanes").is_none());
}
