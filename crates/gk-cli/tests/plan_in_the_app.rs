// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The project's own plan kept in the app, end to end: the plan's
//! Markdown files become wiki pages, the roadmap becomes tasks with
//! dependencies, and an agent (`glyphkeel mcp` as its own process) keeps
//! the wiki and reschedules the roadmap while the person checks the result
//! from the command line and reverts the agent's work.

#![allow(clippy::unwrap_used)]

mod common;

use std::path::Path;

use common::{Agent, Workspace, dates, problems, titles};
use serde_json::{Value, json};

/// The plan as a folder of Markdown files: a file per page, a folder per
/// page with pages inside it (its own text in `README.md`), a relative link
/// between two files and an image one of them shows.
const PLAN: &[(&str, &str)] = &[
    (
        "Plan.md",
        "# Plan\n\nRead [[Engine]] first, then [[Screens]].\n\n## Working rules\n\n- [ ] small commits\n- [ ] tests first\n",
    ),
    (
        "Engine/README.md",
        "# Engine\n\nThe data lives in [[Data model]]; the order of work is in [[Plan#Working rules]].\n",
    ),
    (
        "Engine/Data model.md",
        "# Data model\n\n## Tasks\n\nTasks have keys such as GK-1.\n",
    ),
    (
        "Screens.md",
        "# Screens\n\nThe Gantt chart shows [[Engine]] work as bars, with [its tasks](Engine/Data%20model.md#tasks).\n\n![The board](shots/board.png)\n",
    ),
];

/// A one-pixel PNG image.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn write_plan(root: &Path) {
    for (path, text) in PLAN {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    std::fs::create_dir_all(root.join("shots")).unwrap();
    std::fs::write(root.join("shots/board.png"), PNG).unwrap();
}

fn tree(ws: &Workspace) -> Vec<String> {
    ws.json(&["wiki", "list", "--project", "GK"])["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            format!(
                "{}{}",
                "  ".repeat(usize::try_from(p["depth"].as_u64().unwrap()).unwrap()),
                p["title"].as_str().unwrap()
            )
        })
        .collect()
}

#[test]
fn the_plan_lives_in_the_wiki_and_an_agent_keeps_it_and_the_roadmap() {
    let ws = Workspace::new();
    let source = ws.home().join("plan");
    write_plan(&source);
    ws.ok(&[
        "-q",
        "project",
        "create",
        "--name",
        "Glyphkeel",
        "--key",
        "GK",
    ]);

    // 1. The plan's folder becomes the wiki in one import: a file per page,
    // the folder a page with its files inside it and its README as its text,
    // the image attached to the page that shows it.
    let source_arg = source.to_string_lossy().into_owned();
    let imported = ws.json(&["wiki", "import", &source_arg, "--project", "GK"]);
    assert_eq!(imported["pages"], 4, "{imported}");
    assert_eq!(imported["attachments"], 1, "{imported}");
    assert!(imported.get("renamed").is_none(), "{imported}");
    assert_eq!(tree(&ws), ["Engine", "  Data model", "Plan", "Screens"]);
    for page in ["Plan", "Engine", "Data model", "Screens"] {
        let refs = ws.json(&["wiki", "references", page, "--project", "GK"]);
        let missing: Vec<&Value> = refs["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "page" && r["found"] != true)
            .collect();
        assert!(missing.is_empty(), "{page}: {refs}");
    }
    // The relative link is a wiki link now, and the image is the page's
    // attachment, readable from the command line.
    let screens = ws.json(&["wiki", "get", "Screens", "--project", "GK"]);
    let body = screens["body"].as_str().unwrap();
    assert!(body.contains("[[Data model#Tasks|its tasks]]"), "{body}");
    let file = body
        .split("](gk-file://")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .unwrap_or_else(|| panic!("{body}"));
    let shown = ws.json(&["file", "show", file]);
    assert_eq!(shown["name"], "board.png", "{shown}");
    assert_eq!(shown["page"], screens["id"], "{shown}");
    assert_eq!(shown["kind"], "image", "{shown}");
    let found = ws.json(&["wiki", "search", "data model#tasks"]);
    assert_eq!(found["items"][0]["heading"]["text"], "Tasks", "{found}");
    // An export gives back a file per page with the same text.
    let copy = ws.home().join("copy");
    let copy_arg = copy.to_string_lossy().into_owned();
    let out = ws.json(&[
        "wiki",
        "export",
        "--project",
        "GK",
        "--format",
        "folder",
        "--path",
        &copy_arg,
    ]);
    assert_eq!(out["pages"], 4, "{out}");
    let mut exported = Vec::new();
    collect_md(&copy, &mut exported);
    assert_eq!(exported.len(), 4, "{exported:?}");
    for (_, text) in PLAN {
        let heading = text.lines().next().unwrap();
        assert!(
            exported.iter().any(|t| t.lines().any(|l| l == heading)),
            "{heading} in the export: {exported:?}"
        );
    }

    // 2. The roadmap: a milestone parent with work packages in order.
    for args in [
        &[
            "--title",
            "Wiki and Gantt",
            "--start-date",
            "2027-01-04",
            "--due-date",
            "2027-01-29",
        ][..],
        &[
            "--title",
            "Engine work",
            "--parent",
            "GK-1",
            "--start-date",
            "2027-01-04",
            "--due-date",
            "2027-01-08",
        ],
        &[
            "--title",
            "Screens work",
            "--parent",
            "GK-1",
            "--start-date",
            "2027-01-11",
            "--due-date",
            "2027-01-15",
        ],
        &[
            "--title",
            "Tests",
            "--parent",
            "GK-1",
            "--start-date",
            "2027-01-18",
            "--due-date",
            "2027-01-22",
        ],
        &[
            "--title",
            "Wiki and Gantt done",
            "--type",
            "Milestone",
            "--due-date",
            "2027-01-29",
        ],
    ] {
        let mut all = vec!["-q", "task", "create", "--project", "GK"];
        all.extend_from_slice(args);
        ws.ok(&all);
    }
    for (from, to) in [("GK-2", "GK-3"), ("GK-3", "GK-4"), ("GK-4", "GK-5")] {
        ws.ok(&[
            "-q", "task", "link", from, "--kind", "blocks", "--other", to,
        ]);
    }
    let s = ws.json(&["schedule", "--project", "GK"]);
    let shapes: Vec<(String, String)> = s["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["key"].as_str().unwrap().to_owned(),
                t["shape"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        shapes,
        [
            ("GK-1".to_owned(), "summary".to_owned()),
            ("GK-2".to_owned(), "bar".to_owned()),
            ("GK-3".to_owned(), "bar".to_owned()),
            ("GK-4".to_owned(), "bar".to_owned()),
            ("GK-5".to_owned(), "milestone".to_owned()),
        ],
        "{s}"
    );
    assert_eq!(s["dependencies"].as_array().unwrap().len(), 3);
    assert!(problems(&s).is_empty(), "{s}");

    // 3. The agent keeps the wiki. The person marks the brief first.
    ws.ok(&[
        "-q",
        "wiki",
        "put",
        "Plan",
        "--project",
        "GK",
        "--agent-brief",
        "true",
    ]);
    let mut agent = Agent::start(ws.home(), &ws.path(), "Plan Keeper");
    let brief = agent.ok("get_agent_brief", json!({ "project": "GK" }));
    assert_eq!(brief["source"], "wiki_page", "{brief}");
    assert!(brief["body"].as_str().unwrap().contains("Working rules"));

    let engine_page = agent.ok(
        "get_wiki_page",
        json!({ "project": "GK", "page": "Engine", "include": ["backlinks"] }),
    );
    let mut linking = titles(&engine_page["backlinks"]);
    linking.sort();
    assert_eq!(linking, ["Plan", "Screens"], "{engine_page}");
    let base = engine_page["revision"].as_str().unwrap().to_owned();
    let saved = agent.ok(
        "update_wiki_page",
        json!({ "project": "GK", "page": "Engine", "base_revision": base,
                "body": "# Engine\n\nThe data lives in [[Data model]]; the order of work is in [[Plan#Working rules]].\n\nUpdated by the agent.\n" }),
    );
    assert_eq!(saved["save"]["outcome"], "saved", "{saved}");
    let base = saved["save"]["revision"].as_str().unwrap().to_owned();
    // The person edits the first line from the command line meanwhile.
    ws.ok(&[
        "-q",
        "wiki",
        "put",
        "Engine",
        "--project",
        "GK",
        "--edits",
        &json!([{ "old_text": "# Engine", "new_text": "# The engine" }]).to_string(),
    ]);
    // A save from the agent's now old base merges with the person's change.
    let stale = agent.ok(
        "update_wiki_page",
        json!({ "project": "GK", "page": "Engine", "base_revision": base,
                "body": "# Engine\n\nThe data lives in [[Data model]]; the order of work is in [[Plan#Working rules]].\n\nUpdated by the agent, twice.\n" }),
    );
    assert_eq!(stale["save"]["outcome"], "merged", "{stale}");
    let merged = ws.json(&["wiki", "get", "Engine", "--project", "GK"]);
    let body = merged["body"].as_str().unwrap();
    assert!(
        body.starts_with("# The engine") && body.contains("twice"),
        "{body}"
    );

    // A move to another parent, and a tick in the brief's checklist.
    agent.ok(
        "update_wiki_page",
        json!({ "project": "GK", "page": "Screens", "parent": "Plan" }),
    );
    assert_eq!(tree(&ws), ["Engine", "  Data model", "Plan", "  Screens"]);
    let ticked = agent.ok(
        "update_checklist",
        json!({ "project": "GK", "page": "Plan",
                "toggle": [{ "number": 2, "text": "tests first", "checked": true }] }),
    );
    assert_eq!(
        ticked["checklist"],
        json!({ "done": 1, "total": 2 }),
        "{ticked}"
    );
    let history = agent.ok(
        "get_wiki_history",
        json!({ "project": "GK", "page": "Engine" }),
    );
    assert!(
        history.to_string().contains("Plan Keeper"),
        "the history names the agent: {history}"
    );
    // Deleting a page goes to the trash, within the agent's limits.
    agent.ok(
        "create_wiki_page",
        json!({ "project": "GK", "title": "Scratch", "body": "Draft." }),
    );
    let deleted = agent.ok(
        "delete_wiki_page",
        json!({ "project": "GK", "page": "Scratch" }),
    );
    assert_eq!(deleted["items"][0]["title"], "Scratch", "{deleted}");
    let trash = ws.json(&["trash", "list"]);
    assert_eq!(
        trash["items"][0]["deleted_by"]["name"], "Plan Keeper",
        "{trash}"
    );

    // 4. The person moves a date by hand; the agent sees the problem and
    // fixes it by moving the dependents.
    ws.ok(&["-q", "task", "edit", "GK-2", "--due-date", "2027-01-12"]);
    let s = agent.ok("get_schedule", json!({ "project": "GK" }));
    assert_eq!(
        problems(&s),
        [("starts_before_blocker_ends".to_owned(), "GK-3".to_owned())]
    );
    let changed = agent.ok(
        "update_task",
        json!({ "key": "GK-2", "due_date": "2027-01-14", "start_date": "2027-01-05",
                "move_dependents": true }),
    );
    let moved: Vec<&str> = changed["moved"]
        .as_array()
        .unwrap_or_else(|| panic!("{changed}"))
        .iter()
        .map(|m| m["key"].as_str().unwrap())
        .collect();
    // Each as far as it must; the milestone has room and stays.
    assert_eq!(moved, ["GK-3", "GK-4"], "{changed}");
    let s = agent.ok("get_schedule", json!({ "project": "GK" }));
    assert!(problems(&s).is_empty(), "{s}");
    assert_eq!(
        dates(&s, "GK-3"),
        (json!("2027-01-15"), json!("2027-01-19"))
    );
    assert_eq!(
        dates(&s, "GK-4"),
        (json!("2027-01-20"), json!("2027-01-24"))
    );
    assert_eq!(dates(&s, "GK-5"), (Value::Null, json!("2027-01-29")));
    // The person sees the same from the command line, without restarting.
    let seen = ws.json(&["schedule", "--project", "GK"]);
    assert_eq!(dates(&seen, "GK-4"), dates(&s, "GK-4"));
    let group = changed["group"].as_str().unwrap().to_owned();
    let one = ws.json(&["activity", "--group", &group]);
    let keys_changed: Vec<&str> = one["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["task"]["key"].as_str())
        .collect();
    for key in ["GK-2", "GK-3", "GK-4"] {
        assert!(keys_changed.contains(&key), "{key} in one change: {one}");
    }
    agent.finish();

    // 5. The person reverts everything the agent's session did.
    let sessions = ws.json(&["agents", "sessions"]);
    let session = sessions["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Plan Keeper")
        .unwrap_or_else(|| panic!("{sessions}"))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let reverted = ws.json(&["revert", "--session", &session]);
    assert!(
        !reverted["reverted"].as_array().unwrap().is_empty(),
        "{reverted}"
    );
    let s = ws.json(&["schedule", "--project", "GK"]);
    assert_eq!(
        dates(&s, "GK-2"),
        (json!("2027-01-04"), json!("2027-01-12"))
    );
    assert_eq!(
        dates(&s, "GK-3"),
        (json!("2027-01-11"), json!("2027-01-15"))
    );
    assert_eq!(
        dates(&s, "GK-4"),
        (json!("2027-01-18"), json!("2027-01-22"))
    );
    assert_eq!(tree(&ws), ["Engine", "  Data model", "Plan", "Screens"]);
}

/// The text of every Markdown file under `dir`.
fn collect_md(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_md(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(std::fs::read_to_string(path).unwrap());
        }
    }
}
