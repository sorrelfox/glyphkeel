// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task queries, saved views and bulk changes over a real workspace.

#![allow(clippy::unwrap_used)]

use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Failure, Role, SessionStart};
use gk_store::WorkspaceLayout;
use gk_store::rusqlite::params;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;
const CHILD: &str = "GK_QUERY_TEST_CHILD";

fn layout(dir: &TempDir) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.path().join("Personal"))
}

fn config(role: Role) -> EngineConfig {
    let mut config = EngineConfig::new(role);
    config.clock = Arc::new(|| NOW);
    config.time_zone = Some("UTC".to_owned());
    config
}

fn open(dir: &TempDir, role: Role) -> Engine {
    Engine::open(&layout(dir), config(role)).unwrap()
}

fn app() -> Context {
    Context::user(Via::App)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&app(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, op: &str, input: Value) -> OpError {
    e.call(&app(), op, input).expect_err(op)
}

fn keys(out: &Value) -> Vec<String> {
    out["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn find(e: &Engine, query: &str) -> Vec<String> {
    let mut all = keys(&call(
        e,
        "task.search",
        json!({ "query": query, "limit": 200 }),
    ));
    all.sort();
    all
}

fn sorted(e: &Engine, query: &str) -> Vec<String> {
    keys(&call(
        e,
        "task.search",
        json!({ "query": query, "limit": 200 }),
    ))
}

fn task(e: &Engine, input: Value) -> String {
    call(e, "task.create", input)["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Projects TM (with sub-project SITE) and OLD (archived), labels, and
/// tasks with every kind of field.
fn data_set(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(
        e,
        "project.create",
        json!({ "name": "Web", "key": "SITE", "parent": "TM" }),
    );
    call(e, "project.create", json!({ "name": "Old", "key": "OLD" }));
    for (p, l) in [
        ("TM", "bug"),
        ("TM", "crash"),
        ("TM", "ui"),
        ("SITE", "bug"),
    ] {
        call(e, "label.create", json!({ "project": p, "name": l }));
    }
    // TM-1: an overdue bug with an estimate and logged time.
    task(
        e,
        json!({ "project": "TM", "title": "Login error on submit", "type": "Bug",
                "priority": "high", "labels": ["bug"], "due_date": "2026-11-20",
                "estimate": "4h", "description": "The connection timeout hits users." }),
    );
    // TM-2: due within the week, label crash.
    task(
        e,
        json!({ "project": "TM", "title": "Crash on start", "priority": "urgent",
                "labels": ["crash", "ui"], "due_date": "2026-11-26", "estimate": "1h" }),
    );
    // TM-3: done, no labels.
    let done = task(
        e,
        json!({ "project": "TM", "title": "Write release notes", "type": "Feature" }),
    );
    call(e, "task.move", json!({ "key": done, "status": "Done" }));
    // TM-4: a subtask of TM-1, scheduled later.
    task(
        e,
        json!({ "project": "TM", "title": "Reproduce the login error", "parent": "TM-1",
                "start_date": "2026-12-01", "due_date": "2026-12-03" }),
    );
    // TM-5: blocked by TM-2, unscheduled.
    task(e, json!({ "project": "TM", "title": "Ship the hotfix" }));
    call(
        e,
        "relation.add",
        json!({ "key": "TM-2", "kind": "blocks", "other": "TM-5" }),
    );
    // SITE-1: in the sub-project.
    task(
        e,
        json!({ "project": "SITE", "title": "Broken layout", "labels": ["bug"],
                "priority": "low" }),
    );
    // OLD-1: in an archived project.
    task(e, json!({ "project": "OLD", "title": "Old login bug" }));
    call(e, "project.archive", json!({ "project": "OLD" }));
    call(
        e,
        "time.log",
        json!({ "key": "TM-1", "duration": "5h", "date": "2026-11-22" }),
    );
    call(
        e,
        "comment.add",
        json!({ "key": "TM-5", "body": "Waiting for the flaky deploy." }),
    );
}

fn workspace() -> (TempDir, Engine) {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, Role::App);
    data_set(&e);
    (dir, e)
}

fn v(keys: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = keys.iter().map(|k| (*k).to_owned()).collect();
    v.sort();
    v
}

#[test]
fn the_planned_examples_find_the_right_tasks() {
    let (_dir, e) = workspace();
    let cases: &[(&str, &[&str])] = &[
        ("", &["TM-1", "TM-2", "TM-3", "TM-4", "TM-5", "SITE-1"]),
        ("is:open due<+7d", &["TM-1", "TM-2"]),
        ("label:bug,crash -status:Done", &["TM-1", "TM-2", "SITE-1"]),
        ("label:bug label:ui", &[]),
        ("label:crash label:ui", &["TM-2"]),
        ("-label:bug", &["TM-2", "TM-3", "TM-4", "TM-5"]),
        ("label!=bug", &["TM-2", "TM-3", "TM-4", "TM-5"]),
        ("label:none", &["TM-3", "TM-4", "TM-5"]),
        ("label:crash,none", &["TM-2", "TM-3", "TM-4", "TM-5"]),
        ("-label:none", &["TM-1", "TM-2", "SITE-1"]),
        ("label!=none", &["TM-1", "TM-2", "SITE-1"]),
        ("type:Feature", &["TM-3"]),
        ("priority>=High", &["TM-1", "TM-2"]),
        ("prio<normal", &["TM-3", "TM-4", "TM-5", "SITE-1"]),
        ("project:TM", &["TM-1", "TM-2", "TM-3", "TM-4", "TM-5"]),
        (
            "project:TM/*",
            &["TM-1", "TM-2", "TM-3", "TM-4", "TM-5", "SITE-1"],
        ),
        ("project:OLD", &["OLD-1"]),
        ("is:overdue", &["TM-1"]),
        (
            "(label:bug OR type:Bug) is:open -is:blocked",
            &["TM-1", "SITE-1"],
        ),
        ("is:blocked", &["TM-5"]),
        ("blocks:any", &["TM-2"]),
        ("blocked-by:TM-2", &["TM-5"]),
        ("estimate:1h..4h -has:start", &["TM-1", "TM-2"]),
        ("parent:TM-1", &["TM-4"]),
        ("under:TM-1", &["TM-4"]),
        ("parent:none is:open", &["TM-1", "TM-2", "TM-5", "SITE-1"]),
        ("TM-41", &[]),
        ("TM-4", &["TM-4"]),
        ("timeout", &["TM-1"]),
        ("\"connection timeout\"", &["TM-1"]),
        ("login", &["TM-1", "TM-4"]),
        ("login -error", &[]),
        ("description:timeout", &["TM-1"]),
        ("comment:flaky", &["TM-5"]),
        ("title:\"release notes\"", &["TM-3"]),
        ("title=\"crash on start\"", &["TM-2"]),
        ("is:closed", &["TM-3"]),
        (
            "category:open status!=\"In progress\" has:due",
            &["TM-1", "TM-2", "TM-4"],
        ),
        ("spent>4h", &["TM-1"]),
        ("is:over-estimate", &["TM-1"]),
        ("remaining<0m", &["TM-1"]),
        ("has:time", &["TM-1"]),
        ("comments>0", &["TM-5"]),
        ("subtasks:1", &["TM-1"]),
        ("is:parent", &["TM-1"]),
        ("is:subtask", &["TM-4"]),
        ("is:unscheduled", &["TM-3", "TM-5", "SITE-1"]),
        ("due:this-week", &["TM-2"]),
        ("due:none", &["TM-3", "TM-5", "SITE-1"]),
        (
            "due!=today",
            &["TM-1", "TM-2", "TM-3", "TM-4", "TM-5", "SITE-1"],
        ),
        ("start>=2026-12-01", &["TM-4"]),
        ("during:2026-11-20..2026-11-26", &["TM-1", "TM-2"]),
        ("during:2026-12-02", &["TM-4"]),
        ("during:2026-11-27..2026-11-30", &[]),
        ("during:2026-12-03..2027-01-31 is:open", &["TM-4"]),
        (
            "created:today author:me",
            &["TM-1", "TM-2", "TM-3", "TM-4", "TM-5", "SITE-1"],
        ),
        ("created<today", &[]),
        ("updated>-3d closed:today", &["TM-3"]),
        ("progress:100", &["TM-3"]),
        ("has:links", &["TM-2", "TM-5"]),
        ("is:claimed", &[]),
        ("author:agent", &[]),
        ("status:\"in progress\"", &[]),
    ];
    for (query, expected) in cases {
        assert_eq!(find(&e, query), v(expected), "{query}");
    }
}

#[test]
fn negation_twice_and_de_morgan_hold_on_real_data() {
    let (_dir, e) = workspace();
    for x in [
        "label:bug",
        "due<+7d",
        "is:blocked",
        "estimate:1h..4h",
        "timeout",
    ] {
        assert_eq!(find(&e, x), find(&e, &format!("--{x}")), "{x}");
        assert_eq!(find(&e, x), find(&e, &format!("NOT -{x}")), "{x}");
    }
    for (a, b) in [
        ("label:bug", "due<+7d"),
        ("is:open", "has:estimate"),
        ("type:Bug", "login"),
    ] {
        assert_eq!(
            find(&e, &format!("-({a} OR {b})")),
            find(&e, &format!("-{a} -{b}")),
            "{a} {b}"
        );
        assert_eq!(
            find(&e, &format!("-({a} {b})")),
            find(&e, &format!("-{a} OR -{b}")),
            "{a} {b}"
        );
    }
}

#[test]
fn sorting_paging_and_totals() {
    let (_dir, e) = workspace();
    assert_eq!(
        sorted(&e, "project:TM sort:due,-priority"),
        ["TM-1", "TM-2", "TM-4", "TM-3", "TM-5"]
    );
    assert_eq!(
        sorted(&e, "project:TM sort:-due"),
        ["TM-4", "TM-2", "TM-1", "TM-3", "TM-5"],
        "empty values last in both directions"
    );
    assert_eq!(
        sorted(&e, "sort:-priority project:TM")[..2],
        ["TM-2", "TM-1"]
    );

    let page = call(&e, "task.search", json!({ "query": "is:open", "limit": 2 }));
    assert_eq!(page["total"], 5);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["next_cursor"], "2");
    assert_eq!(page["sort"], json!(["status", "rank"]));
    let totals = &page["totals"];
    assert_eq!(totals["count"], 5);
    assert_eq!(totals["open"], 5);
    assert_eq!(totals["estimate_minutes"], 300);
    assert_eq!(totals["spent_minutes"], 300);
    assert_eq!(totals["remaining_minutes"], 0, "-60 for TM-1, +60 for TM-2");
    let last = call(
        &e,
        "task.search",
        json!({ "query": "is:open", "limit": 2, "cursor": "4" }),
    );
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert!(last.get("next_cursor").is_none());

    let text = call(&e, "task.search", json!({ "query": "login" }));
    assert_eq!(text["sort"], json!(["relevance"]));
    let scoped = call(
        &e,
        "task.search",
        json!({ "query": "label:bug", "project": "TM", "include_subprojects": true, "sort": "-key" }),
    );
    assert_eq!(keys(&scoped), ["TM-1", "SITE-1"]);
    assert_eq!(scoped["query"], "label:bug");
    let item = &call(&e, "task.search", json!({ "query": "TM-1" }))["items"][0];
    assert_eq!(item["labels"][0]["name"], "bug");
    assert_eq!(item["remaining_minutes"], -60);
    assert_eq!(item["subtask_count"]["total"], 1);
}

#[test]
fn mistakes_and_unknown_names_are_errors_with_choices() {
    let (_dir, e) = workspace();
    let err = fails(&e, "task.search", json!({ "query": "is:open stauts:Done" }));
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(
        err.message,
        "Unknown field \"stauts\". Did you mean \"status\"?"
    );
    assert_eq!(err.details["message_id"], "query-unknown-field");
    assert_eq!(err.details["from"], 8);
    assert_eq!(err.details["to"], 14);
    assert_eq!(err.details["path"], "query");

    let err = fails(&e, "task.search", json!({ "query": "status:Doing" }));
    assert_eq!(err.details["message_id"], "query-unknown-status");
    assert_eq!(
        err.message,
        "No status \"Doing\". Statuses: Backlog, Done, In progress, Review, To do."
    );
    assert_eq!(err.details["choices"][0], "Backlog");
    let err = fails(&e, "task.search", json!({ "query": "label:nope" }));
    assert_eq!(err.details["message_id"], "query-unknown-label");
    let err = fails(&e, "task.search", json!({ "query": "project:NOPE" }));
    assert_eq!(err.details["message_id"], "query-unknown-project");
    let err = fails(&e, "task.search", json!({ "query": "parent:TM-99" }));
    assert_eq!(err.details["message_id"], "query-unknown-task");
    let err = fails(&e, "task.search", json!({ "query": "", "sort": "stauts" }));
    assert_eq!(err.details["path"], "sort");
    let err = fails(&e, "task.search", json!({ "query": "", "limit": 0 }));
    assert_eq!(err.details["path"], "limit");
    let err = fails(&e, "task.search", json!({ "query": "", "cursor": "x" }));
    assert_eq!(err.details["path"], "cursor");
}

#[test]
fn history_conditions_are_answered_from_the_activity() {
    let (_dir, e) = workspace();
    call(
        &e,
        "task.move",
        json!({ "key": "TM-1", "status": "In progress" }),
    );
    call(&e, "task.move", json!({ "key": "TM-1", "status": "To do" }));
    call(&e, "task.update", json!({ "key": "TM-1", "type": "Task" }));
    call(
        &e,
        "task.update",
        json!({ "key": "TM-2", "labels": ["ui"], "priority": "low" }),
    );
    let agent = agent_of(&e, Profile::Standard);
    e.call(
        &agent,
        "task.update",
        json!({ "key": "TM-4", "due_date": "2026-12-05" }),
    )
    .unwrap();
    // TM-1's changes were a month ago.
    e.store()
        .write("test.history", |tx| {
            tx.execute(
                "UPDATE activity SET at = ?1 WHERE entity_kind = 'task'
                    AND entity_id = (SELECT id FROM tasks WHERE number = 1
                        AND project_id = (SELECT id FROM projects WHERE key = 'TM'))",
                params![NOW - 30 * 86_400_000],
            )?;
            Ok::<_, Failure>(())
        })
        .unwrap();

    assert_eq!(find(&e, "was:status:\"in progress\""), v(&["TM-1"]));
    assert_eq!(find(&e, "was:status:Done"), v(&["TM-3"]));
    assert_eq!(find(&e, "was:category:in_progress"), v(&["TM-1"]));
    assert_eq!(
        find(&e, "project:TM -was:status:\"In progress\""),
        v(&["TM-2", "TM-3", "TM-4", "TM-5"])
    );
    assert_eq!(find(&e, "was:type:Bug"), v(&["TM-1"]));
    assert_eq!(find(&e, "type:Bug"), Vec::<String>::new());
    assert_eq!(find(&e, "was:label:crash"), v(&["TM-2"]));
    assert_eq!(find(&e, "was:priority:urgent"), v(&["TM-2"]));
    assert_eq!(find(&e, "was:priority:low"), v(&["SITE-1", "TM-2"]));

    // Creation is not a change.
    assert_eq!(find(&e, "changed:status"), v(&["TM-1", "TM-3"]));
    assert_eq!(find(&e, "changed:status>-7d"), v(&["TM-3"]));
    assert_eq!(find(&e, "changed:status<-7d"), v(&["TM-1"]));
    assert_eq!(find(&e, "changed:due"), v(&["TM-4"]));
    assert_eq!(find(&e, "changed:due,labels"), v(&["TM-2", "TM-4"]));
    assert_eq!(find(&e, "changed:type:-30d"), v(&["TM-1"]));
    assert_eq!(find(&e, "changed-by:agent"), v(&["TM-4"]));
    assert_eq!(find(&e, "changed-by:\"test AGENT\""), v(&["TM-4"]));
    assert!(!find(&e, "changed-by:me").contains(&"TM-4".to_owned()));

    let err = fails(&e, "task.search", json!({ "query": "was:status:Doing" }));
    assert_eq!(err.details["message_id"], "query-unknown-status");
    let err = fails(&e, "task.search", json!({ "query": "was:due:today" }));
    assert_eq!(err.details["message_id"], "query-history-field");
}

#[test]
fn the_query_field_gets_tokens_chips_and_errors() {
    let (_dir, e) = workspace();
    let out = call(
        &e,
        "query.parse",
        json!({ "query": "prio>=high -label:bug \"tête\" sort:due" }),
    );
    assert_eq!(out["valid"], true);
    assert_eq!(out["advanced"], false);
    assert_eq!(out["normalized"], "prio>=high -label:bug \"tête\" sort:due");
    assert_eq!(out["sort"], json!(["due"]));
    assert_eq!(out["chips"][0]["field"], "priority");
    assert_eq!(out["chips"][0]["op"], ">=");
    assert_eq!(out["chips"][1]["negated"], true);
    assert_eq!(
        out["chips"][1]["ranges"][0],
        json!({ "from": 11, "to": 21 })
    );
    assert_eq!(out["chips"][2]["values"], json!(["tête"]));
    // UTF-16 ranges: "tête" is 6 characters with its quotes.
    let text_token = out["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["kind"] == "text")
        .unwrap();
    assert_eq!(text_token["range"], json!({ "from": 22, "to": 28 }));

    let out = call(&e, "query.parse", json!({ "query": "a OR b" }));
    assert_eq!(out["advanced"], true);
    assert!(out.get("chips").is_none());

    let out = call(&e, "query.parse", json!({ "query": "status:Doing (a" }));
    assert_eq!(out["valid"], false);
    assert_eq!(out["error"]["message_id"], "query-unclosed-paren");
    assert_eq!(out["error"]["range"], json!({ "from": 13, "to": 14 }));

    let out = call(
        &e,
        "query.parse",
        json!({ "query": "status:Doing", "fields": true }),
    );
    assert_eq!(out["valid"], false);
    assert_eq!(out["error"]["message_id"], "query-unknown-status");
    assert_eq!(out["error"]["args"]["value"], "Doing");
    assert!(out["fields"].as_array().unwrap().len() > 30);

    let agent = agent_of(&e, Profile::Full);
    let err = e
        .call(&agent, "query.parse", json!({ "query": "x" }))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
}

fn agent_of(e: &Engine, profile: Profile) -> Context {
    let session = e
        .start_session(&SessionStart {
            name: "Test agent".to_owned(),
            client_id: None,
            client_version: None,
            profile,
        })
        .unwrap();
    Context::agent("Test agent", session, profile)
}

#[test]
fn saved_views_are_kept_checked_and_undone() {
    let (_dir, e) = workspace();
    let created = call(
        &e,
        "view.create",
        json!({ "name": "Due soon", "project": "TM", "query": "is:open due<+7d",
                "sort": "due,-priority", "columns": ["key", "title", "due"],
                "totals": ["estimate"], "is_default": true }),
    );
    let view = &created["view"];
    assert_eq!(view["project"], "TM");
    assert_eq!(view["kind"], "list");
    assert_eq!(view["sort"], "due,-priority");
    assert_eq!(view["is_default"], true);
    let second = call(
        &e,
        "view.create",
        json!({ "name": "Bugs", "project": "TM", "query": "type:Bug", "is_default": true }),
    );
    let list = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"][0]["name"], "Due soon");
    assert_eq!(
        list["items"][0]["is_default"], false,
        "one default per screen"
    );
    assert_eq!(list["items"][1]["is_default"], true);
    assert_eq!(call(&e, "view.list", json!({}))["total"], 0);
    assert_eq!(call(&e, "view.list", json!({ "all": true }))["total"], 2);

    let err = fails(
        &e,
        "view.create",
        json!({ "name": "bugs", "project": "TM", "query": "x" }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);
    let err = fails(
        &e,
        "view.create",
        json!({ "name": "Broken", "query": "status:Doing" }),
    );
    assert_eq!(err.details["message_id"], "query-unknown-status");
    let err = fails(
        &e,
        "view.create",
        json!({ "name": "Broken", "query": "x", "sort": "nothing" }),
    );
    assert_eq!(err.details["path"], "sort");

    let changed = call(
        &e,
        "view.update",
        json!({ "view": "Bugs", "project": "TM", "name": "Open bugs",
                "query": "type:Bug is:open", "clear": ["columns"] }),
    );
    assert_eq!(changed["view"]["name"], "Open bugs");
    call(&e, "history.undo", json!({}));
    let list = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(list["items"][1]["name"], "Bugs");

    let id = second["view"]["id"].as_str().unwrap();
    let deleted = call(&e, "view.delete", json!({ "view": id }));
    assert_eq!(deleted["deleted"], true);
    assert_eq!(
        call(&e, "view.list", json!({ "project": "TM" }))["total"],
        1
    );
    call(&e, "history.undo", json!({}));
    let list = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(list["total"], 2);
    assert_eq!(list["items"][1]["query"], "type:Bug");
    let group = deleted["group"].as_str().unwrap();
    assert!(!group.is_empty());
}

#[test]
fn saved_views_survive_reopening_the_workspace() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, Role::App);
    data_set(&e);
    call(
        &e,
        "view.create",
        json!({ "name": "Due soon", "project": "TM", "query": "is:open due<+7d",
                "sort": "due,-priority", "columns": ["key", "title", "due"],
                "group_by": "status", "totals": ["estimate"], "is_default": true }),
    );
    call(
        &e,
        "view.create",
        json!({ "name": "Web", "project": "SITE", "query": "label:bug",
                "columns": ["key", "title"] }),
    );
    let before = call(&e, "view.list", json!({ "all": true }));
    e.close();
    let e = open(&dir, Role::App);
    assert_eq!(call(&e, "view.list", json!({ "all": true })), before);
    // Still one list per project.
    assert_eq!(
        call(&e, "view.list", json!({ "project": "TM" }))["total"],
        1
    );
    assert_eq!(
        call(&e, "view.list", json!({ "project": "SITE" }))["items"][0]["columns"],
        json!(["key", "title"])
    );
}

#[test]
fn a_saved_view_evaluates_its_relative_dates_when_it_runs() {
    use std::sync::atomic::{AtomicI64, Ordering};
    const DAY: i64 = 24 * 60 * 60 * 1000;
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(NOW));
    let mut config = config(Role::App);
    let reading = Arc::clone(&clock);
    config.clock = Arc::new(move || reading.load(Ordering::SeqCst));
    let e = Engine::open(&layout(&dir), config).unwrap();
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    task(
        &e,
        json!({ "project": "TM", "title": "Soon", "due_date": "2026-11-25" }),
    );
    task(
        &e,
        json!({ "project": "TM", "title": "Later", "due_date": "2026-12-05" }),
    );
    task(
        &e,
        json!({ "project": "TM", "title": "Last week", "due_date": "2026-11-17" }),
    );
    call(
        &e,
        "view.create",
        json!({ "name": "Next week", "project": "TM", "query": "due:today..+7d" }),
    );
    let run_view = || {
        let view = call(&e, "view.list", json!({ "project": "TM" }))["items"][0].clone();
        assert_eq!(view["query"], "due:today..+7d", "the view keeps its text");
        find(&e, view["query"].as_str().unwrap())
    };
    // Monday 23 November: the week ahead holds the 25th.
    assert_eq!(run_view(), v(&["TM-1"]));
    // Ten days on, the same view holds the 5th of December instead.
    clock.store(NOW + 10 * DAY, Ordering::SeqCst);
    assert_eq!(run_view(), v(&["TM-2"]));
    // Six days earlier, on the 17th, the week ahead ends on the 24th.
    clock.store(NOW - 6 * DAY, Ordering::SeqCst);
    assert_eq!(run_view(), v(&["TM-3"]));
}

#[test]
fn renaming_a_name_rewrites_the_saved_queries_that_use_it() {
    let (_dir, e) = workspace();
    let view = |name: &str, project: Option<&str>, query: &str| {
        let mut input = json!({ "name": name, "query": query });
        if let Some(p) = project {
            input["project"] = json!(p);
        }
        call(&e, "view.create", input);
    };
    view(
        "Triage",
        Some("TM"),
        "status:\"To do\" type:Bug label:bug,crash -label:ui sort:due",
    );
    view("Keys", None, "project:TM/* OR TM-2 OR parent:TM-1");
    view("Web bugs", Some("SITE"), "label:bug");
    let query_of = |name: &str| -> String {
        let all = call(&e, "view.list", json!({ "all": true }));
        all["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .unwrap_or_else(|| panic!("{name} in {all}"))["query"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let before = find(&e, &query_of("Triage"));

    // Each rename rewrites the queries of the project's views, in the same
    // change, and says which views it rewrote.
    let renamed = call(
        &e,
        "status.update",
        json!({ "project": "TM", "status": "to do", "name": "Ready" }),
    );
    assert_eq!(renamed["views_rewritten"], json!(["Triage"]));
    call(
        &e,
        "type.update",
        json!({ "project": "TM", "type": "Bug", "name": "Defect" }),
    );
    call(
        &e,
        "label.update",
        json!({ "project": "TM", "label": "crash", "name": "Crash report" }),
    );
    assert_eq!(
        query_of("Triage"),
        "status:\"Ready\" type:Defect label:bug,\"Crash report\" -label:ui sort:due"
    );
    // The view still finds the same tasks.
    assert_eq!(find(&e, &query_of("Triage")), before);
    // The label of the same name in the sub-project is another label.
    assert_eq!(query_of("Web bugs"), "label:bug");

    // A new project key reaches every view, task keys included.
    let rekeyed = call(
        &e,
        "project.change_key",
        json!({ "project": "TM", "key": "TEAM" }),
    );
    assert_eq!(rekeyed["views_rewritten"], json!(["Keys"]));
    assert_eq!(
        query_of("Keys"),
        "project:TEAM/* OR TEAM-2 OR parent:TEAM-1"
    );
    // Undo takes the rewrite back with the key; Redo makes it again.
    call(&e, "history.undo", json!({}));
    assert_eq!(query_of("Keys"), "project:TM/* OR TM-2 OR parent:TM-1");
    call(&e, "history.redo", json!({}));
    assert_eq!(
        query_of("Keys"),
        "project:TEAM/* OR TEAM-2 OR parent:TEAM-1"
    );

    // Deleting a label takes it out of the queries and says so.
    let deleted = e
        .call(
            &app().with_confirmation(Confirmation::Given),
            "label.delete",
            json!({ "project": "TEAM", "label": "ui" }),
        )
        .unwrap();
    assert_eq!(deleted["views_rewritten"], json!(["Triage"]));
    assert_eq!(
        query_of("Triage"),
        "status:\"Ready\" type:Defect label:bug,\"Crash report\" sort:due"
    );

    // The view's history notes every rewrite.
    let id = call(&e, "view.list", json!({ "project": "TEAM" }))["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let feed = call(&e, "history.feed", json!({ "project": "TEAM" }));
    let rewrites = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["entity"] == "view" && entry["id"] == id.as_str())
        .count();
    assert_eq!(rewrites, 5, "{feed}");
    // A query that doesn't name the label is left alone.
    let untouched = call(
        &e,
        "label.update",
        json!({ "project": "TEAM", "label": "bug", "name": "bug" }),
    );
    assert!(untouched.get("views_rewritten").is_none());
}

#[test]
fn a_bulk_change_is_previewed_then_made_as_one_undo() {
    let (_dir, e) = workspace();
    let preview = call(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM is:open", "changes": {
            "add_labels": ["ui"], "priority": "high", "shift_due": "+1w" } }),
    );
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["count"], 4);
    assert!(preview.get("group").is_none());
    let tm1 = &preview["items"][0];
    assert_eq!(tm1["key"], "TM-1");
    let fields: Vec<&str> = tm1["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["field"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["labels", "due_date"]);
    assert_eq!(tm1["changes"][1]["after"], "2026-11-27");
    // Nothing changed.
    assert_eq!(find(&e, "label:ui"), v(&["TM-2"]));

    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM is:open", "changes": { "add_labels": ["ui"] },
                "dry_run": false }),
    );
    assert_eq!(err.details["path"], "expected_count");
    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM is:open", "changes": { "add_labels": ["ui"] },
                "dry_run": false, "expected_count": 3 }),
    );
    assert_eq!(err.code, ErrorCode::Conflict);

    let done = call(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM is:open", "changes": {
            "add_labels": ["ui"], "priority": "high", "shift_due": "+1w" },
            "dry_run": false, "expected_count": 4 }),
    );
    assert_eq!(done["changed"], 4);
    let group = done["group"].as_str().unwrap().to_owned();
    assert_eq!(find(&e, "label:ui"), v(&["TM-1", "TM-2", "TM-4", "TM-5"]));
    assert_eq!(find(&e, "due:2026-11-27"), v(&["TM-1"]));
    let feed = call(&e, "history.feed", json!({ "group": group, "limit": 50 }));
    assert_eq!(
        feed["items"].as_array().unwrap().len(),
        4,
        "one group, four tasks"
    );

    call(&e, "history.undo", json!({}));
    assert_eq!(find(&e, "label:ui"), v(&["TM-2"]));
    assert_eq!(find(&e, "priority:high"), v(&["TM-1"]));
    assert_eq!(find(&e, "due:2026-11-20"), v(&["TM-1"]));

    let moved = call(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-5", "TM-2"], "changes": { "status": "Review", "due_date": "fri" },
                "dry_run": false, "expected_count": 2 }),
    );
    assert_eq!(moved["items"][0]["changes"][0]["after"], "Review");
    assert_eq!(
        find(&e, "status:Review due:2026-11-27"),
        v(&["TM-2", "TM-5"])
    );

    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-1"], "changes": { "due_date": "someday" } }),
    );
    assert_eq!(err.details["path"], "changes.due_date");
    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-1"], "changes": {} }),
    );
    assert_eq!(err.details["path"], "changes");
    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-1"], "changes": { "add_labels": ["nope"] } }),
    );
    assert_eq!(
        err.code,
        ErrorCode::NotFound,
        "a preview runs the real checks"
    );
}

#[test]
fn agents_bulk_change_within_their_profile() {
    let (_dir, e) = workspace();
    let reader = agent_of(&e, Profile::ReadOnly);
    let err = e
        .call(
            &reader,
            "task.bulk_update",
            json!({ "keys": ["TM-1"], "changes": { "priority": "low" } }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
    let out = e
        .call(&reader, "task.search", json!({ "query": "is:open" }))
        .unwrap();
    assert_eq!(out["total"], 5);
}

// ---------------------------------------------------------------------
// Two processes

/// The other process: the command line making a bulk change. Does nothing
/// unless started by the test below.
#[test]
fn child_bulk() {
    let Ok(root) = std::env::var(CHILD) else {
        return;
    };
    let e = Engine::open(&WorkspaceLayout::new(root), config(Role::Cli)).unwrap();
    let out = e
        .call(
            &Context::user(Via::Cli),
            "task.bulk_update",
            json!({ "query": "label:bug", "changes": { "add_labels": ["crash"] },
                    "dry_run": false, "expected_count": 1 }),
        )
        .unwrap();
    println!("group={}", out["group"].as_str().unwrap());
}

#[test]
fn a_bulk_change_from_another_process_is_seen_and_reverted_as_one() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, Role::App);
    call(&e, "project.create", json!({ "name": "Team", "key": "TM" }));
    for l in ["bug", "crash"] {
        call(&e, "label.create", json!({ "project": "TM", "name": l }));
    }
    task(
        &e,
        json!({ "project": "TM", "title": "One", "labels": ["bug"] }),
    );
    task(&e, json!({ "project": "TM", "title": "Two" }));
    let events = e.subscribe();

    // The app's own bulk change, undone with Undo.
    let mine = call(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM", "changes": { "priority": "urgent" },
                "dry_run": false, "expected_count": 2 }),
    );
    assert_eq!(mine["changed"], 2);

    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_bulk", "--nocapture", "--test-threads=1"])
        .env(CHILD, layout(&dir).root())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let group = stdout
        .split("group=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("{stdout}"))
        .to_owned();
    assert_eq!(find(&e, "label:crash"), v(&["TM-1"]));

    // The app's Undo takes back its own change only.
    call(&e, "history.undo", json!({}));
    assert_eq!(find(&e, "priority:urgent"), Vec::<String>::new());
    assert_eq!(find(&e, "label:crash"), v(&["TM-1"]));

    // The other process's change reverts as one group.
    let reverted = call(&e, "history.revert", json!({ "groups": [group] }));
    assert!(
        reverted["skipped"].as_array().is_none_or(Vec::is_empty),
        "{reverted}"
    );
    assert_eq!(find(&e, "label:crash"), Vec::<String>::new());
    drop(events);
}

// ---------------------------------------------------------------------
// Performance

/// Writes `n` tasks straight into the tables, with labels, dates,
/// estimates and search entries.
fn generate(e: &Engine, n: usize) {
    call(e, "project.create", json!({ "name": "Big", "key": "BIG" }));
    for l in ["bug", "ui", "backend", "docs"] {
        call(e, "label.create", json!({ "project": "BIG", "name": l }));
    }
    e.store()
        .write("test.generate", move |tx| {
            let project: Vec<u8> =
                tx.query_row("SELECT id FROM projects WHERE key = 'BIG'", [], |r| {
                    r.get(0)
                })?;
            let ids = |sql: &str| -> Result<Vec<Vec<u8>>, Failure> {
                let mut stmt = tx.prepare(sql)?;
                let rows = stmt
                    .query_map([&project], |r| r.get::<_, Vec<u8>>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            };
            let statuses = ids("SELECT id FROM statuses WHERE project_id = ?1 ORDER BY rank")?;
            let types = ids("SELECT id FROM task_types WHERE project_id = ?1 ORDER BY rank")?;
            let labels = ids("SELECT id FROM labels WHERE project_id = ?1 ORDER BY rank")?;
            let words = [
                "login", "timeout", "layout", "crash", "deploy", "report", "cache",
            ];
            for i in 0..n {
                let id = gk_core::Id::generate().as_bytes().to_vec();
                let title = format!("{} {} task {i}", words[i % 7], words[(i / 7) % 7]);
                let due = (i % 5 != 0).then(|| {
                    let day: gk_core::Day = "2026-10-01".parse().unwrap();
                    day.add_days((i % 120) as i64).unwrap().to_string()
                });
                let status = &statuses[i % statuses.len()];
                tx.execute(
                    "INSERT INTO tasks (id, project_id, number, title, title_fold, status_id,
                        status_entered_at, type_id, priority, rank, due_date, estimate_min,
                        spent_min, created_at, created_by_kind, created_by_name, edited_at,
                        updated_at, closed_at)
                     VALUES (?1, ?2, ?3, ?4, lower(?4), ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?6,
                        'user', 'You', ?6, ?6, ?13)",
                    params![
                        id,
                        project,
                        (i + 1) as i64,
                        title,
                        status,
                        NOW - (i as i64) * 60_000,
                        types[i % types.len()],
                        (i % 5) as i64,
                        format!("r{i:07}"),
                        due,
                        (i % 3 != 0).then_some(((i % 8) * 30) as i64),
                        ((i % 4) * 15) as i64,
                        (i % statuses.len() == statuses.len() - 1).then_some(NOW),
                    ],
                )?;
                if i % 3 == 0 {
                    tx.execute(
                        "INSERT INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
                        params![id, labels[i % labels.len()]],
                    )?;
                }
                tx.execute(
                    "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                        updated_at) VALUES ('task', ?1, ?1, ?2, ?3, ?4, ?5)",
                    params![id, project, title, format!("Body of {title}."), NOW],
                )?;
                // Its history: the creation, then a change of status, of
                // the due date and of the priority, a day apart, the last
                // ones by an agent for every fourth task.
                let text_id = |b: &[u8]| gk_core::Id::from_slice(b).unwrap().to_string();
                let first = text_id(&statuses[0]);
                let changes = [
                    json!({ "fields": { "created": { "after": true },
                                        "status": { "after": first } } }),
                    json!({ "fields": { "status": { "before": first,
                                                    "after": text_id(status) } } }),
                    json!({ "fields": { "due_date": { "after": due } } }),
                    json!({ "fields": { "priority": { "before": "none",
                                                      "after": "high" } } }),
                ];
                for (k, change) in changes.iter().enumerate() {
                    let agent = i % 4 == 0 && k > 1;
                    tx.execute(
                        "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via,
                            project_id, entity_kind, entity_id, changes)
                         VALUES (?1, ?2, 'task.update', ?3, ?4, 'app', ?5, 'task', ?6, ?7)",
                        params![
                            gk_core::Id::generate().as_bytes().to_vec(),
                            NOW - (i as i64) * 60_000 - (3 - k as i64) * 86_400_000,
                            if agent { "agent" } else { "user" },
                            if agent { "Build bot" } else { "You" },
                            project,
                            id,
                            change.to_string(),
                        ],
                    )?;
                }
            }
            tx.execute(
                "UPDATE projects SET next_number = ?1 WHERE id = ?2",
                params![(n + 1) as i64, project],
            )?;
            Ok::<_, Failure>(())
        })
        .unwrap();
}

/// Filtering and sorting 20,000 tasks stays within the budget (150 ms in
/// a release build). Slow in debug builds, so run it on request:
/// `cargo test -p gk-engine --release --test queries -- --ignored`.
#[test]
#[ignore = "performance; run in a release build on request"]
fn twenty_thousand_tasks_filter_and_sort_within_budget() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, Role::App);
    generate(&e, 20_000);
    let budget_ms = if cfg!(debug_assertions) { 3000 } else { 150 };
    for query in [
        "",
        "is:open due<+7d sort:due",
        "label:bug,ui -status:Done sort:-priority,due",
        "estimate:1h..4h priority>=high sort:updated",
        "(label:backend OR type:Bug) is:open -has:labels sort:title",
        "timeout",
        "login cache sort:due",
        "is:overdue sort:-estimate",
        "was:status:Done",
        "was:category:in_progress is:open",
        "changed:due>-2d",
        "changed:status:this-week sort:due",
        "changed-by:agent",
        "-was:priority:urgent changed-by:me",
    ] {
        // Once to warm the caches, then timed.
        call(&e, "task.search", json!({ "query": query, "limit": 100 }));
        let start = Instant::now();
        let out = call(&e, "task.search", json!({ "query": query, "limit": 100 }));
        let ms = start.elapsed().as_millis();
        eprintln!("{ms:>5} ms  {:>6} tasks  {query}", out["total"]);
        assert!(ms < budget_ms, "{query}: {ms} ms");
    }
}

#[test]
fn a_view_keeps_a_description_of_what_it_is_for() {
    let (_dir, e) = workspace();
    let created = call(
        &e,
        "view.create",
        json!({ "name": "Release blockers", "project": "TM", "query": "is:open priority>=high",
                "description": "What must be fixed before the next release." }),
    );
    assert_eq!(
        created["view"]["description"],
        "What must be fixed before the next release."
    );
    call(
        &e,
        "view.update",
        json!({ "view": "Release blockers", "project": "TM",
                "description": "Fix these first." }),
    );
    let list = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(
        list["items"][0]["description"], "Fix these first.",
        "{list}"
    );
}
