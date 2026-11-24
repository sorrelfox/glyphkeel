// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Global search (matching, ranking, marks, filters, paging), Today in the
//! workspace's time zone, and natural dates in task inputs, over a real
//! workspace. The ranking is checked against golden lists in `snapshots/`.

#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

struct Ws {
    engine: Engine,
    _dir: TempDir,
}

/// A workspace in UTC on a clock that starts at [`NOW`] and moves on a
/// second with every reading, so later changes are newer.
fn open() -> Ws {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(AtomicI64::new(NOW));
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || clock.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    let engine = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
    Ws { engine, _dir: dir }
}

fn me() -> Context {
    Context::user(Via::App)
}

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&me(), op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn fails(&self, op: &str, input: Value) -> OpError {
        self.engine.call(&me(), op, input).expect_err(op)
    }

    fn task(&self, project: &str, title: &str, description: &str) -> String {
        self.call(
            "task.create",
            json!({ "project": project, "title": title, "description": description }),
        )["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn search(&self, input: Value) -> Value {
        self.call("search.query", input)
    }

    fn agent(&self, name: &str) -> Context {
        let session = self
            .engine
            .start_session(&SessionStart {
                name: name.into(),
                client_id: None,
                client_version: None,
                profile: Profile::Standard,
            })
            .unwrap();
        Context::agent(name, session, Profile::Standard)
    }
}

/// `text` with `[` `]` around each range (UTF-16 code units).
fn marked(text: &str, ranges: &Value) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out = Vec::new();
    let mut at = 0usize;
    for r in ranges.as_array().into_iter().flatten() {
        let from = usize::try_from(r["from"].as_u64().unwrap()).unwrap();
        let to = usize::try_from(r["to"].as_u64().unwrap()).unwrap();
        out.extend_from_slice(&units[at..from]);
        out.push(u16::from(b'['));
        out.extend_from_slice(&units[from..to]);
        out.push(u16::from(b']'));
        at = to;
    }
    out.extend_from_slice(&units[at..]);
    String::from_utf16(&out).unwrap()
}

/// One line per result: kind, where it matched, task key, marked title and
/// marked snippet.
fn render(out: &Value) -> String {
    let mut s = String::new();
    for hit in out["items"].as_array().unwrap() {
        let _ = write!(
            s,
            "{:<8} {:<9} {:<6} {}",
            hit["kind"].as_str().unwrap(),
            hit["matched_in"].as_str().unwrap(),
            hit["task"].as_str().unwrap_or("-"),
            marked(hit["title"].as_str().unwrap(), &hit["title_matches"]),
        );
        if let Some(snippet) = hit["snippet"].as_str() {
            let _ = write!(s, " | {}", marked(snippet, &hit["snippet_matches"]));
        }
        s.push('\n');
    }
    let _ = write!(s, "total: {}", out["total"]);
    s
}

/// A small workspace with the word "timeout" in every kind of place.
fn corpus() -> Ws {
    let ws = open();
    ws.call(
        "project.create",
        json!({ "name": "Task Manager", "key": "TM" }),
    );
    ws.call(
        "project.create",
        json!({ "name": "Home lab", "key": "LAB" }),
    );
    let sync = ws.task("TM", "Connection timeout on sync", "Retry later.");
    ws.task(
        "TM",
        "Sync engine",
        "The timeout is too short when the network is slow.",
    );
    let unrelated = ws.task("TM", "Unrelated chores", "Nothing to see.");
    ws.call(
        "comment.add",
        json!({ "key": unrelated, "body": "We saw a **timeout** here yesterday." }),
    );
    ws.task(
        "TM",
        "Error codes",
        "`ERR_CONN` appears when the user_id is missing.",
    );
    ws.task("LAB", "Router timeout", "Replace the router.");
    ws.call(
        "file.attach",
        json!({ "task": sync, "name": "timeout-trace.txt", "data_base64": "aGVsbG8=" }),
    );
    ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Timeout policy",
                "body": "# Defaults\n\nThe default is 30 s.\n" }),
    );
    ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Network",
                "body": "## Handling\n\nEvery request has a timeout and two retries.\n" }),
    );
    ws
}

#[test]
fn ranking_puts_titles_before_text_before_comments_and_files() {
    let ws = corpus();
    insta::assert_snapshot!(
        "rank_timeout",
        render(&ws.search(json!({ "text": "timeout" })))
    );
}

#[test]
fn words_match_beginnings_and_code_matches_on_fragments() {
    let ws = corpus();
    insta::assert_snapshot!("rank_user", render(&ws.search(json!({ "text": "user" }))));
    insta::assert_snapshot!("rank_conn", render(&ws.search(json!({ "text": "CONN" }))));
    insta::assert_snapshot!(
        "rank_fragment",
        render(&ws.search(json!({ "text": "onnect" })))
    );
    insta::assert_snapshot!(
        "rank_phrase",
        render(&ws.search(json!({ "text": "\"too short\" network" })))
    );
}

#[test]
fn english_words_match_their_other_forms() {
    let ws = open();
    ws.call(
        "project.create",
        json!({ "name": "Task Manager", "key": "TM" }),
    );
    let run = ws.task("TM", "Run the nightly job", "It should not stop.");
    let backups = ws.task("TM", "Check disks", "The backups were running late.");
    ws.task("TM", "Unrelated chores", "Nothing to see.");
    let found = |text: &str| -> Vec<String> {
        let mut keys: Vec<String> = ws.search(json!({ "text": text, "kinds": ["task"] }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["task"].as_str().unwrap().to_owned())
            .collect();
        keys.sort();
        keys
    };
    // "running" finds "run", and "run" finds "running".
    assert_eq!(found("running"), [run.clone(), backups.clone()]);
    assert_eq!(found("runs"), [run.clone(), backups.clone()]);
    assert_eq!(found("run"), [run, backups.clone()]);
    // "backup" finds "backups"; "stopping" finds "stop".
    assert_eq!(found("backup"), [backups]);
    assert_eq!(found("stopping").len(), 1);
}

#[test]
fn keys_jump_to_their_task_or_project() {
    let ws = corpus();
    let out = ws.search(json!({ "text": "tm-2" }));
    assert_eq!(out["items"][0]["kind"], "task");
    assert_eq!(out["items"][0]["matched_in"], "key");
    assert_eq!(out["items"][0]["task"], "TM-2");
    assert_eq!(out["items"][0]["title"], "Sync engine");
    let out = ws.search(json!({ "text": "LAB" }));
    assert_eq!(out["items"][0]["kind"], "project");
    assert_eq!(out["items"][0]["title"], "Home lab");
    assert_eq!(out["total"], 1);
}

#[test]
fn filters_by_kind_project_and_task_query() {
    let ws = corpus();
    let kinds = |out: &Value| -> Vec<String> {
        out["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["kind"].as_str().unwrap().to_owned())
            .collect()
    };
    let wiki = ws.search(json!({ "text": "timeout", "kinds": ["wiki"] }));
    assert_eq!(kinds(&wiki), ["wiki", "wiki"]);
    assert_eq!(wiki["items"][0]["slug"], "timeout-policy");
    let lab = ws.search(json!({ "text": "timeout", "project": "LAB" }));
    assert_eq!(
        render(&lab),
        "task     title     LAB-1  Router [timeout] | total: 1".replace(" | ", "\n")
    );
    ws.call("task.move", json!({ "key": "TM-1", "status": "Done" }));
    let open = ws.search(json!({ "text": "timeout", "query": "is:open" }));
    assert!(
        open["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["task"] != "TM-1" && h["kind"] != "wiki"),
        "{open}"
    );
    assert_eq!(kinds(&open), ["task", "task", "comment"]);
}

/// The next page starts after the last result shown, even when a result
/// above it went away in between.
#[test]
fn the_cursor_survives_a_change_between_pages() {
    let ws = corpus();
    let all: Vec<Value> = ws.search(json!({ "text": "timeout" }))["items"]
        .as_array()
        .unwrap()
        .clone();
    let first = ws.search(json!({ "text": "timeout", "limit": 3 }));
    let gone = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["kind"] == "task")
        .expect("a task on the first page");
    ws.call("task.delete", json!({ "keys": [gone["task"]] }));
    let second =
        ws.search(json!({ "text": "timeout", "limit": 3, "cursor": first["next_cursor"] }));
    assert_eq!(second["items"][0]["id"], all[3]["id"], "{second}");
}

#[test]
fn pages_follow_the_cursor() {
    let ws = corpus();
    let first = ws.search(json!({ "text": "timeout", "limit": 3 }));
    assert_eq!(first["items"].as_array().unwrap().len(), 3);
    assert_eq!(first["total"], 7);
    let second =
        ws.search(json!({ "text": "timeout", "limit": 3, "cursor": first["next_cursor"] }));
    let third =
        ws.search(json!({ "text": "timeout", "limit": 3, "cursor": second["next_cursor"] }));
    assert!(third.get("next_cursor").is_none());
    let all = ws.search(json!({ "text": "timeout" }));
    let ids: Vec<&Value> = [&first, &second, &third]
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .map(|h| &h["id"])
        .collect();
    let expected: Vec<&Value> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| &h["id"])
        .collect();
    assert_eq!(ids, expected);
    let bad = ws.fails("search.query", json!({ "text": "x", "cursor": "later" }));
    assert_eq!(bad.code, ErrorCode::Invalid);
    assert_eq!(bad.details["message_id"], "list-cursor");
    let empty = ws.fails("search.query", json!({ "text": "  --  " }));
    assert_eq!(empty.details["message_id"], "search-empty");
}

#[test]
fn among_equal_matches_newer_comes_first_and_changes_show_at_once() {
    let ws = open();
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    ws.task("TM", "Backup disk", "");
    ws.task("TM", "Backup disk", "");
    let out = ws.search(json!({ "text": "backup" }));
    assert_eq!(out["items"][0]["task"], "TM-2");
    // The same words: as good a match, but newer.
    ws.call(
        "task.update",
        json!({ "key": "TM-1", "title": "Backup, disk" }),
    );
    let out = ws.search(json!({ "text": "backup" }));
    assert_eq!(out["items"][0]["task"], "TM-1");
    ws.call(
        "task.update",
        json!({ "key": "TM-2", "description": "now changed" }),
    );
    let out = ws.search(json!({ "text": "changed" }));
    assert_eq!(out["items"][0]["task"], "TM-2");
    let comment = ws.call(
        "comment.add",
        json!({ "key": "TM-2", "body": "zebra crossing" }),
    );
    assert_eq!(ws.search(json!({ "text": "zebra" }))["total"], 1);
    ws.call(
        "comment.delete",
        json!({ "comment": comment["comment"]["id"] }),
    );
    assert_eq!(ws.search(json!({ "text": "zebra" }))["total"], 0);
    ws.call("project.archive", json!({ "project": "TM" }));
    assert_eq!(ws.search(json!({ "text": "backup" }))["total"], 0);
    assert_eq!(
        ws.search(json!({ "text": "backup", "project": "TM" }))["total"],
        2
    );
}

// ---------------------------------------------------------------------
// Today

fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t.get("key")
                .or_else(|| t["task"].get("key"))
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn due(ws: &Ws, title: &str, date: &str) -> String {
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": title, "due_date": date }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn today_lists_what_needs_attention_once() {
    let ws = open();
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    ws.call("project.create", json!({ "name": "Old", "key": "OLD" }));
    let late = due(&ws, "Late", "2026-11-20");
    let late_working = due(&ws, "Late and in progress", "2026-11-22");
    ws.call(
        "task.move",
        json!({ "key": late_working, "status": "In progress" }),
    );
    let today = due(&ws, "Today", "2026-11-23");
    let done = due(&ws, "Late but done", "2026-11-21");
    ws.call("task.move", json!({ "key": done, "status": "Done" }));
    due(&ws, "Later", "2026-11-30");
    let working = ws.task("TM", "Working on it", "");
    ws.call(
        "task.move",
        json!({ "key": working, "status": "In progress" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "OLD", "title": "Archived and late", "due_date": "2026-11-01" }),
    );
    ws.call("project.archive", json!({ "project": "OLD" }));
    ws.call(
        "reminder.set",
        json!({ "key": today, "at": "2026-11-23 15:00", "note": "Call" }),
    );
    let agent = ws.agent("Helper");
    let claimable = ws.task("TM", "For the agent", "");
    ws.engine
        .call(&agent, "task.claim", json!({ "key": claimable }))
        .unwrap();
    ws.call("timer.start", json!({ "key": working }));

    let out = ws.call("today.get", json!({}));
    assert_eq!(out["date"], "2026-11-23");
    assert_eq!(keys(&out["overdue"]), [late.clone(), late_working.clone()]);
    assert_eq!(keys(&out["due_today"]), std::slice::from_ref(&today));
    assert_eq!(
        keys(&out["in_progress"]),
        [working.clone(), claimable.clone()]
    );
    assert_eq!(keys(&out["claimed"]), std::slice::from_ref(&claimable));
    assert_eq!(out["reminders"][0]["task"]["key"], today);
    assert_eq!(out["reminders"][0]["missed"], false);
    assert_eq!(out["timer"]["task"]["key"], working);
    let recent = out["recent"].as_array().unwrap();
    assert_eq!(recent[0]["task"]["key"], claimable);
    assert_eq!(recent[0]["changed_by"]["name"], "Helper");
    assert_eq!(
        out["counts"],
        json!({ "today": 5, "overdue": 2, "due_today": 1, "in_progress": 2,
                "reminders": 1, "claimed": 1, "recent": 7, "open": 6 })
    );
    let limited = ws.call("today.get", json!({ "limit": 1 }));
    assert_eq!(keys(&limited["overdue"]), [late]);
    assert_eq!(limited["counts"]["overdue"], 2);
    let old = ws.call("today.get", json!({ "project": "OLD" }));
    assert_eq!(old["counts"]["overdue"], 1);
}

#[test]
fn today_follows_the_time_zone() {
    let ws = open();
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let key = due(&ws, "Due on the 23rd", "2026-11-23");
    assert_eq!(
        keys(&ws.call("today.get", json!({}))["due_today"]),
        std::slice::from_ref(&key)
    );
    // 10:00 UTC is already the 24th at UTC+14 and still the 23rd at UTC-10.
    ws.engine.set_time_zone("Pacific/Kiritimati").unwrap();
    let east = ws.call("today.get", json!({}));
    assert_eq!(east["date"], "2026-11-24");
    assert_eq!(keys(&east["overdue"]), std::slice::from_ref(&key));
    ws.engine.set_time_zone("Pacific/Honolulu").unwrap();
    let west = ws.call("today.get", json!({}));
    assert_eq!(west["date"], "2026-11-23");
    assert_eq!(keys(&west["due_today"]), [key]);
}

// ---------------------------------------------------------------------
// Natural dates

#[test]
fn task_dates_accept_natural_forms_in_the_workspace_zone() {
    let ws = open();
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let made = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Plan", "start_date": "today", "due_date": "fri" }),
    );
    assert_eq!(made["task"]["start_date"], "2026-11-23");
    assert_eq!(made["task"]["due_date"], "2026-11-27");
    for (text, date) in [
        ("in 3d", "2026-11-26"),
        ("+1w", "2026-11-30"),
        ("next fri", "2026-12-04"),
        ("dec 24", "2026-12-24"),
        ("2027-01-15", "2027-01-15"),
    ] {
        let out = ws.call("task.update", json!({ "key": "TM-1", "due_date": text }));
        assert_eq!(out["task"]["due_date"], date, "{text}");
    }
    let bad = ws.fails(
        "task.update",
        json!({ "key": "TM-1", "due_date": "someday" }),
    );
    assert_eq!(bad.code, ErrorCode::Invalid);
    assert_eq!(bad.details["message_id"], "date-invalid");
    assert_eq!(bad.details["path"], "due_date");
    let impossible = ws.fails(
        "task.update",
        json!({ "key": "TM-1", "due_date": "2026-02-30" }),
    );
    assert_eq!(impossible.code, ErrorCode::Invalid);
    // Not the natural forms' message: what is wrong with the date itself.
    assert_eq!(
        impossible.details["message_id"],
        "value-day-not-in-calendar"
    );
    ws.engine.set_time_zone("Pacific/Kiritimati").unwrap();
    let east = ws.call(
        "task.update",
        json!({ "key": "TM-1", "start_date": "today" }),
    );
    assert_eq!(east["task"]["start_date"], "2026-11-24");
}
