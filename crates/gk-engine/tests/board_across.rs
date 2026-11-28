// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The board across projects: cards of several projects in the columns of
//! their status categories, archived projects left out, a caller limited
//! to one project seeing only it, a query and a folder narrowing it,
//! swimlanes by project, moves into a category's status with the status
//! rules applied and one undo, saved views of it, and a large workspace
//! read within the board's bound.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;
const DAY: i64 = 86_400_000;

fn open(dir: &TempDir) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(|| NOW);
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

/// A call the person confirmed, as for what can't be undone.
fn sure(e: &Engine, op: &str, input: Value) -> Value {
    let ctx = Context::user(Via::Cli).with_confirmation(Confirmation::Given);
    e.call(&ctx, op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).unwrap_err()
}

fn column<'a>(board: &'a Value, id: &str) -> &'a Value {
    board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no column {id}"))
}

fn keys(board: &Value, id: &str) -> Vec<String> {
    let mut out: Vec<String> = column(board, id)["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap().to_owned())
        .collect();
    out.sort();
    out
}

fn create(e: &Engine, project: &str, title: &str, status: Option<&str>) -> String {
    let mut input = json!({ "project": project, "title": title });
    if let Some(s) = status {
        input["status"] = json!(s);
    }
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn status_id(e: &Engine, project: &str, name: &str) -> String {
    call(e, "project.get", json!({ "project": project }))["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Projects TM (the default statuses), ACM (its own: Queued, Building,
/// Shipped) and ZZ (archived), with tasks in each category.
fn seed(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(
        e,
        "project.create",
        json!({ "name": "Acme", "key": "ACM", "hue": "orange" }),
    );
    for (old, new) in [
        ("Backlog", "Queued"),
        ("In progress", "Building"),
        ("Done", "Shipped"),
    ] {
        call(
            e,
            "status.update",
            json!({ "project": "ACM", "status": old, "name": new }),
        );
    }
    for gone in ["To do", "Review"] {
        sure(
            e,
            "status.delete",
            json!({ "project": "ACM", "status": gone }),
        );
    }
    call(
        e,
        "status.create",
        json!({ "project": "ACM", "name": "Dropped", "category": "cancelled" }),
    );
    call(e, "project.create", json!({ "name": "Old", "key": "ZZ" }));
    create(e, "TM", "Write the plan", Some("To do"));
    create(e, "TM", "Review the plan", Some("Review"));
    create(e, "TM", "Ship it", Some("Done"));
    create(e, "ACM", "Order parts", None);
    create(e, "ACM", "Build the frame", Some("Building"));
    create(e, "ACM", "Deliver", Some("Shipped"));
    create(e, "ACM", "Abandoned", Some("Dropped"));
    create(e, "ZZ", "Forgotten", None);
    call(e, "project.archive", json!({ "project": "ZZ" }));
}

#[test]
fn cards_of_several_projects_land_in_the_columns_of_their_categories() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let board = call(&e, "board.get_across", json!({}));
    let ids: Vec<&str> = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["todo", "in_progress", "done"]);
    assert_eq!(keys(&board, "todo"), ["ACM-1", "TM-1"]);
    assert_eq!(keys(&board, "in_progress"), ["ACM-2", "TM-2"]);
    assert_eq!(keys(&board, "done"), ["ACM-3", "TM-3"]);
    // Every card carries its real status.
    let review = column(&board, "in_progress")["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == "TM-2")
        .unwrap()
        .clone();
    assert_eq!(review["status"]["name"], "Review");
    assert_eq!(review["project"], "TM");
    // A column names each project's first status of its category.
    assert_eq!(
        column(&board, "in_progress")["statuses"]["TM"],
        status_id(&e, "TM", "In progress")
    );
    assert_eq!(
        column(&board, "todo")["statuses"]["ACM"],
        status_id(&e, "ACM", "Queued")
    );
    // The projects come with their statuses, cancelled ones aside, and
    // their colours; archived ZZ is not there.
    let projects: Vec<&str> = board["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["key"].as_str().unwrap())
        .collect();
    assert_eq!(projects, ["ACM", "TM"]);
    let acme = &board["projects"][0];
    assert_eq!(acme["hue"], "orange");
    let names: Vec<&str> = acme["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Queued", "Building", "Shipped"]);
    // Swimlanes by project by default, none of them takes drops.
    assert_eq!(board["lanes_by"], "project");
    let lanes: Vec<(&str, bool)> = board["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["id"].as_str().unwrap(), l["settable"].as_bool().unwrap()))
        .collect();
    assert_eq!(lanes, [("ACM", false), ("TM", false)]);
    assert_eq!(board["lanes"][1]["counts"]["in_progress"], 1);
    assert_eq!(board["lanes"][0]["name"], "Acme");
}

#[test]
fn archived_projects_stay_out_even_when_the_query_names_them() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let board = call(&e, "board.get_across", json!({ "query": "project:ZZ" }));
    for id in ["todo", "in_progress", "done"] {
        assert!(keys(&board, id).is_empty(), "{id}");
    }
    call(&e, "project.unarchive", json!({ "project": "ZZ" }));
    let board = call(&e, "board.get_across", json!({}));
    assert_eq!(keys(&board, "todo"), ["ACM-1", "TM-1", "ZZ-1"]);
}

#[test]
fn a_caller_limited_to_one_project_sees_only_it() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let ctx = Context::user(Via::Mcp).within_project("ACM");
    let board = e
        .call(&ctx, "board.get_across", json!({ "lanes": "none" }))
        .unwrap();
    assert_eq!(keys(&board, "todo"), ["ACM-1"]);
    assert_eq!(keys(&board, "in_progress"), ["ACM-2"]);
    assert_eq!(keys(&board, "done"), ["ACM-3"]);
    assert_eq!(board["projects"].as_array().unwrap().len(), 1);
    assert!(board.get("lanes").is_none());
}

#[test]
fn a_query_and_a_folder_narrow_the_board() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "priority": "high" }),
    );
    let board = call(&e, "board.get_across", json!({ "query": "priority>=high" }));
    assert_eq!(keys(&board, "todo"), ["TM-1"]);
    assert!(keys(&board, "in_progress").is_empty());
    assert_eq!(board["query"], "priority>=high");

    call(&e, "folder.create", json!({ "name": "Work" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "ACM", "folder": "Work" }),
    );
    call(
        &e,
        "project.create",
        json!({ "name": "Acme parts", "key": "ACP", "parent": "ACM" }),
    );
    create(&e, "ACP", "Bolts", None);
    let board = call(&e, "board.get_across", json!({ "folder": "work" }));
    assert_eq!(board["folder"], "Work");
    assert_eq!(keys(&board, "todo"), ["ACM-1", "ACP-1"]);
    assert!(!keys(&board, "todo").contains(&"TM-1".to_owned()));
    let wrong = fails(&e, "board.get_across", json!({ "folder": "Home" }));
    assert_eq!(wrong.code, ErrorCode::NotFound);
    let wrong = fails(&e, "board.get_across", json!({ "query": "label:" }));
    assert_eq!(wrong.code, ErrorCode::Invalid);
    let wrong = fails(&e, "board.get_across", json!({ "lanes": "field" }));
    assert_eq!(wrong.code, ErrorCode::Invalid);
}

#[test]
fn done_cards_show_for_two_weeks_and_older_ones_are_counted() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    e.store()
        .write("test.old", |tx| {
            tx.execute(
                "UPDATE tasks SET closed_at = ?1 WHERE number = 3
                   AND project_id = (SELECT id FROM projects WHERE key = 'TM')",
                [NOW - 30 * DAY],
            )?;
            Ok::<_, gk_engine::Failure>(())
        })
        .unwrap();
    let board = call(&e, "board.get_across", json!({}));
    assert_eq!(keys(&board, "done"), ["ACM-3"]);
    assert_eq!(column(&board, "done")["older"], 1);
    assert_eq!(column(&board, "done")["count"], 2);
    let all = call(&e, "board.get_across", json!({ "show_older": true }));
    assert_eq!(keys(&all, "done"), ["ACM-3", "TM-3"]);
}

#[test]
fn a_move_into_a_category_status_obeys_the_rules_and_one_undo_takes_it_back() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let board = call(&e, "board.get_across", json!({}));
    // The column's status for ACM is its first in-progress one.
    let building = column(&board, "in_progress")["statuses"]["ACM"].clone();
    call(
        &e,
        "task.move",
        json!({ "key": "ACM-1", "status": building, "at": "bottom" }),
    );
    let board = call(&e, "board.get_across", json!({}));
    assert_eq!(keys(&board, "in_progress"), ["ACM-1", "ACM-2", "TM-2"]);
    call(&e, "history.undo", json!({}));
    let board = call(&e, "board.get_across", json!({}));
    assert_eq!(keys(&board, "todo"), ["ACM-1", "TM-1"]);

    // TM has two in-progress statuses; a person may pick Review.
    let tm = board["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == "TM")
        .unwrap()
        .clone();
    let in_progress: Vec<&str> = tm["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["category"] == "in_progress")
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(in_progress, ["In progress", "Review"]);

    // A status that requires an estimate refuses the card with its message.
    call(
        &e,
        "rules.update",
        json!({
            "project": "TM",
            "statuses": [{ "status": "In progress", "rules": { "required": ["estimate"] } }],
        }),
    );
    let refused = fails(
        &e,
        "task.move",
        json!({ "key": "TM-1", "status": status_id(&e, "TM", "In progress") }),
    );
    assert_eq!(refused.code, ErrorCode::Invalid, "{refused:?}");
    assert!(refused.message.contains("estimate"), "{}", refused.message);
    let board = call(&e, "board.get_across", json!({}));
    assert_eq!(keys(&board, "todo"), ["ACM-1", "TM-1"]);
}

#[test]
fn saved_views_of_the_board_across_projects_keep_query_folder_and_lanes() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    call(&e, "folder.create", json!({ "name": "Work" }));
    call(
        &e,
        "folder.move",
        json!({ "project": "ACM", "folder": "Work" }),
    );
    let made = call(
        &e,
        "view.create",
        json!({
            "name": "Work",
            "kind": "board_across",
            "query": "is:open",
            "folder": "Work",
            "group_by": "priority",
        }),
    );
    assert_eq!(made["view"]["kind"], "board_across");
    assert_eq!(made["view"]["folder"], "Work");
    call(
        &e,
        "view.create",
        json!({ "name": "Everything", "kind": "board_across", "query": "" }),
    );
    let list = call(&e, "view.list", json!({ "kind": "board_across" }));
    let names: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Work", "Everything"]);
    let board = call(&e, "board.get_across", json!({ "view": "Work" }));
    assert_eq!(board["view"], made["view"]["id"]);
    assert_eq!(board["lanes_by"], "priority");
    assert_eq!(board["folder"], "Work");
    assert_eq!(keys(&board, "todo"), ["ACM-1"]);
    assert!(keys(&board, "done").is_empty());
    // The input overrides the view.
    let board = call(
        &e,
        "board.get_across",
        json!({ "view": "Work", "query": "", "lanes": "none" }),
    );
    assert_eq!(keys(&board, "done"), ["ACM-3"]);
    assert_eq!(board["lanes_by"], "none");

    // A view of the whole workspace only, with lanes the board offers,
    // and a folder only for this kind.
    let wrong = fails(
        &e,
        "view.create",
        json!({ "name": "X", "kind": "board_across", "query": "", "project": "TM" }),
    );
    assert_eq!(wrong.code, ErrorCode::Invalid);
    let wrong = fails(
        &e,
        "view.create",
        json!({ "name": "X", "kind": "board_across", "query": "", "group_by": "status" }),
    );
    assert_eq!(wrong.code, ErrorCode::Invalid);
    let wrong = fails(
        &e,
        "view.create",
        json!({ "name": "X", "query": "", "folder": "Work" }),
    );
    assert_eq!(wrong.code, ErrorCode::Invalid);
    let wrong = fails(
        &e,
        "board.get_across",
        json!({ "view": made["view"]["id"].as_str().unwrap().replace('0', "1") }),
    );
    assert_eq!(wrong.code, ErrorCode::NotFound);

    // A change and its undo; the folder can be cleared.
    call(
        &e,
        "view.update",
        json!({ "view": "Work", "clear": ["folder"] }),
    );
    let board = call(&e, "board.get_across", json!({ "view": "Work" }));
    assert!(board.get("folder").is_none());
    call(&e, "history.undo", json!({}));
    let board = call(&e, "board.get_across", json!({ "view": "Work" }));
    assert_eq!(board["folder"], "Work");

    // Views of the board across projects survive reopening the workspace.
    drop(e);
    let e = open(&dir);
    let list = call(&e, "view.list", json!({ "kind": "board_across" }));
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"][0]["folder"], "Work");
    assert_eq!(list["items"][0]["group_by"], "priority");
}

#[test]
fn project_lanes_on_a_project_board_with_sub_projects() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    call(
        &e,
        "project.create",
        json!({ "name": "Team docs", "key": "TMD", "parent": "TM" }),
    );
    create(&e, "TMD", "Guide", None);
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "include_subprojects": true, "lanes": "project" }),
    );
    let lanes: Vec<&str> = board["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(lanes, ["TM", "TMD"]);
    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "lanes": "project" }),
    );
    let layout = call(&e, "layout.get", json!({ "project": "TM" }));
    assert_eq!(layout["layout"]["lanes"], "project");
}

/// `projects` projects of `per` tasks each, of which `open` are open and
/// the rest closed long ago: data set L's twenty thousand tasks spread
/// over several projects.
fn large_workspace(e: &Engine, projects: usize, per: usize, open: usize) {
    use gk_store::rusqlite::params;
    for p in 0..projects {
        call(
            e,
            "project.create",
            json!({ "name": format!("Bulk {p}"), "key": format!("BK{p}") }),
        );
    }
    e.store()
        .write("test.large", move |tx| {
            for p in 0..projects {
                let key = format!("BK{p}");
                let project: Vec<u8> =
                    tx.query_row("SELECT id FROM projects WHERE key = ?1", [&key], |r| {
                        r.get(0)
                    })?;
                let ids = |sql: &str| -> Result<Vec<Vec<u8>>, gk_engine::Failure> {
                    let mut stmt = tx.prepare(sql)?;
                    let rows = stmt
                        .query_map([&project], |r| r.get::<_, Vec<u8>>(0))?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(rows)
                };
                let statuses = ids("SELECT id FROM statuses WHERE project_id = ?1 ORDER BY rank")?;
                let types = ids("SELECT id FROM task_types WHERE project_id = ?1 ORDER BY rank")?;
                let done = statuses.last().unwrap().clone();
                let long_ago = NOW - 400 * DAY;
                for i in 0..per {
                    let id = gk_core::Id::generate().as_bytes().to_vec();
                    let is_open = i < open;
                    let status = if is_open {
                        statuses[i % (statuses.len() - 1)].clone()
                    } else {
                        done.clone()
                    };
                    let at = if is_open {
                        NOW - (i as i64) * 60_000
                    } else {
                        long_ago
                    };
                    tx.execute(
                        "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                            status_entered_at, type_id, priority, rank, created_at,
                            created_by_kind, created_by_name, edited_at, updated_at, closed_at)
                         VALUES (?1, ?2, ?3, ?4, lower(?4), ?5, ?6, ?7, ?8, ?9, ?6, 'user',
                            'You', ?6, ?6, ?10)",
                        params![
                            id,
                            project,
                            (i + 1) as i64,
                            format!("Task {i}"),
                            status,
                            at,
                            types[i % types.len()],
                            (i % 5) as i64,
                            format!("r{i:07}"),
                            (!is_open).then_some(at),
                        ],
                    )?;
                }
                tx.execute(
                    "UPDATE projects SET next_number = ?1 WHERE id = ?2",
                    params![(per + 1) as i64, project],
                )?;
            }
            Ok::<_, gk_engine::Failure>(())
        })
        .unwrap();
}

/// Data set L's size across ten projects stays within the board's bound:
/// generous in a debug build, tight in a release build.
#[test]
fn the_board_across_twenty_thousand_tasks_stays_within_the_board_bound() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    large_workspace(&e, 10, 2_000, 100);
    let bound = if cfg!(debug_assertions) {
        std::time::Duration::from_secs(3)
    } else {
        std::time::Duration::from_millis(500)
    };
    for (query, lanes) in [
        ("", "project"),
        ("priority>=normal", "priority"),
        ("", "none"),
    ] {
        let board = || {
            call(
                &e,
                "board.get_across",
                json!({ "query": query, "lanes": lanes }),
            )
        };
        board();
        let (took, out) = (0..3)
            .map(|_| {
                let start = std::time::Instant::now();
                let out = board();
                (start.elapsed(), out)
            })
            .min_by_key(|(t, _)| *t)
            .unwrap();
        let shown: usize = out["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["cards"].as_array().unwrap().len())
            .sum();
        if query.is_empty() {
            assert_eq!(shown, 1_000);
            assert_eq!(column(&out, "done")["older"], 19_000);
        }
        eprintln!("board across 20,000 tasks, query {query:?}, lanes {lanes}: {took:?}");
        assert!(took < bound, "{query} {lanes}: {took:?}");
    }
}
