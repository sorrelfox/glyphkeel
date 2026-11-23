// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki pages over a real workspace: names unique ignoring case, the tree,
//! finding pages and headings, revisions with merges and conflict copies
//! (in one process and across two), history, Undo and revert, backlinks.
//!
//! The two-process test starts this test binary again with an environment
//! variable that makes `child_editor` save one page edit as an agent, the
//! way `glyphkeel mcp` would, while this process holds the base revision it
//! read before.

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
const CHILD: &str = "GK_ENGINE_WIKI_CHILD";
const BASE: &str = "# Release plan\n\nFirst paragraph.\n\n## Dates\n\nSecond paragraph.\n";

struct Ws {
    engine: Engine,
    layout: WorkspaceLayout,
    _dir: TempDir,
}

fn open_with(config: EngineConfig) -> Ws {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    let engine = Engine::open(&layout, config).unwrap();
    engine
        .call(
            &me(),
            "project.create",
            json!({ "name": "Task Manager", "key": "TM" }),
        )
        .unwrap();
    Ws {
        engine,
        layout,
        _dir: dir,
    }
}

fn open() -> Ws {
    let clock = Arc::new(AtomicI64::new(START));
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || clock.fetch_add(1, Ordering::SeqCst));
    open_with(config)
}

fn me() -> Context {
    Context::user(Via::App)
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

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&me(), op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn fails(&self, op: &str, input: Value) -> OpError {
        self.engine.call(&me(), op, input).expect_err(op)
    }

    fn page(&self, title: &str, body: &str) -> Value {
        self.call(
            "wiki.create",
            json!({ "project": "TM", "title": title, "body": body }),
        )["page"]
            .clone()
    }

    fn child(&self, title: &str, parent: &str) -> Value {
        self.call(
            "wiki.create",
            json!({ "project": "TM", "title": title, "parent": parent }),
        )["page"]
            .clone()
    }

    fn get(&self, page: &str) -> Value {
        self.call("wiki.get", json!({ "project": "TM", "page": page }))
    }

    fn titles(&self) -> Vec<String> {
        self.call("wiki.list", json!({ "project": "TM" }))["items"]
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

    fn search(&self, text: &str) -> Value {
        self.call("search.query", json!({ "text": text, "kinds": ["wiki"] }))
    }
}

#[test]
fn titles_are_unique_ignoring_case_and_keep_link_characters_out() {
    let ws = open();
    let setup = ws.page("Setup", "");
    assert_eq!(setup["slug"], "setup");
    let taken = ws.fails(
        "wiki.create",
        json!({ "project": "TM", "title": " SETUP " }),
    );
    assert_eq!(taken.code, ErrorCode::Conflict);
    assert_eq!(taken.details["message_id"], "wiki-title-taken");
    assert_eq!(taken.details["args"]["title"], "Setup");
    for (title, id) in [
        ("C# notes", "wiki-title-reserved"),
        ("[[x]]", "wiki-title-reserved"),
        ("a|b", "wiki-title-reserved"),
        ("  ", "wiki-title-empty"),
    ] {
        let e = ws.fails("wiki.create", json!({ "project": "TM", "title": title }));
        assert_eq!(e.code, ErrorCode::Invalid, "{title}");
        assert_eq!(e.details["message_id"], id, "{title}");
    }
    let other = ws.page("Other", "");
    let renamed = ws.fails(
        "wiki.update",
        json!({ "project": "TM", "page": other["id"], "title": "setup" }),
    );
    assert_eq!(renamed.code, ErrorCode::Conflict);
    // Another project may use the same title.
    ws.call("project.create", json!({ "name": "Lab", "key": "LAB" }));
    ws.call("wiki.create", json!({ "project": "LAB", "title": "Setup" }));
}

#[test]
fn pages_form_a_tree_in_order() {
    let ws = open();
    ws.page("Guide", "");
    ws.child("Install", "guide");
    ws.child("Configure", "Guide");
    ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Overview", "parent": "Guide", "at": "top" }),
    );
    ws.child("Linux", "Install");
    ws.page("FAQ", "");
    assert_eq!(
        ws.titles(),
        [
            "Guide",
            "  Overview",
            "  Install",
            "    Linux",
            "  Configure",
            "FAQ"
        ]
    );
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Configure", "before": "Install" }),
    );
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Linux", "clear": ["parent"], "at": "top" }),
    );
    assert_eq!(
        ws.titles(),
        [
            "Linux",
            "Guide",
            "  Overview",
            "  Configure",
            "  Install",
            "FAQ"
        ]
    );
    let page = ws.get("Install");
    assert_eq!(page["parent"]["title"], "Guide");
    assert_eq!(page["path"][0]["title"], "Guide");
    let guide = ws.get("guide");
    let children: Vec<&str> = guide["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(children, ["Overview", "Configure", "Install"]);
    let looped = ws.fails(
        "wiki.update",
        json!({ "project": "TM", "page": "Guide", "parent": "Install" }),
    );
    assert_eq!(looped.details["message_id"], "wiki-parent-loop");
    let mut parent = "Install".to_owned();
    for depth in 2..=8 {
        let title = format!("Level {depth}");
        ws.child(&title, &parent);
        parent = title;
    }
    let deep = ws.fails(
        "wiki.create",
        json!({ "project": "TM", "title": "Too deep", "parent": parent }),
    );
    assert_eq!(deep.details["message_id"], "wiki-too-deep");
}

#[test]
fn pages_are_found_by_title_slug_old_slug_or_id() {
    let ws = open();
    let page = ws.page("Release Plan", BASE);
    let id = page["id"].as_str().unwrap();
    assert_eq!(ws.get("release plan")["id"], id);
    assert_eq!(ws.get("release-plan")["id"], id);
    assert_eq!(
        ws.call("wiki.get", json!({ "page": id }))["title"],
        "Release Plan"
    );
    let headings: Vec<(u64, &str)> = page["headings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| (h["level"].as_u64().unwrap(), h["text"].as_str().unwrap()))
        .collect();
    assert_eq!(headings, [(1, "Release plan"), (2, "Dates")]);
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Release Plan", "title": "Roadmap" }),
    );
    assert_eq!(ws.get("release-plan")["title"], "Roadmap");
    assert_eq!(ws.get("roadmap")["slug"], "roadmap");
    let missing = ws.fails("wiki.get", json!({ "project": "TM", "page": "Nope" }));
    assert_eq!(missing.code, ErrorCode::NotFound);
    assert_eq!(missing.details["message_id"], "wiki-page-not-found");
    assert_eq!(missing.details["choices"], json!(["Roadmap"]));
    let needs = ws.fails("wiki.get", json!({ "page": "Roadmap" }));
    assert_eq!(needs.details["message_id"], "wiki-project-needed");
    // A new page of the old name takes the old address back.
    let again = ws.page("Release plan", "");
    assert_eq!(again["slug"], "release-plan");
    assert_eq!(ws.get("release-plan")["id"], again["id"]);
}

#[test]
fn saves_merge_or_leave_conflict_copies_like_descriptions() {
    let ws = open();
    let base = ws.page("Plan", BASE)["revision"].clone();
    let agent = agent_of(&ws.engine, "Writer");
    let theirs = BASE.replace("First paragraph.", "First, by the agent.");
    let out = ws
        .engine
        .call(
            &agent,
            "wiki.update",
            json!({ "project": "TM", "page": "Plan", "body": theirs,
                    "base_revision": base, "message": "Agent's edit" }),
        )
        .unwrap();
    assert_eq!(out["save"]["outcome"], "saved");
    let mine = BASE.replace("Second paragraph.", "Second, by me.");
    let merged = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan", "body": mine, "base_revision": base }),
    );
    assert_eq!(merged["save"]["outcome"], "merged");
    assert_eq!(
        merged["page"]["body"],
        BASE.replace("First paragraph.", "First, by the agent.")
            .replace("Second paragraph.", "Second, by me.")
    );
    let clash = BASE.replace("First paragraph.", "First, by me.");
    let conflict = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan", "body": clash, "base_revision": base }),
    );
    assert_eq!(conflict["save"]["outcome"], "conflict");
    assert_eq!(conflict["page"]["conflict_copies"], 1);
    let copy = conflict["save"]["conflict_copy"].as_str().unwrap();
    let history = ws.call("wiki.history", json!({ "project": "TM", "page": "Plan" }));
    let kinds: Vec<&str> = history["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["conflict", "merged", "normal", "normal"]);
    assert_eq!(history["items"][2]["message"], "Agent's edit");
    assert_eq!(history["items"][2]["author"]["name"], "Writer");
    let old = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Plan", "revision": copy }),
    );
    assert_eq!(old["body"], clash);
    assert_eq!(old["current"], false);
    let resolved = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan",
                "resolve_conflict": { "copy": copy, "action": "use_copy" } }),
    );
    assert_eq!(resolved["page"]["body"], clash);
    assert_eq!(resolved["page"]["conflict_copies"], 0);
    let edited = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan",
                "edits": [{ "old_text": "## Dates", "new_text": "## Schedule" }] }),
    );
    assert_eq!(edited["page"]["headings"][1]["text"], "Schedule");
    let bad = ws.fails(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan",
                "edits": [{ "old_text": "nowhere", "new_text": "x" }] }),
    );
    assert_eq!(bad.details["path"], "edits[0].old_text");
}

#[test]
fn undo_and_revert_take_changes_back() {
    let ws = open();
    let created = ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Notes", "body": "Alpha zebra.\n" }),
    );
    let updated = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Notes", "title": "Journal", "body": "Beta.\n" }),
    );
    assert_eq!(ws.search("zebra")["total"], 0);
    assert_eq!(ws.search("journal")["total"], 1);
    ws.call("history.undo", json!({}));
    let page = ws.get("Notes");
    assert_eq!(page["body"], "Alpha zebra.\n");
    assert_eq!(page["slug"], "notes");
    assert_eq!(ws.search("zebra")["total"], 1);
    assert_eq!(ws.search("journal")["total"], 0);
    ws.call("history.redo", json!({}));
    assert_eq!(ws.get("Journal")["body"], "Beta.\n");
    let feed = ws.call("history.feed", json!({ "group": updated["group"] }));
    let fields = &feed["items"][0]["fields"];
    assert!(
        fields.get("title").is_some() && fields.get("body").is_some(),
        "{feed}"
    );
    ws.call("history.revert", json!({ "groups": [created["group"]] }));
    assert_eq!(ws.titles(), Vec::<String>::new());
    assert_eq!(ws.search("beta")["total"], 0);
    // The name is free again.
    ws.page("Journal", "");
}

#[test]
fn search_finds_titles_then_headings_then_text() {
    let ws = open();
    ws.page("Release plan", BASE);
    ws.page(
        "Setup",
        "# Install\n\n## Release builds\n\nRun the release script.\n",
    );
    ws.page("Prerelease checks", "Nothing yet.\n");
    ws.page("Glossary", "A release is a published version.\n");
    let hits = ws.call("wiki.search", json!({ "text": "release", "project": "TM" }));
    let rows: Vec<String> = hits["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            format!(
                "{} {}{}",
                h["matched_in"].as_str().unwrap(),
                h["page"]["title"].as_str().unwrap(),
                h["heading"]["text"]
                    .as_str()
                    .map(|t| format!("#{t}"))
                    .unwrap_or_default()
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            "title Release plan",
            "title Prerelease checks",
            // Headings of the most recently changed pages first.
            "heading Setup#Release builds",
            "heading Release plan#Release plan",
            "text Setup",
            "text Glossary",
        ]
    );
    assert_eq!(
        hits["items"][0]["title_matches"],
        json!([{ "from": 0, "to": 7 }])
    );
    let headings = ws.call("wiki.search", json!({ "text": "setup#", "project": "TM" }));
    let texts: Vec<&str> = headings["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["heading"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["Install", "Release builds"]);
    let one = ws.call("wiki.search", json!({ "text": "rel pl#dat" }));
    assert_eq!(one["items"][0]["heading"]["text"], "Dates");
    assert_eq!(one["total"], 1);
    let recent = ws.call("wiki.search", json!({ "text": "", "limit": 2 }));
    assert_eq!(recent["items"][0]["page"]["title"], "Glossary");
    assert_eq!(recent["total"], 4);
}

#[test]
fn backlinks_list_pages_and_tasks_that_link_here() {
    let ws = open();
    ws.page("Release plan", BASE);
    ws.page("Index", "See [[release plan#Dates]] and [[Setup]].\n");
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Ship", "description": "Follow [[Release plan]]." }),
    );
    let page = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Release plan", "include": ["backlinks"] }),
    );
    assert_eq!(
        page["backlinks"],
        json!([
            { "page": { "title": "Index", "slug": "index", "id": ws.get("Index")["id"] },
              "title": "Index", "heading": "Dates" },
            { "task": "TM-1", "title": "Ship" }
        ])
    );
}

// ---------------------------------------------------------------------
// Two processes

/// The other process: an agent saving one page edit. Does nothing unless
/// started by the test below.
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
        .call(&agent, "wiki.update", input["update"].clone())
        .unwrap();
    println!("{}", out["save"]["outcome"]);
}

#[test]
fn an_edit_from_another_process_is_merged() {
    let ws = open_with(EngineConfig::new(Role::App));
    let base = ws.page("Plan", BASE)["revision"].clone();
    let theirs = BASE.replace("Second paragraph.", "Second, from the other process.");
    let exe = std::env::current_exe().unwrap();
    let spec = json!({ "root": ws.layout.root(), "update":
        { "project": "TM", "page": "plan", "body": theirs, "base_revision": base } })
    .to_string();
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

    let mine = BASE.replace("First paragraph.", "First, from this process.");
    let out = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Plan", "body": mine, "base_revision": base }),
    );
    assert_eq!(out["save"]["outcome"], "merged");
    assert_eq!(
        out["page"]["body"],
        BASE.replace("First paragraph.", "First, from this process.")
            .replace("Second paragraph.", "Second, from the other process.")
    );
    let history = ws.call("wiki.history", json!({ "project": "TM", "page": "Plan" }));
    assert_eq!(
        history["items"][1]["author"]["name"],
        "Agent in another process"
    );
    let found = ws.call("search.query", json!({ "text": "other process" }));
    assert_eq!(found["items"][0]["kind"], "wiki");
}
