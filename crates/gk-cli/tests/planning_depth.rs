// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The command line for the deeper planning tools: status rules and rules
//! for agents, moves to another project (checked first, with a mapping),
//! new project keys, workspaces, kinds of work, lags in the schedule,
//! closing duplicates, WIP minimums, due offsets, history queries, and a CSV
//! import with a saved preset.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Workspace, code, dates, glyphkeel, keys, problems, stderr, stdout};
use serde_json::{Value, json};

fn named<'a>(list: &'a Value, name: &str) -> &'a Value {
    list.as_array()
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("{name} in {list}"))
}

fn sorted_keys(list: &Value) -> Vec<String> {
    let mut k = keys(&list["items"]);
    k.sort();
    k
}

#[test]
fn status_rules_and_rules_for_agents_are_set_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Team", "--key", "TM"]);
    let changed = ws.json(&[
        "project",
        "rules",
        "TM",
        "--statuses",
        r#"[{"status": "Done", "rules": {"required": ["estimate"], "agents_may_not_enter": true}}]"#,
        "--agents",
        r#"{"read_only": ["priority"]}"#,
    ]);
    let done = named(&changed["project"]["statuses"], "Done");
    assert_eq!(
        done["rules"],
        json!({ "required": ["estimate"], "agents_may_not_enter": true })
    );
    assert_eq!(
        changed["project"]["agent_rules"],
        json!({ "read_only": ["priority"] })
    );
    let shown = ws.json(&["project", "show", "TM"]);
    assert_eq!(named(&shown["statuses"], "Done")["rules"], done["rules"]);

    ws.ok(&["task", "create", "--project", "TM", "--title", "Ship"]);
    let err = ws.fails(1, &["task", "move", "TM-1", "--status", "Done"]);
    assert!(err.contains("estimate"), "{err}");
    ws.ok(&["task", "edit", "TM-1", "--estimate", "2h"]);
    ws.ok(&["task", "move", "TM-1", "--status", "Done"]);

    let err = ws.fails(
        2,
        &[
            "project",
            "rules",
            "TM",
            "--statuses",
            r#"[{"status": "Nope", "rules": {}}]"#,
        ],
    );
    assert!(err.contains("Nope"), "{err}");
    // Empty rules remove them.
    let cleared = ws.json(&[
        "project",
        "rules",
        "TM",
        "--statuses",
        r#"[{"status": "Done", "rules": {}}]"#,
        "--agents",
        "{}",
    ]);
    assert!(
        named(&cleared["project"]["statuses"], "Done")
            .get("rules")
            .is_none(),
        "{cleared}"
    );
}

#[test]
fn a_move_to_another_project_is_checked_then_made_with_a_mapping() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Team", "--key", "TM"]);
    ws.ok(&["project", "create", "--name", "Other", "--key", "OT"]);
    ws.ok(&[
        "status",
        "add",
        "--project",
        "TM",
        "--name",
        "QA",
        "--category",
        "in_progress",
    ]);
    ws.ok(&["task", "create", "--project", "TM", "--title", "Parent"]);
    ws.ok(&["task", "create", "--project", "TM", "--title", "Child"]);
    ws.ok(&["task", "edit", "TM-2", "--parent", "TM-1"]);
    ws.ok(&["task", "move", "TM-1", "--status", "QA"]);

    let check = ws.json(&["task", "move-check", "TM-1", "--project", "OT"]);
    assert_eq!(
        check["keys"],
        json!([{ "from": "TM-1", "to": "OT-1" }, { "from": "TM-2", "to": "OT-2" }]),
        "{check}"
    );
    assert!(named(&check["statuses"], "QA").get("target").is_none());
    assert_eq!(check["fallback_status"], "Backlog");
    let text = ws.ok(&["task", "move-check", "TM-1", "--project", "OT"]);
    assert!(text.contains("OT-1") && text.contains("QA"), "{text}");
    // The check changed nothing.
    assert_eq!(
        ws.json(&["task", "show", "TM-1"])["items"][0]["key"],
        "TM-1"
    );

    let err = ws.fails(
        2,
        &[
            "task",
            "move",
            "TM-1",
            "--project",
            "OT",
            "--mapping",
            r#"{"statuses": {"QA": "Nope"}}"#,
        ],
    );
    assert!(err.contains("Nope"), "{err}");
    let moved = ws.json(&[
        "task",
        "move",
        "TM-1",
        "--project",
        "OT",
        "--mapping",
        r#"{"statuses": {"QA": "Review"}}"#,
    ]);
    assert_eq!(moved["task"]["key"], "OT-1");
    assert_eq!(moved["task"]["status"]["name"], "Review");
    // The old keys still find both.
    let found = ws.json(&["task", "show", "TM-1", "TM-2"]);
    assert_eq!(keys(&found["items"]), ["OT-1", "OT-2"]);
}

#[test]
fn a_project_gets_a_new_key_and_the_old_one_keeps_working() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Team", "--key", "TM"]);
    ws.ok(&["project", "create", "--name", "Other", "--key", "OT"]);
    ws.ok(&["task", "create", "--project", "TM", "--title", "Kept"]);
    let text = ws.ok(&["project", "rekey", "TM", "--key", "TEAM"]);
    assert!(text.contains("Team"), "{text}");
    assert_eq!(
        ws.json(&["task", "show", "TM-1"])["items"][0]["key"],
        "TEAM-1"
    );
    assert_eq!(ws.json(&["project", "show", "TM"])["key"], "TEAM");
    let created = ws.json(&["task", "create", "--project", "TM", "--title", "New"]);
    assert_eq!(created["task"]["key"], "TEAM-2");
    // Old keys stay reserved; a bad key is refused.
    let err = ws.fails(3, &["project", "rekey", "OT", "--key", "TM"]);
    assert!(err.contains("TM"), "{err}");
    ws.fails(1, &["project", "rekey", "OT", "--key", "bad"]);
}

#[test]
fn workspaces_are_listed_made_renamed_and_opened() {
    let ws = Workspace::new();
    let run = |args: &[&str]| {
        let mut cmd = glyphkeel(ws.home());
        cmd.args(["--tz", "UTC"]).args(args);
        cmd.output().unwrap()
    };
    let ok = |args: &[&str]| {
        let out = run(args);
        assert_eq!(code(&out), 0, "{args:?}: {}", stderr(&out));
        stdout(&out)
    };
    let json = |args: &[&str]| -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&ok(&all)).unwrap()
    };
    let names = |list: &Value| -> Vec<(String, bool)> {
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| {
                (
                    w["name"].as_str().unwrap().to_owned(),
                    w["current"].as_bool().unwrap(),
                )
            })
            .collect()
    };

    assert_eq!(json(&["workspace", "list"])["items"], json!([]));
    assert!(!ok(&["workspace", "list"]).is_empty());
    let made = json(&["workspace", "create", "Work"]);
    assert_eq!(made["name"], "Work");
    assert!(ok(&["workspace", "create", "Side Job"]).contains("Side Job"));
    assert_eq!(
        names(&json(&["workspace", "list"])),
        [("Side Job".to_owned(), false), ("Work".to_owned(), false)]
    );
    let out = run(&["workspace", "create", "Work"]);
    assert_eq!(code(&out), 3, "{}", stderr(&out));

    let renamed = json(&["workspace", "rename", "Work", "Office"]);
    assert_eq!(renamed["name"], "Office");
    let out = run(&["workspace", "rename", "Nope", "Other"]);
    assert_eq!(code(&out), 2, "{}", stderr(&out));

    json(&["workspace", "open", "Office"]);
    assert_eq!(
        names(&json(&["workspace", "list"])),
        [("Office".to_owned(), true), ("Side Job".to_owned(), false)]
    );
    // Commands without `--workspace` now work in it.
    ok(&["project", "create", "--name", "Desk", "--key", "DK"]);
    let projects = json(&["--workspace", "Office", "project", "list"]);
    assert!(projects.to_string().contains("\"DK\""), "{projects}");
    let other = json(&["--workspace", "Side Job", "project", "list"]);
    assert!(!other.to_string().contains("\"DK\""), "{other}");
}

#[test]
fn kinds_of_work_lags_duplicates_minimums_and_offsets_from_the_command_line() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Team", "--key", "TM"]);
    ws.ok(&[
        "task",
        "create",
        "--project",
        "TM",
        "--title",
        "Pour",
        "--start-date",
        "2026-12-01",
        "--due-date",
        "2026-12-02",
    ]);
    ws.ok(&[
        "task",
        "create",
        "--project",
        "TM",
        "--title",
        "Build on it",
        "--start-date",
        "2026-12-03",
        "--due-date",
        "2026-12-04",
    ]);

    // A lag shows in the schedule and makes the plan a problem.
    let link = ws.json(&[
        "task",
        "link",
        "TM-1",
        "--kind",
        "blocks",
        "--other",
        "TM-2",
        "--lag-days",
        "2",
    ]);
    assert_eq!(link["task"]["links"][0]["lag_days"], 2, "{link}");
    let schedule = ws.json(&["schedule", "--project", "TM"]);
    assert_eq!(schedule["dependencies"][0]["lag_days"], 2, "{schedule}");
    assert_eq!(
        problems(&schedule),
        [("starts_before_blocker_ends".to_owned(), "TM-2".to_owned())]
    );
    ws.ok(&[
        "task",
        "edit",
        "TM-1",
        "--due-date",
        "2026-12-03",
        "--move-dependents",
    ]);
    let schedule = ws.json(&["schedule", "--project", "TM"]);
    assert_eq!(
        dates(&schedule, "TM-2"),
        (json!("2026-12-06"), json!("2026-12-07"))
    );
    assert!(problems(&schedule).is_empty(), "{schedule}");
    ws.fails(
        1,
        &[
            "task",
            "link",
            "TM-1",
            "--kind",
            "relates",
            "--other",
            "TM-2",
            "--lag-days",
            "1",
        ],
    );

    // Kinds of work.
    ws.ok(&["work-kind", "create", "--name", "Development"]);
    ws.fails(3, &["work-kind", "create", "--name", "development"]);
    let logged = ws.json(&[
        "time",
        "log",
        "TM-1",
        "--duration",
        "1h",
        "--work-kind",
        "development",
    ]);
    assert_eq!(logged["entry"]["work_kind"]["name"], "Development");
    let timer = ws.json(&["timer", "start", "TM-2", "--work-kind", "Development"]);
    assert_eq!(timer["timer"]["work_kind"]["name"], "Development");
    ws.ok(&["timer", "stop", "--duration", "15m"]);
    let report = ws.json(&["time", "report", "--group-by", "work_kind"]);
    assert!(report.to_string().contains("Development"), "{report}");
    ws.ok(&["work-kind", "edit", "Development", "--archived", "true"]);
    let listed = ws.json(&["work-kind", "list"]);
    assert_eq!(listed["items"], json!([]), "{listed}");

    // Closing a task closes its duplicates when asked.
    ws.ok(&["task", "create", "--project", "TM", "--title", "Crash"]);
    ws.ok(&["task", "create", "--project", "TM", "--title", "Same crash"]);
    ws.ok(&[
        "task",
        "link",
        "TM-4",
        "--kind",
        "duplicates",
        "--other",
        "TM-3",
    ]);
    let closed = ws.json(&[
        "task",
        "move",
        "TM-3",
        "--status",
        "Done",
        "--close-duplicates",
        "true",
    ]);
    assert_eq!(closed["duplicates_closed"], json!(["TM-4"]), "{closed}");
    assert_eq!(
        ws.json(&["task", "show", "TM-4"])["items"][0]["status"]["name"],
        "Done"
    );

    // A WIP minimum and a type's due offset.
    let status = ws.json(&[
        "status",
        "edit",
        "--project",
        "TM",
        "--status",
        "In progress",
        "--wip-limit",
        "3",
        "--wip-min",
        "2",
    ]);
    assert!(status.to_string().contains("\"wip_min\":2"), "{status}");
    let board = ws.json(&["board", "TM"]);
    let column = named(&board["columns"], "In progress");
    assert_eq!(column["under_minimum"], true, "{board}");
    ws.fails(
        1,
        &[
            "status",
            "edit",
            "--project",
            "TM",
            "--status",
            "In progress",
            "--wip-min",
            "4",
        ],
    );
    ws.ok(&[
        "type",
        "edit",
        "--project",
        "TM",
        "--type",
        "Bug",
        "--due-offset-days",
        "3",
    ]);
    let bug = ws.json(&[
        "task",
        "create",
        "--project",
        "TM",
        "--title",
        "Bug",
        "--type",
        "Bug",
    ]);
    assert!(bug["task"]["due_date"].is_string(), "{bug}");
    let plain = ws.json(&["task", "create", "--project", "TM", "--title", "Plain"]);
    assert!(plain["task"].get("due_date").is_none(), "{plain}");
}

#[test]
fn tasks_are_found_by_their_history() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Team", "--key", "TM"]);
    for title in ["Went through", "Never moved", "Reprioritised"] {
        ws.ok(&["task", "create", "--project", "TM", "--title", title]);
    }
    ws.ok(&["task", "move", "TM-1", "--status", "In progress"]);
    ws.ok(&["task", "move", "TM-1", "--status", "Backlog"]);
    ws.ok(&["task", "edit", "TM-3", "--priority", "urgent"]);
    let find = |q: &str| sorted_keys(&ws.json(&["task", "list", "--query", q]));
    assert_eq!(find("was:status:\"In progress\""), ["TM-1"]);
    assert_eq!(
        find("project:TM -was:status:\"In progress\""),
        ["TM-2", "TM-3"]
    );
    assert_eq!(find("changed:status"), ["TM-1"]);
    assert_eq!(find("changed:priority>-1d"), ["TM-3"]);
    assert_eq!(find("changed-by:me"), ["TM-1", "TM-3"]);
    assert!(find("changed-by:agent").is_empty());
    let err = ws.fails(1, &["task", "list", "--query", "was:status:Doing"]);
    assert!(err.contains("Doing"), "{err}");
}

#[test]
fn a_saved_preset_imports_a_semicolon_separated_file() {
    let ws = Workspace::new();
    ws.ok(&["project", "create", "--name", "Shop", "--key", "SHOP"]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tickets.csv");
    std::fs::write(
        &path,
        "Name;Kind;Notes\nMend the gate;Bug;Hinge is loose\nPaint the fence;Task;\n",
    )
    .unwrap();
    let path = path.to_string_lossy().into_owned();
    ws.ok(&[
        "csv-preset",
        "save",
        "Garden",
        "--columns",
        r#"[{"column": "Name", "to": "title"}, {"column": "Kind", "to": "type"}, {"column": "Notes", "to": "description"}]"#,
    ]);
    let presets = ws.json(&["csv-preset", "list"]);
    assert!(presets.to_string().contains("Garden"), "{presets}");

    let check = ws.json(&[
        "import",
        "csv",
        &path,
        "--project",
        "SHOP",
        "--preset",
        "Garden",
        "--dry-run",
    ]);
    assert_eq!(check["dry_run"], true, "{check}");
    assert_eq!(check["preset"], "Garden", "{check}");
    assert_eq!(check["counts"]["created"], 2, "{check}");
    ws.ok(&[
        "import",
        "csv",
        &path,
        "--project",
        "SHOP",
        "--preset",
        "Garden",
    ]);
    let tasks = ws.json(&["task", "show", "SHOP-1", "SHOP-2"]);
    assert_eq!(tasks["items"][0]["title"], "Mend the gate");
    assert_eq!(tasks["items"][0]["type"]["name"], "Bug");
    assert_eq!(tasks["items"][1]["title"], "Paint the fence");

    let out = ws.run(&[
        "import",
        "csv",
        &path,
        "--project",
        "SHOP",
        "--preset",
        "Nope",
        "--dry-run",
    ]);
    assert_ne!(code(&out), 0, "{}", stdout(&out));
}
