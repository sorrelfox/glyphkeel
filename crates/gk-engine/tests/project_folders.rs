// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Project folders: making, renaming, reordering and deleting them, putting
//! top-level projects in and out (their sub-projects follow), the order
//! inside a folder, undo, projects that become sub-projects, and archives
//! that carry folders.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &TempDir, name: &str) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::new(AtomicI64::new(NOW));
    config.clock = Arc::new(move || clock.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join(name)), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fail(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).expect_err(op)
}

fn setup(dir: &TempDir) -> Engine {
    let e = open(dir, "W");
    for (name, key) in [("Alpha", "AA"), ("Bravo", "BB"), ("Charlie", "CC")] {
        call(&e, "project.create", json!({ "name": name, "key": key }));
    }
    call(
        &e,
        "project.create",
        json!({ "name": "Bravo kid", "key": "BK", "parent": "BB" }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "BK", "title": "Under Bravo" }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "BB", "title": "Bravo" }),
    );
    e
}

fn folders(e: &Engine) -> Vec<Value> {
    call(e, "folder.list", json!({}))["items"]
        .as_array()
        .unwrap()
        .clone()
}

fn names(e: &Engine) -> Vec<String> {
    folders(e)
        .iter()
        .map(|f| f["name"].as_str().unwrap().to_owned())
        .collect()
}

/// Top-level keys with their folder name ("" for none), in tree order.
fn tree(e: &Engine) -> Vec<(String, String)> {
    let by_id: Vec<(String, String)> = folders(e)
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_owned(),
                f["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    call(e, "project.list", json!({}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["depth"] == 0)
        .map(|p| {
            let folder = p["folder"].as_str().map_or(String::new(), |id| {
                by_id
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, n)| n.clone())
                    .unwrap()
            });
            (p["key"].as_str().unwrap().to_owned(), folder)
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn folders_are_made_renamed_reordered_and_names_are_unique() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    let made = call(
        &e,
        "folder.create",
        json!({ "name": "Clients", "hue": "teal" }),
    );
    assert_eq!(made["folder"]["name"], "Clients");
    assert_eq!(made["folder"]["hue"], "teal");
    assert_eq!(made["folder"]["created_by"]["kind"], "user");
    assert!(made["group"].is_string());
    call(&e, "folder.create", json!({ "name": "Home" }));
    call(
        &e,
        "folder.create",
        json!({ "name": "Archive box", "at": "top" }),
    );
    assert_eq!(names(&e), ["Archive box", "Clients", "Home"]);

    let taken = fail(&e, "folder.create", json!({ "name": "clients" }));
    assert_eq!(taken.code, ErrorCode::Conflict);
    let empty = fail(&e, "folder.create", json!({ "name": "  " }));
    assert_eq!(empty.code, ErrorCode::Invalid);

    call(
        &e,
        "folder.rename",
        json!({ "folder": "home", "name": "Personal" }),
    );
    call(
        &e,
        "folder.update",
        json!({ "folder": "Personal", "before": "Archive box", "clear": ["hue"] }),
    );
    assert_eq!(names(&e), ["Personal", "Archive box", "Clients"]);
    call(
        &e,
        "folder.update",
        json!({ "folder": "Clients", "hue": "red", "after": "Personal" }),
    );
    assert_eq!(names(&e), ["Personal", "Clients", "Archive box"]);
    let missing = fail(&e, "folder.delete", json!({ "folder": "Nope" }));
    assert_eq!(missing.code, ErrorCode::NotFound);
    let itself = fail(
        &e,
        "folder.update",
        json!({ "folder": "Clients", "before": "Clients" }),
    );
    assert_eq!(itself.code, ErrorCode::Invalid);

    call(&e, "history.undo", json!({}));
    assert_eq!(names(&e), ["Personal", "Archive box", "Clients"]);
}

#[test]
fn projects_go_in_and_out_with_their_sub_projects_and_keep_their_order() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "CC", "folder": "Clients" }),
    );
    let moved = call(
        &e,
        "folder.move",
        json!({ "project": "BB", "folder": "Clients" }),
    );
    assert_eq!(moved["project"], "BB");
    assert_eq!(moved["folder"]["projects"], json!(["CC", "BB"]));
    // Bravo's two open tasks, its own and its sub-project's.
    assert_eq!(moved["folder"]["open_tasks"], 2);
    assert_eq!(
        tree(&e),
        pairs(&[("AA", ""), ("CC", "Clients"), ("BB", "Clients")])
    );
    // The sub-project is still under Bravo and names no folder itself.
    let kid = call(&e, "project.get", json!({ "project": "BK" }));
    assert_eq!(kid["parent"], "BB");
    assert!(kid.get("folder").is_none());
    let bravo = call(&e, "project.get", json!({ "project": "BB" }));
    assert_eq!(bravo["folder"], moved["folder"]["id"]);

    // Within the folder: first, or after another project there.
    call(&e, "folder.move", json!({ "project": "BB", "at": "top" }));
    assert_eq!(
        tree(&e),
        pairs(&[("AA", ""), ("BB", "Clients"), ("CC", "Clients")])
    );
    let outside = fail(&e, "folder.move", json!({ "project": "BB", "after": "AA" }));
    assert_eq!(outside.code, ErrorCode::Invalid);
    let sub = fail(
        &e,
        "folder.move",
        json!({ "project": "BK", "folder": "Clients" }),
    );
    assert_eq!(sub.code, ErrorCode::Invalid);

    // Out again, to the end of the projects in no folder.
    call(&e, "folder.move", json!({ "project": "BB", "none": true }));
    assert_eq!(
        tree(&e),
        pairs(&[("AA", ""), ("BB", ""), ("CC", "Clients")])
    );
    call(&e, "history.undo", json!({}));
    assert_eq!(
        tree(&e),
        pairs(&[("AA", ""), ("BB", "Clients"), ("CC", "Clients")])
    );
}

#[test]
fn deleting_a_folder_keeps_its_projects_and_undo_brings_it_back() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "AA", "folder": "Clients" }),
    );
    call(
        &e,
        "folder.move",
        json!({ "project": "CC", "folder": "Clients" }),
    );
    let gone = call(&e, "folder.delete", json!({ "folder": "Clients" }));
    assert_eq!(gone["deleted"], true);
    assert_eq!(gone["released"], json!(["AA", "CC"]));
    assert!(folders(&e).is_empty());
    // Each keeps its place among the top-level projects.
    assert_eq!(tree(&e), pairs(&[("AA", ""), ("CC", ""), ("BB", "")]));
    call(&e, "history.undo", json!({}));
    assert_eq!(
        tree(&e),
        pairs(&[("AA", "Clients"), ("CC", "Clients"), ("BB", "")])
    );
    call(&e, "history.redo", json!({}));
    assert!(folders(&e).is_empty());

    // Reverting a folder's making while projects are in it lets go of
    // them first.
    let made = call(&e, "folder.create", json!({ "name": "Later" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "BB", "folder": "Later" }),
    );
    let done = call(&e, "history.revert", json!({ "groups": [made["group"]] }));
    assert_eq!(done["reverted"].as_array().unwrap().len(), 1, "{done}");
    assert!(folders(&e).is_empty());
    assert_eq!(tree(&e), pairs(&[("AA", ""), ("CC", ""), ("BB", "")]));
}

#[test]
fn a_project_that_becomes_a_sub_project_leaves_its_folder() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "CC", "folder": "Clients" }),
    );
    call(
        &e,
        "project.move",
        json!({ "project": "CC", "parent": "AA" }),
    );
    let cc = call(&e, "project.get", json!({ "project": "CC" }));
    assert!(cc.get("folder").is_none(), "{cc}");
    assert_eq!(folders(&e)[0]["projects"], json!([]));
    call(&e, "history.undo", json!({}));
    assert_eq!(folders(&e)[0]["projects"], json!(["CC"]));
}

#[test]
fn archives_carry_folders_by_name() {
    let dir = TempDir::new().unwrap();
    let a = setup(&dir);
    call(
        &a,
        "folder.create",
        json!({ "name": "Clients", "hue": "purple" }),
    );
    call(
        &a,
        "folder.move",
        json!({ "project": "BB", "folder": "Clients" }),
    );
    let whole = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": whole.to_string_lossy() }),
    );

    let b = open(&dir, "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": whole.to_string_lossy() }),
    );
    let got = folders(&b);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["name"], "Clients");
    assert_eq!(got[0]["hue"], "purple");
    assert_eq!(got[0]["projects"], json!(["BB"]));

    // Again under new keys: the folder of that name is used, not doubled.
    call(
        &b,
        "transfer.import",
        json!({ "path": whole.to_string_lossy(),
                "keys": [{ "from": "AA", "to": "AB" }, { "from": "BB", "to": "BC" },
                         { "from": "CC", "to": "CD" }, { "from": "BK", "to": "BL" }] }),
    );
    let got = folders(&b);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["projects"], json!(["BB", "BC"]));
}

#[test]
fn an_agent_limited_to_a_project_cannot_change_folders() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    let mut ctx = Context::user(Via::Mcp);
    ctx.project_scope = Some("BB".to_owned());
    let refused = e
        .call(&ctx, "folder.create", json!({ "name": "Mine" }))
        .expect_err("scoped");
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert!(folders(&e).is_empty());
}

#[test]
fn a_new_project_goes_straight_into_a_folder_and_one_undo_removes_it() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "AA", "folder": "Clients" }),
    );
    let made = call(
        &e,
        "project.create",
        json!({ "name": "Delta", "key": "DD", "folder": "clients" }),
    );
    let folder_id = folders(&e)[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(made["project"]["folder"], folder_id.as_str());
    assert_eq!(
        tree(&e),
        pairs(&[("AA", "Clients"), ("BB", ""), ("CC", ""), ("DD", "Clients"),])
    );
    let listed = folders(&e);
    assert_eq!(listed[0]["projects"], json!(["AA", "DD"]), "{listed:?}");

    // Without a folder it lands at the top level.
    call(&e, "project.create", json!({ "name": "Echo", "key": "EE" }));
    assert_eq!(tree(&e).last().unwrap(), &("EE".to_owned(), String::new()));

    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    assert_eq!(
        tree(&e),
        pairs(&[("AA", "Clients"), ("BB", ""), ("CC", "")])
    );
    assert_eq!(names(&e), ["Clients"]);
    assert_eq!(folders(&e)[0]["projects"], json!(["AA"]));
}

#[test]
fn a_project_from_a_template_goes_into_a_folder_in_one_change() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    call(
        &e,
        "template.apply",
        json!({ "template": "software project", "name": "App", "key": "APP", "folder": "Clients" }),
    );
    assert_eq!(folders(&e)[0]["projects"], json!(["APP"]));
    call(&e, "history.undo", json!({}));
    assert_eq!(folders(&e)[0]["projects"], json!([]));
    assert!(tree(&e).iter().all(|(key, _)| key != "APP"));
}

#[test]
fn a_new_project_refuses_a_folder_with_a_parent_or_an_unknown_folder() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    let both = fail(
        &e,
        "project.create",
        json!({ "name": "Kid", "key": "KD", "parent": "BB", "folder": "Clients" }),
    );
    assert_eq!(both.code, ErrorCode::Invalid);
    assert_eq!(both.details["path"], "folder", "{both:?}");
    let missing = fail(
        &e,
        "project.create",
        json!({ "name": "Kid", "key": "KD", "folder": "Nope" }),
    );
    assert_eq!(missing.code, ErrorCode::NotFound);
    let mut ctx = Context::user(Via::Mcp);
    ctx.project_scope = Some("BB".to_owned());
    let scoped = e
        .call(
            &ctx,
            "project.create",
            json!({ "name": "Kid", "key": "KD", "folder": "Clients" }),
        )
        .expect_err("scoped");
    assert_eq!(scoped.code, ErrorCode::Forbidden);
    assert!(
        call(&e, "project.list", json!({}))["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["key"] != "KD")
    );
}

#[test]
fn an_agent_cannot_reach_the_disk_through_a_project_folder() {
    let dir = TempDir::new().unwrap();
    let e = setup(&dir);
    call(&e, "folder.create", json!({ "name": "Clients" }));
    let session = e
        .start_session(&gk_engine::SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: gk_api::Profile::Full,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, gk_api::Profile::Full);
    // A real folder outside the folders agents may use, and one that
    // isn't there yet.
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "x").unwrap();
    let missing = dir.path().join("made-by-agent");
    let before = || {
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|d| d.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let listed = before();
    for path in [&outside, &outside.join("secret.txt"), &missing] {
        let path = path.to_string_lossy().into_owned();
        for (op, input) in [
            (
                "project.create",
                json!({ "name": "Delta", "key": "DD", "folder": path }),
            ),
            (
                "template.apply",
                json!({ "template": "software project", "name": "App", "key": "APP", "folder": path }),
            ),
        ] {
            let err = e.call(&agent, op, input).expect_err(op);
            assert_eq!(err.code, ErrorCode::NotFound, "{op} {path}: {err:?}");
            assert_eq!(err.details["field"], "folder", "{op}: {err:?}");
        }
    }
    assert_eq!(before(), listed, "nothing was made on disk");
    assert!(!missing.exists());
    assert_eq!(
        std::fs::read_to_string(outside.join("secret.txt")).unwrap(),
        "x"
    );
    assert!(
        call(&e, "project.list", json!({}))["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["key"] != "DD" && p["key"] != "APP")
    );
    // The same agent can still name the sidebar folder.
    let made = e
        .call(
            &agent,
            "project.create",
            json!({ "name": "Delta", "key": "DD", "folder": "Clients" }),
        )
        .unwrap();
    assert_eq!(made["project"]["folder"], folders(&e)[0]["id"]);
}
