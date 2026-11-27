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
