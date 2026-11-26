// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Status rules and rules for agents, over a real workspace: what they
//! refuse, what they do to tasks entering a status, who may change them,
//! sub-projects, export and import, and Undo.

#![allow(clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

fn open(dir: &TempDir, name: &str) -> Engine {
    Engine::open(
        &WorkspaceLayout::new(dir.path().join(name)),
        EngineConfig::new(Role::App),
    )
    .unwrap()
}

fn app() -> Context {
    Context::user(Via::App)
}

fn call(e: &Engine, ctx: &Context, op: &str, input: Value) -> Value {
    e.call(ctx, op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, ctx: &Context, op: &str, input: Value) -> OpError {
    e.call(ctx, op, input).expect_err(op)
}

fn agent(e: &Engine) -> Context {
    let session = e
        .start_session(&SessionStart {
            name: "Helper".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    Context::agent("Helper", session, Profile::Standard)
}

fn setup(e: &Engine) {
    call(
        e,
        &app(),
        "project.create",
        json!({ "name": "Rules", "key": "RU" }),
    );
}

fn task(e: &Engine, ctx: &Context, title: &str) -> String {
    call(
        e,
        ctx,
        "task.create",
        json!({ "project": "RU", "title": title }),
    )["task"]["key"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn get(e: &Engine, key: &str) -> Value {
    call(e, &app(), "task.get", json!({ "keys": [key] }))["items"][0].clone()
}

fn status_of(project: &Value, name: &str) -> Value {
    project["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap()
        .clone()
}

fn rules(e: &Engine, input: Value) -> Value {
    call(e, &app(), "rules.update", input)
}

#[test]
fn a_status_requires_fields_sets_others_and_locks_some() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "Personal");
    setup(&e);
    let changed = rules(
        &e,
        json!({
            "project": "RU",
            "statuses": [
                { "status": "In progress", "rules": {
                    "required": ["estimate"],
                    "set": [{ "field": "start_date", "value": "today", "only_if_empty": true }],
                } },
                { "status": "Done", "rules": {
                    "set": [{ "field": "progress", "value": "100" }],
                    "read_only": ["title", "estimate"],
                } },
            ],
        }),
    );
    let project = &changed["project"];
    assert_eq!(
        status_of(project, "In progress")["rules"]["required"],
        json!(["estimate"])
    );
    assert!(status_of(project, "Backlog").get("rules").is_none());

    let key = task(&e, &app(), "Write the report");
    let err = fails(
        &e,
        &app(),
        "task.move",
        json!({ "key": key, "status": "In progress" }),
    );
    assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
    assert_eq!(err.details["message_id"], "rule-required");
    assert_eq!(err.details["missing"], json!(["estimate"]));
    assert!(err.message.contains("needs an estimate"), "{}", err.message);
    assert_eq!(
        err.hint.as_deref(),
        Some("Fill it in first; the project's settings list the rules of each status.")
    );
    assert_eq!(get(&e, &key)["status"]["name"], "Backlog");

    call(
        &e,
        &app(),
        "task.update",
        json!({ "key": key, "estimate": "2h" }),
    );
    call(
        &e,
        &app(),
        "task.move",
        json!({ "key": key, "status": "In progress" }),
    );
    let t = get(&e, &key);
    assert_eq!(t["status"]["name"], "In progress");
    assert!(t["start_date"].is_string(), "{t}");

    call(
        &e,
        &app(),
        "task.move",
        json!({ "key": key, "status": "Done" }),
    );
    assert_eq!(get(&e, &key)["progress"], 100);
    let err = fails(
        &e,
        &app(),
        "task.update",
        json!({ "key": key, "title": "Renamed" }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert_eq!(err.details["message_id"], "rule-read-only");
    // Other fields still change.
    call(
        &e,
        &app(),
        "task.update",
        json!({ "key": key, "priority": "high" }),
    );

    // Undo takes back the move and what the status did on the way in.
    call(
        &e,
        &app(),
        "task.update",
        json!({ "key": key, "priority": "none" }),
    );
    call(&e, &app(), "history.undo", json!({}));
    call(&e, &app(), "history.undo", json!({}));
    call(&e, &app(), "history.undo", json!({}));
    let t = get(&e, &key);
    assert_eq!(t["status"]["name"], "In progress");
    assert!(t.get("progress").is_none_or(Value::is_null), "{t}");
}

#[test]
fn agents_keep_out_of_statuses_and_fields_the_person_closed() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "Personal");
    setup(&e);
    rules(
        &e,
        json!({
            "project": "RU",
            "statuses": [{ "status": "Done", "rules": { "agents_may_not_enter": true } }],
            "agents": { "read_only": ["due_date"] },
        }),
    );
    let bot = agent(&e);
    let key = task(&e, &bot, "Fix the build");
    let err = fails(
        &e,
        &bot,
        "task.move",
        json!({ "key": key, "status": "Done" }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert_eq!(err.details["message_id"], "rule-agents-may-not-enter");
    assert!(err.hint.is_some());
    let err = fails(
        &e,
        &bot,
        "task.create",
        json!({ "project": "RU", "title": "Done already", "status": "Done" }),
    );
    assert_eq!(err.details["message_id"], "rule-agents-may-not-enter");
    let err = fails(
        &e,
        &bot,
        "task.update",
        json!({ "key": key, "due_date": "2026-12-01" }),
    );
    assert_eq!(err.details["message_id"], "rule-agent-field");
    call(
        &e,
        &bot,
        "task.move",
        json!({ "key": key, "status": "Review" }),
    );
    // The person is not held to the rules for agents.
    call(
        &e,
        &app(),
        "task.update",
        json!({ "key": key, "due_date": "2026-12-01" }),
    );
    call(
        &e,
        &app(),
        "task.move",
        json!({ "key": key, "status": "Done" }),
    );
    // Agents can't change the rules.
    let err = fails(
        &e,
        &bot,
        "rules.update",
        json!({ "project": "RU", "agents": {} }),
    );
    assert_eq!(err.code, ErrorCode::Forbidden);
}

#[test]
fn entering_a_status_stops_the_timer_and_releases_the_claim() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "Personal");
    setup(&e);
    rules(
        &e,
        json!({
            "project": "RU",
            "statuses": [{ "status": "Review", "rules": { "stop_timer": true, "release_claim": true } }],
        }),
    );
    let bot = agent(&e);
    let key = task(&e, &app(), "Review me");
    call(&e, &bot, "task.claim", json!({ "key": key }));
    assert!(get(&e, &key).get("claim").is_some_and(|c| !c.is_null()));
    call(&e, &app(), "timer.start", json!({ "key": key }));
    call(
        &e,
        &bot,
        "task.move",
        json!({ "key": key, "status": "Review" }),
    );
    let t = get(&e, &key);
    assert!(t.get("claim").is_none_or(Value::is_null), "{t}");
    assert!(
        call(&e, &app(), "timer.get", json!({}))
            .get("timer")
            .is_none(),
        "the timer stopped"
    );
}

#[test]
fn rules_are_checked_copied_to_sub_projects_and_exported() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "Personal");
    setup(&e);
    for (input, id) in [
        (
            json!({ "status": "Done", "rules": { "required": ["title"] } }),
            "rule-field-not-allowed",
        ),
        (
            json!({ "status": "Done", "rules": { "required": ["estimate", "estimate"] } }),
            "rule-field-twice",
        ),
        (
            json!({ "status": "Done", "rules": { "set": [{ "field": "progress", "value": "120" }] } }),
            "rule-set-value",
        ),
        (
            json!({ "status": "Done", "rules": { "required": ["estimate"], "clear": ["estimate"] } }),
            "rule-required-and-cleared",
        ),
        (
            json!({ "status": "Done", "rules": { "read_only": ["cf:0190e5c4a2b37c3d8e9f0a1b2c3d4e5f"] } }),
            "rule-custom-field-unknown",
        ),
    ] {
        let err = fails(
            &e,
            &app(),
            "rules.update",
            json!({ "project": "RU", "statuses": [input] }),
        );
        assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
        assert_eq!(err.details["message_id"], id, "{err:?}");
        assert!(
            err.details["path"]
                .as_str()
                .unwrap()
                .starts_with("statuses[0]"),
            "{err:?}"
        );
    }
    rules(
        &e,
        json!({
            "project": "RU",
            "statuses": [{ "status": "Done", "rules": { "required": ["estimate"] } }],
            "agents": { "read_only": ["title"] },
        }),
    );
    let sub = call(
        &e,
        &app(),
        "project.create",
        json!({ "name": "Sub", "key": "RUS", "parent": "RU" }),
    )["project"]
        .clone();
    assert_eq!(
        status_of(&sub, "Done")["rules"]["required"],
        json!(["estimate"])
    );
    assert_eq!(sub["agent_rules"]["read_only"], json!(["title"]));

    let archive = dir.path().join("out.zip");
    call(
        &e,
        &app(),
        "transfer.export",
        json!({ "path": archive.to_str().unwrap() }),
    );
    let other = open(&dir, "Other");
    call(
        &other,
        &app(),
        "transfer.import",
        json!({ "path": archive.to_str().unwrap() }),
    );
    let back = call(&other, &app(), "project.get", json!({ "project": "RU" }));
    assert_eq!(
        status_of(&back, "Done")["rules"]["required"],
        json!(["estimate"])
    );
    assert_eq!(back["agent_rules"]["read_only"], json!(["title"]));

    // Removing the rules is one change, which Undo takes back.
    rules(
        &e,
        json!({ "project": "RU", "statuses": [{ "status": "Done", "rules": {} }], "agents": {} }),
    );
    let now = call(&e, &app(), "project.get", json!({ "project": "RU" }));
    assert!(status_of(&now, "Done").get("rules").is_none());
    assert!(now.get("agent_rules").is_none());
    call(&e, &app(), "history.undo", json!({}));
    let back = call(&e, &app(), "project.get", json!({ "project": "RU" }));
    assert_eq!(
        status_of(&back, "Done")["rules"]["required"],
        json!(["estimate"])
    );
}
