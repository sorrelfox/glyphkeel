// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Velocity, burn-up and estimate accuracy from the command line, and the
//! dashboard widgets that show them, against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;

#[test]
fn planning_reports_and_their_widgets_agree() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "-q",
        "version",
        "create",
        "--project",
        "GK",
        "--name",
        "1.0",
    ]);
    for title in ["Engine", "Screen"] {
        ws.ok(&[
            "-q",
            "task",
            "create",
            "--project",
            "GK",
            "--title",
            title,
            "--estimate",
            "1h",
            "--fix-version",
            "1.0",
        ]);
    }
    ws.ok(&[
        "-q",
        "sprint",
        "create",
        "--project",
        "GK",
        "--name",
        "S1",
        "--tasks",
        "GK-1",
        "--tasks",
        "GK-2",
    ]);
    ws.ok(&["-q", "sprint", "start", "S1", "--project", "GK"]);
    ws.ok(&["-q", "time", "log", "GK-1", "--duration", "90m"]);
    ws.ok(&["-q", "task", "move", "GK-1", "--status", "Done"]);
    ws.ok(&["-q", "sprint", "complete", "active", "--project", "GK"]);

    let velocity = ws.json(&["report", "velocity", "GK"]);
    assert_eq!(velocity["sprints"][0]["committed"], 2.0);
    assert_eq!(velocity["sprints"][0]["completed"], 1.0);
    assert_eq!(velocity["average"], 1.0);
    let burnup = ws.json(&["report", "burnup", "--version", "1.0", "--project", "GK"]);
    assert_eq!(burnup["days"][0]["scope"], 2.0);
    assert_eq!(burnup["days"][0]["completed"], 1.0);
    let estimates = ws.json(&[
        "report",
        "estimates",
        "--project",
        "GK",
        "--group-by",
        "type",
    ]);
    assert_eq!(estimates["total"]["median_ratio"], 1.5);

    ws.ok(&[
        "-q",
        "dashboard",
        "create",
        "--name",
        "Plan",
        "--project",
        "GK",
        "--widgets",
        r#"[{"kind": "velocity"}, {"kind": "burnup", "settings": {"version": "1.0"}}, {"kind": "estimate_accuracy"}]"#,
    ]);
    let shown = ws.json(&["dashboard", "show", "Plan", "--project", "GK"]);
    assert_eq!(shown["widgets"][0]["velocity"], velocity);
    assert_eq!(shown["widgets"][1]["burnup"], burnup);
    assert_eq!(
        shown["widgets"][2]["estimates"]["total"],
        estimates["total"]
    );

    let text = ws.ok(&["dashboard", "show", "Plan", "--project", "GK"]);
    assert!(
        text.contains("1. Sprint velocity\n  S1: 2 committed, 1 completed"),
        "{text}"
    );
    assert!(
        text.contains("2. Burn-up\n  1.0: 1 of 2 completed"),
        "{text}"
    );
    assert!(
        text.contains("3. Estimate accuracy\n  1 task: median 1.5× the estimate, 0% within 20%"),
        "{text}"
    );
}
