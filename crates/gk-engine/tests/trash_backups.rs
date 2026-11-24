// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The trash and backups over a real workspace: every kind of item goes to
//! the trash and comes back, agents' limits, the purge of expired items,
//! and backups made, rotated, checked and restored.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::backup::BackupKind;
use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

const START: i64 = 1_795_000_000_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
    layout: WorkspaceLayout,
}

fn open(dir: &TempDir) -> Ws {
    open_as(dir, Role::App, Arc::new(AtomicI64::new(START)))
}

fn open_as(dir: &TempDir, role: Role, clock: Arc<AtomicI64>) -> Ws {
    let tick = Arc::clone(&clock);
    let mut config = EngineConfig::new(role);
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

fn sure() -> Context {
    Context::user(Via::Cli).with_confirmation(Confirmation::Given)
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

    fn new_task(&self, project: &str, title: &str) -> String {
        self.call("task.create", json!({ "project": project, "title": title }))["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn trash(&self) -> Value {
        self.call("trash.list", json!({}))
    }

    /// The trash entry of `kind`, its ID.
    fn entry(&self, kind: &str) -> String {
        let trash = self.trash();
        trash["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} in {trash}"))["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn restore(&self, id: &str) -> Value {
        self.call("trash.restore", json!({ "items": [id] }))
    }

    fn count(&self, sql: &str) -> i64 {
        self.engine
            .store()
            .read(|tx| {
                Ok::<_, gk_store::StoreError>(tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)
            })
            .unwrap()
    }
}

fn setup(dir: &TempDir) -> Ws {
    let ws = open(dir);
    ws.call("project.create", json!({ "name": "Tasks", "key": "TM" }));
    ws
}

// ---------------------------------------------------------------------
// The trash, kind by kind

#[test]
fn a_task_goes_to_the_trash_with_its_subtasks_and_comes_back() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let parent = ws.new_task("TM", "Parent");
    let child = ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Child", "parent": parent }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let out = ws.call("task.delete", json!({ "keys": [parent, child] }));
    assert_eq!(out["total"], 2, "{out}");
    assert_eq!(out["items"].as_array().unwrap().len(), 1, "{out}");
    assert_eq!(
        ws.fails(&me(), "task.get", json!({ "keys": [child] })).code,
        ErrorCode::NotFound
    );
    let trash = ws.trash();
    assert_eq!(trash["total"], 1);
    assert_eq!(trash["items"][0]["key"], parent.as_str());
    assert_eq!(trash["items"][0]["contains"], 1);
    assert_eq!(trash["items"][0]["deleted_by"]["name"], "You");
    ws.restore(&ws.entry("task"));
    let back = ws.call("task.get", json!({ "keys": [parent, child] }));
    assert_eq!(back["total"], 2);
    assert_eq!(ws.trash()["total"], 0);
    // An entry that isn't in the trash.
    let err = ws.fails(
        &me(),
        "trash.restore",
        json!({ "items": [gk_core::Id::generate().to_string()] }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.details["message_id"], "trash-item-not-found");
    assert_eq!(err.details["hint_id"], "trash-item-not-found-hint");
}

#[test]
fn comments_files_and_time_entries_come_back_one_by_one() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Holder");
    let comment = ws.call("comment.add", json!({ "key": task, "body": "Hello" }))["comment"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let file = ws.call(
        "file.attach",
        json!({ "task": task, "name": "a.txt", "data_base64": gk_core::base64_encode(b"abc") }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let entry = ws.call("time.log", json!({ "key": task, "duration": "45m" }))["entry"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    ws.call("comment.delete", json!({ "comment": comment }));
    ws.call("file.delete", json!({ "attachment": file }));
    ws.call("time.delete", json!({ "entry": entry }));
    let trash = ws.trash();
    assert_eq!(trash["total"], 3, "{trash}");
    let details = ws.call(
        "task.get",
        json!({ "keys": [task], "include": ["comments", "files"] }),
    );
    assert_eq!(details["items"][0]["comments"], json!([]), "{details}");
    assert_eq!(details["items"][0]["file_count"], 0);
    assert_eq!(details["items"][0]["spent_minutes"], 0);

    for (kind, check) in [
        (
            "comment",
            "SELECT count(*) FROM comments WHERE deleted_at IS NULL",
        ),
        (
            "file",
            "SELECT count(*) FROM attachments WHERE deleted_at IS NULL",
        ),
        (
            "time_entry",
            "SELECT count(*) FROM time_entries WHERE deleted_at IS NULL",
        ),
    ] {
        assert_eq!(ws.count(check), 0, "{kind}");
        ws.restore(&ws.entry(kind));
        assert_eq!(ws.count(check), 1, "{kind}");
    }
    assert_eq!(ws.trash()["total"], 0);
    let details = ws.call("task.get", json!({ "keys": [task] }));
    assert_eq!(details["items"][0]["file_count"], 1);
    assert_eq!(details["items"][0]["spent_minutes"], 45);
}

#[test]
fn something_in_a_trashed_task_comes_back_only_with_it() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Holder");
    let comment = ws.call("comment.add", json!({ "key": task, "body": "Hello" }))["comment"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ws.call("comment.delete", json!({ "comment": comment }));
    ws.call("task.delete", json!({ "keys": [task] }));
    let err = ws.fails(&me(), "trash.restore", json!({ "items": [comment] }));
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(err.details["message_id"], "trash-restore-task-first");
    assert_eq!(err.details["hint_id"], "trash-restore-first-hint");
    ws.restore(&ws.entry("task"));
    ws.restore(&comment);
    assert_eq!(
        ws.count("SELECT count(*) FROM comments WHERE deleted_at IS NULL"),
        1
    );
}

#[test]
fn wiki_pages_go_with_the_pages_inside_them_and_keep_the_brief() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let home = ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Home", "body": "Read me first" }),
    )["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Inside", "parent": "Home" }),
    );
    // Home is the project's instructions for agents.
    ws.engine
        .store()
        .write("test.brief", |tx| {
            tx.execute(
                "UPDATE wiki_pages SET is_agent_brief = 1 WHERE title = 'Home'",
                [],
            )?;
            Ok::<_, gk_store::StoreError>(())
        })
        .unwrap();
    let brief = ws.call("project.brief", json!({ "project": "TM" }));
    assert!(brief.to_string().contains("Read me first"), "{brief}");

    let out = ws.call("wiki.delete", json!({ "project": "TM", "page": "Home" }));
    assert_eq!(out["total"], 2, "{out}");
    assert_eq!(
        ws.fails(
            &me(),
            "wiki.get",
            json!({ "project": "TM", "page": "Inside" })
        )
        .code,
        ErrorCode::NotFound
    );
    let listed = ws.call("wiki.list", json!({ "project": "TM" }));
    assert!(!listed.to_string().contains("Inside"), "{listed}");
    let brief = ws.call("project.brief", json!({ "project": "TM" }));
    assert!(!brief.to_string().contains("Read me first"), "{brief}");
    let trash = ws.trash();
    assert_eq!(trash["items"][0]["kind"], "wiki_page");
    assert_eq!(trash["items"][0]["contains"], 1);

    ws.restore(&home);
    let page = ws.call("wiki.get", json!({ "project": "TM", "page": "Inside" }));
    assert_eq!(page["title"], "Inside", "{page}");
    let home_page = ws.call("wiki.get", json!({ "project": "TM", "page": "Home" }));
    assert_eq!(home_page["agent_brief"], true, "{home_page}");
    let brief = ws.call("project.brief", json!({ "project": "TM" }));
    assert!(brief.to_string().contains("Read me first"), "{brief}");
}

#[test]
fn a_running_timer_stops_when_its_task_goes_to_the_trash() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Timed");
    let other = ws.new_task("TM", "Other");
    ws.call("timer.start", json!({ "key": task }));
    ws.advance(20 * MINUTE);
    // Deleting another task leaves it running.
    ws.call("task.delete", json!({ "keys": [other] }));
    assert!(ws.call("timer.get", json!({}))["timer"].is_object());

    let gone = ws.call("task.delete", json!({ "keys": [task] }));
    assert!(ws.call("timer.get", json!({}))["timer"].is_null());
    // Undoing the deletion brings the timer back, without the entry.
    ws.call("history.revert", json!({ "groups": [gone["group"]] }));
    let timer = ws.call("timer.get", json!({}));
    assert_eq!(timer["timer"]["task"]["key"], task.as_str(), "{timer}");
    assert_eq!(
        ws.call("task.get", json!({ "keys": [task] }))["items"][0]["spent_minutes"],
        0
    );
    // Restoring it from the trash instead: its time was logged on the task
    // and comes back with it.
    ws.call("task.delete", json!({ "keys": [task] }));
    assert!(ws.call("timer.get", json!({}))["timer"].is_null());
    ws.restore(&ws.entry("task"));
    let back = ws.call("task.get", json!({ "keys": [task] }));
    assert_eq!(back["items"][0]["spent_minutes"], 20, "{back}");
}

#[test]
fn search_leaves_out_files_of_trashed_tasks() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Holder");
    ws.call(
        "file.attach",
        json!({ "task": task, "name": "zebrafinch.txt", "data_base64": gk_core::base64_encode(b"x") }),
    );
    let found = ws.call("search.query", json!({ "text": "zebrafinch" }));
    assert_eq!(found["items"].as_array().unwrap().len(), 1, "{found}");
    ws.call("task.delete", json!({ "keys": [task] }));
    let found = ws.call("search.query", json!({ "text": "zebrafinch" }));
    assert_eq!(found["items"], json!([]), "{found}");
    ws.restore(&ws.entry("task"));
    let found = ws.call("search.query", json!({ "text": "zebrafinch" }));
    assert_eq!(found["items"].as_array().unwrap().len(), 1, "{found}");
}

// ---------------------------------------------------------------------
// Agents

#[test]
fn agents_move_a_limited_number_of_items_to_the_trash() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let keys: Vec<String> = (0..61)
        .map(|i| ws.new_task("TM", &format!("T{i}")))
        .collect();
    let agent = ws.agent("Helper", Profile::Standard);

    // At most 20 per call.
    let err = ws.fails(&agent, "task.delete", json!({ "keys": keys[..21] }));
    assert_eq!(err.code, ErrorCode::Limit, "{err:?}");
    assert_eq!(err.details["message_id"], "trash-call-limit");
    assert_eq!(err.details["hint_id"], "trash-call-limit-hint");
    assert_eq!(ws.trash()["total"], 0);

    // At most 50 an hour per session.
    ws.as_(&agent, "task.delete", json!({ "keys": keys[..20] }));
    ws.advance(10 * MINUTE);
    ws.as_(&agent, "task.delete", json!({ "keys": keys[20..40] }));
    let err = ws.fails(&agent, "task.delete", json!({ "keys": keys[40..60] }));
    assert_eq!(err.code, ErrorCode::Limit, "{err:?}");
    assert_eq!(err.details["message_id"], "trash-hour-limit");
    let wait = err.details["retry_after"].as_i64().unwrap();
    assert!((2900..=3000).contains(&wait), "{wait}");
    assert!(
        err.hint.as_deref().unwrap().contains(&wait.to_string()),
        "{:?}",
        err.hint
    );
    // Ten more fit; another session has its own allowance.
    ws.as_(&agent, "task.delete", json!({ "keys": keys[40..50] }));
    let other = ws.agent("Other", Profile::Standard);
    ws.as_(&other, "task.delete", json!({ "keys": keys[50..60] }));
    // An hour after the first call, the first twenty no longer count.
    ws.advance(51 * MINUTE);
    ws.as_(&agent, "task.delete", json!({ "keys": [keys[60]] }));
    // The person has no such limit.
    let restored: Vec<String> = ws.trash()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .take(30)
        .collect();
    ws.call("trash.restore", json!({ "items": restored }));
    assert_eq!(
        ws.count("SELECT count(*) FROM tasks WHERE deleted_at IS NULL"),
        30
    );
}

#[test]
fn deleting_projects_and_purging_need_the_full_profile() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Doomed");
    ws.call("task.delete", json!({ "keys": [task] }));
    let id = ws.entry("task");

    let standard = ws.agent("Standard", Profile::Standard);
    let err = ws.fails(&standard, "project.delete", json!({ "project": "TM" }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    let err = ws.fails(&standard, "trash.purge", json!({ "items": [id] }));
    assert_eq!(err.code, ErrorCode::Forbidden);

    let app = Context::user(Via::App);
    ws.as_(
        &app,
        "agent.policy.update",
        json!({ "max_profile": "full" }),
    );
    let full = ws.agent("Full", Profile::Full);
    let err = ws.fails(&full, "trash.purge", json!({ "all": true }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert_eq!(err.details["message_id"], "trash-purge-all-agents");
    assert_eq!(err.details["hint_id"], "trash-purge-all-agents-hint");
    assert_eq!(ws.trash()["total"], 1);
    let purged = ws.as_(&full, "trash.purge", json!({ "items": [id] }));
    assert_eq!(purged["total"], 1, "{purged}");
    let out = ws.as_(&full, "project.delete", json!({ "project": "TM" }));
    assert_eq!(out["projects"], json!(["TM"]));

    // The person empties the whole trash, with a confirmation.
    let err = ws.fails(&me(), "trash.purge", json!({ "all": true }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    let err = ws.fails(
        &sure(),
        "trash.purge",
        json!({ "all": true, "items": [id] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let purged = ws.as_(&sure(), "trash.purge", json!({ "all": true }));
    assert_eq!(purged["total"], 1, "{purged}");
    assert_eq!(ws.trash()["total"], 0);
}

// ---------------------------------------------------------------------
// Expiry

#[test]
fn items_are_purged_when_their_time_in_the_trash_is_up() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let old = ws.new_task("TM", "Old");
    ws.call(
        "file.attach",
        json!({ "task": old, "name": "only.txt", "data_base64": gk_core::base64_encode(b"only here") }),
    );
    ws.call("task.delete", json!({ "keys": [old] }));
    ws.advance(10 * DAY);
    let newer = ws.new_task("TM", "Newer");
    ws.call("task.delete", json!({ "keys": [newer] }));
    let trash = ws.trash();
    assert_eq!(trash["total"], 2);

    // Day 29 of the first: nothing goes.
    ws.advance(19 * DAY);
    let report = ws.engine.run_daily_jobs().unwrap();
    assert_eq!(report.purged, 0);
    assert!(report.backup.is_some());
    // Day 31: the first goes, with its stored file; the newer one stays.
    ws.advance(2 * DAY);
    let report = ws.engine.run_daily_jobs().unwrap();
    assert_eq!(report.purged, 1);
    let trash = ws.trash();
    assert_eq!(trash["total"], 1);
    assert_eq!(trash["items"][0]["title"], "Newer");
    assert_eq!(ws.count("SELECT count(*) FROM attachments"), 0);
    assert_eq!(ws.count("SELECT count(*) FROM blobs"), 0);
    // A shorter time applies to what is already in the trash.
    ws.call("settings.update", json!({ "trash_days": 7 }));
    let report = ws.engine.run_daily_jobs().unwrap();
    assert_eq!(report.purged, 1);
    assert_eq!(ws.trash()["total"], 0);
    // History keeps that it happened.
    let feed = ws.call("history.feed", json!({ "limit": 5 }));
    assert_eq!(feed["items"][0]["op"], "trash.purge", "{feed}");
}

// ---------------------------------------------------------------------
// Backups

#[test]
fn backups_are_made_rotated_checked_and_deleted() {
    let dir = TempDir::new().unwrap();
    let ws = setup(&dir);
    let task = ws.new_task("TM", "Kept");
    ws.call(
        "file.attach",
        json!({ "task": task, "name": "a.txt", "data_base64": gk_core::base64_encode(b"backed up") }),
    );
    let made = ws.call("backup.create", json!({}));
    assert_eq!(made["kind"], "manual");
    assert_eq!(made["files"], 1, "{made}");
    let check = ws.call("backup.verify", json!({ "name": made["name"] }));
    assert_eq!(check["ok"], true, "{check}");
    assert_eq!(check["files_checked"], 1);

    // A day at a time for three weeks: seven daily ones stay, older ones
    // become weekly ones.
    for _ in 0..21 {
        ws.engine.run_daily_jobs().unwrap();
        ws.advance(DAY);
    }
    let list = ws.call("backup.list", json!({}));
    let kinds: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "daily").count(),
        7,
        "{kinds:?}"
    );
    let weekly = kinds.iter().filter(|k| **k == "weekly").count();
    assert!((2..=4).contains(&weekly), "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == "manual").count(), 1);
    // Twice in the same day: no second daily backup.
    assert!(ws.engine.run_daily_jobs().unwrap().backup.is_some());
    assert!(ws.engine.run_daily_jobs().unwrap().backup.is_none());

    // A damaged stored file shows in the check. (Stored files may be
    // hard links of the workspace's, so the damaged one is a new file.)
    let blobs = ws.engine.backups().root().join("blobs");
    let file = walk(&blobs).into_iter().next().unwrap();
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, b"damaged").unwrap();
    let check = ws.call("backup.verify", json!({ "name": made["name"] }));
    assert_eq!(check["ok"], false, "{check}");
    assert_eq!(check["problems"][0]["kind"], "damaged_file", "{check}");
    std::fs::remove_file(&file).unwrap();
    let check = ws.call("backup.verify", json!({ "name": made["name"] }));
    assert_eq!(check["problems"][0]["kind"], "missing_file", "{check}");

    // Deleting a backup can't be undone, so the person confirms it.
    let err = ws.fails(&me(), "backup.delete", json!({ "name": made["name"] }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    let deleted = ws.as_(&sure(), "backup.delete", json!({ "name": made["name"] }));
    assert_eq!(deleted["name"], made["name"]);
    let err = ws.fails(&me(), "backup.verify", json!({ "name": made["name"] }));
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.details["hint_id"], "backup-not-found-hint");
    // Agents can't delete or restore backups.
    let agent = ws.agent("Agent", Profile::Standard);
    let name = ws.call("backup.list", json!({}))["items"][0]["name"].clone();
    for op in ["backup.delete", "backup.restore"] {
        assert_eq!(
            ws.fails(&agent, op, json!({ "name": name })).code,
            ErrorCode::Forbidden,
            "{op}"
        );
    }
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            out.extend(walk(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

#[test]
fn a_restore_puts_the_backup_back_unless_the_app_has_the_workspace() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let ws = open_as(&dir, Role::Cli, Arc::clone(&clock));
    ws.call("project.create", json!({ "name": "Tasks", "key": "TM" }));
    let task = ws.new_task("TM", "Before");
    ws.call(
        "file.attach",
        json!({ "task": task, "name": "a.txt", "data_base64": gk_core::base64_encode(b"restored") }),
    );
    let backup = ws.engine.create_backup(BackupKind::Manual).unwrap();
    ws.new_task("TM", "After");
    ws.call("task.delete", json!({ "keys": [task] }));
    ws.as_(&sure(), "trash.purge", json!({ "all": true }));

    // Not without a confirmation.
    let err = ws.fails(&me(), "backup.restore", json!({ "name": backup.name }));
    assert_eq!(err.code, ErrorCode::Forbidden);
    // Not while the app has the workspace open.
    let lock = gk_store::LockFile::try_acquire(&ws.layout.app_lock())
        .unwrap()
        .unwrap();
    let err = ws.fails(&sure(), "backup.restore", json!({ "name": backup.name }));
    assert_eq!(err.code, ErrorCode::Busy, "{err:?}");
    assert_eq!(err.details["message_id"], "backup-restore-app-open");
    assert_eq!(err.details["hint_id"], "backup-restore-app-open-hint");
    drop(lock);
    assert_eq!(ws.count("SELECT count(*) FROM tasks"), 1);

    let out = ws.as_(&sure(), "backup.restore", json!({ "name": backup.name }));
    assert_eq!(out["previous"]["kind"], "pre_restore", "{out}");
    // This engine has to go; a new one reads the backup's data.
    assert!(ws.engine.must_exit());
    assert_eq!(
        ws.fails(&me(), "workspace.get", json!({})).code,
        ErrorCode::WorkspaceUpgraded
    );
    let layout = ws.layout.clone();
    drop(ws);
    let again = open_as(&dir, Role::Cli, clock);
    assert_eq!(again.layout.root(), layout.root());
    let tasks = again.call("task.get", json!({ "keys": ["TM-1"] }));
    assert_eq!(tasks["items"][0]["title"], "Before");
    assert_eq!(again.count("SELECT count(*) FROM tasks"), 1);
    let file = again.call(
        "task.get",
        json!({ "keys": ["TM-1"], "include": ["files"] }),
    );
    let id = file["items"][0]["files"][0]["id"].clone();
    let content = again.call("file.read", json!({ "attachment": id }));
    assert_eq!(content["text"], "restored", "{content}");
    // The state before the restore was kept as a backup.
    let list = again.call("backup.list", json!({}));
    assert!(list.to_string().contains("pre_restore"), "{list}");
}
