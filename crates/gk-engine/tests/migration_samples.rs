// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Workspaces written by earlier releases, opened by this engine as the app
//! opens them: backed up, upgraded, and with everything their release wrote
//! still there and still working.
//!
//! The samples are in `crates/gk-store/tests/samples/`; `cargo xtask
//! migration-samples` makes each by running the same commands with a
//! released `glyphkeel`, as far as that release has them. The values below
//! are the ones those commands wrote.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use gk_store::rusqlite::Connection;
use serde_json::{Value, json};

struct Sample {
    file: PathBuf,
    release: (u64, u64, u64),
    schema: i64,
    sql: String,
}

fn samples() -> Vec<Sample> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gk-store/tests/samples");
    let mut found: Vec<Sample> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .map(|file| {
            let sql = fs::read_to_string(&file).unwrap();
            let field = |name: &str| {
                sql.lines()
                    .find_map(|l| l.strip_prefix(&format!("-- {name}: ")))
                    .unwrap()
                    .trim()
                    .to_owned()
            };
            let parts: Vec<u64> = field("release")
                .split('.')
                .map(|p| p.parse().unwrap())
                .collect();
            let schema = field("schema").parse().unwrap();
            Sample {
                file,
                release: (parts[0], parts[1], parts[2]),
                schema,
                sql,
            }
        })
        .collect();
    assert!(!found.is_empty(), "no samples in {}", dir.display());
    found.sort_by_key(|s| s.release);
    found
}

struct Opened {
    engine: Engine,
    _dir: tempfile::TempDir,
}

fn open(sample: &Sample) -> Opened {
    let dir = tempfile::tempdir().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Sample"));
    layout.create_dirs().unwrap();
    Connection::open(layout.database())
        .unwrap()
        .execute_batch(&sample.sql)
        .unwrap();
    let mut config = EngineConfig::new(Role::App);
    config.time_zone = Some("UTC".to_owned());
    config.backups = Some(dir.path().join("backups"));
    let engine = Engine::open(&layout, config)
        .unwrap_or_else(|e| panic!("{}: {e:?}", sample.file.display()));
    Opened { engine, _dir: dir }
}

impl Opened {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&Context::user(Via::App), op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn task(&self, key: &str) -> Value {
        self.call("task.get", json!({ "keys": [key] }))["items"][0].clone()
    }
}

#[test]
fn every_sample_opens_in_the_app_with_its_data() {
    for sample in samples() {
        let name = sample.file.display().to_string();
        let ws = open(&sample);
        let since = |v: (u64, u64, u64)| sample.release >= v;

        // An older workspace is backed up before it is upgraded.
        let backups = ws.call("backup.list", json!({}))["items"].clone();
        let current = sample.schema == gk_store::SCHEMA_VERSION;
        if current {
            assert_eq!(backups, json!([]), "{name}");
        } else {
            assert_eq!(backups.as_array().unwrap().len(), 1, "{name}: {backups}");
            assert_eq!(backups[0]["kind"], "pre_migration", "{name}");
            assert_eq!(backups[0]["schema_version"], sample.schema, "{name}");
        }

        let projects = ws.call("project.list", json!({}))["items"].clone();
        let keys: Vec<&str> = projects
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, ["TM", "ACM"], "{name}");
        assert_eq!(projects[0]["name"], "Home tasks", "{name}");
        assert_eq!(projects[0]["icon"], "rocket", "{name}");
        assert_eq!(projects[0]["hue"], "green", "{name}");

        let plan = ws.task("TM-1");
        assert_eq!(plan["title"], "Plan the garden", "{name}");
        assert_eq!(
            plan["description"], "Beds, paths, a pond and a shed.\n\n- [ ] measure\n- [x] sketch",
            "{name}"
        );
        assert_eq!(
            plan["checklist"],
            json!({ "done": 1, "total": 2 }),
            "{name}"
        );
        assert_eq!(plan["priority"], "high", "{name}");
        assert_eq!(plan["due_date"], "2026-12-01", "{name}");
        assert_eq!(plan["estimate_minutes"], 120, "{name}");
        assert_eq!(plan["labels"][0]["name"], "Urgent", "{name}");
        assert_eq!(plan["comment_count"], 1, "{name}");
        assert_eq!(plan["subtask_count"]["total"], 1, "{name}");
        assert_eq!(ws.task("TM-2")["parent"], "TM-1", "{name}");
        assert_eq!(ws.task("TM-3")["status"]["category"], "done", "{name}");
        assert_eq!(ws.task("ACM-1")["title"], "Launch the homepage", "{name}");

        let comments = ws.call("comment.list", json!({ "key": "TM-1" }));
        assert_eq!(
            comments["items"][0]["body"], "The neighbours like the garden plan.",
            "{name}"
        );
        let found = ws.call("search.query", json!({ "text": "garden" }))["items"].clone();
        assert!(
            found
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["task"] == "TM-1"),
            "{name}: {found}"
        );
        let timer = ws.call("timer.get", json!({}));
        assert_eq!(timer["timer"]["state"], "running", "{name}");
        assert_eq!(timer["timer"]["task"]["key"], "TM-2", "{name}");
        let report = ws.call(
            "time.report",
            json!({ "from": "2026-11-01", "to": "2026-11-30" }),
        );
        let logged = if since((0, 1, 6)) { 135 } else { 90 };
        assert_eq!(report["total_minutes"], logged, "{name}");

        if since((0, 0, 6)) {
            let page = ws.call(
                "wiki.get",
                json!({ "project": "TM", "page": "Garden notes" }),
            );
            assert_eq!(
                page["body"], "The plan is in TM-1. Seeds are on [[Seeds]].",
                "{name}"
            );
            assert_eq!(page["children"][0]["title"], "Seeds", "{name}");
            let seeds = ws.call("wiki.get", json!({ "project": "TM", "page": "Seeds" }));
            assert_eq!(seeds["body"], "Tomatoes, beans and peas.", "{name}");
            let links = ws.call(
                "wiki.backlinks",
                json!({ "project": "TM", "page": "Seeds" }),
            );
            assert!(
                links["items"].to_string().contains("Garden notes"),
                "{name}: links to Seeds: {links}"
            );
        }
        if since((0, 1, 4)) {
            let page = ws.call(
                "wiki.get",
                json!({ "project": "TM", "page": "Garden notes" }),
            );
            assert_eq!(page["home"], true, "{name}");
            let pages = ws.call("wiki.list", json!({ "project": "TM" }))["items"].clone();
            let template = pages
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["title"] == "Meeting notes")
                .unwrap();
            assert_eq!(template["template"], true, "{name}");
        }
        if since((0, 1, 6)) {
            assert_eq!(plan["fix_version"]["name"], "Spring", "{name}");
            assert_eq!(plan["fields"][0]["name"], "Cost", "{name}");
            assert_eq!(plan["fields"][0]["value"], 12.5, "{name}");
            let kinds = ws.call("work_kind.list", json!({}))["items"].clone();
            assert_eq!(kinds[0]["name"], "Digging", "{name}");
            let types = ws.call("project.get", json!({ "project": "TM" }))["types"].clone();
            assert!(
                types.to_string().contains("\"glyph\":\"epic\""),
                "{name}: {types}"
            );
            assert!(
                ws.task("TM-4").get("recurrence").is_some(),
                "{name}: TM-4 repeats"
            );
        }

        // And the upgraded workspace takes new work.
        let made = ws.call(
            "task.create",
            json!({ "project": "TM", "title": "After the upgrade" }),
        );
        let next = if since((0, 1, 6)) { "TM-5" } else { "TM-4" };
        assert_eq!(made["task"]["key"], next, "{name}");
    }
}
