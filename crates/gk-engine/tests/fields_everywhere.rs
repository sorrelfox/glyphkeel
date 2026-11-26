// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields beyond the task panel: in the history of queries, in bulk
//! changes, in full-text search, and checked again when a task takes
//! another type or moves to another project.

#![allow(clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_api::{OpError, Profile};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role, SessionStart};
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
    Context::user(Via::App)
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

fn project(e: &Engine, key: &str) {
    call(
        e,
        "project.create",
        json!({ "name": format!("Project {key}"), "key": key }),
    );
}

fn task(e: &Engine, project: &str, title: &str) {
    call(
        e,
        "task.create",
        json!({ "project": project, "title": title }),
    );
}

fn value(e: &Engine, key: &str, field: &str) -> Value {
    call(e, "task.get", json!({ "keys": [key] }))["items"][0]["fields"]
        .as_array()
        .and_then(|a| a.iter().find(|f| f["name"] == field))
        .map_or(Value::Null, |f| f["value"].clone())
}

fn search(e: &Engine, query: &str) -> Vec<String> {
    call(e, "task.search", json!({ "query": query }))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn update(e: &Engine, key: &str, fields: Value) {
    call(e, "task.update", json!({ "key": key, "fields": fields }));
}

#[test]
fn history_conditions_look_at_custom_fields() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "select",
                "options": ["Front end", "Back end"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Platforms", "kind": "multi_select",
                "options": ["Linux", "Windows"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    for title in ["One", "Two", "Three"] {
        task(&e, "TM", title);
    }
    update(
        &e,
        "TM-1",
        json!({ "Area": "Front end", "Platforms": ["Linux"] }),
    );
    update(
        &e,
        "TM-1",
        json!({ "Area": "Back end", "Platforms": ["Windows"] }),
    );
    update(&e, "TM-2", json!({ "Points": 3 }));
    update(&e, "TM-2", json!({ "Points": 5 }));

    assert_eq!(search(&e, "was:cf.area:\"front end\""), ["TM-1"]);
    assert_eq!(
        message_id(&fails(
            &e,
            "task.search",
            json!({ "query": "was:cf.area:back" })
        )),
        "query-bad-choice",
        "options are named in full"
    );
    assert_eq!(search(&e, "was:cf.platforms:linux"), ["TM-1"]);
    assert_eq!(search(&e, "was:cf.points:3"), ["TM-2"]);
    assert_eq!(search(&e, "was:cf.points!=3"), ["TM-1", "TM-3"]);
    assert_eq!(search(&e, "changed:cf.points"), ["TM-2"]);
    assert_eq!(search(&e, "changed:cf.area,cf.points"), ["TM-1", "TM-2"]);
    assert_eq!(search(&e, "changed:cf.area:today"), ["TM-1"]);
    assert_eq!(search(&e, "changed:cf.area<-1d"), Vec::<String>::new());
    assert_eq!(search(&e, "-changed:cf.points"), ["TM-1", "TM-3"]);

    // Changes by an agent count for `changed-by`, custom fields included.
    let session = e
        .start_session(&SessionStart {
            name: "Helper".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    let helper = Context::agent("Helper", session, Profile::Standard);
    e.call(
        &helper,
        "task.update",
        json!({ "key": "TM-3", "fields": { "Points": 1 } }),
    )
    .unwrap();
    assert_eq!(search(&e, "changed-by:agent"), ["TM-3"]);
    assert_eq!(search(&e, "changed-by:agent changed:cf.points"), ["TM-3"]);

    // A renamed field keeps its history, and saved views follow the name.
    call(
        &e,
        "view.create",
        json!({ "name": "Moved points", "project": "TM",
                "query": "changed:cf.points was:cf.points:3" }),
    );
    call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Points", "name": "Size" }),
    );
    assert_eq!(search(&e, "was:cf.size:3"), ["TM-2"]);
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["query"], "changed:cf.size was:cf.size:3");
}

#[test]
fn a_type_change_needs_the_required_fields_of_the_new_type() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Customer", "kind": "text", "required": true,
                "types": ["Bug"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Severity", "kind": "select", "required": true,
                "options": ["Low", "High"], "default": "Low", "types": ["Bug"] }),
    );
    task(&e, "TM", "Plain");
    let err = fails(&e, "task.update", json!({ "key": "TM-1", "type": "Bug" }));
    assert_eq!(message_id(&err), "field-required-after-change");
    assert_eq!(err.details["missing"], json!(["Customer"]));
    assert_eq!(
        value(&e, "TM-1", "Severity"),
        Value::Null,
        "nothing changed"
    );
    // Given in the same change, it passes; the default fills the other.
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "type": "Bug", "fields": { "Customer": "ACME" } }),
    );
    assert_eq!(value(&e, "TM-1", "Customer"), json!("ACME"));
    assert_eq!(value(&e, "TM-1", "Severity"), json!("Low"));
    // Bulk changes are held to it per task.
    task(&e, "TM", "Second");
    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-2"], "changes": { "type": "Bug" },
                "dry_run": false, "expected_count": 1 }),
    );
    assert_eq!(message_id(&err), "field-required-after-change");
    call(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-2"], "changes": { "type": "Bug", "fields": { "Customer": "Initech" } },
                "dry_run": false, "expected_count": 1 }),
    );
    assert_eq!(value(&e, "TM-2", "Customer"), json!("Initech"));
}

#[test]
fn a_move_checks_and_fills_the_target_s_required_fields() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    project(&e, "OPR");
    call(
        &e,
        "field.create",
        json!({ "project": "OPR", "name": "Customer", "kind": "text", "required": true }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "OPR", "name": "Team", "kind": "select", "required": true,
                "options": ["Red", "Blue"], "default": "Blue" }),
    );
    task(&e, "TM", "Moving");
    task(&e, "TM", "Also moving");
    call(
        &e,
        "task.create",
        json!({ "project": "TM", "title": "A part", "parent": "TM-1" }),
    );
    let check = call(
        &e,
        "task.move_check",
        json!({ "keys": ["TM-1", "TM-2"], "project": "OPR" }),
    );
    assert_eq!(
        check["required_fields"],
        json!([
            { "name": "Customer", "tasks": 3, "has_default": false },
            { "name": "Team", "tasks": 3, "has_default": true },
        ])
    );
    let err = fails(&e, "task.move", json!({ "key": "TM-1", "project": "OPR" }));
    assert_eq!(message_id(&err), "field-required-after-change");
    let err = fails(
        &e,
        "task.move",
        json!({ "key": "TM-1", "fields": { "Customer": "ACME" } }),
    );
    assert_eq!(message_id(&err), "task-move-fields-without-project");
    call(
        &e,
        "task.move",
        json!({ "key": "TM-1", "project": "OPR", "fields": { "Customer": "ACME" } }),
    );
    assert_eq!(value(&e, "OPR-1", "Customer"), json!("ACME"));
    assert_eq!(value(&e, "OPR-1", "Team"), json!("Blue"));
    assert_eq!(
        value(&e, "OPR-2", "Customer"),
        json!("ACME"),
        "the subtask too"
    );
    // A bulk move takes the values with it, also for a task already there.
    call(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-2", "OPR-1"],
                "changes": { "project": "OPR", "fields": { "Customer": "Initech" } },
                "dry_run": false, "expected_count": 2 }),
    );
    assert_eq!(value(&e, "OPR-3", "Customer"), json!("Initech"));
    assert_eq!(value(&e, "OPR-1", "Customer"), json!("Initech"));
    // A task that already has the value needs nothing.
    let check = call(
        &e,
        "task.move_check",
        json!({ "keys": ["OPR-1"], "project": "TM" }),
    );
    assert!(check.get("required_fields").is_none(), "{check}");
}

#[test]
fn bulk_changes_set_and_clear_custom_fields_as_one_change() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Platforms", "kind": "multi_select",
                "options": ["Linux", "Windows"] }),
    );
    for title in ["One", "Two", "Three"] {
        task(&e, "TM", title);
    }
    update(&e, "TM-3", json!({ "Points": 8 }));
    let preview = call(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM", "changes": { "fields": { "points": 5, "Platforms": ["Linux", "Windows"] } } }),
    );
    assert_eq!(preview["count"], 3);
    assert_eq!(preview["changed"], 3);
    assert_eq!(
        preview["items"][2]["changes"],
        json!([
            { "field": "cf.points", "before": "8", "after": "5" },
            { "field": "cf.platforms", "after": "Linux, Windows" },
        ])
    );
    assert_eq!(value(&e, "TM-1", "Points"), Value::Null, "a preview");
    let done = call(
        &e,
        "task.bulk_update",
        json!({ "query": "project:TM", "changes": { "fields": { "points": 5, "Platforms": ["Linux", "Windows"] } },
                "dry_run": false, "expected_count": 3 }),
    );
    assert!(done["group"].is_string());
    for key in ["TM-1", "TM-2", "TM-3"] {
        assert_eq!(value(&e, key, "Points"), json!(5));
        assert_eq!(value(&e, key, "Platforms"), json!(["Linux", "Windows"]));
    }
    call(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-1", "TM-2"], "changes": { "fields": { "Points": null } },
                "dry_run": false, "expected_count": 2 }),
    );
    assert_eq!(value(&e, "TM-1", "Points"), Value::Null);
    assert_eq!(value(&e, "TM-3", "Points"), json!(5));
    // One Undo takes back each bulk change as a whole.
    call(&e, "history.undo", json!({}));
    assert_eq!(value(&e, "TM-1", "Points"), json!(5));
    call(&e, "history.undo", json!({}));
    assert_eq!(value(&e, "TM-1", "Points"), Value::Null);
    assert_eq!(value(&e, "TM-3", "Points"), json!(8));
    assert_eq!(value(&e, "TM-2", "Platforms"), Value::Null);
    // A field the tasks' project lacks is an error, before anything changes.
    let err = fails(
        &e,
        "task.bulk_update",
        json!({ "keys": ["TM-1"], "changes": { "fields": { "Nothing": 1 } } }),
    );
    assert_eq!(err.code, gk_api::ErrorCode::NotFound, "{err:?}");
}

#[test]
fn text_fields_are_found_by_full_text_search() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Customer", "kind": "text" }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Notes", "kind": "markdown" }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    task(&e, "TM", "Plain title");
    task(&e, "TM", "Another");
    update(
        &e,
        "TM-1",
        json!({ "Customer": "Zanzibar Freight", "Notes": "Call **Quixotic** first", "Points": 77 }),
    );
    let hits = |text: &str| -> Vec<String> {
        call(&e, "search.query", json!({ "text": text }))["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|h| h["task"].as_str().map(str::to_owned))
            .collect()
    };
    assert_eq!(hits("zanzibar"), ["TM-1"]);
    assert_eq!(hits("quixotic"), ["TM-1"]);
    assert!(hits("77").is_empty(), "numbers aren't text");
    assert_eq!(search(&e, "freight"), ["TM-1"], "queries find them too");
    // The index follows changes, moves of values and deletion.
    update(&e, "TM-1", json!({ "Customer": "Acme" }));
    assert!(hits("zanzibar").is_empty());
    assert_eq!(hits("acme"), ["TM-1"]);
    call(
        &e,
        "task.update",
        json!({ "key": "TM-1", "title": "Renamed title" }),
    );
    assert_eq!(hits("acme"), ["TM-1"], "a title change keeps the values");
    e.call(
        &Context::user(Via::App).with_confirmation(Confirmation::Given),
        "field.delete",
        json!({ "project": "TM", "field": "Customer", "expected_task_count": 1 }),
    )
    .unwrap();
    assert!(hits("acme").is_empty());
    assert_eq!(hits("quixotic"), ["TM-1"]);
}

#[test]
fn board_cards_show_the_custom_fields_the_layout_chooses() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Story points", "kind": "integer" }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Customer", "kind": "text" }),
    );
    task(&e, "TM", "One");
    update(&e, "TM-1", json!({ "Story points": 5, "Customer": "ACME" }));
    let card = |e: &Engine| {
        let board = call(e, "board.get", json!({ "project": "TM" }));
        board["columns"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|c| c["cards"].as_array().cloned().unwrap_or_default())
            .next()
            .unwrap()
    };
    assert!(card(&e).get("fields").is_none(), "none chosen yet");
    let change = call(
        &e,
        "layout.update",
        json!({ "project": "TM", "card_fields": ["due", "cf.Story-Points", "cf.story-points"] }),
    );
    assert_eq!(
        change["layout"]["card_fields"],
        json!(["due", "cf.story-points"])
    );
    assert_eq!(card(&e)["fields"], json!({ "story-points": 5 }));
    let err = fails(
        &e,
        "layout.update",
        json!({ "project": "TM", "card_fields": ["cf.nothing"] }),
    );
    assert_eq!(message_id(&err), "board-card-custom-field");
    // A rename follows into the layout; a deletion takes the field out.
    call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Story points", "name": "Points" }),
    );
    let layout = call(&e, "board.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["card_fields"], json!(["due", "cf.points"]));
    assert_eq!(card(&e)["fields"], json!({ "points": 5 }));
    e.call(
        &Context::user(Via::App).with_confirmation(Confirmation::Given),
        "field.delete",
        json!({ "project": "TM", "field": "Points", "expected_task_count": 1 }),
    )
    .unwrap();
    let layout = call(&e, "board.get", json!({ "project": "TM" }))["layout"].clone();
    assert_eq!(layout["card_fields"], json!(["due"]));
}

#[test]
fn lists_are_grouped_by_select_and_yes_no_fields() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    project(&e, "TM");
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Area", "kind": "select",
                "options": ["Front end", "Back end", "Docs"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Platforms", "kind": "multi_select",
                "options": ["Windows", "Linux"] }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Approved", "kind": "boolean" }),
    );
    call(
        &e,
        "field.create",
        json!({ "project": "TM", "name": "Points", "kind": "integer" }),
    );
    for title in ["One", "Two", "Three", "Four"] {
        task(&e, "TM", title);
    }
    update(
        &e,
        "TM-1",
        json!({ "Area": "Back end", "Platforms": ["Linux", "Windows"], "Approved": true }),
    );
    update(
        &e,
        "TM-2",
        json!({ "Area": "Front end", "Platforms": ["Linux"] }),
    );
    update(&e, "TM-3", json!({ "Area": "Back end", "Approved": false }));
    let grouped = |field: &str| {
        let result = call(
            &e,
            "task.search",
            json!({ "query": "", "project": "TM", "group_by_field": field }),
        );
        let groups: Vec<(Value, u64)> = result["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| (g["value"].clone(), g["count"].as_u64().unwrap()))
            .collect();
        let keys: Vec<String> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["key"].as_str().unwrap().to_owned())
            .collect();
        (groups, keys)
    };
    let (groups, keys) = grouped("cf.area");
    assert_eq!(
        groups,
        [
            (json!("Front end"), 1),
            (json!("Back end"), 2),
            (Value::Null, 1)
        ],
        "in the field's order, none last"
    );
    assert_eq!(keys, ["TM-2", "TM-1", "TM-3", "TM-4"]);
    let (groups, _) = grouped("Platforms");
    assert_eq!(
        groups,
        [(json!("Windows"), 1), (json!("Linux"), 1), (Value::Null, 2)],
        "a multi-select by its first option"
    );
    let (groups, keys) = grouped("cf.approved");
    assert_eq!(groups, [(json!("yes"), 1), (json!("no"), 3)]);
    assert_eq!(keys[0], "TM-1");
    let err = fails(
        &e,
        "task.search",
        json!({ "query": "", "group_by_field": "cf.points" }),
    );
    assert_eq!(message_id(&err), "list-group-field-kind");
    let err = fails(
        &e,
        "task.search",
        json!({ "query": "", "group_by_field": "cf.nothing" }),
    );
    assert_eq!(message_id(&err), "list-group-field-unknown");
    let err = fails(
        &e,
        "task.search",
        json!({ "query": "", "group_by": "status", "group_by_field": "cf.area" }),
    );
    assert_eq!(message_id(&err), "list-group-field-and-grouping");
    // A saved view grouped by the field follows a rename.
    call(
        &e,
        "view.create",
        json!({ "name": "By area", "project": "TM", "query": "is:open", "group_by": "cf.area" }),
    );
    call(
        &e,
        "field.update",
        json!({ "project": "TM", "field": "Area", "name": "Component" }),
    );
    let views = call(&e, "view.list", json!({ "project": "TM" }));
    assert_eq!(views["items"][0]["group_by"], "cf.component");
    // Folded groups of a custom grouping are kept in the person's layout.
    call(
        &e,
        "list.layout.update",
        json!({ "project": "TM", "collapsed": { "cf.area": ["Back end"] } }),
    );
}

#[test]
fn templates_carry_custom_fields_and_their_tasks_meet_the_required_ones() {
    let dir = TempDir::new().unwrap();
    let e = open(&dir);
    // The built-in Software project brings a required severity for bugs,
    // with a default, and story points for stories and tasks.
    call(
        &e,
        "template.apply",
        json!({ "template": "Software project", "name": "App", "key": "ABC" }),
    );
    let fields = call(&e, "field.list", json!({ "project": "ABC" }))["fields"].clone();
    let names: Vec<&str> = fields
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["project"] == "ABC")
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Severity", "Story points"], "{fields}");
    let bug = call(
        &e,
        "template.apply",
        json!({ "template": "Bug report", "project": "ABC" }),
    );
    let key = bug["task"]["key"].as_str().unwrap().to_owned();
    assert_eq!(value(&e, &key, "Severity"), "Major");
    // A story that becomes a bug takes the default, so the change goes
    // through.
    task(&e, "ABC", "Login");
    call(&e, "task.update", json!({ "key": "ABC-2", "type": "Bug" }));
    assert_eq!(value(&e, "ABC-2", "Severity"), "Major");

    // A project template whose task lacks a required value without a
    // default couldn't be used: it is refused when it is saved.
    let setup = json!({
        "types": [{ "name": "Task", "glyph": "task", "hue": "blue", "is_default": true }],
        "fields": [{ "id": "f1", "name": "Customer", "kind": "text", "required": true }],
        "tasks": [{ "title": "Kick-off" }],
    });
    let err = fails(
        &e,
        "template.create",
        json!({ "name": "Client work", "kind": "project", "setup": setup }),
    );
    assert_eq!(message_id(&err), "field-required");
    assert_eq!(
        err.details["path"], "setup.tasks[0].fields.Customer",
        "{err:?}"
    );
    let mut setup = setup;
    setup["tasks"][0]["fields"] = json!({ "customer": "ACME" });
    setup["task_templates"] = json!([{ "name": "Visit", "task": {
        "title": "Visit", "subtasks": [{ "title": "Book", "fields": { "Customer": "ACME" } }]
    } }]);
    let err = fails(
        &e,
        "template.create",
        json!({ "name": "Client work", "kind": "project", "setup": setup }),
    );
    assert_eq!(
        err.details["path"], "setup.task_templates[0].task.fields.Customer",
        "{err:?}"
    );
    setup["task_templates"][0]["task"]["fields"] = json!({ "Customer": "ACME" });
    call(
        &e,
        "template.create",
        json!({ "name": "Client work", "kind": "project", "setup": setup }),
    );
    let made = call(
        &e,
        "template.apply",
        json!({ "template": "Client work", "name": "Client", "key": "OPR" }),
    );
    assert_eq!(made["tasks"], 1, "{made}");
    assert_eq!(value(&e, "OPR-1", "Customer"), "ACME");
}
