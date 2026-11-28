// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The digest from the command line: changes made through it are listed
//! as the command line's, and dismissing moves the person's marker.

#![allow(clippy::unwrap_used)]

mod common;

use common::Workspace;

#[test]
fn the_digest_lists_command_line_changes_and_dismissing_moves_the_marker() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "From a script",
    ]);
    let digest = ws.json(&["digest", "show", "--since", "today"]);
    assert_eq!(digest["since_from"], "given");
    assert_eq!(digest["cli"]["tasks"][0]["key"], "GK-1", "{digest}");
    assert_eq!(digest["counts"]["items"], 1);

    let dismissed = ws.json(&["digest", "dismiss"]);
    let marker = dismissed["marker"].as_str().unwrap().to_owned();
    let after = ws.json(&["digest", "show"]);
    assert_eq!(after["since_from"], "marker");
    assert_eq!(after["since"], marker.as_str());
    assert_eq!(after["counts"]["items"], 0, "{after}");
}
