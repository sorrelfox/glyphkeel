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
