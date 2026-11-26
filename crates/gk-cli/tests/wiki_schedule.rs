// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The schedule, wiki links, page moves, exports and the trash, task links
//! and the time report from the command line, each command its own
//! process against a temporary workspace.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Workspace, dates, keys, problems, titles};
use serde_json::{Value, json};

/// A project GK with a milestone parent and two work packages, the first
/// blocking the second.
fn roadmap() -> Workspace {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    for args in [
        &[
            "--title",
            "Spring release",
            "--start-date",
            "2027-01-04",
            "--due-date",
            "2027-01-29",
        ][..],
        &[
            "--title",
            "Engine",
            "--parent",
            "GK-1",
            "--start-date",
            "2027-01-04",
            "--due-date",
            "2027-01-08",
        ],
        &[
            "--title",
            "Screens",
            "--parent",
            "GK-1",
            "--start-date",
            "2027-01-11",
            "--due-date",
            "2027-01-15",
        ],
        &[
            "--title",
            "Release",
            "--type",
            "Milestone",
            "--due-date",
            "2027-02-01",
        ],
        &["--title", "Someday"],
    ] {
        let mut all = vec!["-q", "task", "create", "--project", "GK"];
        all.extend_from_slice(args);
        ws.ok(&all);
    }
    ws.ok(&[
        "-q", "task", "link", "GK-2", "--kind", "blocks", "--other", "GK-3",
    ]);
    ws
}

#[test]
fn the_schedule_shows_bars_summaries_milestones_and_problems() {
    let ws = roadmap();
    let s = ws.json(&["schedule", "--project", "GK"]);
    assert_eq!(keys(&s["tasks"]), ["GK-1", "GK-2", "GK-3", "GK-4"], "{s}");
    let shapes: Vec<&str> = s["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["shape"].as_str().unwrap())
        .collect();
    assert_eq!(shapes, ["summary", "bar", "bar", "milestone"]);
    assert_eq!(
        s["tasks"][0]["summary"],
        json!({ "start": "2027-01-04", "end": "2027-01-29" })
    );
    assert_eq!(keys(&s["unscheduled"]), ["GK-5"]);
    assert_eq!(s["dependencies"][0]["from"], "GK-2");
    assert_eq!(s["dependencies"][0]["violated"], false);
    // A task without any date is listed apart and is no problem.
    assert!(problems(&s).is_empty(), "{s}");

    // A blocker that now ends later is a problem, named for people too.
    ws.ok(&["-q", "task", "edit", "GK-2", "--due-date", "2027-01-12"]);
    let s = ws.json(&["schedule", "--project", "GK", "--query", "is:open has:due"]);
    assert_eq!(
        problems(&s),
        [("starts_before_blocker_ends".to_owned(), "GK-3".to_owned())]
    );
    assert_eq!(s["dependencies"][0]["violated"], true);
    let text = ws.ok(&["schedule", "--project", "GK"]);
    assert!(
        text.contains("GK-3 starts on 2027-01-11, before GK-2 ends on 2027-01-12."),
        "{text}"
    );

    // A window narrows the rows.
    let window = ws.json(&[
        "schedule",
        "--project",
        "GK",
        "--from",
        "2027-01-20",
        "--to",
        "2027-02-05",
    ]);
    assert_eq!(keys(&window["tasks"]), ["GK-1", "GK-4"], "{window}");

    // Neither a project nor a query: a usage error naming the field.
    let err = ws.fails(1, &["schedule"]);
    assert!(err.contains("project"), "{err}");
}

#[test]
fn a_date_change_can_move_the_dependents_and_be_reverted() {
    let ws = roadmap();
    let changed = ws.json(&[
        "task",
        "edit",
        "GK-2",
        "--due-date",
        "2027-01-13",
        "--move-dependents",
    ]);
    assert_eq!(changed["moved"][0]["key"], "GK-3", "{changed}");
    assert_eq!(changed["moved"][0]["days"], 3);
    let s = ws.json(&["schedule", "--project", "GK", "--query", "has:due"]);
    assert!(problems(&s).is_empty(), "{s}");
    assert_eq!(
        dates(&s, "GK-3"),
        (json!("2027-01-14"), json!("2027-01-18"))
    );

    let group = changed["group"].as_str().unwrap();
    ws.ok(&["-q", "revert", group]);
    let s = ws.json(&["schedule", "--project", "GK"]);
    assert_eq!(
        dates(&s, "GK-2"),
        (json!("2027-01-04"), json!("2027-01-08"))
    );
    assert_eq!(
        dates(&s, "GK-3"),
        (json!("2027-01-11"), json!("2027-01-15"))
    );
}

#[test]
fn tasks_link_and_list_their_backlinks_and_references() {
    let ws = roadmap();
    let linked = ws.json(&[
        "task", "link", "GK-3", "--kind", "relates", "--other", "GK-4",
    ]);
    let links: Vec<(Value, Value)> = linked["task"]["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["kind"].clone(), l["task"]["key"].clone()))
        .collect();
    assert_eq!(
        links,
        [
            (json!("blocked_by"), json!("GK-2")),
            (json!("relates"), json!("GK-4"))
        ],
        "{linked}"
    );
    // A link that would close a loop is refused.
    let err = ws.fails(
        1,
        &[
            "task", "link", "GK-3", "--kind", "blocks", "--other", "GK-2",
        ],
    );
    assert!(err.contains("loop"), "{err}");

    ws.ok(&[
        "-q",
        "wiki",
        "new",
        "--project",
        "GK",
        "--title",
        "Plan",
        "--body",
        "Engine first: GK-2, then [[Notes]].\n",
    ]);
    ws.ok(&[
        "-q",
        "task",
        "edit",
        "GK-3",
        "--description",
        "Needs GK-2 and [[Plan]], not GK-99.",
    ]);
    let back = ws.json(&["task", "backlinks", "gk-2"]);
    assert_eq!(titles(&back["items"]), ["Plan", "Screens"], "{back}");
    assert_eq!(back["items"][1]["task"], "GK-3");
    let refs = ws.json(&["task", "references", "GK-3"]);
    let found: Vec<(Value, Value, Value)> = refs["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["kind"].clone(), r["target"].clone(), r["found"].clone()))
        .collect();
    assert_eq!(
        found,
        [
            (json!("page"), json!("Plan"), json!(true)),
            (json!("task"), json!("GK-2"), json!(true)),
            (json!("task"), json!("GK-99"), json!(false)),
        ],
        "{refs}"
    );
    let text = ws.ok(&["task", "references", "GK-3"]);
    assert!(text.contains("GK-99"), "{text}");
}

#[test]
fn wiki_pages_move_link_export_and_go_to_the_trash() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    let new = |title: &str, body: &str, parent: Option<&str>| {
        let body = format!("--body={body}");
        let mut args = vec![
            "-q",
            "wiki",
            "new",
            "--project",
            "GK",
            "--title",
            title,
            &body,
        ];
        if let Some(p) = parent {
            args.extend_from_slice(&["--parent", p]);
        }
        ws.ok(&args);
    };
    new("Guide", "Start with [[Setup#Steps]] and [[Later]].\n", None);
    new("Reference", "", None);
    new(
        "Setup",
        "## Steps\n\n- [ ] install\n- [ ] run\n",
        Some("Guide"),
    );
    let tree = |ws: &Workspace| -> Vec<String> {
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
    };
    assert_eq!(tree(&ws), ["Guide", "  Setup", "Reference"]);

    ws.ok(&[
        "-q",
        "wiki",
        "move",
        "Setup",
        "--project",
        "GK",
        "--parent",
        "Reference",
    ]);
    assert_eq!(tree(&ws), ["Guide", "Reference", "  Setup"]);
    ws.ok(&[
        "-q",
        "wiki",
        "move",
        "setup",
        "--project",
        "GK",
        "--top-level",
        "true",
        "--at",
        "top",
    ]);
    assert_eq!(tree(&ws), ["Setup", "Guide", "Reference"]);
    ws.ok(&[
        "-q",
        "wiki",
        "move",
        "Reference",
        "--project",
        "GK",
        "--before",
        "Guide",
    ]);
    assert_eq!(tree(&ws), ["Setup", "Reference", "Guide"]);
    ws.ok(&[
        "-q",
        "wiki",
        "move",
        "Guide",
        "--project",
        "GK",
        "--parent",
        "Reference",
    ]);
    let err = ws.fails(
        1,
        &[
            "wiki",
            "move",
            "Reference",
            "--project",
            "GK",
            "--parent",
            "Guide",
        ],
    );
    assert!(!err.is_empty());

    // Links keep finding the moved page.
    let back = ws.json(&["wiki", "backlinks", "Setup", "--project", "GK"]);
    assert_eq!(titles(&back["items"]), ["Guide"], "{back}");
    assert_eq!(back["items"][0]["heading"], "Steps");
    let refs = ws.json(&["wiki", "references", "Guide", "--project", "GK"]);
    let found: Vec<(Value, Value)> = refs["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["target"].clone(), r["found"].clone()))
        .collect();
    assert_eq!(
        found,
        [
            (json!("Setup"), json!(true)),
            (json!("Later"), json!(false))
        ]
    );

    // A checkbox on a page is ticked by number and text.
    let toggle = json!([{ "number": 2, "text": "run", "checked": true }]).to_string();
    let ticked = ws.json(&[
        "task",
        "check",
        "--page",
        "Setup",
        "--project",
        "GK",
        "--toggle",
        &toggle,
    ]);
    assert_eq!(
        ticked["checklist"],
        json!({ "done": 1, "total": 2 }),
        "{ticked}"
    );
    let page = ws.json(&["wiki", "get", "Setup", "--project", "GK"]);
    assert!(
        page["body"].as_str().unwrap().contains("- [x] run"),
        "{page}"
    );
    let stale = json!([{ "number": 1, "text": "something else", "checked": true }]).to_string();
    ws.fails(
        3,
        &[
            "task",
            "check",
            "--page",
            "Setup",
            "--project",
            "GK",
            "--toggle",
            &stale,
        ],
    );

    // Exports: a folder with the subpages, and a zip of the whole wiki.
    let folder = ws.home().join("reference");
    let folder_arg = folder.to_string_lossy().into_owned();
    let out = ws.json(&[
        "wiki",
        "export",
        "Reference",
        "--project",
        "GK",
        "--subpages",
        "true",
        "--format",
        "folder",
        "--path",
        &folder_arg,
    ]);
    assert_eq!(out["pages"], 2, "{out}");
    let mut files: Vec<String> = walk(&folder);
    files.sort();
    assert_eq!(files.len(), 2, "{files:?}");
    let guide = files
        .iter()
        .find(|f| f.ends_with("guide.md"))
        .unwrap_or_else(|| panic!("{files:?}"));
    let text = std::fs::read_to_string(folder.join(guide)).unwrap();
    // Setup isn't in the export, so its link stays a wiki link.
    assert!(text.contains("Setup"), "{text}");
    let zip = ws.home().join("wiki.zip");
    let zip_arg = zip.to_string_lossy().into_owned();
    let out = ws.json(&["wiki", "export", "--project", "GK", "--path", &zip_arg]);
    assert_eq!(
        (&out["format"], &out["pages"]),
        (&json!("zip"), &json!(3)),
        "{out}"
    );
    assert!(zip.is_file());
    // An existing target is never overwritten (a storage error, as for
    // project exports).
    ws.fails(
        4,
        &["wiki", "export", "--project", "GK", "--path", &zip_arg],
    );

    // Deleting takes the pages inside along; links stop resolving until
    // the page comes back.
    let deleted = ws.json(&["wiki", "delete", "Reference", "--project", "GK"]);
    assert_eq!(deleted["items"][0]["contains"], 1, "{deleted}");
    assert_eq!(tree(&ws), ["Setup"]);
    ws.fails(2, &["wiki", "get", "Guide", "--project", "GK"]);
    let back = ws.json(&["wiki", "backlinks", "Setup", "--project", "GK"]);
    assert_eq!(back["total"], 0, "{back}");
    let id = deleted["items"][0]["id"].as_str().unwrap();
    ws.ok(&["-q", "trash", "restore", id]);
    assert_eq!(tree(&ws), ["Setup", "Reference", "  Guide"]);
    let back = ws.json(&["wiki", "backlinks", "Setup", "--project", "GK"]);
    assert_eq!(back["total"], 1, "{back}");
}

/// The files under `dir`, relative to it, with `/` between parts.
fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap();
                out.push(
                    rel.components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            }
        }
    }
    out
}

#[test]
fn the_time_report_groups_by_person_and_month_as_csv() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Glyphkeel", "--key", "GK"]);
    ws.ok(&[
        "-q",
        "task",
        "create",
        "--project",
        "GK",
        "--title",
        "Notes",
    ]);
    for (day, duration) in [
        ("2026-10-30", "1h"),
        ("2026-11-02", "30m"),
        ("2026-11-03", "2h"),
    ] {
        ws.ok(&[
            "-q",
            "time",
            "log",
            "GK-1",
            "--duration",
            duration,
            "--date",
            day,
        ]);
    }
    let by_month = ws.ok(&[
        "time",
        "report",
        "--from",
        "2026-10-01",
        "--to",
        "2026-11-30",
        "--group-by",
        "month",
        "--csv",
        "groups",
    ]);
    let lines: Vec<&str> = by_month.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "{by_month}");
    assert!(lines[0].starts_with("month,"), "{by_month}");
    assert!(
        lines[1].starts_with("2026-10,") && lines[1].contains(",60,"),
        "{by_month}"
    );
    assert!(
        lines[2].starts_with("2026-11,") && lines[2].contains(",150,"),
        "{by_month}"
    );

    let by_person = ws.json(&[
        "time",
        "report",
        "--from",
        "2026-10-01",
        "--to",
        "2026-11-30",
        "--group-by",
        "person",
        "--person",
        "me",
    ]);
    assert_eq!(
        by_person["items"].as_array().unwrap().len(),
        1,
        "{by_person}"
    );
    assert_eq!(by_person["total_duration"], "3h 30m");
    let none = ws.json(&[
        "time",
        "report",
        "--from",
        "2026-10-01",
        "--to",
        "2026-11-30",
        "--person",
        "agent",
    ]);
    assert_eq!(none["items"], json!([]), "{none}");

    let entries = ws.ok(&[
        "time",
        "report",
        "--from",
        "2026-11-01",
        "--to",
        "2026-11-30",
        "--csv",
        "entries",
    ]);
    let rows: Vec<&str> = entries.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(rows.len(), 3, "a header and two entries: {entries}");
    assert!(
        rows[1].contains("2026-11-02") && rows[2].contains("2026-11-03"),
        "{entries}"
    );
}
