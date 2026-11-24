// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Drafts: an editor's unsaved text, kept per person, outside history,
//! offered again after a restart, gone when the text is saved or discarded,
//! and forgotten after 30 days.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use gk_api::ops::common::{Actor, ActorKind, Via};
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

const DAY: i64 = 24 * 60 * 60 * 1000;
/// 2026-11-23T11:00:00Z.
const START: i64 = 1_795_431_600_000;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
}

fn open(dir: &TempDir, clock: &Arc<AtomicI64>) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    let time = Arc::clone(clock);
    config.clock = Arc::new(move || time.load(Ordering::SeqCst));
    Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
}

impl Ws {
    fn new(dir: &TempDir) -> Self {
        let clock = Arc::new(AtomicI64::new(START));
        let engine = open(dir, &clock);
        let ws = Self { engine, clock };
        ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
        ws.call(
            "task.create",
            json!({ "project": "TM", "title": "A", "description": "First" }),
        );
        ws.call("task.create", json!({ "project": "TM", "title": "B" }));
        ws
    }

    fn call(&self, op: &str, input: Value) -> Value {
        self.call_as(&me(), op, input)
    }

    fn call_as(&self, ctx: &Context, op: &str, input: Value) -> Value {
        self.engine
            .call(ctx, op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn fails(&self, ctx: &Context, op: &str, input: Value) -> OpError {
        self.engine.call(ctx, op, input).unwrap_err()
    }

    fn later(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn draft(&self, ctx: &Context, kind: &str, target: &str) -> Value {
        self.call_as(ctx, "draft.get", json!({ "kind": kind, "target": target }))["draft"].clone()
    }

    fn history_total(&self) -> u64 {
        self.call("history.feed", json!({ "limit": 200 }))["total"]
            .as_u64()
            .unwrap()
    }
}

fn me() -> Context {
    Context::user(Via::App)
}

/// Another person using the same workspace.
fn robin() -> Context {
    let mut ctx = Context::user(Via::App);
    ctx.actor = Actor {
        kind: ActorKind::User,
        name: "Robin".to_owned(),
    };
    ctx
}

#[test]
fn a_draft_is_kept_per_editor_outside_history_and_without_events() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    let before = ws.history_total();
    let got = ws.call("task.get", json!({ "keys": ["TM-1"] }));
    let task = &got["items"][0];
    let revision = task["description_revision"].as_str().unwrap();
    let events = ws.engine.subscribe();

    let saved = ws.call(
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": "First, then more",
                "base_revision": revision }),
    );
    assert_eq!(saved["draft"]["body"], "First, then more");
    ws.call(
        "draft.save",
        json!({ "kind": "new_comment", "target": "TM-1", "body": "A reply" }),
    );
    let again = ws.call(
        "draft.save",
        json!({ "kind": "new_comment", "target": "TM-1", "body": "A longer reply" }),
    );
    assert_eq!(again["draft"]["body"], "A longer reply");

    assert!(
        events.recv_timeout(Duration::from_millis(200)).is_err(),
        "drafts send no change events"
    );
    assert_eq!(ws.history_total(), before, "drafts are not history");

    let description = ws.draft(&me(), "task", "TM-1");
    assert_eq!(description["body"], "First, then more");
    assert_eq!(description["base_revision"], revision);
    assert_eq!(description["task"], "TM-1");
    assert_eq!(description["target"], task["id"], "kept by the task's ID");
    assert_eq!(
        ws.draft(&me(), "new_comment", "TM-1")["body"],
        "A longer reply"
    );
    assert_eq!(ws.draft(&me(), "task", "TM-2"), Value::Null);

    let listed = ws.call("draft.list", json!({ "task": "TM-1" }));
    assert_eq!(listed["items"].as_array().unwrap().len(), 2);
    assert_eq!(listed["items"][0]["kind"], "new_comment", "newest first");
    let none = ws.call("draft.list", json!({ "task": "TM-2" }));
    assert_eq!(none["items"], json!([]));
}

#[test]
fn each_person_has_their_own_drafts() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    ws.call(
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": "Mine" }),
    );
    ws.call_as(
        &robin(),
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": "Robin's" }),
    );
    assert_eq!(ws.draft(&me(), "task", "TM-1")["body"], "Mine");
    assert_eq!(ws.draft(&robin(), "task", "TM-1")["body"], "Robin's");

    // Saving the description clears only the saver's draft.
    ws.call_as(
        &robin(),
        "task.update",
        json!({ "key": "TM-1", "description": "Robin's" }),
    );
    assert_eq!(ws.draft(&robin(), "task", "TM-1"), Value::Null);
    assert_eq!(ws.draft(&me(), "task", "TM-1")["body"], "Mine");
    let mine = ws.call("draft.list", json!({}));
    assert_eq!(mine["items"].as_array().unwrap().len(), 1);
}

#[test]
fn agents_and_the_command_line_cannot_reach_drafts() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    let session = ws
        .engine
        .start_session(&SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Full,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, Profile::Full);
    let cli = Context::user(Via::Cli);
    let calls = [
        ("draft.get", json!({ "kind": "task", "target": "TM-1" })),
        ("draft.list", json!({})),
        (
            "draft.save",
            json!({ "kind": "task", "target": "TM-1", "body": "x" }),
        ),
        ("draft.discard", json!({ "kind": "task", "target": "TM-1" })),
    ];
    for (op, input) in calls {
        for ctx in [&agent, &cli] {
            let err = ws.fails(ctx, op, input.clone());
            assert_eq!(err.code, ErrorCode::Forbidden, "{op}: {err:?}");
        }
    }
}

#[test]
fn drafts_survive_a_restart() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    let comment = ws.call("comment.add", json!({ "key": "TM-1", "body": "Hello" }));
    let id = comment["comment"]["id"].as_str().unwrap().to_owned();
    ws.call(
        "draft.save",
        json!({ "kind": "comment", "target": id, "body": "Hello, edited" }),
    );
    ws.call(
        "draft.save",
        json!({ "kind": "project", "target": "TM", "body": "About the team" }),
    );
    let clock = Arc::clone(&ws.clock);
    ws.engine.close();

    let engine = open(&dir, &clock);
    let ws = Ws { engine, clock };
    let kept = ws.draft(&me(), "comment", &id);
    assert_eq!(kept["body"], "Hello, edited");
    assert_eq!(kept["task"], "TM-1");
    assert_eq!(ws.draft(&me(), "project", "TM")["body"], "About the team");
    let listed = ws.call("draft.list", json!({ "task": "TM-1" }));
    assert_eq!(listed["items"][0]["target"], id.as_str());
}

#[test]
fn saving_or_discarding_the_text_removes_its_draft() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    let save = |kind: &str, target: &str| {
        ws.call(
            "draft.save",
            json!({ "kind": kind, "target": target, "body": "unsaved" }),
        );
    };

    save("task", "TM-1");
    ws.call(
        "task.update",
        json!({ "key": "TM-1", "description": "Saved" }),
    );
    assert_eq!(ws.draft(&me(), "task", "TM-1"), Value::Null);

    save("new_comment", "TM-1");
    let comment = ws.call("comment.add", json!({ "key": "TM-1", "body": "Sent" }));
    assert_eq!(ws.draft(&me(), "new_comment", "TM-1"), Value::Null);

    let id = comment["comment"]["id"].as_str().unwrap();
    save("comment", id);
    ws.call(
        "comment.update",
        json!({ "comment": id, "body": "Sent, edited" }),
    );
    assert_eq!(ws.draft(&me(), "comment", id), Value::Null);

    save("project", "TM");
    ws.call(
        "project.update",
        json!({ "project": "TM", "description": "About" }),
    );
    assert_eq!(ws.draft(&me(), "project", "TM"), Value::Null);

    save("task", "TM-2");
    let discarded = ws.call("draft.discard", json!({ "kind": "task", "target": "TM-2" }));
    assert_eq!(discarded["discarded"], true);
    let again = ws.call("draft.discard", json!({ "kind": "task", "target": "TM-2" }));
    assert_eq!(again["discarded"], false);

    // An empty text is no draft.
    save("new_comment", "TM-2");
    let emptied = ws.call(
        "draft.save",
        json!({ "kind": "new_comment", "target": "TM-2", "body": "" }),
    );
    assert_eq!(emptied.get("draft"), None);
    assert_eq!(ws.draft(&me(), "new_comment", "TM-2"), Value::Null);
}

#[test]
fn a_wiki_page_keeps_its_unsaved_title_and_text() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    ws.call(
        "project.update",
        json!({ "project": "TM", "modules": { "wiki": true } }),
    );
    ws.call(
        "draft.save",
        json!({ "kind": "new_wiki_page", "target": "TM", "title": "Plan", "body": "Steps" }),
    );
    let kept = ws.draft(&me(), "new_wiki_page", "TM");
    assert_eq!(
        (kept["title"].as_str(), kept["body"].as_str()),
        (Some("Plan"), Some("Steps"))
    );
    let page = ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Plan", "body": "Steps" }),
    );
    assert_eq!(ws.draft(&me(), "new_wiki_page", "TM"), Value::Null);

    let id = page["page"]["id"].as_str().unwrap();
    ws.call(
        "draft.save",
        json!({ "kind": "wiki_page", "target": id, "body": "Steps and more" }),
    );
    ws.call(
        "wiki.update",
        json!({ "page": id, "body": "Steps and more" }),
    );
    assert_eq!(ws.draft(&me(), "wiki_page", id), Value::Null);
}

#[test]
fn drafts_expire_after_thirty_days() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    ws.call(
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": "Old" }),
    );
    ws.later(29 * DAY);
    ws.call(
        "draft.save",
        json!({ "kind": "task", "target": "TM-2", "body": "Newer" }),
    );
    assert_eq!(ws.draft(&me(), "task", "TM-1")["body"], "Old");

    ws.later(2 * DAY);
    assert_eq!(ws.draft(&me(), "task", "TM-1"), Value::Null, "expired");
    assert_eq!(ws.draft(&me(), "task", "TM-2")["body"], "Newer");
    // The next save deletes expired drafts for good.
    ws.call(
        "draft.save",
        json!({ "kind": "new_comment", "target": "TM-2", "body": "x" }),
    );
    let rows: i64 = ws
        .engine
        .store()
        .read(|tx| {
            tx.query_row("SELECT count(*) FROM drafts", [], |r| r.get(0))
                .map_err(gk_store::StoreError::from)
        })
        .unwrap();
    assert_eq!(rows, 2);
}

#[test]
fn drafts_are_checked() {
    let dir = TempDir::new().unwrap();
    let ws = Ws::new(&dir);
    let err = ws.fails(
        &me(),
        "draft.save",
        json!({ "kind": "task", "target": "TM-9", "body": "x" }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = ws.fails(
        &me(),
        "draft.save",
        json!({ "kind": "comment", "target": "TM-1", "body": "x" }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = ws.fails(
        &me(),
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": "x", "base_revision": "nope" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let long = "a".repeat(gk_core::text::MARKDOWN_MAX_BYTES + 1);
    let err = ws.fails(
        &me(),
        "draft.save",
        json!({ "kind": "task", "target": "TM-1", "body": long }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = ws.fails(
        &me(),
        "draft.save",
        json!({ "kind": "new_wiki_page", "target": "TM", "body": "x", "title": "t".repeat(501) }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
}
