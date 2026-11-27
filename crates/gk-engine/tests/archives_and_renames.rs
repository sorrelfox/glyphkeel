// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards in archives, project keys that change on import, and the
//! workspace's rules, views and dashboards that are about one project
//! following its renames and deletions.

#![allow(clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
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

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&Context::user(Via::App), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn sure(e: &Engine, op: &str, input: Value) -> Value {
    let ctx = Context::user(Via::Cli).with_confirmation(Confirmation::Given);
    e.call(&ctx, op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn seed(e: &Engine) {
    call(e, "project.create", json!({ "name": "Team", "key": "TM" }));
    call(e, "project.create", json!({ "name": "Other", "key": "ZZ" }));
    call(
        e,
        "project.update",
        json!({ "project": "TM", "statuses": [{ "name": "Parked" }] }),
    );
    call(
        e,
        "label.create",
        json!({ "project": "TM", "name": "chore" }),
    );
    call(e, "task.create", json!({ "project": "TM", "title": "One" }));
}

fn dashboards(e: &Engine) -> Vec<Value> {
    call(e, "dashboard.list", json!({ "all": true }))["items"]
        .as_array()
        .unwrap()
        .clone()
}

fn widget_query(e: &Engine, dashboard: &str, project: Option<&str>, at: usize) -> String {
    let mut input = json!({ "dashboard": dashboard });
    if let Some(p) = project {
        input["project"] = json!(p);
    }
    call(e, "dashboard.show", input)["dashboard"]["widgets"][at]["settings"]["query"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn dashboards_travel_in_archives_with_their_projects_and_keys() {
    let dir = TempDir::new().unwrap();
    let a = open(&dir, "A");
    seed(&a);
    call(
        &a,
        "dashboard.create",
        json!({ "name": "Everything", "widgets": [
            { "kind": "query_count", "title": "Team open",
              "settings": { "query": "project:TM is:open TM-1" } },
            { "kind": "overdue", "settings": { "project": "TM" } },
            { "kind": "note", "settings": { "text": "Hello" } },
        ] }),
    );
    call(
        &a,
        "dashboard.create",
        json!({ "name": "Team board", "project": "TM", "widgets": [
            { "kind": "query_list", "settings": { "query": "status:Parked" } },
        ] }),
    );
    // A project export takes its pinned dashboards only.
    let one = dir.path().join("tm.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": one.to_string_lossy(), "projects": ["TM"] }),
    );
    let whole = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": whole.to_string_lossy() }),
    );

    let b = open(&dir, "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": one.to_string_lossy() }),
    );
    let names: Vec<String> = dashboards(&b)
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Team board"]);

    let c = open(&dir, "C");
    call(
        &c,
        "transfer.import",
        json!({ "path": whole.to_string_lossy() }),
    );
    let all = dashboards(&c);
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(all[0]["name"], "Everything");
    assert_eq!(all[1]["project"], "TM");
    let shown = call(&c, "dashboard.show", json!({ "dashboard": "Everything" }));
    // The widget's project is the imported project, shown by its key.
    assert_eq!(
        shown["dashboard"]["widgets"][1]["settings"]["project"],
        "TM"
    );
    assert!(shown["widgets"][1].get("error").is_none(), "{shown}");
    assert_eq!(shown["widgets"][0]["count"], 1, "{shown}");
    assert_eq!(
        shown["dashboard"]["widgets"][2]["settings"]["text"],
        "Hello"
    );

    // Again, under new keys: the workspace dashboard's name is taken and
    // it is left out; the pinned one comes with its project, its queries
    // naming the new keys.
    call(
        &c,
        "transfer.import",
        json!({ "path": whole.to_string_lossy(),
                "keys": [{ "from": "TM", "to": "TN" }, { "from": "ZZ", "to": "ZY" }] }),
    );
    let all = dashboards(&c);
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(all[2]["project"], "TN");
    assert_eq!(
        widget_query(&c, "Team board", Some("TN"), 0),
        "status:Parked"
    );
}

#[test]
fn an_import_under_a_new_key_rewrites_the_queries_that_name_it() {
    let dir = TempDir::new().unwrap();
    let a = open(&dir, "A");
    seed(&a);
    call(
        &a,
        "view.create",
        json!({ "name": "Mine", "project": "TM", "query": "project:TM parent:TM-1" }),
    );
    call(
        &a,
        "layout.update",
        json!({ "project": "TM", "colour_by": "query",
                "colour_rules": [{ "query": "project:TM label:chore", "hue": "green" }] }),
    );
    call(
        &a,
        "automation.create",
        json!({ "project": "TM", "name": "Park", "condition": "project:TM is:open",
                "trigger": { "on": "comment_added" },
                "actions": [{ "kind": "move_to_status", "status": "Parked" }] }),
    );
    call(
        &a,
        "automation.create",
        json!({ "name": "Everywhere", "condition": "project:TM OR TM-1",
                "trigger": { "on": "comment_added" },
                "actions": [{ "kind": "notify", "text": "{key}" }] }),
    );
    call(
        &a,
        "dashboard.create",
        json!({ "name": "Team board", "project": "TM", "widgets": [
            { "kind": "query_count", "settings": { "query": "project:TM" } },
        ] }),
    );
    let zip = dir.path().join("all.zip");
    call(
        &a,
        "transfer.export",
        json!({ "path": zip.to_string_lossy() }),
    );

    let b = open(&dir, "B");
    call(
        &b,
        "transfer.import",
        json!({ "path": zip.to_string_lossy(),
                "keys": [{ "from": "TM", "to": "TK" }] }),
    );
    let views = call(&b, "view.list", json!({ "project": "TK" }));
    assert_eq!(views["items"][0]["query"], "project:TK parent:TK-1");
    let layout = call(&b, "layout.get", json!({ "project": "TK" }));
    assert_eq!(
        layout["layout"]["colour_rules"][0]["query"], "project:TK label:chore",
        "{layout}"
    );
    let rule = call(
        &b,
        "automation.get",
        json!({ "rule": "Park", "project": "TK" }),
    );
    assert_eq!(rule["condition"], "project:TK is:open");
    let rule = call(&b, "automation.get", json!({ "rule": "Everywhere" }));
    assert_eq!(rule["condition"], "project:TK OR TK-1");
    assert_eq!(widget_query(&b, "Team board", Some("TK"), 0), "project:TK");
}

#[test]
fn workspace_rules_views_and_widgets_about_one_project_follow_its_names() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "A");
    seed(&e);
    call(
        &e,
        "project.update",
        json!({ "project": "ZZ", "statuses": [{ "name": "Parked" }] }),
    );
    // About the project by its condition.
    call(
        &e,
        "automation.create",
        json!({ "name": "Park team", "condition": "project:TM status:Parked",
                "trigger": { "on": "moved_to_status", "status": "Parked" },
                "actions": [{ "kind": "add_label", "label": "chore" }] }),
    );
    // Not about it: both projects have a status named Parked.
    call(
        &e,
        "automation.create",
        json!({ "name": "Park all",
                "trigger": { "on": "moved_to_status", "status": "Parked" },
                "actions": [{ "kind": "notify", "text": "{key}" }] }),
    );
    // A label only the project has.
    call(
        &e,
        "automation.create",
        json!({ "name": "Chores",
                "trigger": { "on": "comment_added" },
                "actions": [{ "kind": "add_label", "label": "chore" }] }),
    );
    call(
        &e,
        "view.create",
        json!({ "name": "Parked here", "query": "project:TM status:Parked" }),
    );
    call(
        &e,
        "dashboard.create",
        json!({ "name": "Everything", "widgets": [
            { "kind": "query_count", "settings": { "query": "project:TM status:Parked" } },
            { "kind": "query_count", "settings": { "query": "status:Parked" } },
        ] }),
    );

    call(
        &e,
        "project.update",
        json!({ "project": "TM", "statuses": [{ "status": "Parked", "name": "On hold" }] }),
    );
    let rule = call(&e, "automation.get", json!({ "rule": "Park team" }));
    assert_eq!(rule["trigger"]["status"], "On hold");
    assert_eq!(rule["condition"], "project:TM status:\"On hold\"");
    let rule = call(&e, "automation.get", json!({ "rule": "Park all" }));
    assert_eq!(rule["trigger"]["status"], "Parked");
    let views = call(&e, "view.list", json!({}));
    let view = views["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "Parked here")
        .unwrap();
    assert_eq!(view["query"], "project:TM status:\"On hold\"");
    assert_eq!(
        widget_query(&e, "Everything", None, 0),
        "project:TM status:\"On hold\""
    );
    assert_eq!(widget_query(&e, "Everything", None, 1), "status:Parked");

    // A label only this project had is renamed, then removed: the rule
    // follows, then is turned off and says why.
    call(
        &e,
        "project.update",
        json!({ "project": "TM", "labels": [{ "label": "chore", "name": "chores" }] }),
    );
    let rule = call(&e, "automation.get", json!({ "rule": "Chores" }));
    assert_eq!(rule["actions"][0]["label"], "chores");
    assert_eq!(rule["enabled"], true);
    sure(
        &e,
        "project.update",
        json!({ "project": "TM", "labels": [{ "label": "chores", "remove": true }] }),
    );
    let rule = call(&e, "automation.get", json!({ "rule": "Chores" }));
    assert_eq!(rule["enabled"], false);
    let runs = call(&e, "automation.runs", json!({ "rule": "Chores" }));
    let message = runs["items"][0]["error"]["message"].as_str().unwrap();
    assert!(message.contains("chores"), "{message}");
    // So is the rule about the project that adds it.
    let rule = call(&e, "automation.get", json!({ "rule": "Park team" }));
    assert_eq!(rule["enabled"], false);
    let rule = call(&e, "automation.get", json!({ "rule": "Park all" }));
    assert_eq!(rule["enabled"], true);
}

#[test]
fn a_workspace_rule_setting_a_projects_field_follows_its_rename_and_deletion() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "A");
    seed(&e);
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    call(
        &e,
        "automation.create",
        json!({ "name": "Size", "trigger": { "on": "comment_added" },
                "actions": [{ "kind": "set_fields", "fields": { "Points": 3 } }] }),
    );
    call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Points", "name": "Effort" }),
    );
    let rule = call(&e, "automation.get", json!({ "rule": "Size" }));
    assert_eq!(rule["actions"][0]["fields"], json!({ "Effort": 3 }));
    sure(
        &e,
        "field.delete",
        json!({ "project": "TM", "field": "Effort", "expected_task_count": 0 }),
    );
    let rule = call(&e, "automation.get", json!({ "rule": "Size" }));
    assert_eq!(rule["enabled"], false);
}

#[test]
fn a_dashboard_is_pinned_to_a_project_and_back_and_undone() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir, "A");
    seed(&e);
    call(&e, "dashboard.create", json!({ "name": "Team" }));
    let pinned = call(
        &e,
        "dashboard.update",
        json!({ "dashboard": "Team", "pin_to": "TM" }),
    );
    assert_eq!(pinned["dashboard"]["project"], "TM");
    let err = e
        .call(
            &Context::user(Via::App),
            "dashboard.update",
            json!({ "dashboard": "Team", "project": "TM", "pin_to": "ZZ", "clear": ["pin_to"] }),
        )
        .unwrap_err();
    assert_eq!(err.details["path"], "clear", "{err:?}");
    let back = call(
        &e,
        "dashboard.update",
        json!({ "dashboard": "Team", "project": "TM", "clear": ["pin_to"] }),
    );
    assert!(back["dashboard"].get("project").is_none(), "{back}");
    call(&e, "history.undo", json!({}));
    assert_eq!(dashboards(&e)[0]["project"], "TM");
}
