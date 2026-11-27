// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Card colours and covers on the board: what colours a card's edge, which
//! image is its cover, and that both are layout settings like the others,
//! checked, recorded and undone.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC, when the tests start.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &TempDir) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    // A millisecond on with every reading, so files attached one after the
    // other have their order.
    let tick = AtomicI64::new(0);
    config.clock = Arc::new(move || NOW + tick.fetch_add(1, Ordering::Relaxed));
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).unwrap_err()
}

/// A small real PNG image.
fn png() -> String {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255]))
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    gk_core::base64_encode(&out.into_inner())
}

/// Project TM: TM-1 urgent with label "bug", TM-2 low with "docs", TM-3
/// with no priority or label.
fn seed(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(
        e,
        "label.create",
        json!({ "project": "TM", "name": "bug", "hue": "red" }),
    );
    call(
        e,
        "label.create",
        json!({ "project": "TM", "name": "docs", "hue": "teal" }),
    );
    for t in [
        json!({ "project": "TM", "title": "Crash", "priority": "urgent", "labels": ["bug"] }),
        json!({ "project": "TM", "title": "Guide", "priority": "low", "labels": ["docs"] }),
        json!({ "project": "TM", "title": "Tidy" }),
    ] {
        call(e, "task.create", t);
    }
}

/// Every shown card of the board, by key.
fn cards(e: &Engine) -> Vec<Value> {
    let board = call(e, "board.get", json!({ "project": "TM" }));
    board["columns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["cards"].as_array().unwrap().clone())
        .collect()
}

fn card(e: &Engine, key: &str) -> Value {
    cards(e).into_iter().find(|c| c["key"] == key).unwrap()
}

fn edge(e: &Engine, key: &str) -> Value {
    card(e, key)["edge"].clone()
}

#[test]
fn cards_are_coloured_by_type_priority_label_or_the_first_matching_rule() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    // Off by default: no edges.
    assert!(cards(&e).iter().all(|c| c.get("edge").is_none()));

    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_by": "priority" }),
    );
    assert_eq!(
        edge(&e, "TM-1"),
        json!({ "hue": "red", "by": "priority", "name": "urgent" })
    );
    assert_eq!(edge(&e, "TM-2")["hue"], "grey");
    assert_eq!(edge(&e, "TM-3"), Value::Null);

    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_by": "label" }),
    );
    assert_eq!(
        edge(&e, "TM-2"),
        json!({ "hue": "teal", "by": "label", "name": "docs" })
    );
    assert_eq!(edge(&e, "TM-3"), Value::Null);

    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_by": "type" }),
    );
    let by_type = edge(&e, "TM-3");
    assert_eq!(by_type["by"], "type");
    assert_eq!(by_type["name"], card(&e, "TM-3")["type"]["name"]);

    // Rules in order; the first that matches wins; a rule without a name
    // is named by its query.
    let changed = call(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_by": "query", "colour_rules": [
            { "query": "label:bug", "hue": "pink", "name": "Bugs" },
            { "query": " priority:urgent,low ", "hue": "yellow" },
        ] }),
    );
    assert_eq!(
        changed["layout"]["colour_rules"][1],
        json!({ "query": "priority:urgent,low", "hue": "yellow" })
    );
    assert_eq!(
        edge(&e, "TM-1"),
        json!({ "hue": "pink", "by": "query", "name": "Bugs" })
    );
    assert_eq!(
        edge(&e, "TM-2"),
        json!({ "hue": "yellow", "by": "query", "name": "priority:urgent,low" })
    );
    assert_eq!(edge(&e, "TM-3"), Value::Null);

    // A query is checked when it is saved, and the number of rules too.
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_rules": [{ "query": "label:nope", "hue": "red" }] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(err.details["path"], "colour_rules[0].query");
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_rules": [{ "query": "  ", "hue": "red" }] }),
    );
    assert_eq!(err.details["message_id"], "board-colour-rule-empty");
    let many: Vec<Value> = (0..21)
        .map(|_| json!({ "query": "label:bug", "hue": "red" }))
        .collect();
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "colour_rules": many }),
    );
    assert_eq!(err.details["message_id"], "board-colour-rules-max");

    // A renamed label is renamed in the rules too.
    call(
        &e,
        "label.update",
        json!({ "project": "TM", "label": "bug", "name": "defect" }),
    );
    let layout = call(&e, "layout.get", json!({ "project": "TM" }));
    assert_eq!(layout["layout"]["colour_rules"][0]["query"], "label:defect");
    assert_eq!(edge(&e, "TM-1")["name"], "Bugs");

    // Undo takes the rename's rewrite back, then the rules themselves.
    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    let layout = call(&e, "layout.get", json!({ "project": "TM" }));
    assert_eq!(layout["layout"]["colour_by"], "type");
    assert_eq!(layout["layout"]["colour_rules"], json!([]));
}

#[test]
fn a_card_shows_its_chosen_cover_or_its_first_image() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let attach = |name: &str, data: &str| {
        call(
            &e,
            "file.attach",
            json!({ "task": "TM-1", "name": name, "data_base64": data }),
        )["file"]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let notes = attach("notes.txt", "aGVsbG8=");
    let first = attach("sketch.png", &png());
    let second = attach("photo.png", &png());

    // Off by default.
    assert!(card(&e, "TM-1").get("cover").is_none());
    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "covers": "first_image" }),
    );
    let cover = card(&e, "TM-1")["cover"].clone();
    assert_eq!(cover["attachment"], first.as_str());
    assert_eq!(cover["name"], "sketch.png");
    assert_eq!(cover["mime"], "image/png");
    assert_eq!(cover["width"], 4);
    assert!(card(&e, "TM-2").get("cover").is_none());

    // Only the task's own images can be its cover.
    let err = fails(
        &e,
        "file.cover",
        json!({ "task": "TM-1", "attachment": notes }),
    );
    assert_eq!(err.details["message_id"], "file-cover-not-image");
    let err = fails(
        &e,
        "file.cover",
        json!({ "task": "TM-2", "attachment": second }),
    );
    assert_eq!(err.details["message_id"], "file-cover-other-task");

    let chosen = call(
        &e,
        "file.cover",
        json!({ "task": "TM-1", "attachment": second }),
    );
    assert_eq!(chosen["task"], "TM-1");
    assert_eq!(chosen["cover"]["cover"], true);
    assert!(chosen["group"].is_string());
    assert_eq!(card(&e, "TM-1")["cover"]["attachment"], second.as_str());
    let files = call(&e, "file.get", json!({ "attachment": second }));
    assert_eq!(files["cover"], true);

    // "Chosen" shows only chosen covers.
    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "covers": "chosen" }),
    );
    assert_eq!(card(&e, "TM-1")["cover"]["name"], "photo.png");

    // A cover in the trash isn't shown; restoring the file brings it back.
    call(&e, "file.delete", json!({ "attachment": second }));
    assert!(card(&e, "TM-1").get("cover").is_none());
    call(&e, "history.undo", json!({}));
    assert_eq!(card(&e, "TM-1")["cover"]["name"], "photo.png");

    // Choosing none, and undoing that.
    call(&e, "file.cover", json!({ "task": "TM-1" }));
    assert!(card(&e, "TM-1").get("cover").is_none());
    call(&e, "history.undo", json!({}));
    assert_eq!(card(&e, "TM-1")["cover"]["name"], "photo.png");

    // A duplicate keeps the copy of its cover.
    let copy = call(&e, "task.duplicate", json!({ "key": "TM-1" }));
    let key = copy["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(card(&e, &key)["cover"]["name"], "photo.png");
    assert_ne!(card(&e, &key)["cover"]["attachment"], second.as_str());
}

#[test]
fn covers_and_colour_rules_travel_with_an_export() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    seed(&e);
    let id = call(
        &e,
        "file.attach",
        json!({ "task": "TM-2", "name": "shot.png", "data_base64": png() }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        &e,
        "file.cover",
        json!({ "task": "TM-2", "attachment": id }),
    );
    call(
        &e,
        "layout.update",
        json!({ "project": "TM", "covers": "chosen", "colour_by": "query",
                "colour_rules": [{ "query": "label:docs", "hue": "green" }] }),
    );
    let zip = dir.path().join("tm.zip");
    call(
        &e,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );
    let other = TempDir::new().unwrap();
    let b = open(&other);
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    let copy = card(&b, "TM-2");
    assert_eq!(copy["cover"]["name"], "shot.png");
    assert_eq!(copy["edge"]["hue"], "green");
}

/// A board like data set L's: project BIG with `total` tasks, of which
/// `open` are open across the first statuses and the rest closed long ago
/// in the last one; four labels; two images on each open task, sharing the
/// stored file of `sketch.png`.
fn large_board(e: &Engine, total: usize, open: usize) {
    use gk_store::rusqlite::params;
    call(e, "project.create", json!({ "name": "Big", "key": "BIG" }));
    for l in ["bug", "ui", "backend", "docs"] {
        call(e, "label.create", json!({ "project": "BIG", "name": l }));
    }
    let first = call(
        e,
        "task.create",
        json!({ "project": "BIG", "title": "Seed" }),
    );
    call(
        e,
        "file.attach",
        json!({ "task": first["task"]["key"], "name": "sketch.png", "data_base64": png() }),
    );
    e.store()
        .write("test.large_board", move |tx| {
            let project: Vec<u8> =
                tx.query_row("SELECT id FROM projects WHERE key = 'BIG'", [], |r| {
                    r.get(0)
                })?;
            let sha: String =
                tx.query_row("SELECT sha256 FROM attachments LIMIT 1", [], |r| r.get(0))?;
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
                let title = format!("Task {i}");
                tx.execute(
                    "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                        status_entered_at, type_id, priority, rank, created_at, created_by_kind,
                        created_by_name, edited_at, updated_at, closed_at)
                     VALUES (?1, ?2, ?3, ?4, lower(?4), ?5, ?6, ?7, ?8, ?9, ?6, 'user', 'You',
                        ?6, ?6, ?10)",
                    params![
                        id,
                        project,
                        (i + 2) as i64,
                        title,
                        status,
                        at,
                        types[i % types.len()],
                        (i % 5) as i64,
                        format!("r{i:07}"),
                        (!is_open).then_some(at),
                    ],
                )?;
                if i % 2 == 0 {
                    tx.execute(
                        "INSERT INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
                        params![id, labels[i % labels.len()]],
                    )?;
                }
                if is_open {
                    for (k, name) in ["one.png", "two.png"].iter().enumerate() {
                        tx.execute(
                            "INSERT INTO attachments (id, owner_kind, owner_id, file_name, mime,
                                size, sha256, width, height, actor_kind, actor_name, created_at)
                             VALUES (?1, 'task', ?2, ?3, 'image/png', 70, ?4, 4, 3, 'user',
                                'You', ?5)",
                            params![
                                gk_core::Id::generate().as_bytes().to_vec(),
                                id,
                                name,
                                sha,
                                at + k as i64,
                            ],
                        )?;
                    }
                }
            }
            tx.execute(
                "UPDATE projects SET next_number = ?1 WHERE id = ?2",
                params![(total + 2) as i64, project],
            )?;
            Ok::<_, gk_engine::Failure>(())
        })
        .unwrap();
}

/// The board of data set L's size with twenty colour rules and covers from
/// the first image stays within a bound: generous in a debug build, where
/// it runs with the other tests, and tight in a release build.
#[test]
fn colour_rules_and_covers_on_a_large_board_stay_within_a_bound() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    large_board(&e, 20_000, 1_000);
    let board = || call(&e, "board.get", json!({ "project": "BIG" }));
    let timed = || {
        // Once to warm the caches, then the best of three, so a busy
        // machine's pause doesn't count.
        board();
        (0..3)
            .map(|_| {
                let start = std::time::Instant::now();
                let out = board();
                (start.elapsed(), out)
            })
            .min_by_key(|(t, _)| *t)
            .unwrap()
    };
    let (plain, out) = timed();
    let shown: usize = out["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["cards"].as_array().unwrap().len())
        .sum();
    assert_eq!(shown, 1_001, "the open tasks and the seed task");

    let words = ["bug", "ui", "backend", "docs"];
    let rules: Vec<Value> = (0..20)
        .map(|i| {
            let levels = ["low", "normal", "high", "urgent"];
            let hues = ["red", "blue", "green", "pink"];
            json!({
                "query": format!("label:{} priority>={}", words[i % 4], levels[i / 5]),
                "hue": hues[i % 4],
            })
        })
        .collect();
    call(
        &e,
        "layout.update",
        json!({ "project": "BIG", "colour_by": "query", "colour_rules": rules, "covers": "first_image" }),
    );
    let (coloured, out) = timed();
    let cards: Vec<&Value> = out["columns"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["cards"].as_array().unwrap())
        .collect();
    assert_eq!(cards.len(), 1_001);
    let with_cover = cards.iter().filter(|c| c.get("cover").is_some()).count();
    assert_eq!(
        with_cover, 1_001,
        "every open task and the seed have an image"
    );
    let edged = cards.iter().filter(|c| c.get("edge").is_some()).count();
    assert!(edged > 0 && edged < cards.len(), "{edged}");
    assert!(
        cards
            .iter()
            .filter(|c| c["key"] != "BIG-1")
            .all(|c| c["cover"]["name"] == "one.png")
    );

    let bound = if cfg!(debug_assertions) {
        std::time::Duration::from_secs(3)
    } else {
        std::time::Duration::from_millis(500)
    };
    eprintln!("board of 20,000 tasks: {plain:?} plain, {coloured:?} with 20 rules and covers");
    assert!(coloured < bound, "{coloured:?} (plain {plain:?})");
}
