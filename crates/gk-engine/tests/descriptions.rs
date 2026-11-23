// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Descriptions over a real workspace: revisions, merging concurrent edits
//! (in one process and across two), conflict copies and their resolution,
//! targeted edits, checklists, progress and totals, the reader's HTML, and
//! the search text.
//!
//! The two-process tests start this test binary again with an environment
//! variable that makes `child_editor` save one description edit as an
//! agent, the way `glyphkeel mcp` would, while this process holds the base
//! revision it read before.

#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

const START: i64 = 1_795_000_000_000;
const CHILD: &str = "GK_ENGINE_DESCRIPTION_CHILD";

struct Ws {
    engine: Engine,
    layout: WorkspaceLayout,
}

fn open(dir: &TempDir) -> Ws {
    let clock = Arc::new(AtomicI64::new(START));
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || clock.fetch_add(1, Ordering::SeqCst));
    open_with(dir, config)
}

/// A workspace on the real clock, as the other process uses.
fn open_real(dir: &TempDir) -> Ws {
    open_with(dir, EngineConfig::new(Role::App))
}

fn open_with(dir: &TempDir, config: EngineConfig) -> Ws {
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    Ws {
        engine: Engine::open(&layout, config).unwrap(),
        layout,
    }
}

fn me() -> Context {
    Context::user(Via::App)
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

    fn agent(&self, name: &str) -> Context {
        agent_of(&self.engine, name)
    }

    fn create(&self, title: &str, description: &str) -> String {
        self.call(
            "task.create",
            json!({ "project": "TM", "title": title, "description": description }),
        )["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn revision(&self, key: &str) -> String {
        self.task(key)["description_revision"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn kinds(&self, key: &str) -> Vec<String> {
        self.call("description.history", json!({ "task": key }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn agent_of(engine: &Engine, name: &str) -> Context {
    let session = engine
        .start_session(&SessionStart {
            name: name.into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    Context::agent(name, session, Profile::Standard)
}

fn workspace() -> (TempDir, Ws) {
    let dir = tempfile::tempdir().unwrap();
    let ws = open(&dir);
    ws.call("project.create", json!({ "name": "Tasks", "key": "TM" }));
    (dir, ws)
}

fn real_workspace() -> (TempDir, Ws) {
    let dir = tempfile::tempdir().unwrap();
    let ws = open_real(&dir);
    ws.call("project.create", json!({ "name": "Tasks", "key": "TM" }));
    (dir, ws)
}

const BASE: &str = "# Release\r\n\r\nFirst paragraph.\r\n\r\nSecond paragraph.\r\n\r\n- [ ] notes\r\n- [ ] tag\r\n";

#[test]
fn every_save_is_a_revision_with_a_base() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", BASE);
    let first = ws.revision(&key);
    assert_eq!(ws.task(&key)["conflict_copies"], 0);

    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": "New text\n", "base_revision": first }),
    );
    assert_eq!(out["description_save"]["outcome"], "saved");
    let second = out["description_save"]["revision"].as_str().unwrap();
    assert_eq!(out["task"]["description_revision"], second);

    let history = ws.call("description.history", json!({ "task": key }));
    assert_eq!(history["total"], 2);
    assert_eq!(history["items"][0]["id"], second);
    assert_eq!(history["items"][0]["current"], true);
    assert_eq!(history["items"][0]["base"], first.as_str());
    assert_eq!(history["items"][1]["current"], false);

    let detail = ws.call(
        "description.revision",
        json!({ "revision": second, "diff_from": first }),
    );
    assert_eq!(detail["owner"], key.as_str());
    assert_eq!(detail["body"], "New text\n");
    assert!(detail["diff"].as_str().unwrap().contains("+New text\n"));

    // Saving the same text again changes nothing.
    let again = ws.call(
        "task.update",
        json!({ "key": key, "description": "New text\n", "base_revision": second }),
    );
    assert_eq!(again["description_save"]["outcome"], "unchanged");
    assert!(again.get("group").is_none());

    // History points at revisions, not at the text.
    let feed = ws.call("history.feed", json!({ "limit": 1 }));
    let change = &feed["items"][0]["fields"]["description"];
    assert_eq!(change["after"]["revision"], second);
    assert_eq!(change["before"]["revision"], first.as_str());

    // A base from another task is refused.
    let other = ws.create("Other", "x");
    let err = ws.fails(
        &me(),
        "task.update",
        json!({ "key": other, "description": "y", "base_revision": first }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn edits_of_different_paragraphs_merge() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", BASE);
    let base = ws.revision(&key);
    let agent = ws.agent("Agent");
    ws.as_(
        &agent,
        "task.update",
        json!({ "key": key, "description_edits": [
            { "old_text": "Second paragraph.", "new_text": "Second paragraph, by the agent." }
        ] }),
    );
    let mine = BASE.replace("First paragraph.", "First paragraph, by me.");
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": mine, "base_revision": base }),
    );
    assert_eq!(out["description_save"]["outcome"], "merged");
    assert_eq!(
        out["task"]["description"],
        BASE.replace("First paragraph.", "First paragraph, by me.")
            .replace("Second paragraph.", "Second paragraph, by the agent.")
    );
    assert_eq!(ws.kinds(&key), ["merged", "normal", "normal"]);
}

#[test]
fn edits_of_the_same_lines_keep_a_conflict_copy_until_resolved() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", BASE);
    let base = ws.revision(&key);
    let agent = ws.agent("Agent");
    let theirs = BASE.replace("First paragraph.", "First, as the agent wrote it.");
    ws.as_(
        &agent,
        "task.update",
        json!({ "key": key, "description": theirs, "base_revision": base }),
    );
    let mine = BASE.replace("First paragraph.", "First, as I wrote it.");
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": mine, "base_revision": base, "priority": "high" }),
    );
    assert_eq!(out["description_save"]["outcome"], "conflict");
    assert_eq!(out["task"]["description"], theirs);
    assert_eq!(out["task"]["priority"], "high");
    assert_eq!(out["task"]["conflict_copies"], 1);
    assert_eq!(out["warnings"].as_array().unwrap().len(), 1);
    let copy = out["description_save"]["conflict_copy"]
        .as_str()
        .unwrap()
        .to_owned();
    let detail = ws.call("description.revision", json!({ "revision": copy }));
    assert_eq!(detail["body"], mine);
    assert_eq!(detail["revision"]["kind"], "conflict");
    assert!(detail["revision"].get("resolved_at").is_none());

    // Resolving against a stale base is refused.
    let err = ws.fails(
        &me(),
        "task.resolve_conflict",
        json!({ "key": key, "copy": copy, "action": "use_copy", "base_revision": base }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);

    let done = ws.call(
        "task.resolve_conflict",
        json!({ "key": key, "copy": copy, "action": "use_copy" }),
    );
    assert_eq!(done["task"]["description"], mine);
    assert_eq!(done["task"]["conflict_copies"], 0);
    assert_eq!(done["description_save"]["outcome"], "saved");
    assert_eq!(ws.kinds(&key)[0], "resolved");
    let err = ws.fails(
        &me(),
        "task.resolve_conflict",
        json!({ "key": key, "copy": copy, "action": "keep_current" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
}

#[test]
fn a_conflict_can_be_resolved_through_update_with_a_combined_text() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", "one\n");
    let base = ws.revision(&key);
    ws.call(
        "task.update",
        json!({ "key": key, "description": "one!\n", "base_revision": base }),
    );
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": "one?\n", "base_revision": base }),
    );
    let copy = out["description_save"]["conflict_copy"].as_str().unwrap();
    let agent = ws.agent("Agent");
    let done = ws.as_(
        &agent,
        "task.update",
        json!({ "key": key, "resolve_conflict": {
            "copy": copy, "action": "use_text", "text": "one?!\n" } }),
    );
    assert_eq!(done["task"]["description"], "one?!\n");
    assert_eq!(done["task"]["conflict_copies"], 0);
}

#[test]
fn targeted_edits_need_text_found_exactly_once() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", "alpha beta alpha\n");
    let err = ws.fails(
        &me(),
        "task.update",
        json!({ "key": key, "description_edits": [{ "old_text": "alpha", "new_text": "a" }] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["path"], "description_edits[0].old_text");
    let err = ws.fails(
        &me(),
        "task.update",
        json!({ "key": key, "description_edits": [
            { "old_text": "beta", "new_text": "b" }, { "old_text": "gamma", "new_text": "c" }
        ] }),
    );
    assert_eq!(err.details["path"], "description_edits[1].old_text");
    assert_eq!(ws.task(&key)["description"], "alpha beta alpha\n");
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description_edits": [{ "old_text": "beta", "new_text": "b" }] }),
    );
    assert_eq!(out["task"]["description"], "alpha b alpha\n");
}

#[test]
fn checklist_ticks_change_one_byte_and_refuse_stale_items() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", BASE);
    let task = ws.task_with(&key, &["checklist"]);
    assert_eq!(task["checklist_items"][1]["text"], "tag");
    assert_eq!(task["checklist_items"][1]["number"], 2);
    let before = task["description"].as_str().unwrap().to_owned();

    let out = ws.call(
        "checklist.update",
        json!({ "key": key, "toggle": [{ "number": 2, "text": "tag", "checked": true }] }),
    );
    assert_eq!(out["checklist"], json!({ "done": 1, "total": 2 }));
    let after = ws.task(&key)["description"].as_str().unwrap().to_owned();
    assert_eq!(before.len(), after.len());
    let differ = before
        .bytes()
        .zip(after.bytes())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differ, 1);
    assert_eq!(ws.task(&key)["progress_percent"], 50);

    // With two items of the same text, a stale number is refused.
    ws.call(
        "checklist.update",
        json!({ "key": key, "add": [{ "text": "notes", "after": 1 }] }),
    );
    let err = ws.fails(
        &me(),
        "checklist.update",
        json!({ "key": key, "toggle": [{ "number": 3, "text": "notes", "checked": true }] }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(
        err.message.contains("please tick it again"),
        "{}",
        err.message
    );
    ws.call(
        "checklist.update",
        json!({ "key": key, "remove": [{ "number": 2, "text": "notes" }] }),
    );
    // Somebody adds an item in front; the old number still finds "notes"
    // by its unique text.
    ws.call(
        "task.update",
        json!({ "key": key, "description_edits": [
            { "old_text": "# Release", "new_text": "- [ ] first\r\n\r\n# Release" }
        ] }),
    );
    let out = ws.call(
        "checklist.update",
        json!({ "key": key, "toggle": [{ "number": 1, "text": "notes" }] }),
    );
    assert_eq!(out["items"][1]["text"], "notes");
    assert_eq!(out["items"][1]["checked"], true);
    assert_eq!(out["checklist"], json!({ "done": 2, "total": 3 }));

    // A card shows the counts and the progress at once.
    let board = ws.call("board.get", json!({ "project": "TM" }));
    let card = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["cards"].as_array().unwrap())
        .find(|c| c["key"] == key.as_str())
        .unwrap()
        .clone();
    assert_eq!(card["checklist"], json!({ "done": 2, "total": 3 }));
    assert_eq!(card["progress_percent"], 67);
}

#[test]
fn comments_have_checklists_too() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", "");
    let c = ws.call(
        "comment.add",
        json!({ "key": key, "body": "Before merging:\n\n- [ ] review\n" }),
    );
    let id = c["comment"]["id"].as_str().unwrap();
    let out = ws.call(
        "checklist.update",
        json!({ "comment": id, "toggle": [{ "number": 1, "text": "review" }] }),
    );
    assert_eq!(out["comment"], id);
    assert_eq!(out["version"], 2);
    let task = ws.task_with(&key, &["comments"]);
    assert_eq!(
        task["comments"][0]["body"],
        "Before merging:\n\n- [x] review\n"
    );
}

#[test]
fn undo_brings_the_earlier_revision_back_as_a_new_one() {
    let (_dir, ws) = workspace();
    let key = ws.create("Release", "first\n");
    let base = ws.revision(&key);
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": "second\n", "base_revision": base }),
    );
    ws.call(
        "history.revert",
        json!({ "groups": [out["group"].as_str().unwrap()] }),
    );
    let task = ws.task(&key);
    assert_eq!(task["description"], "first\n");
    assert_ne!(task["description_revision"], base.as_str());
    assert_eq!(ws.kinds(&key), ["restored", "normal", "normal"]);
}

#[test]
fn progress_and_totals_roll_up_from_subtasks() {
    let (_dir, ws) = workspace();
    let parent = ws.create("Parent", "");
    let small = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Small", "parent": parent, "estimate": "1h" }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Big", "parent": parent, "estimate": "3h",
                "description": "- [x] a\n- [ ] b\n" }),
    );
    // 1 h at 0 %, 3 h at 50 %: 38 %.
    assert_eq!(ws.task(&parent)["progress_percent"], 38);
    ws.call("task.move", json!({ "key": small, "status": "Done" }));
    // 1 h at 100 %, 3 h at 50 %: 63 %.
    let p = ws.task(&parent);
    assert_eq!(p["progress_percent"], 63);
    assert_eq!(
        p["totals"],
        json!({ "estimate_minutes": 240, "spent_minutes": 0, "remaining_minutes": 240 })
    );
    assert!(ws.task(&small).get("totals").is_none());
    ws.call("task.update", json!({ "key": parent, "progress": 10 }));
    assert_eq!(ws.task(&parent)["progress_percent"], 10);
}

#[test]
fn the_reader_gets_sanitized_html_with_task_links() {
    let (_dir, ws) = workspace();
    let key = ws.create(
        "Release",
        "See TM-1 and TM-99.[^n]\n\n[^n]: A note.\n\n- [ ] one\n\n<script>alert(1)</script>\n",
    );
    let c = ws.call(
        "comment.add",
        json!({ "key": key, "body": "Also[^n].\n\n[^n]: Mine.\n" }),
    );
    let out = ws.call(
        "markdown.render",
        json!({ "documents": [
            { "task": key },
            { "comment": c["comment"]["id"] },
            { "source": "**bold** TM-1", "document_id": "draft" },
        ] }),
    );
    let task_html = out["items"][0]["html"].as_str().unwrap();
    assert!(task_html.contains(r#"data-ref="TM-1""#), "{task_html}");
    assert!(!task_html.contains(r#"data-ref="TM-99""#), "{task_html}");
    assert!(!task_html.contains("<script"), "{task_html}");
    assert!(task_html.contains(r#"data-item="1""#), "{task_html}");
    assert_eq!(out["items"][0]["checklist"][0]["text"], "one");
    assert_eq!(
        out["items"][0]["revision"],
        ws.task(&key)["description_revision"]
    );
    let comment_html = out["items"][1]["html"].as_str().unwrap();
    let ids = |html: &str| -> Vec<String> {
        html.split("id=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().to_owned())
            .collect()
    };
    let (a, b) = (ids(task_html), ids(comment_html));
    assert!(!a.is_empty() && !b.is_empty());
    assert!(a.iter().all(|id| !b.contains(id)), "{a:?} {b:?}");
    assert!(
        out["items"][2]["html"]
            .as_str()
            .unwrap()
            .contains("<strong>bold</strong>")
    );
    assert_eq!(out["items"][2]["document_id"], "draft");

    // Agents read Markdown; the reader's HTML is the app's.
    let agent = ws.agent("Agent");
    let err = ws.fails(
        &agent,
        "markdown.render",
        json!({ "documents": [{ "source": "x" }] }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
}

#[test]
fn search_indexes_plain_text() {
    let (_dir, ws) = workspace();
    ws.create("Release", "| a | b |\n|---|---|\n| **bold** | `code` |\n");
    let body: String = ws
        .engine
        .store()
        .read(|tx| {
            tx.query_row(
                "SELECT body FROM search_docs WHERE kind = 'task'",
                [],
                |r| r.get(0),
            )
            .map_err(gk_store::StoreError::from)
        })
        .unwrap();
    assert!(!body.contains('|') && !body.contains("**"), "{body:?}");
    assert!(body.contains("bold") && body.contains("code"), "{body:?}");
}

#[test]
fn project_descriptions_keep_revisions() {
    let (_dir, ws) = workspace();
    ws.call(
        "project.update",
        json!({ "project": "TM", "description": "Brief one" }),
    );
    ws.call(
        "project.update",
        json!({ "project": "TM", "description": "Brief two" }),
    );
    let history = ws.call("description.history", json!({ "project": "TM" }));
    assert_eq!(history["total"], 2);
    let out = ws.call(
        "markdown.render",
        json!({ "documents": [{ "project": "TM" }] }),
    );
    assert!(
        out["items"][0]["html"]
            .as_str()
            .unwrap()
            .contains("Brief two")
    );
}

// ---------------------------------------------------------------------
// Two processes

/// The other process: an agent saving one edit. Does nothing unless started
/// by the tests below.
#[test]
fn child_editor() {
    let Ok(spec) = std::env::var(CHILD) else {
        return;
    };
    let input: Value = serde_json::from_str(&spec).unwrap();
    let layout = WorkspaceLayout::new(input["root"].as_str().unwrap());
    let engine = Engine::open(&layout, EngineConfig::new(Role::Mcp)).unwrap();
    let agent = agent_of(&engine, "Agent in another process");
    let out = engine
        .call(&agent, "task.update", input["update"].clone())
        .unwrap();
    println!("{}", out["description_save"]["outcome"]);
}

/// Runs `update` as an agent in a second process.
fn in_other_process(ws: &Ws, update: Value) {
    let exe = std::env::current_exe().unwrap();
    let spec = json!({ "root": ws.layout.root(), "update": update }).to_string();
    let out = Command::new(exe)
        .args(["--exact", "child_editor", "--nocapture", "--test-threads=1"])
        .env(CHILD, spec)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("\"saved\""), "{stdout}");
}

#[test]
fn an_edit_from_another_process_is_merged() {
    let (_dir, ws) = real_workspace();
    let key = ws.create("Release", BASE);
    let base = ws.revision(&key);
    in_other_process(
        &ws,
        json!({ "key": key, "description":
            BASE.replace("Second paragraph.", "Second, from the other process."),
            "base_revision": base }),
    );
    let mine = BASE.replace("- [ ] tag", "- [ ] tag 1.0");
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": mine, "base_revision": base }),
    );
    assert_eq!(out["description_save"]["outcome"], "merged");
    assert_eq!(
        out["task"]["description"],
        BASE.replace("Second paragraph.", "Second, from the other process.")
            .replace("- [ ] tag", "- [ ] tag 1.0")
    );
    let history = ws.call("description.history", json!({ "task": key }));
    assert_eq!(
        history["items"][1]["author"]["name"],
        "Agent in another process"
    );
}

#[test]
fn a_clashing_edit_from_another_process_leaves_a_conflict_copy() {
    let (_dir, ws) = real_workspace();
    let key = ws.create("Release", BASE);
    let base = ws.revision(&key);
    let theirs = BASE.replace("Second paragraph.", "Second, from the other process.");
    in_other_process(
        &ws,
        json!({ "key": key, "description": theirs, "base_revision": base }),
    );
    let mine = BASE.replace("Second paragraph.", "Second, from this process.");
    let out = ws.call(
        "task.update",
        json!({ "key": key, "description": mine, "base_revision": base }),
    );
    assert_eq!(out["description_save"]["outcome"], "conflict");
    assert_eq!(out["task"]["description"], theirs);
    let task = ws.task_with(&key, &["revisions"]);
    assert_eq!(task["conflict_copies"], 1);
    let copy = &task["revisions"][0];
    assert_eq!(copy["kind"], "conflict");
    assert_eq!(copy["id"], out["description_save"]["conflict_copy"]);
}
