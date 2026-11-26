// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Importing CSV files: exports shaped like Jira's and Redmine's in several
//! encodings, the dry run, presets, Undo, and files that are refused.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

fn open(dir: &TempDir) -> Engine {
    let e = Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(Role::App),
    )
    .unwrap();
    call(
        &e,
        "project.create",
        json!({ "name": "Web", "key": "SHOP" }),
    );
    e
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::Cli), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/csv")
        .join(name)
}

fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

fn get(e: &Engine, key: &str) -> Value {
    call(
        e,
        "task.get",
        json!({ "keys": [key], "include": ["links", "subtasks"] }),
    )["items"][0]
        .clone()
}

fn codes(row: &Value) -> Vec<String> {
    row["problems"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| p["code"].as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn task_count(e: &Engine) -> u64 {
    call(e, "task.search", json!({ "query": "project:SHOP" }))["total"]
        .as_u64()
        .unwrap()
}

#[test]
fn a_jira_export_is_checked_then_imported_as_one_change() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    call(
        &e,
        "field.create",
        json!({ "project": "SHOP", "name": "Points", "kind": "integer" }),
    );
    let path = fixture("jira.csv").to_string_lossy().into_owned();
    let input = json!({
        "project": "SHOP",
        "path": path,
        "columns": [{ "column": "Custom field (Story Points)", "to": "field.Points" }],
        "dry_run": true,
    });
    let check = call(&e, "transfer.import_csv", input.clone());
    assert_eq!(check["preset"], "jira", "detected from the header row");
    assert_eq!(check["format"]["encoding"], "utf8");
    assert_eq!(check["format"]["separator"], ",");
    assert_eq!(check["counts"]["rows"], 4);
    assert_eq!(check["counts"]["created"], 3);
    assert_eq!(check["counts"]["skipped"], 1);
    assert_eq!(check["blocked"], true);
    let rows = check["rows"].as_array().unwrap();
    assert_eq!(rows[0]["key"], "SHOP-1");
    assert_eq!(rows[0]["source_key"], "SHOP-1");
    assert_eq!(rows[3]["outcome"], "skipped");
    assert_eq!(codes(&rows[3]), ["no_title"]);
    assert_eq!(
        rows[3]["line"], 6,
        "the description of row 1 spans two lines"
    );
    let summary = check["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Summary")
        .unwrap();
    assert_eq!(summary["to"], "title");
    assert_eq!(summary["samples"][0], "Build the importer");
    assert_eq!(task_count(&e), 0, "a dry run writes nothing");

    // Blocked without skip_invalid; nothing written.
    let mut real = input.clone();
    real["dry_run"] = json!(false);
    let err = e
        .call(
            &Context::user(Via::Cli),
            "transfer.import_csv",
            real.clone(),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(task_count(&e), 0);

    real["skip_invalid"] = json!(true);
    let done = call(&e, "transfer.import_csv", real);
    assert_eq!(done["counts"]["created"], 3);
    assert_eq!(done["counts"]["parents"], 1);
    assert_eq!(done["counts"]["time_entries"], 1);
    assert_eq!(done["counts"]["comments"], 1);
    assert_eq!(done["counts"]["links"], 2);
    assert!(
        done.get("new_statuses").is_none(),
        "Jira's names match ignoring case"
    );
    assert_eq!(done["new_types"], json!(["Story", "Sub-task"]));
    assert_eq!(done["new_labels"], json!(["backend", "csv", "docs"]));

    let one = get(&e, "SHOP-1");
    assert_eq!(one["title"], "Build the importer");
    assert_eq!(one["status"]["name"], "In progress");
    assert_eq!(one["priority"], "high");
    assert_eq!(one["due_date"], "2024-02-02");
    assert_eq!(one["estimate"], "1d");
    let labels: Vec<&str> = one["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["backend", "csv"]);
    let two = get(&e, "SHOP-2");
    assert_eq!(two["parent"], "SHOP-1", "parent by Jira's issue id");
    assert_eq!(get(&e, "SHOP-3")["priority"], "low");
    let comments = call(&e, "comment.list", json!({ "key": "SHOP-1" }));
    assert_eq!(comments["items"][0]["body"], "Looks good; ship it");

    // One Undo takes it all back.
    call(&e, "history.revert", json!({ "groups": [done["group"]] }));
    assert_eq!(task_count(&e), 0);
}

#[test]
fn a_redmine_export_in_windows_1252_keeps_its_accents_and_links() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    // The fixture keeps what Redmine writes: CRLF and Windows-1252.
    let bytes = std::fs::read(fixture("redmine-1252.csv")).unwrap();
    assert!(bytes.windows(2).any(|w| w == b"\r\n"), "CRLF line endings");
    assert!(bytes.contains(&0xE9), "\"Café\" in Windows-1252");
    assert!(std::str::from_utf8(&bytes).is_err(), "not UTF-8");
    let path = fixture("redmine-1252.csv").to_string_lossy().into_owned();
    let done = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "preset": "redmine" }),
    );
    assert_eq!(done["format"]["encoding"], "windows1252");
    assert_eq!(done["format"]["date_order"], "mdy");
    assert_eq!(done["counts"]["created"], 2);
    assert_eq!(done["counts"]["links"], 1, "both sides of one link");
    assert_eq!(done["counts"]["parents"], 1);
    let rows = done["rows"].as_array().unwrap();
    assert_eq!(codes(&rows[0]), ["link_not_found"]);
    let first = get(&e, "SHOP-1");
    assert_eq!(first["title"], "Menü überarbeiten");
    assert_eq!(first["description"], "Die Kosten: 5 € – „heute“");
    assert_eq!(first["start_date"], "2024-01-15");
    assert_eq!(first["due_date"], "2024-02-20");
    assert_eq!(first["estimate"], "2h 30m");
    assert_eq!(first["priority"], "normal");
    assert_eq!(first["labels"][0]["name"], "UI");
    let second = get(&e, "SHOP-2");
    assert_eq!(second["parent"], "SHOP-1");
    assert_eq!(second["priority"], "urgent");
    assert_eq!(second["type"]["name"], "Bug");
}

#[test]
fn utf16_files_and_mappings_by_name() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    let text = std::fs::read_to_string(fixture("generic.csv")).unwrap();
    for (name, bytes) in [
        ("le.csv", {
            let mut b = vec![0xFF, 0xFE];
            b.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            b
        }),
        ("be.csv", {
            let mut b = vec![0xFE, 0xFF];
            b.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
            b
        }),
        ("bom8.csv", {
            let mut b = vec![0xEF, 0xBB, 0xBF];
            b.extend_from_slice(text.as_bytes());
            b
        }),
    ] {
        let path = write(&dir, name, &bytes);
        let check = call(
            &e,
            "transfer.import_csv",
            json!({ "project": "SHOP", "path": path, "dry_run": true }),
        );
        assert_eq!(check["format"]["separator"], ";", "{name}");
        assert_eq!(check["format"]["bom"], true, "{name}");
        assert_eq!(check["format"]["decimal_comma"], true, "{name}");
        assert!(check.get("preset").is_none());
        let rows = check["rows"].as_array().unwrap();
        assert_eq!(rows[0]["title"], "Ein Ärger", "{name}");
        assert_eq!(codes(&rows[1]), ["bad_date"], "{name}");
        let to: Vec<Value> = check["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["to"].clone())
            .collect();
        assert_eq!(
            to,
            [
                json!("title"),
                json!("status"),
                json!("due_date"),
                json!("estimate"),
                Value::Null
            ]
        );
    }
    let path = write(&dir, "plain.csv", text.as_bytes());
    let done = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "values": { "statuses": { "Offen": "To do" } } }),
    );
    assert!(
        done.get("new_statuses").is_none(),
        "mapped to an existing status"
    );
    let first = get(&e, "SHOP-1");
    assert_eq!(first["status"]["name"], "To do");
    assert_eq!(first["due_date"], "2025-12-31");
    assert_eq!(first["estimate"], "2h 30m");
    assert_eq!(get(&e, "SHOP-2")["estimate"], "1h 30m");

    let refused = e
        .call(
            &Context::user(Via::Cli),
            "transfer.import_csv",
            json!({ "project": "SHOP", "path": path, "create_missing": false,
                    "values": { "statuses": { "Offen": "Nowhere" } }, "dry_run": true }),
        )
        .unwrap();
    assert_eq!(codes(&refused["rows"][0]), ["unknown_status"]);
}

#[test]
fn presets_are_saved_listed_and_deleted_with_history() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    let list = call(&e, "csv_preset.list", json!({}));
    let names: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["jira", "redmine"]);
    let saved = call(
        &e,
        "csv_preset.save",
        json!({ "name": "Ours", "columns": [{ "column": "Title", "to": "title" },
                                            { "column": "Due", "to": "ignore" }],
                "format": { "date_order": "dmy" } }),
    );
    assert_eq!(saved["items"].as_array().unwrap().len(), 3);
    let path = write(
        &dir,
        "g.csv",
        &std::fs::read(fixture("generic.csv")).unwrap(),
    );
    let check = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "preset": "ours", "dry_run": true }),
    );
    assert_eq!(check["preset"], "Ours");
    assert_eq!(
        check["columns"][1].get("to"),
        None,
        "the preset maps only its columns"
    );
    for bad in [
        json!({ "name": "jira" }),
        json!({ "name": "" }),
        json!({ "name": "x", "columns": [{ "column": "a", "to": "nowhere" }] }),
    ] {
        let err = e
            .call(&Context::user(Via::Cli), "csv_preset.save", bad.clone())
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Invalid, "{bad}");
    }
    let gone = call(&e, "csv_preset.delete", json!({ "name": "Ours" }));
    assert_eq!(gone["items"].as_array().unwrap().len(), 2);
    call(&e, "history.revert", json!({ "groups": [gone["group"]] }));
    assert_eq!(
        call(&e, "csv_preset.list", json!({}))["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn hostile_files_are_refused() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    let refuse = |name: &str, bytes: &[u8], extra: Value| {
        let path = write(&dir, name, bytes);
        let mut input = json!({ "project": "SHOP", "path": path, "dry_run": true });
        for (k, v) in extra.as_object().unwrap() {
            input[k] = v.clone();
        }
        e.call(&Context::user(Via::Cli), "transfer.import_csv", input)
            .unwrap_err()
    };
    let err = refuse("bin.csv", b"Title\na\0b\n", json!({}));
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = refuse(
        "bad.csv",
        b"Title\nCaf\xe9\n",
        json!({ "format": { "encoding": "utf8" } }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = refuse("open.csv", b"Title\n\"never closed\n", json!({}));
    assert_eq!(err.code, ErrorCode::Invalid);
    let long = format!("Title\n{}\n", "x".repeat(1024 * 1024 + 10));
    let err = refuse("long.csv", long.as_bytes(), json!({}));
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = refuse(
        "sep.csv",
        b"Title\n",
        json!({ "format": { "separator": ",," } }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = refuse(
        "col.csv",
        b"Title\nx\n",
        json!({ "columns": [{ "column": "Nope", "to": "title" }] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let path = write(&dir, "ctl.csv", b"Title,Status\nbell\x07,Done\n");
    let check = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "dry_run": true }),
    );
    assert!(codes(&check["rows"][0]).contains(&"control_character".to_owned()));
    let mut many = String::from("Title\n");
    for i in 0..20_001 {
        many.push_str(&format!("t{i}\n"));
    }
    let err = refuse("many.csv", many.as_bytes(), json!({}));
    assert_eq!(err.code, ErrorCode::Limit);
}

#[test]
fn an_agent_imports_only_from_the_exports_folder() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    let session = e
        .start_session(&gk_engine::SessionStart {
            name: "Agent".into(),
            client_id: None,
            client_version: None,
            profile: gk_api::Profile::Full,
        })
        .unwrap();
    let agent = Context::agent("Agent", session, gk_api::Profile::Full);
    let outside = fixture("jira.csv").to_string_lossy().into_owned();
    let err = e
        .call(
            &agent,
            "transfer.import_csv",
            json!({ "project": "SHOP", "path": outside, "dry_run": true }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    let exports = e.store().layout().exports_dir();
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::copy(fixture("jira.csv"), exports.join("jira.csv")).unwrap();
    let check = e
        .call(
            &agent,
            "transfer.import_csv",
            json!({ "project": "SHOP", "path": "jira.csv", "dry_run": true, "only_problems": true }),
        )
        .unwrap();
    assert_eq!(check["rows"].as_array().unwrap().len(), 1);
}

#[test]
fn a_row_the_task_rules_refuse_stops_the_import_and_leaves_nothing() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    call(
        &e,
        "field.create",
        json!({ "project": "SHOP", "name": "Team", "kind": "text", "required": true }),
    );
    let path = write(
        &dir,
        "req.csv",
        b"Title,Type,Team\nHas a team,Epic,Core\nHas none,Epic,\n",
    );
    let input = json!({ "project": "SHOP", "path": path, "dry_run": true });
    let check = call(&e, "transfer.import_csv", input.clone());
    assert_eq!(check["counts"]["created"], 1);
    assert_eq!(codes(&check["rows"][1]), ["refused"]);
    let mut real = input;
    real["dry_run"] = json!(false);
    let err = e
        .call(
            &Context::user(Via::Cli),
            "transfer.import_csv",
            real.clone(),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(task_count(&e), 0);
    let types = call(&e, "project.get", json!({ "project": "SHOP" }))["types"].clone();
    assert!(
        !types
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "Epic"),
        "the type made for the failed import is taken back: {types}"
    );
    real["skip_invalid"] = json!(true);
    let done = call(&e, "transfer.import_csv", real);
    assert_eq!(done["counts"]["created"], 1);
    assert!(done["setup_group"].is_string());
    call(&e, "history.revert", json!({ "groups": [done["group"]] }));
    assert_eq!(task_count(&e), 0, "Undo moves the tasks to the trash");
}

#[test]
fn versions_by_name_and_the_rules_of_each_status_hold_for_every_row() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    call(
        &e,
        "version.create",
        json!({ "project": "SHOP", "name": "1.0" }),
    );
    call(
        &e,
        "rules.update",
        json!({ "project": "SHOP", "statuses": [{ "status": "In progress", "rules": {
            "required": ["estimate"],
            "set": [{ "field": "start_date", "value": "today", "only_if_empty": true }],
        } }] }),
    );
    let path = write(
        &dir,
        "rules.csv",
        b"Title,Status,Fix Version/s,Estimate,Recurrence\n\
          Started,In progress,1.0,2h,every week\n\
          No estimate,In progress,1.0,,\n\
          Later,To do,2.0,,\n",
    );
    let check = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "dry_run": true }),
    );
    let to: Vec<Value> = check["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["to"].clone())
        .collect();
    assert_eq!(
        to,
        [
            json!("title"),
            json!("status"),
            json!("fix_version"),
            json!("estimate"),
            Value::Null
        ],
        "a repeat rule is not imported"
    );
    let rows = check["rows"].as_array().unwrap();
    assert_eq!(rows[0]["outcome"], "created");
    assert_eq!(
        codes(&rows[1]),
        ["status_rule"],
        "the status needs an estimate"
    );
    assert_eq!(rows[1]["outcome"], "skipped");
    assert!(
        rows[1]["problems"][0]["message"]
            .as_str()
            .unwrap()
            .contains("estimate"),
        "{}",
        rows[1]
    );
    assert_eq!(codes(&rows[2]), ["unknown_version"], "no such version");
    assert_eq!(rows[2]["outcome"], "created");
    assert_eq!(task_count(&e), 0, "a dry run writes nothing");

    let done = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path, "skip_invalid": true }),
    );
    assert_eq!(done["counts"]["created"], 2);
    let started = get(&e, "SHOP-1");
    assert_eq!(started["fix_version"]["name"], "1.0");
    assert!(
        started["start_date"].is_string(),
        "the status's actions apply: {started}"
    );
    assert!(started.get("recurrence").is_none());
    let later = get(&e, "SHOP-2");
    assert_eq!(later["title"], "Later");
    assert!(later.get("fix_version").is_none());
}

/// A large file: every row becomes a task in one change, the tasks keep the
/// file's order in their columns, one Undo takes them back, and it takes
/// seconds, not minutes. Making each task once cost a read of its whole
/// column and a search of everything the import had changed so far, so
/// the time grew with the square of the rows. The bound is generous, for
/// slow machines and unoptimised builds; the release build takes well
/// under a second.
#[test]
fn a_large_file_is_imported_quickly_in_file_order_and_undone_at_once() {
    const ROWS: usize = 4_000;
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    let statuses = ["Backlog", "To do", "In progress", "Done"];
    let mut text = String::from("Title,Status,Priority,Labels,Estimate\r\n");
    for n in 0..ROWS {
        text.push_str(&format!(
            "Imported task {n},{},{},{},{}h\r\n",
            statuses[n % 4],
            ["low", "medium", "high"][n % 3],
            ["web", "api"][n % 2],
            n % 8 + 1
        ));
    }
    let path = write(&dir, "large.csv", text.as_bytes());
    let started = std::time::Instant::now();
    let done = call(
        &e,
        "transfer.import_csv",
        json!({ "project": "SHOP", "path": path }),
    );
    let took = started.elapsed();
    assert_eq!(done["counts"]["created"], ROWS as u64);
    assert_eq!(task_count(&e), ROWS as u64);
    assert!(
        took < std::time::Duration::from_secs(20),
        "{ROWS} rows took {took:?}"
    );

    // Each column holds its rows in the file's order.
    let board = call(
        &e,
        "board.get",
        json!({ "project": "SHOP", "show_older": true }),
    );
    for column in board["columns"].as_array().unwrap() {
        let numbers: Vec<u64> = column["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["key"].as_str().unwrap()["SHOP-".len()..].parse().unwrap())
            .collect();
        assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{numbers:?}");
    }
    assert_eq!(get(&e, "SHOP-2")["labels"][0]["name"], "api");

    call(&e, "history.revert", json!({ "groups": [done["group"]] }));
    assert_eq!(task_count(&e), 0);
}
