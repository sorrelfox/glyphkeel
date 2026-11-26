// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki pages over a real workspace: names unique ignoring case, the tree,
//! finding pages and headings, revisions with merges and conflict copies
//! (in one process and across two), history, Undo and revert, links and
//! backlinks, moves.
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
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, SessionStart};
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
    ws.call(
        "comment.add",
        json!({ "key": "TM-1", "body": "Dates are in [[release-plan#Dates]]." }),
    );
    let page = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Release plan", "include": ["backlinks"] }),
    );
    let comment = ws.call(
        "task.get",
        json!({ "keys": ["TM-1"], "include": ["comments"] }),
    )["items"][0]["comments"][0]["id"]
        .clone();
    assert_eq!(
        page["backlinks"],
        json!([
            { "page": { "title": "Index", "slug": "index", "id": ws.get("Index")["id"] },
              "project": "TM", "title": "Index", "heading": "Dates" },
            { "task": "TM-1", "project": "TM", "title": "Ship" },
            { "task": "TM-1", "comment": comment, "project": "TM", "title": "Ship",
              "heading": "Dates" }
        ])
    );
    let alone = ws.call(
        "wiki.backlinks",
        json!({ "project": "TM", "page": "Release plan" }),
    );
    assert_eq!(alone["items"], page["backlinks"]);
    assert_eq!(alone["total"], 3);

    // What the index links to: one page that exists, one that doesn't yet.
    let refs = ws.call(
        "wiki.references",
        json!({ "project": "TM", "page": "Index" }),
    );
    assert_eq!(
        refs["items"],
        json!([
            { "kind": "page", "target": "release plan", "heading": "Dates", "found": true,
              "page": { "title": "Release plan", "slug": "release-plan",
                        "id": ws.get("Release plan")["id"] },
              "project": "TM", "title": "Release plan" },
            { "kind": "page", "target": "Setup", "found": false }
        ])
    );
    // A page made later is found by the links already written.
    ws.page("Setup", "");
    let setup = ws.call(
        "wiki.backlinks",
        json!({ "project": "TM", "page": "Setup" }),
    );
    assert_eq!(setup["items"][0]["title"], "Index");
}

#[test]
fn task_keys_link_pages_tasks_and_comments_to_a_task() {
    let ws = open();
    ws.call("task.create", json!({ "project": "TM", "title": "Target" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Other", "description": "Blocked by TM-1, see TM-99." }),
    );
    ws.call(
        "comment.add",
        json!({ "key": "TM-2", "body": "TM-1 first, then [[Plan]]." }),
    );
    // Mentions of a task in its own text are not backlinks.
    ws.call(
        "comment.add",
        json!({ "key": "TM-1", "body": "This is TM-1." }),
    );
    ws.page("Plan", "Work on TM-1 and `TM-2` (code doesn't count).\n");
    let back = ws.call("task.backlinks", json!({ "key": "tm-1" }));
    let kinds: Vec<(Value, Value, bool)> = back["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["title"].clone(),
                b["task"].clone(),
                b.get("comment").is_some(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (json!("Plan"), Value::Null, false),
            (json!("Other"), json!("TM-2"), false),
            (json!("Other"), json!("TM-2"), true),
        ]
    );
    // The same through task.get.
    let task = ws.call(
        "task.get",
        json!({ "keys": ["TM-1"], "include": ["backlinks"] }),
    );
    assert_eq!(task["items"][0]["backlinks"], back["items"]);
    assert!(
        ws.call("task.backlinks", json!({ "key": "TM-2" }))["items"]
            .as_array()
            .unwrap()
            .is_empty(),
        "a key in code is not a link"
    );

    let refs = ws.call("task.references", json!({ "key": "TM-2" }));
    let found: Vec<(Value, Value, bool, bool)> = refs["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["kind"].clone(),
                r["target"].clone(),
                r["found"].as_bool().unwrap(),
                r.get("comment").is_some(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (json!("task"), json!("TM-1"), true, false),
            (json!("task"), json!("TM-99"), false, false),
            (json!("page"), json!("Plan"), true, true),
            (json!("task"), json!("TM-1"), true, true),
        ]
    );
    // Edits replace a text's links.
    ws.call(
        "task.update",
        json!({ "key": "TM-2", "description": "Nothing here." }),
    );
    let back = ws.call("task.backlinks", json!({ "key": "TM-1" }));
    assert_eq!(back["total"], 2);
}

#[test]
fn links_survive_renames_moves_and_a_trip_through_the_trash() {
    let ws = open();
    ws.page("Guide", "");
    let plan = ws.page("Release plan", BASE);
    ws.page("Index", "See [[Release plan#Dates]].\n");
    let backlinks = |ws: &Ws| -> Vec<String> {
        ws.call("wiki.backlinks", json!({ "page": plan["id"] }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["title"].as_str().unwrap().to_owned())
            .collect()
    };
    let target_found = |ws: &Ws| -> bool {
        ws.call(
            "wiki.references",
            json!({ "project": "TM", "page": "Index" }),
        )["items"][0]["found"]
            .as_bool()
            .unwrap()
    };
    assert_eq!(backlinks(&ws), ["Index"]);

    // Renamed: the old slug still finds it, so the old link does too.
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Release plan", "title": "Roadmap" }),
    );
    assert_eq!(backlinks(&ws), ["Index"]);
    assert!(target_found(&ws));

    // Moved into another page: the slug stays.
    let moved = ws.call(
        "wiki.move",
        json!({ "project": "TM", "page": "Roadmap", "parent": "Guide" }),
    );
    assert_eq!(moved["page"]["parent"]["title"], "Guide");
    assert_eq!(moved["page"]["slug"], "roadmap");
    assert!(moved["group"].is_string());
    assert_eq!(backlinks(&ws), ["Index"]);

    // The linking page in the trash: no backlink until it is back.
    let index = ws.get("Index")["id"].as_str().unwrap().to_owned();
    ws.call("wiki.delete", json!({ "project": "TM", "page": "Index" }));
    assert!(backlinks(&ws).is_empty());
    ws.call("trash.restore", json!({ "items": [index] }));
    assert_eq!(backlinks(&ws), ["Index"]);

    // The target in the trash (with the page holding it): the link finds
    // nothing until it is back.
    ws.call("wiki.delete", json!({ "project": "TM", "page": "Guide" }));
    assert!(!target_found(&ws));
    let guide = ws.call("trash.list", json!({}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["title"] == "Guide")
        .unwrap()["id"]
        .clone();
    ws.call("trash.restore", json!({ "items": [guide] }));
    assert!(target_found(&ws));
    assert_eq!(backlinks(&ws), ["Index"]);

    // Undo of an edit brings its links back too.
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Index", "edits":
            [{ "old_text": "[[Release plan#Dates]]", "new_text": "nothing" }] }),
    );
    assert!(backlinks(&ws).is_empty());
    ws.call("history.undo", json!({}));
    assert_eq!(backlinks(&ws), ["Index"]);

    // Purged for good: its rows go with it.
    ws.call("wiki.delete", json!({ "project": "TM", "page": "Index" }));
    ws.engine
        .call(
            &Context::user(Via::Cli).with_confirmation(Confirmation::Given),
            "trash.purge",
            json!({ "items": [index] }),
        )
        .unwrap();
    let rows: i64 = ws
        .engine
        .store()
        .read(|tx| {
            tx.query_row(
                "SELECT count(*) FROM text_links WHERE source_kind = 'wiki'",
                [],
                |r| r.get(0),
            )
            .map_err(gk_store::StoreError::from)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn a_page_in_the_trash_keeps_its_old_address_when_a_new_page_takes_it() {
    let ws = open();
    let plan = ws.page("Release plan", BASE);
    let id = plan["id"].as_str().unwrap().to_owned();
    ws.page("Index", "See [[Release plan]].\n");
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Release plan", "title": "Roadmap" }),
    );
    ws.call("wiki.delete", json!({ "project": "TM", "page": "Roadmap" }));

    // A new page of the old name takes the address while it exists.
    let other = ws.page("Release plan", "");
    assert_eq!(other["slug"], "release-plan");
    assert_eq!(ws.get("release-plan")["id"], other["id"]);
    ws.call(
        "wiki.delete",
        json!({ "project": "TM", "page": "Release plan" }),
    );

    // Back from the trash, the renamed page answers to its old address and
    // the link written before the rename finds it again.
    ws.call("trash.restore", json!({ "items": [id] }));
    assert_eq!(ws.get("release-plan")["id"], id.as_str());
    assert_eq!(ws.get("Release plan")["title"], "Roadmap");
    let references = ws.call(
        "wiki.references",
        json!({ "project": "TM", "page": "Index" }),
    );
    assert_eq!(references["items"][0]["found"], true);
    assert_eq!(references["items"][0]["page"]["id"], id.as_str());
    let backlinks = ws.call("wiki.backlinks", json!({ "page": id }));
    assert_eq!(backlinks["items"][0]["title"], "Index");

    // A third page of that name: the title finds it, while the address
    // (still held by the page in the trash) stays with the restored page.
    let third = ws.page("Release plan", "");
    assert_eq!(third["slug"], "release-plan-2");
    assert_eq!(ws.get("Release plan")["id"], third["id"]);
    assert_eq!(ws.get("release-plan")["id"], id.as_str());
}

#[test]
fn a_link_can_name_a_page_of_another_project() {
    let ws = open();
    ws.call("project.create", json!({ "name": "Lab", "key": "LAB" }));
    ws.call("wiki.create", json!({ "project": "LAB", "title": "Setup" }));
    ws.page("Notes", "Use [[LAB:Setup]] and [[Setup]].\n");
    let refs = ws.call(
        "wiki.references",
        json!({ "project": "TM", "page": "Notes" }),
    );
    assert_eq!(refs["items"][0]["found"], true);
    assert_eq!(refs["items"][0]["project"], "LAB");
    assert_eq!(
        refs["items"][1]["found"], false,
        "[[Setup]] is TM's own page"
    );
    let back = ws.call(
        "wiki.backlinks",
        json!({ "project": "LAB", "page": "setup" }),
    );
    assert_eq!(back["items"][0]["title"], "Notes");
    assert_eq!(back["items"][0]["project"], "TM");
    // A page of that whole name in the linking project wins.
    ws.page("LAB:Setup", "");
    let refs = ws.call(
        "wiki.references",
        json!({ "project": "TM", "page": "Notes" }),
    );
    assert_eq!(refs["items"][0]["project"], "TM");
}

#[test]
fn pages_move_within_the_tree_but_never_into_themselves() {
    let ws = open();
    ws.page("Guide", "");
    ws.child("Install", "Guide");
    ws.child("Linux", "Install");
    ws.page("FAQ", "");
    for (input, id) in [
        (json!({ "parent": "Linux" }), "wiki-parent-loop"),
        (json!({ "parent": "Guide" }), "wiki-parent-loop"),
        (
            json!({ "parent": "FAQ", "top_level": true }),
            "wiki-parent-and-top",
        ),
        (json!({}), "wiki-move-nowhere"),
    ] {
        let mut input = input;
        input["project"] = json!("TM");
        input["page"] = json!("Guide");
        let e = ws.fails("wiki.move", input.clone());
        assert_eq!(e.code, ErrorCode::Invalid, "{input}");
        assert_eq!(e.details["message_id"], id, "{input}");
    }
    ws.call(
        "wiki.move",
        json!({ "project": "TM", "page": "Install", "parent": "FAQ" }),
    );
    assert_eq!(ws.titles(), ["Guide", "FAQ", "  Install", "    Linux"]);
    ws.call(
        "wiki.move",
        json!({ "project": "TM", "page": "Linux", "top_level": true, "before": "Guide" }),
    );
    ws.call(
        "wiki.move",
        json!({ "project": "TM", "page": "FAQ", "at": "top" }),
    );
    assert_eq!(ws.titles(), ["FAQ", "  Install", "Linux", "Guide"]);
    // Undo puts the last move back.
    ws.call("history.undo", json!({}));
    assert_eq!(ws.titles(), ["Linux", "Guide", "FAQ", "  Install"]);
}

#[test]
fn texts_written_before_the_link_index_are_read_on_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    {
        let engine = Engine::open(&layout, EngineConfig::new(Role::App)).unwrap();
        let call = |op: &str, input: Value| engine.call(&me(), op, input).unwrap();
        call(
            "project.create",
            json!({ "name": "Task Manager", "key": "TM" }),
        );
        call("wiki.create", json!({ "project": "TM", "title": "Plan" }));
        call(
            "task.create",
            json!({ "project": "TM", "title": "Ship", "description": "See [[Plan]]." }),
        );
        // As a workspace from before the index: no rows, and the mark the
        // schema upgrade leaves.
        engine
            .store()
            .write("test", |tx| {
                tx.execute_batch(
                    "DELETE FROM text_links;
                     INSERT INTO meta (key, value) VALUES ('text_links_pending', '1');",
                )
                .map_err(gk_store::StoreError::from)
            })
            .unwrap();
        let back = call("wiki.backlinks", json!({ "project": "TM", "page": "Plan" }));
        assert_eq!(back["total"], 0);
    }
    let engine = Engine::open(&layout, EngineConfig::new(Role::App)).unwrap();
    let back = engine
        .call(
            &me(),
            "wiki.backlinks",
            json!({ "project": "TM", "page": "Plan" }),
        )
        .unwrap();
    assert_eq!(back["items"][0]["task"], "TM-1");
    let left: i64 = engine
        .store()
        .read(|tx| {
            tx.query_row(
                "SELECT count(*) FROM meta WHERE key = 'text_links_pending'",
                [],
                |r| r.get(0),
            )
            .map_err(gk_store::StoreError::from)
        })
        .unwrap();
    assert_eq!(left, 0);
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

#[test]
fn pages_change_only_while_the_wiki_is_switched_on() {
    let ws = open();
    ws.page("Kept", "Text.\n");
    ws.call(
        "project.update",
        json!({ "project": "TM", "modules": { "wiki": false } }),
    );
    for (op, input) in [
        ("wiki.create", json!({ "project": "TM", "title": "New" })),
        (
            "wiki.update",
            json!({ "project": "TM", "page": "Kept", "title": "Renamed" }),
        ),
        ("wiki.delete", json!({ "project": "TM", "page": "Kept" })),
        (
            "checklist.update",
            json!({ "project": "TM", "page": "Kept", "add": [{ "text": "One" }] }),
        ),
    ] {
        let err = ws.fails(op, input);
        assert_eq!(err.code, ErrorCode::Forbidden, "{op}: {err:?}");
        assert_eq!(err.details["message_id"], "project-wiki-off", "{op}");
        assert_eq!(err.details["hint_id"], "project-wiki-off-hint", "{op}");
    }
    // The pages stay readable, and come back to life with the module.
    assert_eq!(ws.get("Kept")["title"], "Kept");
    ws.call(
        "project.update",
        json!({ "project": "TM", "modules": { "wiki": true } }),
    );
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Kept", "title": "Renamed" }),
    );
    assert_eq!(ws.titles(), ["Renamed"]);
}

#[test]
fn checkboxes_on_a_page_are_ticked_as_a_new_revision_that_undo_takes_back() {
    let ws = open();
    let page = ws.page("Launch", "# Launch\n\n- [ ] Write notes\n- [ ] Tag\n");
    let first = page["revision"].as_str().unwrap().to_owned();
    let got = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Launch", "include": ["checklist"] }),
    );
    assert_eq!(got["checklist_items"][1]["text"], "Tag");

    let change = ws.call(
        "checklist.update",
        json!({
            "project": "TM",
            "page": "Launch",
            "toggle": [{ "number": 2, "text": "Tag", "checked": true }],
        }),
    );
    assert_eq!(change["page"], page["id"]);
    assert_eq!(change["key"], "TM");
    assert_eq!(change["checklist"], json!({ "done": 1, "total": 2 }));
    assert!(change["group"].is_string(), "{change}");
    let after = ws.get("Launch");
    assert_eq!(after["body"], "# Launch\n\n- [ ] Write notes\n- [x] Tag\n");
    assert_eq!(after["revision"], change["revision"]);
    assert_ne!(after["revision"].as_str(), Some(first.as_str()));
    let history = ws.call("wiki.history", json!({ "project": "TM", "page": "Launch" }));
    assert_eq!(history["items"].as_array().unwrap().len(), 2, "{history}");

    // By ID, without the project; a stale text is refused.
    let err = ws.fails(
        "checklist.update",
        json!({ "page": page["id"], "toggle": [{ "number": 1, "text": "Gone" }] }),
    );
    assert_eq!(err.code, ErrorCode::Conflict, "{err:?}");

    ws.call("history.undo", json!({}));
    assert_eq!(
        ws.get("Launch")["body"],
        "# Launch\n\n- [ ] Write notes\n- [ ] Tag\n"
    );

    // One owner only.
    let err = ws.fails(
        "checklist.update",
        json!({ "key": "TM-1", "page": "Launch", "add": [{ "text": "x" }] }),
    );
    assert_eq!(err.details["message_id"], "checklist-one-owner", "{err:?}");
}

#[test]
fn a_page_exports_with_its_subpages_and_files_as_plain_markdown() {
    let ws = open();
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Has a file" }),
    );
    let file = ws.call(
        "file.attach",
        json!({ "task": "TM-1", "name": "plan v2.txt", "data_base64": gk_core::base64_encode(b"abc") }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ws.page(
        "Home",
        "# Top\n\nSee [[Child#Second part|the part]], [[Elsewhere]] and [[#Top]].\n\n`[[Child]]`\n",
    );
    ws.page("Elsewhere", "Not exported.\n");
    ws.call(
        "wiki.create",
        json!({
            "project": "TM",
            "title": "Child",
            "parent": "Home",
            "body": format!("Back to [[Home]]; [notes](gk-file://{file}).\n"),
        }),
    );
    ws.child("Grandchild", "Child");

    let out = ws._dir.path().join("out");
    let done = ws.call(
        "wiki.export",
        json!({
            "project": "TM",
            "page": "Home",
            "subpages": true,
            "path": out.to_string_lossy(),
            "format": "folder",
        }),
    );
    assert_eq!(done["pages"], 3, "{done}");
    assert_eq!(done["attachments"], 1, "{done}");
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap();
    assert_eq!(
        read("home.md"),
        "# Top\n\nSee [the part](home/child.md#second-part), [[Elsewhere]] and [Top](#top).\n\n`[[Child]]`\n"
    );
    assert_eq!(
        read("home/child.md"),
        "Back to [Home](../home.md); [notes](../files/plan%20v2.txt).\n"
    );
    assert_eq!(read("home/child/grandchild.md"), "");
    assert_eq!(read("files/plan v2.txt"), "abc");

    // Without sub-pages, one file in a zip in the exports folder.
    let alone = ws.call("wiki.export", json!({ "project": "TM", "page": "Home" }));
    assert_eq!(alone["pages"], 1, "{alone}");
    assert_eq!(alone["format"], "zip");
    let path = alone["path"].as_str().unwrap();
    assert!(path.ends_with(".zip") && path.contains("home-"), "{path}");
    assert!(std::path::Path::new(path).starts_with(ws.layout.exports_dir()));

    // The whole wiki: every page, the tree as folders.
    let all = ws._dir.path().join("all");
    let done = ws.call(
        "wiki.export",
        json!({ "project": "TM", "path": all.to_string_lossy(), "format": "folder" }),
    );
    assert_eq!(done["pages"], 4, "{done}");
    assert_eq!(
        std::fs::read_to_string(all.join("home.md")).unwrap(),
        "# Top\n\nSee [the part](home/child.md#second-part), [Elsewhere](elsewhere.md) and [Top](#top).\n\n`[[Child]]`\n"
    );
    assert!(all.join("home/child/grandchild.md").is_file());
    let err = ws.fails("wiki.export", json!({}));
    assert_eq!(err.details["message_id"], "wiki-export-what", "{err:?}");

    // An agent names a plain file only.
    let agent = agent_of(&ws.engine, "Helper");
    let err = ws
        .engine
        .call(
            &agent,
            "wiki.export",
            json!({ "project": "TM", "page": "Home", "path": "/tmp/x.zip" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
}

#[test]
fn exported_names_are_valid_on_windows_and_import_back() {
    let ws = open();
    ws.page("CON", "See [[a:b]] and [[con#Part]].\n");
    ws.child("aux", "CON");
    ws.page("a:b", "");
    for name in ["nul.txt", "a:b?.txt", "a_b_.txt"] {
        ws.call(
            "file.attach",
            json!({
                "project": "TM",
                "page": "a:b",
                "name": name,
                "data_base64": gk_core::base64_encode(name.as_bytes()),
            }),
        );
    }

    let out = ws._dir.path().join("out");
    let done = ws.call(
        "wiki.export",
        json!({ "project": "TM", "path": out.to_string_lossy(), "format": "folder" }),
    );
    assert_eq!(done["pages"], 3, "{done}");
    let mut names = Vec::new();
    for entry in walk(&out) {
        let rel = entry
            .strip_prefix(&out)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        names.push(rel);
    }
    names.sort();
    assert_eq!(
        names,
        [
            "_con",
            "_con.md",
            "_con/_aux.md",
            "a-b.md",
            "files",
            "files/_nul.txt",
            "files/a_b_-2.txt",
            "files/a_b_.txt",
        ]
    );
    for name in &names {
        for part in name.split('/') {
            assert_eq!(gk_core::portable_file_name(part), *part, "{name}");
        }
    }
    assert_eq!(
        std::fs::read_to_string(out.join("_con.md")).unwrap(),
        "See [a:b](a-b.md) and [con § Part](#part).\n"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("files/_nul.txt")).unwrap(),
        "nul.txt"
    );

    // The folder imports back under the names it had.
    ws.call("project.create", json!({ "name": "Back", "key": "BK" }));
    let back = ws.call(
        "wiki.import",
        json!({ "project": "BK", "path": out.to_string_lossy() }),
    );
    assert_eq!(back["pages"], 3, "{back}");
    assert!(back.get("renamed").is_none(), "{back}");
    let titles: Vec<String> = ws.call("wiki.list", json!({ "project": "BK" }))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["title"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(titles, ["con", "aux", "a-b"]);
}

/// Every file and folder below `root`.
fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        }
        out.push(path);
    }
    out
}

#[test]
fn one_page_per_project_is_the_agent_brief_and_only_people_choose_it() {
    let ws = open();
    ws.page("Conventions", "Use small commits.\n");
    ws.page("Rules", "Write tests.\n");
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Conventions", "agent_brief": true }),
    );
    assert_eq!(ws.get("Conventions")["agent_brief"], true);
    let brief = ws.call("project.brief", json!({ "project": "TM" }));
    assert!(brief.to_string().contains("Use small commits."), "{brief}");

    // Marking another page moves the mark; Undo moves it back.
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Rules", "agent_brief": true }),
    );
    assert_eq!(ws.get("Conventions").get("agent_brief"), None);
    assert_eq!(ws.get("Rules")["agent_brief"], true);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.get("Conventions")["agent_brief"], true);
    assert_eq!(ws.get("Rules").get("agent_brief"), None);

    let agent = agent_of(&ws.engine, "Helper");
    let err = ws
        .engine
        .call(
            &agent,
            "wiki.update",
            json!({ "project": "TM", "page": "Rules", "agent_brief": true }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden, "{err:?}");
    assert_eq!(err.details["message_id"], "wiki-brief-agents");

    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Conventions", "agent_brief": false }),
    );
    assert_eq!(ws.get("Conventions").get("agent_brief"), None);
}

/// A small real PNG image.
fn png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255]))
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn sure() -> Context {
    Context::user(Via::Cli).with_confirmation(Confirmation::Given)
}

#[test]
fn files_attach_to_pages_and_go_to_the_trash_and_back_with_them() {
    let ws = open();
    let home = ws.page("Home", "Text.\n");
    let page_id = home["id"].as_str().unwrap().to_owned();
    let attached = ws.call(
        "file.attach",
        json!({
            "project": "TM",
            "page": "home",
            "name": "chart.png",
            "data_base64": gk_core::base64_encode(&png()),
        }),
    );
    let file = attached["file"].clone();
    let id = file["id"].as_str().unwrap().to_owned();
    assert_eq!(file["page"], page_id.as_str(), "{file}");
    assert_eq!(file["project"], "TM", "{file}");
    assert_eq!(file["task"], "", "{file}");
    assert_eq!(file["kind"], "image");
    assert_eq!(file["width"], 4);

    let with_files = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Home", "include": ["files"] }),
    );
    assert_eq!(with_files["files"][0]["id"], id.as_str(), "{with_files}");
    assert_eq!(ws.get("Home").get("files"), None);
    let thumb = ws.call("file.thumbnail", json!({ "attachment": id }));
    assert_eq!(thumb["mime"], "image/png", "{thumb}");
    let found = ws.call(
        "search.query",
        json!({ "text": "chart", "kinds": ["file"] }),
    );
    assert_eq!(found["items"][0]["page"], page_id.as_str(), "{found}");
    assert_eq!(found["items"][0]["slug"], "home", "{found}");

    // One owner only; `project` is for pages.
    ws.call("task.create", json!({ "project": "TM", "title": "T" }));
    let err = ws.fails(
        "file.attach",
        json!({ "task": "TM-1", "page": page_id, "name": "a.txt", "data_base64": "YQ==" }),
    );
    assert_eq!(err.details["message_id"], "file-attach-to-what", "{err:?}");
    let err = ws.fails(
        "file.attach",
        json!({ "task": "TM-1", "project": "TM", "name": "a.txt", "data_base64": "YQ==" }),
    );
    assert_eq!(
        err.details["message_id"], "file-project-for-page",
        "{err:?}"
    );

    // The page goes to the trash with its file, and comes back with it.
    let deleted = ws.call("wiki.delete", json!({ "project": "TM", "page": "Home" }));
    let err = ws.fails("file.get", json!({ "attachment": id }));
    assert_eq!(err.code, ErrorCode::NotFound);
    let trash = ws.call("trash.list", json!({}));
    assert_eq!(trash["total"], 1, "{trash}");
    let entry = deleted["items"][0]["id"].clone();
    ws.call("trash.restore", json!({ "items": [entry] }));
    assert_eq!(
        ws.call("file.get", json!({ "attachment": id }))["name"],
        "chart.png"
    );

    // The file alone to the trash and back; Undo too.
    ws.call("file.delete", json!({ "attachment": id }));
    let listed = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "Home", "include": ["files"] }),
    );
    assert_eq!(listed["files"], json!([]), "{listed}");
    let trash = ws.call("trash.list", json!({}));
    assert_eq!(trash["items"][0]["kind"], "file", "{trash}");
    assert_eq!(trash["items"][0]["project"], "TM", "{trash}");
    ws.call("history.undo", json!({}));
    assert_eq!(
        ws.call("file.get", json!({ "attachment": id }))["name"],
        "chart.png"
    );

    // In backups and archives.
    let backup = ws.call("backup.create", json!({}));
    assert_eq!(backup["files"], 1, "{backup}");
    let out = ws._dir.path().join("archive");
    ws.call(
        "transfer.export",
        json!({ "path": out.to_string_lossy(), "format": "folder", "projects": ["TM"] }),
    );
    ws.call(
        "transfer.import",
        json!({ "path": out.to_string_lossy(), "keys": [{ "from": "TM", "to": "CP" }] }),
    );
    let copy = ws.call(
        "wiki.get",
        json!({ "project": "CP", "page": "Home", "include": ["files"] }),
    );
    assert_eq!(copy["files"][0]["name"], "chart.png", "{copy}");
    assert_eq!(copy["files"][0]["sha256"], file["sha256"], "{copy}");

    // Purged with the page: the copy keeps the stored content.
    let deleted = ws.call("wiki.delete", json!({ "project": "TM", "page": "Home" }));
    let entry = deleted["items"][0]["id"].clone();
    ws.engine
        .call(&sure(), "trash.purge", json!({ "items": [entry] }))
        .unwrap();
    let err = ws.fails("file.get", json!({ "attachment": id }));
    assert_eq!(err.code, ErrorCode::NotFound);
    let sha = file["sha256"].as_str().unwrap();
    assert!(gk_store::file_path(&ws.layout, sha).is_some());

    // Not while the wiki is off.
    ws.call(
        "project.update",
        json!({ "project": "CP", "modules": { "wiki": false } }),
    );
    let err = ws.fails(
        "file.attach",
        json!({ "project": "CP", "page": "Home", "name": "a.txt", "data_base64": "YQ==" }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden, "{err:?}");
}

#[test]
fn one_page_is_the_home_page_and_undo_moves_the_mark_back() {
    let ws = open();
    ws.page("Start", "");
    ws.page("Welcome", "");
    let set = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Welcome", "home": true }),
    );
    assert_eq!(set["page"]["home"], true, "{set}");
    let list = ws.call("wiki.list", json!({ "project": "TM" }));
    let homes: Vec<&Value> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["home"] == true)
        .collect();
    assert_eq!(homes.len(), 1, "{list}");
    assert_eq!(homes[0]["title"], "Welcome");

    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Start", "home": true }),
    );
    assert_eq!(ws.get("Welcome").get("home"), None);
    assert_eq!(ws.get("Start")["home"], true);
    ws.call("history.undo", json!({}));
    assert_eq!(ws.get("Welcome")["home"], true);
    assert_eq!(ws.get("Start").get("home"), None);

    // A page in the trash is not the home page; back, it is again.
    let deleted = ws.call("wiki.delete", json!({ "project": "TM", "page": "Welcome" }));
    let list = ws.call("wiki.list", json!({ "project": "TM" }));
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p.get("home").is_none())
    );
    let entry = deleted["items"][0]["id"].clone();
    ws.call("trash.restore", json!({ "items": [entry] }));
    assert_eq!(ws.get("Welcome")["home"], true);

    // Agents may choose it too: it only says where the wiki opens.
    let agent = agent_of(&ws.engine, "Helper");
    ws.engine
        .call(
            &agent,
            "wiki.update",
            json!({ "project": "TM", "page": "Start", "home": true }),
        )
        .unwrap();
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Start", "home": false }),
    );
    let list = ws.call("wiki.list", json!({ "project": "TM" }));
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p.get("home").is_none())
    );
}

#[test]
fn a_branch_exports_as_web_pages_with_its_files_and_a_list_of_pages() {
    let ws = open();
    let home = ws.page("Home", "");
    let file = ws.call(
        "file.attach",
        json!({ "page": home["id"], "name": "chart.png", "data_base64": gk_core::base64_encode(&png()) }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ws.call(
        "wiki.update",
        json!({
            "project": "TM",
            "page": "Home",
            "body": format!("# Top\n\nSee [[Child#Second part|the part]] and [[Elsewhere]].\n\n![chart](gk-file://{file})\n"),
        }),
    );
    ws.page("Elsewhere", "");
    ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Child", "parent": "Home", "body": "## Second part\n\nBack to [[Home]].\n" }),
    );
    let out = ws._dir.path().join("web");
    let done = ws.call(
        "wiki.export",
        json!({
            "project": "TM",
            "page": "Home",
            "subpages": true,
            "text_format": "html",
            "path": out.to_string_lossy(),
            "format": "folder",
        }),
    );
    assert_eq!(done["pages"], 2, "{done}");
    assert_eq!(done["attachments"], 1, "{done}");
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap();
    let home = read("home.html");
    assert!(home.starts_with("<!doctype html>"), "{home}");
    assert!(
        home.contains("href=\"home/child.html#second-part\""),
        "{home}"
    );
    assert!(home.contains("src=\"files/chart.png\""), "{home}");
    assert!(!home.contains("gk-file"), "{home}");
    assert!(home.contains("href=\"index.html\""), "{home}");
    let child = read("home/child.html");
    assert!(child.contains("<h2 id=\"second-part\">"), "{child}");
    assert!(child.contains("href=\"../home.html\""), "{child}");
    let index = read("index.html");
    assert!(index.contains("href=\"home/child.html\""), "{index}");
    assert!(out.join("files/chart.png").is_file());

    // One page alone needs no list.
    let alone = ws._dir.path().join("alone");
    ws.call(
        "wiki.export",
        json!({
            "project": "TM", "page": "Elsewhere", "text_format": "html",
            "path": alone.to_string_lossy(), "format": "folder",
        }),
    );
    assert!(alone.join("elsewhere.html").is_file());
    assert!(!alone.join("index.html").exists());
}

/// Writes the files of a folder to import.
fn folder(root: &std::path::Path, files: &[(&str, &[u8])]) {
    for (path, content) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
}

#[test]
fn a_folder_of_markdown_imports_as_pages_with_links_and_images_in_one_change() {
    let ws = open();
    ws.page("guide", "Already here.\n");
    let dir = ws._dir.path().join("notes");
    let png = png();
    folder(
        &dir,
        &[
            (
                "guide.md",
                b"# Guide\n\nRead [the setup](guide/setup.md#install-it) and [[setup]].\n\n![shot](img/shot.png)\n\n[web](https://example.com/a.md) [out](../secret.md)\n",
            ),
            ("guide/setup.md", b"## Install it\n\nBack to [guide](../guide.md).\n"),
            ("ideas/index.md", b"Ideas, see [[guide]].\n"),
            ("ideas/later/one.md", b"One.\n"),
            ("img/shot.png", &png),
            ("img/unused.png", &png),
            (".git/HEAD", b"ref"),
        ],
    );
    let dry = ws.call(
        "wiki.import",
        json!({ "project": "TM", "path": dir.to_string_lossy(), "dry_run": true }),
    );
    assert_eq!(dry["pages"], 5, "{dry}");
    assert_eq!(dry["attachments"], 1, "{dry}");
    assert_eq!(dry["renamed"][0]["title"], "guide (2)", "{dry}");
    assert_eq!(ws.titles().len(), 1, "a dry run writes nothing");

    let done = ws.call(
        "wiki.import",
        json!({ "project": "TM", "path": dir.to_string_lossy() }),
    );
    assert_eq!(done["pages"], 5, "{done}");
    assert!(done["group"].is_string(), "{done}");
    assert_eq!(
        done["skipped"],
        json!([".git/HEAD", "img/unused.png"]),
        "{done}"
    );
    assert_eq!(
        ws.titles(),
        [
            "guide",
            "guide (2)",
            "  setup",
            "ideas",
            "  later",
            "    one"
        ]
    );
    let guide = ws.call(
        "wiki.get",
        json!({ "project": "TM", "page": "guide (2)", "include": ["files"] }),
    );
    let shot = guide["files"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(guide["files"][0]["name"], "shot.png");
    assert_eq!(
        guide["body"],
        format!(
            "# Guide\n\nRead [[setup#Install it|the setup]] and [[setup]].\n\n![shot](gk-file://{shot})\n\n[web](https://example.com/a.md) [out](../secret.md)\n"
        )
    );
    assert_eq!(
        ws.get("setup")["body"],
        "## Install it\n\nBack to [[guide (2)|guide]].\n"
    );
    assert_eq!(ws.get("ideas")["body"], "Ideas, see [[guide (2)]].\n");
    assert_eq!(ws.get("later")["body"], "");

    // Undo takes the whole import back.
    ws.call("history.undo", json!({}));
    assert_eq!(ws.titles(), ["guide"]);
    let err = ws.fails("file.get", json!({ "attachment": shot }));
    assert_eq!(err.code, ErrorCode::NotFound);

    // Into a page, and as an agent only from the exports folder.
    let inside = ws.call(
        "wiki.import",
        json!({ "project": "TM", "path": dir.join("ideas").to_string_lossy(), "parent": "guide" }),
    );
    assert_eq!(inside["pages"], 3, "{inside}");
    assert_eq!(ws.titles(), ["guide", "  index", "  later", "    one"]);
    let agent = agent_of(&ws.engine, "Helper");
    let err = ws
        .engine
        .call(
            &agent,
            "wiki.import",
            json!({ "project": "TM", "path": dir.to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
}

#[test]
fn a_hostile_folder_is_refused_before_anything_is_written() {
    let ws = open();
    let empty = ws._dir.path().join("empty");
    folder(&empty, &[("a.txt", b"x")]);
    let err = ws.fails(
        "wiki.import",
        json!({ "project": "TM", "path": empty.to_string_lossy() }),
    );
    assert_eq!(err.details["message_id"], "wiki-import-nothing", "{err:?}");

    let big = ws._dir.path().join("big");
    folder(&big, &[("a.md", &vec![b'a'; (1 << 20) + 1])]);
    let err = ws.fails(
        "wiki.import",
        json!({ "project": "TM", "path": big.to_string_lossy() }),
    );
    assert_eq!(
        err.details["message_id"], "transfer-entry-too-large",
        "{err:?}"
    );

    let binary = ws._dir.path().join("binary");
    folder(&binary, &[("a.md", b"\xff\xfe\0bad")]);
    let err = ws.fails(
        "wiki.import",
        json!({ "project": "TM", "path": binary.to_string_lossy() }),
    );
    assert_eq!(err.details["message_id"], "wiki-import-not-text", "{err:?}");

    #[cfg(unix)]
    {
        let linked = ws._dir.path().join("linked");
        folder(&linked, &[("a.md", b"A")]);
        std::os::unix::fs::symlink("/etc/passwd", linked.join("b.md")).unwrap();
        let err = ws.fails(
            "wiki.import",
            json!({ "project": "TM", "path": linked.to_string_lossy() }),
        );
        assert_eq!(err.details["message_id"], "transfer-link", "{err:?}");
    }
    assert!(ws.titles().is_empty());
}

#[test]
fn macros_show_subpages_included_pages_and_live_task_lists() {
    let ws = open();
    ws.call("project.create", json!({ "name": "Other", "key": "OT" }));
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Write notes" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Tag the release" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "OT", "title": "Elsewhere" }),
    );
    let home = ws.page(
        "Home",
        "# Plan\n\n{{toc}}\n\n{{children}}\n\n{{include Shared}}\n\n{{tasks title:notes}}\n\n{{tasks project:OT}}\n\n{{tasks status:nothing}}\n\n{{include Home}}\n",
    );
    ws.child("Child", "Home");
    ws.page("Shared", "Shared text TM-2.\n\n- [ ] not mine\n");
    let render = |ctx: &Context| {
        ws.engine
            .call(
                ctx,
                "markdown.render",
                json!({ "documents": [{ "page": home["id"] }] }),
            )
            .unwrap()["items"][0]
            .clone()
    };
    let item = render(&me());
    let html = item["html"].as_str().unwrap();
    assert!(html.contains("<a data-wiki=\"#Plan\""), "{html}");
    assert!(html.contains("<a data-wiki=\"Child\""), "{html}");
    assert!(html.contains("Shared text <a data-ref=\"TM-2\""), "{html}");
    assert!(
        html.contains("<input type=\"checkbox\" disabled=\"\""),
        "{html}"
    );
    assert!(html.contains("<a data-ref=\"TM-1\""), "{html}");
    assert!(html.contains("Write notes"), "{html}");
    assert!(
        !html.contains("Tag the release"),
        "the query filters: {html}"
    );
    assert!(html.contains("<a data-ref=\"OT-1\""), "{html}");
    assert!(
        html.contains("data-macro-note=\"tasks-error\""),
        "an unknown status is an error: {html}"
    );
    // The error is the query's message by its ID and arguments, for the
    // reader to word in the person's language; no English text.
    assert!(
        html.contains("&quot;id&quot;:&quot;query-unknown-status&quot;"),
        "{html}"
    );
    assert!(
        html.contains("&quot;value&quot;:&quot;nothing&quot;"),
        "{html}"
    );
    assert!(!html.contains("No status"), "{html}");
    assert!(html.contains("data-macro-note=\"include-loop\""), "{html}");
    // The page's own checklist doesn't count the included page's.
    assert_eq!(item["checklist"].as_array().unwrap().len(), 0);

    // The list is live: a new task shows on the next render.
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "More notes" }),
    );
    assert!(
        render(&me())["html"]
            .as_str()
            .unwrap()
            .contains("More notes")
    );

    // A caller limited to TM doesn't see OT's tasks through a macro.
    let limited = me().within_project("TM");
    let html = render(&limited)["html"].as_str().unwrap().to_owned();
    assert!(!html.contains("OT-1"), "{html}");

    // A text rendered as the page's (the editor's preview) expands them too;
    // the same text alone doesn't.
    let preview = ws.call(
        "markdown.render",
        json!({ "documents": [
            { "page": home["id"], "source": "{{children}}\n" },
            { "source": "{{children}}\n" },
        ] }),
    );
    assert!(
        preview["items"][0]["html"]
            .as_str()
            .unwrap()
            .contains("data-wiki=\"Child\"")
    );
    assert_eq!(preview["items"][1]["html"], "<p>{{children}}</p>\n");
    let err = ws.fails(
        "markdown.render",
        json!({ "documents": [{ "page": home["id"], "task": "TM-1" }] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);

    // The web export expands them with their notes in words; the Markdown
    // export keeps them as written.
    let out = ws._dir.path().join("macros");
    ws.call(
        "wiki.export",
        json!({
            "project": "TM", "page": "Home", "text_format": "html",
            "path": out.to_string_lossy(), "format": "folder",
        }),
    );
    let page = std::fs::read_to_string(out.join("home.html")).unwrap();
    assert!(page.contains("href=\"#plan\""), "{page}");
    assert!(page.contains("Write notes"), "{page}");
    assert!(page.contains("Shared text"), "{page}");
    assert!(
        page.contains("is not included again inside itself"),
        "{page}"
    );
    let why = page.split("The task list can").nth(1).unwrap_or_default();
    assert!(why.contains("No status"), "{page}");
    let md = ws._dir.path().join("macros-md");
    ws.call(
        "wiki.export",
        json!({
            "project": "TM", "page": "Home",
            "path": md.to_string_lossy(), "format": "folder",
        }),
    );
    let text = std::fs::read_to_string(md.join("home.md")).unwrap();
    assert!(text.contains("{{tasks title:notes}}"), "{text}");
}

#[test]
fn pages_are_marked_as_templates_and_new_pages_start_from_them() {
    let ws = open();
    let text = "## Agenda\n\n- [ ] item\n\n{{tasks status:open}}\n";
    ws.page("Meeting notes", text);
    let marked = ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Meeting notes", "template": true }),
    );
    assert_eq!(marked["page"]["template"], true, "{marked}");
    let list = ws.call("wiki.list", json!({ "project": "TM" }));
    assert_eq!(list["items"][0]["template"], true, "{list}");

    // Undo takes the mark back; redo by marking again.
    ws.call("history.undo", json!({}));
    assert_eq!(ws.get("Meeting notes").get("template"), None);
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Meeting notes", "template": true }),
    );

    // A new page starts from the template's text, and is not a template.
    let made = ws.call(
        "wiki.create",
        json!({ "project": "TM", "title": "Meeting 3 March", "template": "meeting-notes" }),
    )["page"]
        .clone();
    assert_eq!(made["template"], Value::Null, "{made}");
    assert_eq!(ws.get("Meeting 3 March")["body"], text);
    // Changing the new page leaves the template alone.
    ws.call(
        "wiki.update",
        json!({ "project": "TM", "page": "Meeting 3 March", "body": "changed\n" }),
    );
    assert_eq!(ws.get("Meeting notes")["body"], text);

    // Another project's page by KEY:Page.
    ws.call("project.create", json!({ "name": "Other", "key": "OT" }));
    ws.call(
        "wiki.create",
        json!({ "project": "OT", "title": "Notes", "template": "TM:Meeting notes" }),
    );
    let other = ws.call("wiki.get", json!({ "project": "OT", "page": "Notes" }));
    assert_eq!(other["body"], text);

    let err = ws.fails(
        "wiki.create",
        json!({ "project": "TM", "title": "X", "template": "Meeting notes", "body": "x" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = ws.fails(
        "wiki.create",
        json!({ "project": "TM", "title": "X", "template": "Nowhere" }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.details["field"], "template", "{err:?}");

    // An agent limited to OT can't start from TM's template.
    let agent = agent_of(&ws.engine, "helper").within_project("OT");
    let err = ws
        .engine
        .call(
            &agent,
            "wiki.create",
            json!({ "project": "OT", "title": "Y", "template": "TM:Meeting notes" }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound, "{err:?}");

    // The mark survives the trash.
    let deleted = ws.call(
        "wiki.delete",
        json!({ "project": "TM", "page": "Meeting notes" }),
    );
    let entry = deleted["items"][0]["id"].clone();
    ws.call("trash.restore", json!({ "items": [entry] }));
    assert_eq!(ws.get("Meeting notes")["template"], true);
}
