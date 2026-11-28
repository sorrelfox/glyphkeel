// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Board swimlanes: the cards grouped into lanes by parent, priority,
//! label, sprint, type or a select field, with counts per column; a move
//! into another lane and column changing both in one change; the grouping
//! kept in the layout, recorded and undone.

#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

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

/// Every shown card of `board`.
fn cards(board: &Value) -> Vec<Value> {
    board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["cards"].as_array().unwrap().clone())
        .collect()
}

fn lanes(board: &Value) -> Vec<Value> {
    board["lanes"].as_array().cloned().unwrap_or_default()
}

fn lane_texts(board: &Value, field: &str) -> Vec<String> {
    lanes(board)
        .iter()
        .map(|l| l[field].as_str().unwrap().to_owned())
        .collect()
}

fn lane<'a>(board: &'a Value, id: &str) -> &'a Value {
    board["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id)
        .unwrap_or_else(|| panic!("no lane {id}: {}", board["lanes"]))
}

fn card(board: &Value, key: &str) -> Value {
    cards(board).into_iter().find(|c| c["key"] == key).unwrap()
}

/// The board's lanes are consistent with its cards: every shown card names
/// a lane of the board, IDs are unique, the lane without a value is last,
/// and each lane's counts are its cards in each shown column.
fn check_counts(board: &Value) {
    let lanes = lanes(board);
    assert!(!lanes.is_empty(), "lanes");
    let ids: HashSet<&str> = lanes.iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), lanes.len(), "unique lane IDs");
    for (i, l) in lanes.iter().enumerate() {
        if l["empty"] == true {
            assert_eq!(i, lanes.len() - 1, "the empty lane comes last");
            assert_eq!(l["id"], "none");
        }
    }
    let mut expected: HashMap<String, BTreeMap<String, u64>> = HashMap::new();
    for column in board["columns"].as_array().unwrap() {
        if column["hidden"] == true {
            continue;
        }
        let id = column["id"].as_str().unwrap();
        for c in column["cards"].as_array().unwrap() {
            let lane = c["lane"].as_str().expect("every card has a lane");
            assert!(ids.contains(lane), "card lane {lane} is a board lane");
            *expected
                .entry(lane.to_owned())
                .or_default()
                .entry(id.to_owned())
                .or_insert(0) += 1;
        }
    }
    for l in &lanes {
        let counts: BTreeMap<String, u64> = serde_json::from_value(l["counts"].clone()).unwrap();
        let want = expected
            .remove(l["id"].as_str().unwrap())
            .unwrap_or_default();
        for (column, n) in &counts {
            assert_eq!(
                *n,
                want.get(column).copied().unwrap_or(0),
                "{} in {column}",
                l["id"]
            );
        }
        assert_eq!(l["count"].as_u64().unwrap(), counts.values().sum::<u64>());
    }
    let total: u64 = lanes.iter().map(|l| l["count"].as_u64().unwrap()).sum();
    assert_eq!(total, cards(board).len() as u64);
}

/// A workspace with data set S, seed 1.
fn data_set_s(dir: &TempDir) -> Engine {
    let archive = dir.path().join("seed-S");
    gk_seed::generate(gk_seed::DataSet::Small, 1)
        .unwrap()
        .write_to(&archive)
        .unwrap();
    let e = open(dir);
    call(
        &e,
        "transfer.import",
        json!({ "path": archive.to_string_lossy() }),
    );
    e
}

#[test]
fn every_lane_kind_groups_the_cards_of_data_set_s_with_their_counts() {
    let dir = TempDir::new().unwrap();
    let e = data_set_s(&dir);
    let projects = call(&e, "project.list", json!({}));
    // The project with the most open tasks, and its sub-projects.
    let project = projects["items"]
        .as_array()
        .unwrap()
        .iter()
        .max_by_key(|p| p["open_tasks_with_subprojects"].as_u64().unwrap())
        .unwrap()["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let board = |lanes: &str, sub: bool| {
        call(
            &e,
            "board.get",
            json!({ "project": project, "lanes": lanes, "include_subprojects": sub, "show_older": true }),
        )
    };

    // Without lanes the board has none and cards name none.
    let plain = board("none", false);
    assert!(plain.get("lanes").is_none());
    assert!(cards(&plain).iter().all(|c| c.get("lane").is_none()));

    let by = board("priority", false);
    check_counts(&by);
    assert_eq!(
        lane_texts(&by, "id"),
        ["urgent", "high", "normal", "low", "none"]
    );
    for c in cards(&by) {
        let want = if c["priority"] == "none" {
            "none"
        } else {
            c["priority"].as_str().unwrap()
        };
        assert_eq!(c["lane"], want, "{}", c["key"]);
    }

    for sub in [false, true] {
        let by = board("type", sub);
        check_counts(&by);
        let names: HashSet<String> = lanes(&by)
            .iter()
            .map(|l| l["name"].as_str().unwrap().to_lowercase())
            .collect();
        assert_eq!(names.len(), lanes(&by).len(), "one lane a type name");
        for c in cards(&by) {
            let l = lane(&by, c["lane"].as_str().unwrap());
            assert_eq!(l["name"], c["type"]["name"]);
            assert_eq!(l["value"], c["type"]["name"]);
        }

        let by = board("label", sub);
        check_counts(&by);
        let names: HashSet<String> = lanes(&by)
            .iter()
            .filter(|l| l["empty"] == false)
            .map(|l| l["name"].as_str().unwrap().to_lowercase())
            .collect();
        assert_eq!(names.len() + 1, lanes(&by).len(), "one lane a label name");
        for c in cards(&by) {
            let l = lane(&by, c["lane"].as_str().unwrap());
            match c["labels"].as_array().unwrap().first() {
                Some(first) => assert_eq!(
                    l["name"].as_str().unwrap().to_lowercase(),
                    first["name"].as_str().unwrap().to_lowercase()
                ),
                None => assert_eq!(l["empty"], true),
            }
        }
    }

    let by = board("parent", true);
    check_counts(&by);
    let parented = cards(&by)
        .iter()
        .filter(|c| c.get("parent").is_some())
        .count();
    assert!(parented > 0, "data set S has subtasks");
    for c in cards(&by) {
        let l = lane(&by, c["lane"].as_str().unwrap());
        match c.get("parent") {
            Some(parent) => {
                assert_eq!(&l["key"], parent);
                let title =
                    call(&e, "task.get", json!({ "keys": [parent] }))["items"][0]["title"].clone();
                assert_eq!(l["name"], title);
            }
            None => assert_eq!(l["empty"], true),
        }
    }

    // Sprints: open sprints have a lane even while empty; the backlog last.
    call(
        &e,
        "sprint.create",
        json!({ "project": project, "name": "Next" }),
    );
    let some: Vec<String> = cards(&board("none", false))
        .iter()
        .filter(|c| c["project"] == project.as_str() && c.get("closed_at").is_none())
        .take(3)
        .map(|c| c["key"].as_str().unwrap().to_owned())
        .collect();
    call(
        &e,
        "sprint.create",
        json!({ "project": project, "name": "Now", "tasks": some }),
    );
    let by = board("sprint", false);
    check_counts(&by);
    let names = lane_texts(&by, "name");
    assert!(
        names.iter().any(|n| n == "Next") && names.iter().any(|n| n == "Now"),
        "{names:?}"
    );
    let now = lanes(&by).into_iter().find(|l| l["name"] == "Now").unwrap();
    assert_eq!(now["count"], 3);
    assert_eq!(
        lanes(&by).iter().find(|l| l["name"] == "Next").unwrap()["count"],
        0
    );

    // A select field: a lane per option, in order; values off the list after.
    call(
        &e,
        "field.create",
        json!({ "project": project, "name": "Area", "kind": "select",
                "options": ["Front end", "Back end"] }),
    );
    call(
        &e,
        "task.update",
        json!({ "key": some[0], "fields": { "Area": "Back end" } }),
    );
    let err = fails(
        &e,
        "board.get",
        json!({ "project": project, "lanes": "field" }),
    );
    assert_eq!(err.details["message_id"], "board-lane-field-needed");
    let by = call(
        &e,
        "board.get",
        json!({ "project": project, "lanes": "field", "lane_field": "cf.area", "show_older": true }),
    );
    check_counts(&by);
    assert_eq!(lane_texts(&by, "id"), ["Front end", "Back end", "none"]);
    assert_eq!(card(&by, &some[0])["lane"], "Back end");
}

/// Project TM: TM-1 "Epic", TM-2 and TM-3 its subtasks; labels bug and ui;
/// a sprint; a select field.
fn seed(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    for l in ["bug", "ui"] {
        call(e, "label.create", json!({ "project": "TM", "name": l }));
    }
    call(
        e,
        "task.create",
        json!({ "project": "TM", "title": "Epic" }),
    );
    for t in ["Build", "Test"] {
        call(
            e,
            "task.create",
            json!({ "project": "TM", "title": t, "parent": "TM-1", "priority": "low", "labels": ["bug"] }),
        );
    }
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "select", "options": ["Web", "App"] }),
    );
}

fn status_of(e: &Engine, key: &str) -> String {
    call(e, "task.get", json!({ "keys": [key] }))["items"][0]["status"]["name"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn a_move_into_another_lane_and_column_changes_both_and_one_undo_takes_both_back() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "priority" }),
    );
    let columns: Vec<String> = board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_owned())
        .collect();
    let from = status_of(&e, "TM-2");
    let to = columns.iter().find(|c| **c != from).unwrap().clone();

    let moved = call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "status": to, "at": "top", "changes": { "priority": "urgent" } }),
    );
    assert!(moved["group"].is_string());
    assert_eq!(moved["task"]["priority"], "urgent");
    assert_eq!(moved["task"]["status"]["name"], to.as_str());
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "priority" }),
    );
    assert_eq!(card(&board, "TM-2")["lane"], "urgent");
    check_counts(&board);
    call(&e, "history.undo", json!({}));
    let back = call(&e, "task.get", json!({ "keys": ["TM-2"] }))["items"][0].clone();
    assert_eq!(back["priority"], "low");
    assert_eq!(back["status"]["name"], from.as_str());

    // Labels: off the lane's label, onto the other.
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "remove_labels": ["bug"], "add_labels": ["ui"] } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "label" }),
    );
    assert_eq!(
        lane(&board, card(&board, "TM-2")["lane"].as_str().unwrap())["name"],
        "ui"
    );
    // Into the lane without a value.
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "clear": ["labels"] } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "label" }),
    );
    assert_eq!(card(&board, "TM-2")["lane"], "none");

    // Parents: a task can't go into its own lane; out of a parent's lane.
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "parent" }),
    );
    let epic = lanes(&board)
        .into_iter()
        .find(|l| l["key"] == "TM-1")
        .unwrap();
    assert_eq!(epic["name"], "Epic");
    assert_eq!(epic["count"], 2);
    let err = fails(
        &e,
        "task.move",
        json!({ "key": "TM-1", "changes": { "parent": epic["value"] } }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    call(
        &e,
        "task.move",
        json!({ "key": "TM-3", "status": to, "changes": { "clear": ["parent"] } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "parent" }),
    );
    assert_eq!(card(&board, "TM-3")["lane"], "none");
    assert_eq!(status_of(&e, "TM-3"), to);

    // Sprints: a completed sprint's lane can't take cards.
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "One", "tasks": ["TM-1"] }),
    );
    call(
        &e,
        "sprint.start",
        json!({ "project": "TM", "sprint": "One" }),
    );
    call(
        &e,
        "sprint.complete",
        json!({ "project": "TM", "sprint": "One" }),
    );
    call(
        &e,
        "sprint.create",
        json!({ "project": "TM", "name": "Two" }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "sprint", "show_older": true }),
    );
    check_counts(&board);
    let two = lanes(&board)
        .into_iter()
        .find(|l| l["name"] == "Two")
        .unwrap();
    assert_eq!(two["settable"], true);
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "sprint": two["value"] } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "sprint" }),
    );
    assert_eq!(card(&board, "TM-2")["lane"], two["id"]);

    // A select field, set and cleared with the move.
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "fields": { "cf.area": "App" } } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "field", "lane_field": "Area" }),
    );
    assert_eq!(card(&board, "TM-2")["lane"], "App");
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "fields": { "cf.area": null } } }),
    );
    let board = call(
        &e,
        "board.get",
        json!({ "project": "TM", "lanes": "field", "lane_field": "Area" }),
    );
    assert_eq!(card(&board, "TM-2")["lane"], "none");

    // Type: by name.
    let board = call(&e, "board.get", json!({ "project": "TM", "lanes": "type" }));
    assert!(lanes(&board).iter().all(|l| l["empty"] == false));
    let other = lanes(&board)
        .into_iter()
        .find(|l| l["id"] != card(&board, "TM-2")["lane"])
        .unwrap();
    call(
        &e,
        "task.move",
        json!({ "key": "TM-2", "changes": { "type": other["value"] } }),
    );
    let board = call(&e, "board.get", json!({ "project": "TM", "lanes": "type" }));
    assert_eq!(card(&board, "TM-2")["lane"], other["id"]);

    // Not with a move to another project.
    call(
        &e,
        "project.create",
        json!({ "name": "Other", "key": "ZZ" }),
    );
    let err = fails(
        &e,
        "task.move",
        json!({ "key": "TM-2", "project": "ZZ", "changes": { "priority": "high" } }),
    );
    assert_eq!(err.details["message_id"], "task-move-changes-with-project");
}

#[test]
fn the_grouping_is_kept_in_the_layout_and_undone() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let set = |input: Value| {
        let mut input = input;
        input["project"] = json!("TM");
        call(&e, "layout.update", input)
    };
    let changed = set(json!({ "lanes": "priority" }));
    assert!(changed["group"].is_string());
    assert_eq!(changed["layout"]["lanes"], "priority");
    let board = call(&e, "board.get", json!({ "project": "TM" }));
    assert_eq!(lane(&board, "low")["count"], 2);
    // Which lanes are collapsed is each person's own setting, not the layout's.
    assert!(lane(&board, "low").get("collapsed").is_none());
    assert!(changed["layout"].get("collapsed_lanes").is_none());
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "collapse_lanes": ["low"] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    // The read can ask for another grouping without changing the layout.
    let other = call(&e, "board.get", json!({ "project": "TM", "lanes": "none" }));
    assert!(other.get("lanes").is_none());

    set(json!({ "lanes": "label" }));
    call(&e, "history.undo", json!({}));
    let layout = call(&e, "layout.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["lanes"], "priority");

    // A field: it must be a select field of the project.
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "lanes": "field" }),
    );
    assert_eq!(err.details["message_id"], "board-lane-field-needed");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "lanes": "field", "lane_field": "Points" }),
    );
    assert_eq!(err.details["message_id"], "board-lane-field-unknown");
    let changed = set(json!({ "lanes": "field", "lane_field": "area" }));
    assert_eq!(changed["layout"]["lane_field"], "cf.area");

    // The field renamed: the layout follows; deleted: no lanes.
    call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Area", "name": "Zone" }),
    );
    let layout = call(&e, "layout.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["lane_field"], "cf.zone");
    sure(
        &e,
        "field.delete",
        json!({ "project": "TM", "field": "Zone", "expected_task_count": 0 }),
    );
    let layout = call(&e, "layout.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["lanes"], "none");
    assert_eq!(layout["lane_field"], "");
    let board = call(&e, "board.get", json!({ "project": "TM" }));
    assert!(board.get("lanes").is_none());

    // Lanes off again; undo brings them back.
    set(json!({ "lanes": "priority" }));
    set(json!({ "lanes": "none" }));
    call(&e, "history.undo", json!({}));
    let layout = call(&e, "layout.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["lanes"], "priority");
}

/// Project BIG with `total` tasks of which `open` are open, the rest closed
/// long ago; four labels on every other task; a parent for every tenth open
/// task among twenty epics.
fn large_board(e: &Engine, total: usize, open: usize) {
    use gk_store::rusqlite::params;
    call(e, "project.create", json!({ "name": "Big", "key": "BIG" }));
    for l in ["bug", "ui", "backend", "docs"] {
        call(e, "label.create", json!({ "project": "BIG", "name": l }));
    }
    e.store()
        .write("test.large_board", move |tx| {
            let project: Vec<u8> =
                tx.query_row("SELECT id FROM projects WHERE key = 'BIG'", [], |r| {
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
            let labels = ids("SELECT id FROM labels WHERE project_id = ?1 ORDER BY rank")?;
            let done = statuses.last().unwrap().clone();
            let long_ago = NOW - 400 * 86_400_000;
            let mut epics: Vec<Vec<u8>> = Vec::new();
            for i in 0..total {
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
                let parent =
                    (is_open && i >= 20 && i % 10 == 0).then(|| epics[(i / 10) % 20].clone());
                tx.execute(
                    "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                        status_entered_at, type_id, priority, rank, created_at, created_by_kind,
                        created_by_name, edited_at, updated_at, closed_at, parent_id)
                     VALUES (?1, ?2, ?3, ?4, lower(?4), ?5, ?6, ?7, ?8, ?9, ?6, 'user', 'You',
                        ?6, ?6, ?10, ?11)",
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
                        parent,
                    ],
                )?;
                if i < 20 {
                    epics.push(id.clone());
                }
                if i % 2 == 0 {
                    tx.execute(
                        "INSERT INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
                        params![id, labels[i % labels.len()]],
                    )?;
                }
            }
            tx.execute(
                "UPDATE projects SET next_number = ?1 WHERE id = ?2",
                params![(total + 1) as i64, project],
            )?;
            Ok::<_, gk_engine::Failure>(())
        })
        .unwrap();
}

/// A board of data set L's size grouped into lanes stays within the board's
/// bound: generous in a debug build, tight in a release build.
#[test]
fn lanes_on_a_board_of_twenty_thousand_tasks_stay_within_the_board_bound() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    large_board(&e, 20_000, 1_000);
    let bound = if cfg!(debug_assertions) {
        std::time::Duration::from_secs(3)
    } else {
        std::time::Duration::from_millis(500)
    };
    for by in ["priority", "label", "parent", "type"] {
        let board = || call(&e, "board.get", json!({ "project": "BIG", "lanes": by }));
        board();
        let (took, out) = (0..3)
            .map(|_| {
                let start = std::time::Instant::now();
                let out = board();
                (start.elapsed(), out)
            })
            .min_by_key(|(t, _)| *t)
            .unwrap();
        assert_eq!(cards(&out).len(), 1_000);
        check_counts(&out);
        if by == "parent" {
            // Twenty epics with subtasks, and the lane without a parent.
            assert_eq!(lanes(&out).len(), 21, "{by}");
        }
        eprintln!("board of 20,000 tasks in lanes by {by}: {took:?}");
        assert!(took < bound, "{by}: {took:?}");
    }
}
