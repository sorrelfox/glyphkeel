// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task details over a real workspace: links and subtasks, comments, files,
//! duplicates, claims, deleting a project and the agent brief.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

const START: i64 = 1_795_000_000_000;
const MINUTE: i64 = 60_000;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
    layout: WorkspaceLayout,
}

fn open(dir: &TempDir) -> Ws {
    let clock = Arc::new(AtomicI64::new(START));
    let tick = Arc::clone(&clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    Ws {
        engine: Engine::open(&layout, config).unwrap(),
        clock,
        layout,
    }
}

fn me() -> Context {
    Context::user(Via::Cli)
}

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.as_(&me(), op, input)
    }

    fn as_(&self, ctx: &Context, op: &str, input: Value) -> Value {
        self.engine
            .call(ctx, op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn fails(&self, ctx: &Context, op: &str, input: Value) -> OpError {
        self.engine.call(ctx, op, input).expect_err(op)
    }

    fn task(&self, key: &str) -> Value {
        self.call("task.get", json!({ "keys": [key] }))["items"][0].clone()
    }

    fn task_with(&self, key: &str, include: &[&str]) -> Value {
        self.call("task.get", json!({ "keys": [key], "include": include }))["items"][0].clone()
    }

    fn new_task(&self, project: &str, title: &str) -> String {
        self.call("task.create", json!({ "project": project, "title": title }))["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn agent(&self, name: &str, profile: Profile) -> Context {
        let session = self
            .engine
            .start_session(&SessionStart {
                name: name.into(),
                client_id: None,
                client_version: None,
                profile,
            })
            .unwrap();
        Context::agent(name, session, profile)
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn card(&self, project: &str, key: &str) -> Value {
        let board = self.call(
            "board.get",
            json!({ "project": project, "show_older": true }),
        );
        for column in board["columns"].as_array().unwrap() {
            for card in column["cards"].as_array().unwrap() {
                if card["key"] == key {
                    return card.clone();
                }
            }
        }
        panic!("{key} is not on the board");
    }
}

fn link_kinds(task: &Value) -> Vec<(String, String)> {
    task["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["kind"].as_str().unwrap().to_owned(),
                l["task"]["key"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn links_read_from_both_ends_and_block_while_open() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    ws.call("project.create", json!({ "name": "O", "key": "OT" }));
    let a = ws.new_task("TM", "Blocker");
    let b = ws.new_task("TM", "Blocked one");
    let c = ws.new_task("OT", "Elsewhere");

    let out = ws.call(
        "relation.add",
        json!({ "key": b, "kind": "blocked_by", "other": a }),
    );
    assert_eq!(link_kinds(&out["task"]), [("blocked_by".into(), a.clone())]);
    assert_eq!(ws.task(&b)["blocked"], true);
    assert_eq!(ws.task(&a)["blocked"], false);
    assert_eq!(
        link_kinds(&ws.task_with(&a, &["links"])),
        [("blocks".into(), b.clone())]
    );
    assert_eq!(ws.card("TM", &b)["blocked"], true);
    // The same link again changes nothing; the reverse "relates" is one row.
    let again = ws.call(
        "relation.add",
        json!({ "key": a, "kind": "blocks", "other": b }),
    );
    assert!(again.get("group").is_none());
    ws.call(
        "relation.add",
        json!({ "key": a, "kind": "relates", "other": c }),
    );
    let same = ws.call(
        "relation.add",
        json!({ "key": c, "kind": "relates", "other": a }),
    );
    assert!(same.get("group").is_none());
    assert_eq!(link_kinds(&ws.task_with(&c, &["links"])).len(), 1);

    // Itself, a loop of blocks, a task and its own subtask: refused.
    let invalid = |input: Value| ws.fails(&me(), "relation.add", input).code;
    assert_eq!(
        invalid(json!({ "key": a, "kind": "relates", "other": a })),
        ErrorCode::Invalid
    );
    assert_eq!(
        invalid(json!({ "key": b, "kind": "blocks", "other": a })),
        ErrorCode::Invalid
    );
    let sub = ws.new_task("TM", "Sub");
    ws.call(
        "relation.add",
        json!({ "key": sub, "kind": "subtask_of", "other": b }),
    );
    assert_eq!(ws.task(&sub)["parent"], b.as_str());
    assert_eq!(ws.task(&b)["subtask_count"]["total"], 1);
    assert_eq!(
        invalid(json!({ "key": b, "kind": "duplicates", "other": sub })),
        ErrorCode::Invalid
    );
    let subtasks = ws.task_with(&b, &["subtasks"])["subtasks"].clone();
    assert_eq!(subtasks[0]["key"], sub.as_str());

    // Closing the blocker unblocks; closing a blocked task warns.
    let moved = ws.call("task.move", json!({ "key": b, "status": "Done" }));
    let warnings = moved["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|w| w.as_str().unwrap().contains(&a)));
    ws.call("task.move", json!({ "key": b, "status": "To do" }));
    ws.call("task.move", json!({ "key": a, "status": "Done" }));
    assert_eq!(ws.task(&b)["blocked"], false);

    // Removing a link, and a subtask through a link.
    ws.call(
        "relation.remove",
        json!({ "key": a, "kind": "blocks", "other": b }),
    );
    assert_eq!(link_kinds(&ws.task_with(&b, &["links"])), []);
    ws.call(
        "relation.remove",
        json!({ "key": b, "kind": "parent_of", "other": sub }),
    );
    assert!(ws.task(&sub).get("parent").is_none());
    assert_eq!(
        ws.fails(
            &me(),
            "relation.remove",
            json!({ "key": a, "kind": "blocks", "other": b })
        )
        .code,
        ErrorCode::NotFound
    );
}

#[test]
fn undo_takes_back_links_and_brings_them_back() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    let app = Context::user(Via::App);
    ws.as_(&app, "project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task("TM", "A");
    let b = ws.new_task("TM", "B");
    ws.as_(
        &app,
        "relation.add",
        json!({ "key": a, "kind": "blocks", "other": b }),
    );
    ws.as_(
        &app,
        "relation.remove",
        json!({ "key": a, "kind": "blocks", "other": b }),
    );
    assert_eq!(ws.task(&b)["blocked"], false);
    ws.as_(&app, "history.undo", json!({}));
    assert_eq!(ws.task(&b)["blocked"], true);
    let id = ws.task_with(&b, &["links"])["links"][0]["id"].clone();
    ws.as_(&app, "history.undo", json!({}));
    assert_eq!(ws.task(&b)["blocked"], false);
    ws.as_(&app, "history.redo", json!({}));
    let back = ws.task_with(&b, &["links"]);
    assert_eq!(back["blocked"], true);
    assert_eq!(back["links"][0]["id"], id);
}

#[test]
fn comments_keep_their_earlier_texts_and_go_to_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task("TM", "A");
    let b = ws.new_task("TM", "B");
    let version = ws.task(&a)["version"].clone();
    let first = ws.call(
        "comment.add",
        json!({ "key": a, "body": "First **draft**" }),
    );
    let id = first["comment"]["id"].as_str().unwrap().to_owned();
    assert_eq!(first["comment"]["task"], a.as_str());
    assert_eq!(first["comment"]["version"], 1);
    // A comment is activity on the task, not a change of its fields.
    assert_eq!(ws.task(&a)["version"], version);
    assert_eq!(ws.task(&a)["comment_count"], 1);
    assert_eq!(ws.card("TM", &a)["comment_count"], 1);

    let edited = ws.call(
        "comment.update",
        json!({ "comment": id, "body": "Final", "expected_version": 1 }),
    );
    assert_eq!(edited["comment"]["version"], 2);
    assert!(edited["comment"].get("edited_at").is_some());
    assert_eq!(edited["comment"]["revision_count"], 1);
    let stale = ws.fails(
        &me(),
        "comment.update",
        json!({ "comment": id, "body": "Again", "expected_version": 1 }),
    );
    assert_eq!(stale.code, ErrorCode::Conflict);
    let listed = ws.call("comment.list", json!({ "key": a, "revisions": true }));
    assert_eq!(listed["items"][0]["body"], "Final");
    assert_eq!(
        listed["items"][0]["revisions"][0]["body"],
        "First **draft**"
    );

    // Answers stay on the same task; empty comments are refused.
    let reply = ws.call(
        "comment.add",
        json!({ "key": a, "body": "Reply", "reply_to": id }),
    );
    assert_eq!(reply["comment"]["reply_to"], id.as_str());
    let elsewhere = ws.fails(
        &me(),
        "comment.add",
        json!({ "key": b, "body": "x", "reply_to": id }),
    );
    assert_eq!(elsewhere.code, ErrorCode::Invalid);
    assert_eq!(
        ws.fails(&me(), "comment.add", json!({ "key": a, "body": "  " }))
            .code,
        ErrorCode::Invalid
    );

    // Deleting moves it to the trash; reverting the deletion restores it.
    let deleted = ws.call("comment.delete", json!({ "comment": id }));
    assert_eq!(deleted["deleted"], true);
    assert_eq!(ws.task(&a)["comment_count"], 1);
    assert_eq!(
        ws.fails(
            &me(),
            "comment.update",
            json!({ "comment": id, "body": "x" })
        )
        .code,
        ErrorCode::NotFound
    );
    let group = deleted["group"].as_str().unwrap();
    ws.call("history.revert", json!({ "groups": [group] }));
    let comments = ws.task_with(&a, &["comments"])["comments"].clone();
    assert_eq!(comments.as_array().unwrap().len(), 2);
    assert_eq!(comments[0]["body"], "Final");
    // Reverting an edit brings the earlier text back and keeps both.
    let edit = edited["group"].as_str().unwrap();
    ws.call("history.revert", json!({ "groups": [edit] }));
    let listed = ws.call("comment.list", json!({ "key": a, "revisions": true }));
    assert_eq!(listed["items"][0]["body"], "First **draft**");
    assert_eq!(listed["items"][0]["revision_count"], 2);
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&width.to_be_bytes());
    png.extend_from_slice(&height.to_be_bytes());
    png.extend_from_slice(b"\x08\x06\0\0\0");
    png
}

fn stored_files(layout: &WorkspaceLayout) -> usize {
    walk(&layout.files_dir())
}

fn walk(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .map(|e| {
            let path = e.unwrap().path();
            if path.ends_with("tmp") {
                0
            } else if path.is_dir() {
                walk(&path)
            } else {
                1
            }
        })
        .sum()
}

#[test]
fn files_are_stored_once_typed_by_content_and_limited() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let a = ws.new_task("TM", "A");
    let b = ws.new_task("TM", "B");
    let image = gk_core::base64_encode(&png(640, 480));
    let first = ws.call(
        "file.attach",
        json!({ "task": a, "name": "shot.txt", "data_base64": image }),
    );
    let file = &first["file"];
    assert_eq!(file["mime"], "image/png");
    assert_eq!(file["kind"], "image");
    assert_eq!(file["width"], 640);
    assert_eq!(file["height"], 480);
    assert_eq!(file["task"], a.as_str());

    // The same content again, from a path and on another task: one file.
    let path = dir.path().join("copy.png");
    std::fs::write(&path, png(640, 480)).unwrap();
    let second = ws.call(
        "file.attach",
        json!({ "task": b, "path": path.to_str().unwrap() }),
    );
    assert_eq!(second["file"]["name"], "copy.png");
    assert_eq!(second["file"]["sha256"], file["sha256"]);
    assert_eq!(stored_files(&ws.layout), 1);

    // A text file on a comment counts for its task.
    let comment = ws.call("comment.add", json!({ "key": a, "body": "See the log" }));
    let log = ws.call(
        "file.attach",
        json!({
            "comment": comment["comment"]["id"],
            "name": "../../build.log",
            "data_base64": gk_core::base64_encode(b"line one\nline two\n"),
        }),
    );
    assert_eq!(log["file"]["name"], "build.log");
    assert_eq!(log["file"]["mime"], "text/plain");
    assert_eq!(ws.task(&a)["file_count"], 2);
    assert_eq!(ws.card("TM", &a)["file_count"], 2);
    let read = ws.call("file.read", json!({ "attachment": log["file"]["id"] }));
    assert_eq!(read["text"], "line one\nline two\n");
    let read = ws.call("file.read", json!({ "attachment": file["id"] }));
    assert_eq!(read["data_base64"], image);
    let files = ws.task_with(&a, &["files", "comments"]);
    assert_eq!(files["files"].as_array().unwrap().len(), 1);
    assert_eq!(files["comments"][0]["files"][0]["name"], "build.log");

    // Over the limit: refused before anything is stored.
    ws.call("settings.update", json!({ "file_max_mb": 1 }));
    let big = dir.path().join("big.bin");
    std::fs::write(&big, vec![7u8; 1024 * 1024 + 1]).unwrap();
    let err = ws.fails(
        &me(),
        "file.attach",
        json!({ "task": a, "path": big.to_str().unwrap() }),
    );
    assert_eq!(err.code, ErrorCode::Limit);
    assert_eq!(stored_files(&ws.layout), 2);

    // Deleting moves it to the trash; reverting brings it back.
    let gone = ws.call("file.delete", json!({ "attachment": file["id"] }));
    assert_eq!(gone["deleted"], true);
    assert_eq!(ws.task(&a)["file_count"], 1);
    assert_eq!(
        ws.fails(&me(), "file.get", json!({ "attachment": file["id"] }))
            .code,
        ErrorCode::NotFound
    );
    ws.call("history.revert", json!({ "groups": [gone["group"]] }));
    assert_eq!(ws.task(&a)["file_count"], 2);
    let err = ws.fails(&me(), "file.attach", json!({ "task": a, "name": "x" }));
    assert_eq!(err.code, ErrorCode::Invalid);
}

#[test]
fn a_duplicate_copies_subtasks_labels_checklist_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    ws.call("label.create", json!({ "project": "TM", "name": "ui" }));
    let root = ws.call(
        "task.create",
        json!({
            "project": "TM", "title": "Release", "labels": ["ui"], "priority": "high",
            "description": "- [x] one\n- [ ] two", "estimate": "2h",
        }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    for title in ["First step", "Second step"] {
        ws.call(
            "task.create",
            json!({ "project": "TM", "title": title, "parent": root }),
        );
    }
    ws.call("task.move", json!({ "key": root, "status": "Done" }));
    ws.call(
        "file.attach",
        json!({ "task": root, "name": "notes.md", "data_base64": gk_core::base64_encode(b"# Notes") }),
    );
    ws.call("comment.add", json!({ "key": root, "body": "Not copied" }));

    let copy = ws.call(
        "task.duplicate",
        json!({ "key": root, "title": "Release 2" }),
    );
    let t = &copy["task"];
    assert_eq!(t["key"], "TM-4");
    assert_eq!(t["title"], "Release 2");
    assert_eq!(t["status"]["name"], "Backlog");
    assert!(t.get("closed_at").is_none());
    assert_eq!(t["priority"], "high");
    assert_eq!(t["labels"][0]["name"], "ui");
    assert_eq!(t["checklist"]["done"], 1);
    assert_eq!(t["checklist"]["total"], 2);
    assert_eq!(t["estimate_minutes"], 120);
    assert_eq!(t["comment_count"], 0);
    assert_eq!(t["file_count"], 1);
    let subtasks: Vec<&str> = t["subtasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["title"].as_str().unwrap())
        .collect();
    assert_eq!(subtasks, ["First step", "Second step"]);
    assert_eq!(link_kinds(t), [("clones".into(), root.clone())]);
    assert_eq!(
        link_kinds(&ws.task_with(&root, &["links"])),
        [("cloned_by".into(), "TM-4".into())]
    );
    assert_eq!(stored_files(&ws.layout), 1);

    // One revert takes the whole copy back.
    ws.call("history.revert", json!({ "groups": [copy["group"]] }));
    for key in ["TM-4", "TM-5", "TM-6"] {
        assert_eq!(
            ws.fails(&me(), "task.get", json!({ "keys": [key] })).code,
            ErrorCode::NotFound,
            "{key}"
        );
    }
    assert_eq!(link_kinds(&ws.task_with(&root, &["links"])), []);
}

#[test]
fn claims_keep_other_agents_away_but_never_the_person() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let key = ws.new_task("TM", "Shared queue item");
    let alpha = ws.agent("Alpha", Profile::Standard);
    let beta = ws.agent("Beta", Profile::Standard);

    let claimed = ws.as_(
        &alpha,
        "task.claim",
        json!({ "key": key, "lease_minutes": 30 }),
    );
    assert_eq!(claimed["task"]["status"]["name"], "In progress");
    assert_eq!(claimed["task"]["claim"]["agent"], "Alpha");
    assert_eq!(ws.card("TM", &key)["claim"]["agent"], "Alpha");

    let refused = ws.fails(
        &beta,
        "task.update",
        json!({ "key": key, "priority": "high" }),
    );
    assert_eq!(refused.code, ErrorCode::Conflict);
    assert!(refused.message.contains("Alpha"), "{}", refused.message);
    assert_eq!(
        ws.fails(&beta, "task.claim", json!({ "key": key })).code,
        ErrorCode::Conflict
    );
    let override_ = ws.fails(
        &beta,
        "task.move",
        json!({ "key": key, "status": "Done", "override_claim": true }),
    );
    assert_eq!(override_.code, ErrorCode::Forbidden);
    // The person is never blocked; the claiming agent's changes renew it.
    ws.call("task.update", json!({ "key": key, "priority": "low" }));
    ws.advance(25 * MINUTE);
    ws.as_(
        &alpha,
        "task.update",
        json!({ "key": key, "priority": "normal" }),
    );
    ws.advance(50 * MINUTE);
    assert_eq!(ws.task(&key)["claim"]["agent"], "Alpha");
    assert_eq!(
        ws.fails(&me(), "task.claim", json!({ "key": key })).code,
        ErrorCode::Invalid
    );

    let released = ws.as_(
        &alpha,
        "task.release",
        json!({ "key": key, "status": "Review", "comment": "Ready for a look." }),
    );
    assert!(released["task"].get("claim").is_none());
    assert_eq!(released["task"]["status"]["name"], "Review");
    assert_eq!(released["task"]["comment_count"], 1);
    ws.as_(
        &beta,
        "task.update",
        json!({ "key": key, "priority": "urgent" }),
    );

    // An expired claim simply stops counting.
    ws.as_(
        &beta,
        "task.claim",
        json!({ "key": key, "lease_minutes": 1, "keep_status": true }),
    );
    ws.advance(2 * MINUTE);
    assert!(ws.task(&key).get("claim").is_none());
    ws.as_(
        &alpha,
        "task.update",
        json!({ "key": key, "priority": "low" }),
    );
    // A full agent may override, once the person allows full agents.
    ws.as_(&alpha, "task.claim", json!({ "key": key }));
    ws.call("agent.policy.update", json!({ "max_profile": "full" }));
    let full = ws.agent("Gamma", Profile::Full);
    ws.as_(
        &full,
        "task.update",
        json!({ "key": key, "priority": "high", "override_claim": true }),
    );
}

#[test]
fn deleting_a_project_is_confirmed_and_removes_everything_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "Old", "key": "OLD" }));
    ws.call(
        "project.create",
        json!({ "name": "Sub", "key": "SUB", "parent": "OLD" }),
    );
    ws.call("project.create", json!({ "name": "Keep", "key": "KEEP" }));
    let old = ws.new_task("OLD", "Old work");
    let sub = ws.new_task("SUB", "Sub work");
    let keep = ws.new_task("KEEP", "Stays");
    ws.call(
        "relation.add",
        json!({ "key": keep, "kind": "blocked_by", "other": old }),
    );
    ws.call("comment.add", json!({ "key": sub, "body": "gone soon" }));
    ws.call(
        "file.attach",
        json!({ "task": old, "name": "only.txt", "data_base64": gk_core::base64_encode(b"only here") }),
    );
    let shared = gk_core::base64_encode(b"shared content");
    ws.call(
        "file.attach",
        json!({ "task": old, "name": "a.txt", "data_base64": shared }),
    );
    ws.call(
        "file.attach",
        json!({ "task": keep, "name": "b.txt", "data_base64": shared }),
    );
    let earlier = ws.call(
        "task.update",
        json!({ "key": old, "title": "Secret title" }),
    );
    assert_eq!(stored_files(&ws.layout), 2);
    assert_eq!(ws.task(&keep)["blocked"], true);

    // Not confirmed, a wrong count, an agent without the full profile.
    let err = ws.fails(&me(), "project.delete", json!({ "project": "OLD" }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    let sure = Context::user(Via::Cli).with_confirmation(Confirmation::Given);
    let err = ws.fails(
        &sure,
        "project.delete",
        json!({ "project": "OLD", "expected_task_count": 5 }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    let agent = ws.agent("Agent", Profile::Standard);
    assert_eq!(
        ws.fails(&agent, "project.delete", json!({ "project": "OLD" }))
            .code,
        ErrorCode::Forbidden
    );

    let out = ws.as_(
        &sure,
        "project.delete",
        json!({ "project": "OLD", "expected_task_count": 2 }),
    );
    assert_eq!(out["projects"], json!(["OLD", "SUB"]));
    assert_eq!(out["tasks"], 2);
    assert_eq!(out["comments"], 1);
    assert_eq!(out["files"], 2);
    assert_eq!(
        ws.fails(&me(), "project.get", json!({ "project": "SUB" }))
            .code,
        ErrorCode::NotFound
    );
    // The other project's task lost its link and kept its file.
    let kept = ws.task_with(&keep, &["links"]);
    assert_eq!(kept["blocked"], false);
    assert_eq!(kept["links"], json!([]));
    assert_eq!(kept["file_count"], 1);
    assert_eq!(stored_files(&ws.layout), 1);
    // History keeps that it happened, not what was in it.
    let feed = ws.call("history.feed", json!({ "limit": 200 }));
    let text = feed.to_string();
    assert!(!text.contains("Secret title"));
    assert_eq!(feed["items"][0]["op"], "project.delete");
    let revert = ws.call("history.revert", json!({ "groups": [earlier["group"]] }));
    assert_eq!(revert["reverted"], json!([]));
    assert_eq!(revert["skipped"].as_array().unwrap().len(), 1);
    // The key is free again.
    ws.call("project.create", json!({ "name": "Again", "key": "OLD" }));
}

#[test]
fn the_brief_is_the_description_until_the_wiki_has_one() {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "T", "key": "TM" }));
    let brief = ws.call("project.brief", json!({ "project": "TM" }));
    assert_eq!(brief["source"], "none");
    assert_eq!(brief["body"], "");
    ws.call(
        "project.update",
        json!({ "project": "TM", "description": "Branches are named after the task key." }),
    );
    let brief = ws.call("project.brief", json!({ "project": "tm" }));
    assert_eq!(brief["source"], "description");
    assert_eq!(brief["project"], "TM");
    assert_eq!(brief["body"], "Branches are named after the task key.");
}
