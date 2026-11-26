// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields over a real workspace: definitions, values on tasks,
//! queries, renames, deletion, sub-projects, moves and history.

#![allow(clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

fn open(dir: &TempDir) -> Engine {
    Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        EngineConfig::new(Role::App),
    )
    .unwrap()
}

fn me() -> Context {
    Context::user(Via::Cli)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn fails(e: &Engine, op: &str, input: Value) -> OpError {
    e.call(&me(), op, input).expect_err(op)
}

fn message_id(err: &OpError) -> String {
    err.details
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn project(e: &Engine, key: &str, parent: Option<&str>) {
    let mut input = json!({ "name": format!("Project {key}"), "key": key });
    if let Some(p) = parent {
        input["parent"] = json!(p);
    }
    call(e, "project.create", input);
}

fn get(e: &Engine, key: &str) -> Value {
    call(e, "task.get", json!({ "keys": [key] }))["items"][0].clone()
}

fn fields_of(task: &Value) -> Vec<(String, Value)> {
    task.get("fields")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|f| (f["name"].as_str().unwrap().to_owned(), f["value"].clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn search(e: &Engine, query: &str) -> Vec<String> {
    call(e, "task.search", json!({ "query": query }))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn setup(e: &Engine) {
    project(e, "TM", None);
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Story points", "kind": "integer" }),
    );
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "select",
                "options": ["Front end", "Back end"] }),
    );
    call(
        e,
        "field.create",
        json!({ "name": "Platforms", "kind": "multi_select",
                "options": ["Linux", "Windows", "macOS"] }),
    );
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Approved", "kind": "boolean" }),
    );
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Cost", "kind": "decimal" }),
    );
    call(
        e,
        "field.create",
        json!({ "project": "TM", "name": "Review on", "kind": "date" }),
    );
}

#[test]
fn values_are_set_read_listed_and_queried() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    let list = call(&e, "field.list", json!({ "project": "TM" }));
    let names: Vec<&str> = list["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Platforms",
            "Story points",
            "Area",
            "Approved",
            "Cost",
            "Review on"
        ],
        "fields of every project first"
    );
    assert_eq!(list["fields"][1]["query_name"], "cf.story-points");

    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One",
                "fields": { "Story points": 3, "area": "back END", "cf.platforms": ["Windows", "linux"],
                            "Approved": "yes", "Cost": 2.5, "Review on": "2026-11-30" } }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Two", "fields": { "Story points": "8" } }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Three" }),
    );

    let one = get(&e, "TM-1");
    assert_eq!(
        fields_of(&one),
        vec![
            ("Platforms".to_owned(), json!(["Linux", "Windows"])),
            ("Story points".to_owned(), json!(3)),
            ("Area".to_owned(), json!("Back end")),
            ("Approved".to_owned(), json!(true)),
            ("Cost".to_owned(), json!(2.5)),
            ("Review on".to_owned(), json!("2026-11-30")),
        ]
    );
    let listed = call(&e, "task.search", json!({ "query": "key:TM-1" }));
    assert_eq!(listed["items"][0]["fields"]["story-points"], 3);
    assert_eq!(
        listed["items"][0]["fields"]["platforms"],
        json!(["Linux", "Windows"])
    );

    assert_eq!(search(&e, "cf.story-points>=3"), ["TM-1", "TM-2"]);
    assert_eq!(search(&e, "story-points:1..5"), ["TM-1"]);
    assert_eq!(search(&e, "cf.story-points:none"), ["TM-3"]);
    assert_eq!(search(&e, "cf.area:\"back end\""), ["TM-1"]);
    assert_eq!(search(&e, "-cf.area:\"back end\""), ["TM-2", "TM-3"]);
    assert_eq!(search(&e, "cf.platforms:windows,macos"), ["TM-1"]);
    assert_eq!(search(&e, "cf.approved:no"), ["TM-2", "TM-3"]);
    assert_eq!(search(&e, "cf.cost<3"), ["TM-1"]);
    assert_eq!(search(&e, "cf.review-on:2026-11-01..2026-11-30"), ["TM-1"]);
    assert_eq!(
        search(&e, "project:TM sort:-cf.story-points"),
        ["TM-2", "TM-1", "TM-3"]
    );
    let err = fails(&e, "task.search", json!({ "query": "cf.area:middle" }));
    assert_eq!(message_id(&err), "query-bad-choice");
    let err = fails(&e, "task.search", json!({ "query": "cf.nothing:1" }));
    assert_eq!(message_id(&err), "query-unknown-custom-field-plain");

    // Changing and clearing values; wrong values are refused.
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Story points": 5, "Area": null } }),
    );
    let one = get(&e, "TM-1");
    assert!(
        fields_of(&one)
            .iter()
            .any(|(n, v)| n == "Story points" && *v == json!(5))
    );
    assert!(!fields_of(&one).iter().any(|(n, _)| n == "Area"));
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Area": "Middle" } }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);
    assert_eq!(message_id(&err), "value-field-option");
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Story points": 1.5 } }),
    );
    assert_eq!(message_id(&err), "value-field-integer");
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Nope": 1 } }),
    );
    assert_eq!(err.code, ErrorCode::NotFound);

    // The query field knows the custom fields.
    let parsed = e
        .call(
            &Context::user(Via::App),
            "query.parse",
            json!({ "query": "cf.area:front", "fields": true }),
        )
        .unwrap();
    let area = parsed["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "cf.area")
        .cloned()
        .unwrap();
    assert_eq!(area["choices"], json!(["Front end", "Back end"]));
}

#[test]
fn names_are_unique_and_kinds_agree_across_projects() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    project(&e, "OT", None);
    let err = fails(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "story-points", "kind": "integer" }),
    );
    assert_eq!(message_id(&err), "field-name-taken");
    let err = fails(
        &e,
        "field.create",
        json!({ "project": "OT", "name": "PLATFORMS", "kind": "text" }),
    );
    assert_eq!(
        message_id(&err),
        "field-name-taken",
        "a field of every project"
    );
    let err = fails(
        &e,
        "field.create",
        json!({ "project": "OT", "name": "Story points", "kind": "text" }),
    );
    assert_eq!(message_id(&err), "field-kind-clash");
    call(
        &e,
        "field.create",
        json!({ "project": "OT", "name": "Story points", "kind": "integer" }),
    );
    let err = fails(
        &e,
        "field.create",
        json!({ "project": "OT", "name": "Kind", "kind": "select" }),
    );
    assert_eq!(message_id(&err), "field-needs-options");
    let err = fails(
        &e,
        "field.create",
        json!({ "project": "OT", "name": "Size", "kind": "integer", "options": ["S"] }),
    );
    assert_eq!(message_id(&err), "field-options-not-allowed");
}

#[test]
fn defaults_required_fields_and_task_types() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM", None);
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Severity", "kind": "select",
                "options": ["Low", "High"], "default": "low", "types": ["bug"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Customer", "kind": "text", "required": true,
                "types": ["Bug"] }),
    );
    // A task of another type gets neither the default nor the requirement.
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Plain" }),
    );
    assert!(fields_of(&get(&e, "TM-1")).is_empty());
    let err = fails(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Crash", "type": "Bug" }),
    );
    assert_eq!(message_id(&err), "field-required");
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "Crash", "type": "Bug",
                "fields": { "Customer": "ACME" } }),
    );
    assert_eq!(
        fields_of(&get(&e, "TM-2")),
        vec![
            ("Severity".to_owned(), json!("Low")),
            ("Customer".to_owned(), json!("ACME")),
        ]
    );
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-2", "fields": { "Customer": null } }),
    );
    assert_eq!(message_id(&err), "field-required");
    let err = fails(
        &e,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Severity": "High" } }),
    );
    assert_eq!(message_id(&err), "field-not-for-type");
    // Renaming the type keeps the field shown for it.
    call(
        &e,
        "type.update",
        json!({ "project": "TM", "type": "Bug", "name": "Defect" }),
    );
    let list = call(&e, "field.list", json!({ "project": "TM" }));
    assert_eq!(list["fields"][0]["types"], json!(["Defect"]));
}

#[test]
fn renames_rewrite_values_and_saved_views() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One", "fields": { "Area": "Front end", "Story points": 2 } }),
    );
    call(
        &e,
        "view.create",
        json!({ "name": "Front", "project": "TM", "query": "cf.area:\"front end\" story-points>1",
                "sort": "-cf.story-points", "columns": ["key", "cf.story-points", "cf.area"] }),
    );
    let change = call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Story points", "name": "Points" }),
    );
    assert_eq!(change["field"]["query_name"], "cf.points");
    assert_eq!(change["views_rewritten"], json!(["Front"]));
    let change = call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Area",
                "rename_options": [{ "from": "front end", "to": "UI" }] }),
    );
    assert_eq!(change["field"]["options"], json!(["UI", "Back end"]));
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    let view = &views["items"][0];
    assert_eq!(view["query"], "cf.area:\"UI\" cf.points>1");
    assert_eq!(view["sort"], "-cf.points");
    assert_eq!(view["columns"], json!(["key", "cf.points", "cf.area"]));
    assert!(fields_of(&get(&e, "TM-1")).contains(&("Area".to_owned(), json!("UI"))));
    // Reverting takes the option's new name back, on the task too.
    call(
        &e,
        "history.revert",
        json!({ "groups": [change["group"].as_str().unwrap()] }),
    );
    assert!(fields_of(&get(&e, "TM-1")).contains(&("Area".to_owned(), json!("Front end"))));
    let list = call(&e, "field.list", json!({ "project": "TM" }));
    assert_eq!(
        list["fields"][2]["options"],
        json!(["Front end", "Back end"])
    );

    // Options left out of a new list leave the tasks.
    let change = call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Area", "options": ["Back end", "Docs"] }),
    );
    assert_eq!(change["values_removed"], 1);
    assert!(!fields_of(&get(&e, "TM-1")).iter().any(|(n, _)| n == "Area"));
}

#[test]
fn deleting_a_field_is_confirmed_and_takes_its_values() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One", "fields": { "Story points": 2 } }),
    );
    call(
        &e,
        "view.create",
        json!({ "name": "Big", "project": "TM", "query": "cf.story-points>1 is:open",
                "columns": ["key", "cf.story-points"] }),
    );
    let input = json!({ "project": "TM", "field": "Story points", "expected_task_count": 1 });
    assert_eq!(
        fails(&e, "field.delete", input.clone()).code,
        ErrorCode::Forbidden
    );
    let sure = Context::user(Via::Cli).with_confirmation(Confirmation::Given);
    let err = e
        .call(
            &sure,
            "field.delete",
            json!({ "project": "TM", "field": "Story points", "expected_task_count": 3 }),
        )
        .unwrap_err();
    assert_eq!(message_id(&err), "field-count-differs");
    let change = e.call(&sure, "field.delete", input).unwrap();
    assert_eq!(change["values_removed"], 1);
    assert!(change.get("field").is_none());
    assert!(fields_of(&get(&e, "TM-1")).is_empty());
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "is:open");
    assert_eq!(views["items"][0]["columns"], json!(["key"]));
}

#[test]
fn sub_projects_copy_fields_and_moves_keep_values_by_name() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    project(&e, "SUB", Some("TM"));
    let sub = call(&e, "field.list", json!({ "project": "SUB" }));
    let names: Vec<&str> = sub["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Platforms",
            "Story points",
            "Area",
            "Approved",
            "Cost",
            "Review on"
        ]
    );
    assert_eq!(sub["fields"][1]["project"], "SUB", "a copy of its own");
    project(&e, "OT", None);
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One",
                "fields": { "Story points": 2, "Platforms": ["Linux"], "Area": "Back end" } }),
    );
    let moved = call(&e, "task.move", json!({ "key": "TM-1", "project": "SUB" }));
    let key = moved["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(
        fields_of(&get(&e, &key)),
        vec![
            ("Platforms".to_owned(), json!(["Linux"])),
            ("Story points".to_owned(), json!(2)),
            ("Area".to_owned(), json!("Back end")),
        ]
    );
    let check = call(
        &e,
        "task.move_check",
        json!({ "keys": [key.clone()], "project": "OT" }),
    );
    let kept: Vec<(&str, Option<&str>)> = check["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["name"].as_str().unwrap(), m["target"].as_str()))
        .collect();
    assert_eq!(
        kept,
        [
            ("Platforms", Some("Platforms")),
            ("Story points", None),
            ("Area", None)
        ],
        "the check shows which values would be left off"
    );
    let moved = call(&e, "task.move", json!({ "key": key, "project": "OT" }));
    let key = moved["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(
        fields_of(&get(&e, &key)),
        vec![("Platforms".to_owned(), json!(["Linux"]))]
    );
    assert!(moved["warnings"].as_array().is_some_and(|w| {
        w.iter()
            .any(|w| w.as_str().unwrap().contains("Story points"))
    }));
}

#[test]
fn undo_takes_back_values_and_new_fields() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    let app = Context::user(Via::App);
    e.call(
        &app,
        "task.create",
        json!({ "project": "TM", "title": "One" }),
    )
    .unwrap();
    e.call(
        &app,
        "task.update",
        json!({ "key": "TM-1", "fields": { "Story points": 4 } }),
    )
    .unwrap();
    e.call(&app, "history.undo", json!({})).unwrap();
    assert!(fields_of(&get(&e, "TM-1")).is_empty());
    e.call(&app, "history.redo", json!({})).unwrap();
    assert_eq!(
        fields_of(&get(&e, "TM-1")),
        vec![("Story points".to_owned(), json!(4))]
    );
    e.call(
        &app,
        "field.create",
        json!({ "project": "TM", "name": "Later", "kind": "text" }),
    )
    .unwrap();
    e.call(&app, "history.undo", json!({})).unwrap();
    let list = call(&e, "field.list", json!({ "project": "TM" }));
    assert!(
        !list["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "Later")
    );
}

#[test]
fn archives_carry_fields_and_values() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    setup(&e);
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "One",
                "fields": { "Story points": 3, "Platforms": ["Linux", "macOS"], "Cost": 1.25,
                            "Approved": true } }),
    );
    let zip = dir.path().join("all.zip");
    call(
        &e,
        "transfer.export",
        json!({ "path": zip.to_string_lossy(), "projects": ["TM"] }),
    );
    let other = Engine::open(
        &WorkspaceLayout::new(dir.path().join("Other")),
        EngineConfig::new(Role::App),
    )
    .unwrap();
    call(
        &other,
        "transfer.import",
        json!({ "path": zip.to_string_lossy() }),
    );
    assert_eq!(fields_of(&get(&other, "TM-1")), fields_of(&get(&e, "TM-1")));
    let list = call(&other, "field.list", json!({}));
    assert_eq!(list["fields"][0]["name"], "Platforms");
    assert!(
        list["fields"][0].get("project").is_none(),
        "still a field of every project"
    );
    // Imported again under another key, the shared field is the same one.
    call(
        &other,
        "transfer.import",
        json!({ "path": zip.to_string_lossy(), "keys": [{ "from": "TM", "to": "TN" }] }),
    );
    let list = call(&other, "field.list", json!({}));
    let shared = list["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["name"] == "Platforms")
        .count();
    assert_eq!(shared, 1);
    assert_eq!(search(&other, "cf.platforms:macos"), ["TM-1", "TN-1"]);
}
