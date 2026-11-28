// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The critical path and schedule baselines over a real workspace: known
//! networks with their times, slack and critical set on working days, lags,
//! end targets, the query terms `is:critical`, `slack`, `slip` and
//! `baseline`, saving, renaming and deleting baselines with undo, and
//! archives that carry them.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn open(dir: &Path, name: &str) -> Engine {
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::new(AtomicI64::new(NOW));
    config.clock = Arc::new(move || clock.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    Engine::open(&WorkspaceLayout::new(dir.join(name)), config).unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fail(e: &Engine, op: &str, input: Value) -> gk_api::OpError {
    e.call(&Context::user(Via::App), op, input).expect_err(op)
}

fn task(e: &Engine, project: &str, title: &str, extra: Value) -> String {
    let mut input = json!({ "project": project, "title": title });
    for (k, v) in extra.as_object().unwrap() {
        input[k] = v.clone();
    }
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn blocks(e: &Engine, from: &str, to: &str, lag: i64) {
    let mut input = json!({ "key": from, "kind": "blocks", "other": to });
    if lag > 0 {
        input["lag_days"] = json!(lag);
    }
    call(e, "relation.add", input);
}

fn timing(schedule: &Value) -> BTreeMap<String, Value> {
    schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .chain(schedule["unscheduled"].as_array().unwrap())
        .filter(|t| t.get("timing").is_some())
        .map(|t| (t["key"].as_str().unwrap().to_owned(), t["timing"].clone()))
        .collect()
}

fn found(e: &Engine, query: &str) -> Vec<String> {
    let mut keys: Vec<String> =
        call(e, "task.search", json!({ "query": query, "limit": 200 }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["key"].as_str().unwrap().to_owned())
            .collect();
    keys.sort();
    keys
}

fn sorted(keys: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = keys.iter().map(|k| (*k).to_owned()).collect();
    v.sort();
    v
}

/// The seven-activity textbook network A(3) → B(4) → D(5) → F(2), A → C(2)
/// → E(3) → F, C → G(1), as estimates without dates in project TM. Returns
/// the keys A to G.
fn textbook(e: &Engine) -> Vec<String> {
    call(e, "project.create", json!({ "name": "Plan", "key": "TM" }));
    let lengths = [3, 4, 2, 5, 3, 2, 1];
    let keys: Vec<String> = ["A", "B", "C", "D", "E", "F", "G"]
        .iter()
        .zip(lengths)
        .map(|(name, days)| task(e, "TM", name, json!({ "estimate": format!("{days}d") })))
        .collect();
    for (from, to) in [(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5), (2, 6)] {
        blocks(e, &keys[from], &keys[to], 0);
    }
    keys
}

#[test]
fn a_textbook_network_gets_its_times_slack_and_critical_path_on_working_days() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    let k = textbook(&e);
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    let t = timing(&s);
    assert_eq!(t.len(), 7, "{s}");
    let slack: Vec<i64> = k
        .iter()
        .map(|key| t[key]["slack_days"].as_i64().unwrap())
        .collect();
    assert_eq!(slack, vec![0, 0, 4, 0, 4, 0, 8]);
    // A begins with the plan, on today (Monday); B runs over the weekend.
    assert_eq!(t[&k[0]]["earliest_start"], "2026-11-23");
    assert_eq!(t[&k[0]]["earliest_finish"], "2026-11-25");
    assert_eq!(t[&k[1]]["earliest_start"], "2026-11-26");
    assert_eq!(t[&k[1]]["earliest_finish"], "2026-12-01");
    assert_eq!(t[&k[3]]["earliest_start"], "2026-12-02");
    assert_eq!(t[&k[3]]["earliest_finish"], "2026-12-08");
    assert_eq!(t[&k[5]]["latest_finish"], "2026-12-10");
    assert_eq!(t[&k[2]]["latest_start"], "2026-12-02");
    let path = &s["critical_path"];
    assert_eq!(path["finish"], "2026-12-10");
    assert_eq!(path["end"], "2026-12-10");
    assert_eq!(path["working_days_per_week"], 5);
    let mut on_path: Vec<String> = path["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    on_path.sort();
    assert_eq!(on_path, sorted(&[&k[0], &k[1], &k[3], &k[5]]));

    // Only the critical tasks, when asked.
    let only = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "critical": true }),
    );
    assert_eq!(timing(&only).len(), 4);

    // The queries.
    assert_eq!(
        found(&e, "is:critical"),
        sorted(&[&k[0], &k[1], &k[3], &k[5]])
    );
    assert_eq!(found(&e, "slack<2d"), sorted(&[&k[0], &k[1], &k[3], &k[5]]));
    assert_eq!(found(&e, "slack>3d"), sorted(&[&k[2], &k[4], &k[6]]));
    assert_eq!(found(&e, "slack:4d"), sorted(&[&k[2], &k[4]]));
    assert_eq!(found(&e, "-is:critical project:TM").len(), 3);
}

#[test]
fn lags_parallel_chains_and_seven_day_weeks_change_the_path() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    let k = textbook(&e);
    // Five days between C and E: the lower chain becomes the longer one.
    blocks(&e, &k[2], &k[4], 5);
    let t = timing(&call(&e, "schedule.get", json!({ "project": "TM" })));
    let slack = |i: usize| t[&k[i]]["slack_days"].as_i64().unwrap();
    assert_eq!((slack(0), slack(2), slack(4), slack(5)), (0, 0, 0, 0));
    assert_eq!((slack(1), slack(3)), (1, 1));
    assert_eq!(t[&k[4]]["earliest_start"], "2026-12-07");
    assert_eq!(t[&k[5]]["earliest_finish"], "2026-12-11");

    // Every day a working day: the same plan ends sooner.
    call(&e, "settings.update", json!({ "days_per_week": 7 }));
    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    assert_eq!(s["critical_path"]["working_days_per_week"], 7);
    assert_eq!(s["critical_path"]["finish"], "2026-12-07");
}

#[test]
fn dated_tasks_targets_parents_and_tasks_left_out() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    call(&e, "project.create", json!({ "name": "Plan", "key": "TM" }));
    let a = task(
        &e,
        "TM",
        "Design",
        json!({ "start_date": "2026-11-23", "due_date": "2026-11-25" }),
    );
    let b = task(
        &e,
        "TM",
        "Build",
        json!({ "start_date": "2026-11-30", "due_date": "2026-12-04" }),
    );
    let side = task(
        &e,
        "TM",
        "Side",
        json!({ "start_date": "2026-11-24", "due_date": "2026-11-24" }),
    );
    let nothing = task(&e, "TM", "Someday", json!({}));
    blocks(&e, &a, &b, 0);
    let parent = task(&e, "TM", "Phase", json!({}));
    call(&e, "task.update", json!({ "key": a, "parent": parent }));
    call(&e, "task.update", json!({ "key": b, "parent": parent }));
    call(
        &e,
        "version.create",
        json!({ "project": "TM", "name": "1.0", "due_date": "2026-12-11" }),
    );

    let s = call(&e, "schedule.get", json!({ "project": "TM" }));
    let t = timing(&s);
    // Build waits for its own start; Design may slip until then.
    assert_eq!(t[&b]["earliest_start"], "2026-11-30");
    assert_eq!(t[&b]["slack_days"], 0);
    assert_eq!(t[&a]["slack_days"], 2);
    assert_eq!(t[&side]["slack_days"], 8);
    // The parent rolls up from its subtasks.
    assert_eq!(t[&parent]["critical"], true);
    assert_eq!(t[&parent]["earliest_start"], "2026-11-23");
    assert_eq!(t[&parent]["latest_finish"], "2026-12-04");
    let left = &s["critical_path"]["left_out"];
    assert_eq!(
        left,
        &json!([{ "key": nothing, "reason": "no_dates_or_estimate" }])
    );

    // Counting back from the version's due date gives everyone a week.
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "target": "1.0" }),
    );
    assert_eq!(s["critical_path"]["end"], "2026-12-11");
    assert_eq!(timing(&s)[&b]["slack_days"], 5);
    assert!(s["critical_path"]["tasks"].as_array().unwrap().is_empty());
    // A day earlier than the plan can make: negative slack, all critical.
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "TM", "target": "2026-12-02" }),
    );
    assert_eq!(timing(&s)[&b]["slack_days"], -2);
    // A task without a due date can't be a target.
    let err = fail(
        &e,
        "schedule.get",
        json!({ "project": "TM", "target": nothing }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = fail(
        &e,
        "schedule.get",
        json!({ "project": "TM", "target": "no such thing" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
}

fn baselines(e: &Engine) -> Vec<(String, u64)> {
    call(e, "baseline.list", json!({ "project": "ZZ" }))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["name"].as_str().unwrap().to_owned(),
                b["tasks"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn row<'a>(schedule: &'a Value, key: &str) -> &'a Value {
    schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .unwrap_or_else(|| panic!("{key} is not in {schedule}"))
}

/// Project ZZ with three dated tasks and one without dates, a baseline
/// "Plan A" of it, and then changes: the first ends three days later, the
/// second a day earlier, the third is deleted and a fourth is added.
fn slipped(e: &Engine) -> [String; 4] {
    call(e, "project.create", json!({ "name": "Zeta", "key": "ZZ" }));
    let t1 = task(
        e,
        "ZZ",
        "One",
        json!({ "start_date": "2026-12-01", "due_date": "2026-12-03" }),
    );
    let t2 = task(e, "ZZ", "Two", json!({ "due_date": "2026-12-02" }));
    let t3 = task(
        e,
        "ZZ",
        "Three",
        json!({ "start_date": "2026-12-05", "due_date": "2026-12-07" }),
    );
    task(e, "ZZ", "Undated", json!({}));
    let made = call(
        e,
        "baseline.create",
        json!({ "project": "ZZ", "name": "Plan A" }),
    );
    assert_eq!(made["baseline"]["tasks"], 3);
    assert_eq!(made["baseline"]["created_by"]["kind"], "user");
    assert!(made["group"].is_string());
    call(
        e,
        "task.update",
        json!({ "key": t1, "due_date": "2026-12-06" }),
    );
    call(
        e,
        "task.update",
        json!({ "key": t2, "due_date": "2026-12-01" }),
    );
    call(e, "task.delete", json!({ "keys": [t3] }));
    let t4 = task(e, "ZZ", "Four", json!({ "due_date": "2026-12-09" }));
    [t1, t2, t3, t4]
}

#[test]
fn a_baseline_shows_slip_new_tasks_and_removed_ones() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    let [t1, t2, t3, t4] = slipped(&e);
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "ZZ", "baseline": "plan a" }),
    );
    assert_eq!(s["baseline"]["name"], "Plan A");
    assert_eq!(
        row(&s, &t1)["baseline"],
        json!({ "start_date": "2026-12-01", "due_date": "2026-12-03", "slip_days": 3 })
    );
    assert_eq!(row(&s, &t2)["baseline"]["slip_days"], -1);
    assert_eq!(row(&s, &t4)["new_since_baseline"], true);
    assert!(row(&s, &t1).get("new_since_baseline").is_none());
    assert_eq!(s["baseline"]["removed_total"], 1);
    assert_eq!(s["baseline"]["removed"][0]["key"], t3);
    assert_eq!(s["baseline"]["removed"][0]["title"], "Three");
    assert_eq!(s["baseline"]["removed"][0]["deleted"], true);
    // The deleted task keeps its entry.
    assert_eq!(baselines(&e), vec![("Plan A".to_owned(), 3)]);

    // Queries against the newest baseline, or a named one.
    assert_eq!(found(&e, "slip>2d"), vec![t1.clone()]);
    assert_eq!(found(&e, "slip<0"), vec![t2.clone()]);
    assert_eq!(found(&e, "slip<=-1d"), vec![t2.clone()]);
    assert_eq!(found(&e, "slip:0..5"), vec![t1.clone()]);
    call(
        &e,
        "baseline.create",
        json!({ "project": "ZZ", "name": "Plan B" }),
    );
    assert!(
        found(&e, "slip>2d").is_empty(),
        "against Plan B, the newest"
    );
    assert_eq!(found(&e, "baseline:\"Plan A\" slip>2d"), vec![t1.clone()]);
    assert_eq!(found(&e, "baseline:\"Plan B\"").len(), 3);
    assert!(found(&e, "baseline:Nothing").is_empty());

    // A baseline needs its project; an unknown one is named.
    let err = fail(
        &e,
        "schedule.get",
        json!({ "query": "is:open", "baseline": "Plan A" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    let err = fail(
        &e,
        "schedule.get",
        json!({ "project": "ZZ", "baseline": "Plan C" }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn saving_renaming_and_deleting_baselines_can_be_undone() {
    let dir = TempDir::new().unwrap();
    let e = open(dir.path(), "W");
    slipped(&e);
    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    call(&e, "history.undo", json!({}));
    assert!(baselines(&e).is_empty(), "undo takes the baseline back");
    call(&e, "history.redo", json!({}));
    assert_eq!(
        baselines(&e),
        vec![("Plan A".to_owned(), 3)],
        "and redo too"
    );

    let err = fail(
        &e,
        "baseline.create",
        json!({ "project": "ZZ", "name": "PLAN A" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    call(
        &e,
        "baseline.create",
        json!({ "project": "ZZ", "name": "Plan B" }),
    );
    let err = fail(
        &e,
        "baseline.rename",
        json!({ "project": "ZZ", "baseline": "Plan B", "name": "plan a" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    call(
        &e,
        "baseline.rename",
        json!({ "project": "ZZ", "baseline": "Plan B", "name": "Kick-off" }),
    );
    assert_eq!(
        baselines(&e),
        vec![("Kick-off".to_owned(), 3), ("Plan A".to_owned(), 3)]
    );
    call(&e, "history.undo", json!({}));
    assert_eq!(baselines(&e)[0].0, "Plan B");

    let gone = call(
        &e,
        "baseline.delete",
        json!({ "project": "ZZ", "baseline": "Plan A" }),
    );
    assert_eq!(gone["deleted"], true);
    assert_eq!(baselines(&e).len(), 1);
    call(&e, "history.revert", json!({ "groups": [gone["group"]] }));
    assert_eq!(
        baselines(&e),
        vec![("Plan B".to_owned(), 3), ("Plan A".to_owned(), 3)]
    );

    // An archived project's baselines are read, not changed.
    call(&e, "project.archive", json!({ "project": "ZZ" }));
    let err = fail(
        &e,
        "baseline.create",
        json!({ "project": "ZZ", "name": "Late" }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert_eq!(baselines(&e).len(), 2);
}

#[test]
fn archives_carry_baselines() {
    let dir = TempDir::new().unwrap();
    let a = open(dir.path(), "A");
    let [t1, ..] = slipped(&a);
    let zip = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let b = open(dir.path(), "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    assert_eq!(baselines(&b), vec![("Plan A".to_owned(), 3)]);
    let s = call(
        &b,
        "schedule.get",
        json!({ "project": "ZZ", "baseline": "Plan A" }),
    );
    assert_eq!(row(&s, &t1)["baseline"]["slip_days"], 3);
    assert_eq!(s["baseline"]["removed_total"], 1);
    assert_eq!(found(&b, "slip>2d"), vec![t1.clone()]);

    // Again under a new key: a copy with tasks of its own.
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy(), "keys": [{ "from": "ZZ", "to": "ZY" }] }),
    );
    let copy = call(&b, "baseline.list", json!({ "project": "ZY" }));
    assert_eq!(copy["total"], 1);
    let s = call(
        &b,
        "schedule.get",
        json!({ "project": "ZY", "baseline": "Plan A" }),
    );
    assert_eq!(row(&s, "ZY-1")["baseline"]["slip_days"], 3);
    assert_eq!(found(&b, "slip>2d"), vec!["ZY-1".to_owned(), t1]);
}

/// 20,000 tasks in one project, 500 of them dated, with 300 links: the
/// critical path is worked out quickly for the schedule and for queries.
/// Run with `cargo test -p gk-engine --release --test baselines_critical -- --ignored`.
#[test]
#[ignore = "a timing run on the large data set"]
fn data_set_l_works_out_the_critical_path_quickly() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("seed-L");
    gk_seed::generate(gk_seed::DataSet::Large, 1)
        .unwrap()
        .write_to(&out)
        .unwrap();
    let e = open(dir.path(), "L");
    call(
        &e,
        "transfer.import",
        json!({ "path": out.to_string_lossy() }),
    );
    let at = Instant::now();
    let s = call(
        &e,
        "schedule.get",
        json!({ "project": "BULK", "critical": true }),
    );
    let schedule = at.elapsed();
    let at = Instant::now();
    let critical = call(
        &e,
        "task.search",
        json!({ "query": "is:critical", "limit": 50 }),
    );
    let search = at.elapsed();
    let at = Instant::now();
    call(
        &e,
        "task.search",
        json!({ "query": "slack<2d", "limit": 50 }),
    );
    let slack = at.elapsed();
    println!(
        "data set L: critical schedule {schedule:.2?}, is:critical {search:.2?} ({} found), slack<2d {slack:.2?}; {} critical rows",
        critical["total"],
        s["tasks"].as_array().unwrap().len()
    );
    assert!(search.as_millis() < 200, "{search:?}");
    assert!(slack.as_millis() < 200, "{slack:?}");
}
