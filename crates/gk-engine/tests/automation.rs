// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Automation rules over a real workspace: triggers, conditions and
//! actions, the rule as the actor in history, Undo of a change with what
//! the rules did after it, loops, the depth and rate limits, status rules
//! and the rules for agents, due dates and the schedule, notifications and
//! the dry run.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ops::common::Via;
use gk_api::{ErrorCode, OpError, Profile};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// 2026-11-23T06:00:00Z, a Monday: 07:00 in Berlin.
const START: i64 = 1_795_413_600_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
    _dir: TempDir,
}

fn open() -> Ws {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let tick = Arc::clone(&clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    config.time_zone = Some("Europe/Berlin".into());
    let engine = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), config).unwrap();
    let ws = Ws {
        engine,
        clock,
        _dir: dir,
    };
    ws.call(
        "project.create",
        json!({ "name": "Automation", "key": "AU" }),
    );
    ws.call("label.create", json!({ "project": "AU", "name": "triage" }));
    ws.call("label.create", json!({ "project": "AU", "name": "urgent" }));
    ws
}

fn app() -> Context {
    Context::user(Via::App)
}

impl Ws {
    fn call_as(&self, ctx: &Context, op: &str, input: Value) -> Value {
        self.engine
            .call(ctx, op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn call(&self, op: &str, input: Value) -> Value {
        self.call_as(&app(), op, input)
    }

    fn fails(&self, op: &str, input: Value) -> OpError {
        self.engine.call(&app(), op, input).expect_err(op)
    }

    fn task(&self, key: &str) -> Value {
        self.call(
            "task.get",
            json!({ "keys": [key], "include": ["comments", "subtasks", "links"] }),
        )["items"][0]
            .clone()
    }

    fn new_task(&self, ctx: &Context, input: Value) -> String {
        self.call_as(ctx, "task.create", input)["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn rule(&self, input: Value) -> Value {
        self.call("automation.create", input)["rule"].clone()
    }

    fn runs(&self, rule: &str) -> Vec<Value> {
        self.call("automation.runs", json!({ "rule": rule, "project": "AU" }))["items"]
            .as_array()
            .unwrap()
            .clone()
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn labels(&self, key: &str) -> Vec<String> {
        self.task(key)["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["name"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn agent(ws: &Ws) -> Context {
    let session = ws
        .engine
        .start_session(&SessionStart {
            name: "Helper".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    Context::agent("Helper", session, Profile::Standard)
}

#[test]
fn a_new_task_gets_the_rules_changes_by_the_rule_in_the_same_group() {
    let ws = open();
    ws.rule(json!({
        "project": "AU",
        "name": "Triage bugs",
        "trigger": { "on": "task_created" },
        "condition": "title:crash",
        "actions": [
            { "kind": "set_fields", "fields": { "priority": "high", "due_date": "+3d" } },
            { "kind": "add_label", "label": "triage" },
            { "kind": "add_comment", "text": "{key} was triaged by {rule} after {actor}." },
        ],
    }));
    let quiet = ws.new_task(
        &app(),
        json!({ "project": "AU", "title": "Polish the icons" }),
    );
    assert_eq!(
        ws.task(&quiet)["priority"],
        "none",
        "the condition holds it back"
    );

    let created = ws.call(
        "task.create",
        json!({ "project": "AU", "title": "App crash on start" }),
    );
    let key = created["task"]["key"].as_str().unwrap().to_owned();
    let group = created["group"].as_str().unwrap().to_owned();
    let task = ws.task(&key);
    assert_eq!(task["priority"], "high");
    assert_eq!(task["due_date"], "2026-11-26");
    assert_eq!(ws.labels(&key), ["triage"]);
    let comment = &task["comments"][0];
    assert_eq!(
        comment["body"],
        format!("{key} was triaged by Triage bugs after You.")
    );
    assert_eq!(
        comment["author"],
        json!({ "kind": "automation", "name": "Triage bugs" })
    );

    let feed = ws.call("history.feed", json!({ "group": group }));
    let actors: Vec<&Value> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["actor"]["kind"])
        .collect();
    assert!(actors.contains(&&json!("user")), "{feed}");
    assert!(actors.contains(&&json!("automation")), "{feed}");

    let runs = ws.runs("Triage bugs");
    assert_eq!(runs.len(), 1, "one run: the task that matched");
    assert_eq!(runs[0]["result"], "done");
    assert_eq!(runs[0]["task"], key.as_str());
    assert_eq!(runs[0]["group"], group.as_str());

    // One Undo takes back the task and everything the rule did.
    ws.call("history.undo", json!({}));
    let gone = ws
        .engine
        .call(&app(), "task.get", json!({ "keys": [key] }))
        .map(|v| v["items"].as_array().map_or(0, Vec::len))
        .unwrap_or(0);
    assert_eq!(gone, 0, "the task is gone again");
}

#[test]
fn undo_of_a_change_takes_back_what_a_rule_did_to_the_same_field() {
    let ws = open();
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Ship it" }));
    ws.rule(json!({
        "project": "AU",
        "name": "Urgent means review",
        "trigger": { "on": "field_changed", "fields": ["priority"] },
        "condition": "priority:urgent",
        "actions": [
            { "kind": "move_to_status", "status": "Review" },
            { "kind": "set_fields", "fields": { "priority": "high" } },
        ],
    }));
    ws.call("task.update", json!({ "key": key, "priority": "urgent" }));
    let task = ws.task(&key);
    assert_eq!(task["status"]["name"], "Review");
    assert_eq!(task["priority"], "high", "the rule changed the field again");
    ws.call("history.undo", json!({}));
    let task = ws.task(&key);
    assert_eq!(task["status"]["name"], "Backlog");
    assert_eq!(task["priority"], "none");
}

#[test]
fn moving_to_a_status_and_comments_set_rules_off() {
    let ws = open();
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Review me" }));
    ws.rule(json!({
        "project": "AU",
        "name": "Done is complete",
        "trigger": { "on": "moved_to_status", "status": "done" },
        "actions": [{ "kind": "set_fields", "fields": { "progress": 100 } }],
    }));
    ws.rule(json!({
        "name": "Comments need triage",
        "trigger": { "on": "comment_added" },
        "actions": [{ "kind": "add_label", "label": "triage" }],
    }));
    ws.call("task.move", json!({ "key": key, "status": "In progress" }));
    assert!(ws.task(&key).get("progress").is_none());
    ws.call("task.move", json!({ "key": key, "status": "Done" }));
    assert_eq!(ws.task(&key)["progress"], 100);
    ws.call("comment.add", json!({ "key": key, "body": "Looks good" }));
    assert_eq!(
        ws.labels(&key),
        ["triage"],
        "a workspace rule acts in every project"
    );
}

#[test]
fn a_rule_never_sets_itself_off_and_chains_stop_at_the_depth_limit() {
    let ws = open();
    // Each rule comments, which would set every rule off again: each
    // chain ends once every rule is in it.
    for i in 0..3 {
        ws.rule(json!({
            "project": "AU",
            "name": format!("Echo {i}"),
            "trigger": { "on": "comment_added" },
            "actions": [{ "kind": "add_comment", "text": "echo {rule}" }],
        }));
    }
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Talk" }));
    ws.call("comment.add", json!({ "key": key, "body": "hello" }));
    let comments = ws.task(&key)["comments"].as_array().unwrap().len();
    assert_eq!(
        comments,
        1 + 3 + 3 * 2 + 3 * 2,
        "the person's, then each chain"
    );

    // A chain of rules, each setting off the next, stops after five.
    let ws = open();
    let steps = [
        ("priority", json!({ "progress": 10 })),
        ("progress", json!({ "estimate": "1h" })),
        ("estimate", json!({ "start_date": "today" })),
        ("start_date", json!({ "due_date": "+5d" })),
        ("due_date", json!({ "description": "Planned." })),
        ("description", json!({ "title": "Too far" })),
    ];
    for (i, (watch, set)) in steps.iter().enumerate() {
        ws.rule(json!({
            "project": "AU",
            "name": format!("Step {i}"),
            "trigger": { "on": "field_changed", "fields": [watch] },
            "actions": [{ "kind": "set_fields", "fields": set }],
        }));
    }
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Chain" }));
    ws.call("task.update", json!({ "key": key, "priority": "high" }));
    let task = ws.task(&key);
    assert_eq!(task["description"], "Planned.", "five steps ran");
    assert_eq!(task["title"], "Chain", "the sixth didn't");
    let last = &ws.runs("Step 5")[0];
    assert_eq!(last["result"], "limited");
    assert_eq!(last["depth"], 5);
    assert_eq!(
        last["error"]["details"]["message_id"],
        "automation-depth-limit"
    );

    // A rule whose action changes what it watches runs once.
    let ws = open();
    ws.rule(json!({
        "project": "AU",
        "name": "Bump",
        "trigger": { "on": "field_changed", "fields": ["progress"] },
        "actions": [{ "kind": "set_fields", "fields": { "progress": 50 } }],
    }));
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Count" }));
    ws.call("task.update", json!({ "key": key, "progress": 10 }));
    assert_eq!(ws.task(&key)["progress"], 50);
    assert_eq!(ws.runs("Bump").len(), 1);
}

#[test]
fn a_rule_runs_at_most_a_hundred_times_an_hour() {
    let ws = open();
    ws.rule(json!({
        "project": "AU",
        "name": "Label all",
        "trigger": { "on": "task_created" },
        "actions": [{ "kind": "add_label", "label": "triage" }],
    }));
    let mut keys = Vec::new();
    for i in 0..102 {
        keys.push(ws.new_task(&app(), json!({ "project": "AU", "title": format!("T{i}") })));
    }
    assert_eq!(ws.labels(&keys[99]), ["triage"]);
    assert!(
        ws.labels(&keys[100]).is_empty(),
        "the 101st run is held back"
    );
    let runs = ws.runs("Label all");
    assert_eq!(runs[0]["result"], "limited", "{:?}", runs[0]);
    assert_eq!(runs[0]["task"], keys[100].as_str());
    assert!(
        runs.iter().filter(|r| r["result"] == "limited").count() == 1,
        "the limit is logged once an hour"
    );
    ws.advance(HOUR + MINUTE);
    let later = ws.new_task(&app(), json!({ "project": "AU", "title": "Later" }));
    assert_eq!(ws.labels(&later), ["triage"]);
}

#[test]
fn a_failed_run_keeps_nothing_and_the_change_still_happens() {
    let ws = open();
    ws.call(
        "rules.update",
        json!({ "project": "AU", "statuses": [
            { "status": "Done", "rules": { "required": ["estimate"] } },
        ] }),
    );
    ws.rule(json!({
        "project": "AU",
        "name": "Close at once",
        "trigger": { "on": "task_created" },
        "actions": [
            { "kind": "add_label", "label": "triage" },
            { "kind": "move_to_status", "status": "Done" },
        ],
    }));
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "No estimate" }));
    let task = ws.task(&key);
    assert_eq!(task["status"]["name"], "Backlog");
    assert!(
        ws.labels(&key).is_empty(),
        "the label went with the failed run"
    );
    let run = &ws.runs("Close at once")[0];
    assert_eq!(run["result"], "failed");
    assert_eq!(run["error"]["details"]["message_id"], "rule-required");
    assert!(
        run["error"]["details"].get("action").is_none(),
        "the status rule refused the run as a whole, not one action"
    );

    let with = ws.new_task(
        &app(),
        json!({ "project": "AU", "title": "Estimated", "estimate": "2h" }),
    );
    assert_eq!(ws.task(&with)["status"]["name"], "Done");
}

#[test]
fn rules_after_an_agent_or_made_by_one_are_held_to_the_rules_for_agents() {
    let ws = open();
    ws.call(
        "rules.update",
        json!({ "project": "AU", "agents": { "read_only": ["priority"] } }),
    );
    ws.rule(json!({
        "project": "AU",
        "name": "High by default",
        "trigger": { "on": "task_created" },
        "actions": [{ "kind": "set_fields", "fields": { "priority": "high" } }],
    }));
    let helper = agent(&ws);
    let by_agent = ws.new_task(&helper, json!({ "project": "AU", "title": "Agent's" }));
    assert_eq!(ws.task(&by_agent)["priority"], "none");
    assert_eq!(ws.runs("High by default")[0]["result"], "failed");
    let by_person = ws.new_task(&app(), json!({ "project": "AU", "title": "Mine" }));
    assert_eq!(ws.task(&by_person)["priority"], "high");

    // A rule an agent made is held to them whoever sets it off.
    let made = ws.call_as(
        &helper,
        "automation.create",
        json!({
            "project": "AU",
            "name": "Agent's rule",
            "trigger": { "on": "comment_added" },
            "actions": [{ "kind": "set_fields", "fields": { "priority": "urgent" } }],
        }),
    );
    assert_eq!(made["rule"]["held_as_agent"], true);
    ws.call("comment.add", json!({ "key": by_person, "body": "hi" }));
    assert_eq!(ws.task(&by_person)["priority"], "high");
    assert_eq!(ws.runs("Agent's rule")[0]["result"], "failed");
    // Once the person changes it, it is theirs.
    let changed = ws.call(
        "automation.update",
        json!({ "rule": "Agent's rule", "project": "AU", "name": "Reviewed rule" }),
    );
    assert!(changed["rule"].get("held_as_agent").is_none());
    ws.call("comment.add", json!({ "key": by_person, "body": "again" }));
    assert_eq!(ws.task(&by_person)["priority"], "urgent");
}

#[test]
fn due_dates_set_rules_off_once_per_due_date() {
    let ws = open();
    ws.rule(json!({
        "project": "AU",
        "name": "Due today",
        "trigger": { "on": "due_date_reached" },
        "condition": "is:open",
        "actions": [{ "kind": "add_label", "label": "urgent" }],
    }));
    ws.rule(json!({
        "project": "AU",
        "name": "Late",
        "trigger": { "on": "overdue" },
        "actions": [{ "kind": "add_comment", "text": "{key} is late (due {due_date})." }],
    }));
    let key = ws.new_task(
        &app(),
        json!({ "project": "AU", "title": "Pay", "due_date": "2026-11-24" }),
    );
    assert_eq!(ws.engine.run_automation().unwrap(), 0, "not due yet");
    ws.advance(DAY);
    assert_eq!(ws.engine.run_automation().unwrap(), 1);
    assert_eq!(ws.labels(&key), ["urgent"]);
    assert_eq!(ws.engine.run_automation().unwrap(), 0, "once per due date");
    ws.advance(DAY);
    assert_eq!(ws.engine.run_automation().unwrap(), 1, "now it is late");
    let comments = ws.task(&key)["comments"].clone();
    assert_eq!(
        comments[0]["body"],
        format!("{key} is late (due 2026-11-24).")
    );
    // A new due date acts again.
    ws.call(
        "task.update",
        json!({ "key": key, "due_date": "2026-11-25" }),
    );
    ws.call(
        "task.update",
        json!({ "key": key, "remove_labels": ["urgent"] }),
    );
    assert_eq!(
        ws.engine.run_automation().unwrap(),
        1,
        "due today again, not late"
    );
    assert_eq!(ws.labels(&key), ["urgent"]);
}

#[test]
fn a_scheduled_rule_runs_once_at_its_time_for_the_matching_tasks() {
    let ws = open();
    let open_one = ws.new_task(&app(), json!({ "project": "AU", "title": "Open" }));
    let closed = ws.new_task(&app(), json!({ "project": "AU", "title": "Closed" }));
    ws.call("task.move", json!({ "key": closed, "status": "Done" }));
    let err = ws.fails(
        "automation.create",
        json!({
            "project": "AU",
            "name": "Nightly",
            "trigger": { "on": "schedule", "every": "daily", "time": "08:00" },
            "actions": [{ "kind": "notify", "text": "{title} is still open" }],
        }),
    );
    assert_eq!(
        err.details["message_id"],
        "automation-schedule-needs-condition"
    );
    ws.rule(json!({
        "project": "AU",
        "name": "Nightly",
        "trigger": { "on": "schedule", "every": "daily", "time": "08:00" },
        "condition": "is:open",
        "actions": [{ "kind": "notify", "text": "{title} is still open" }],
    }));
    // 07:00 in Berlin: not yet.
    assert_eq!(ws.engine.run_automation().unwrap(), 0);
    ws.advance(HOUR + MINUTE);
    assert_eq!(ws.engine.run_automation().unwrap(), 1);
    assert_eq!(ws.engine.run_automation().unwrap(), 0, "once a day");
    let due = ws.call("reminder.due", json!({}));
    let notice = due["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "automation")
        .unwrap()
        .clone();
    assert_eq!(notice["note"], "Open is still open");
    assert_eq!(notice["task"]["key"], open_one.as_str());
    ws.call("reminder.acknowledge", json!({ "ids": [notice["id"]] }));
    let due = ws.call("reminder.due", json!({}));
    assert!(
        due["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["kind"] != "automation")
    );
    ws.advance(DAY);
    assert_eq!(ws.engine.run_automation().unwrap(), 1, "the next day");
}

#[test]
fn a_dry_run_shows_the_changes_and_makes_none() {
    let ws = open();
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Try" }));
    let result = ws.call(
        "automation.test",
        json!({
            "task": key,
            "draft": {
                "trigger": { "on": "task_created" },
                "actions": [
                    { "kind": "set_fields", "fields": { "priority": "urgent" } },
                    { "kind": "create_subtask", "text": "Check {title}" },
                    { "kind": "create_linked_task", "text": "Follow up", "link": "blocks" },
                ],
            },
        }),
    );
    assert_eq!(result["matches"], true);
    assert_eq!(result["ok"], true, "{result}");
    let changes = result["changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["field"] == "priority" && c["after"] == "urgent"),
        "{result}"
    );
    assert!(
        changes.iter().any(|c| c["entity"] == "relation"),
        "{result}"
    );
    let task = ws.task(&key);
    assert_eq!(task["priority"], "none", "nothing changed");
    assert!(task["subtasks"].as_array().is_none_or(Vec::is_empty));

    let not = ws.call(
        "automation.test",
        json!({
            "task": key,
            "draft": {
                "trigger": { "on": "task_created" },
                "condition": "priority:urgent",
                "actions": [{ "kind": "add_label", "label": "triage" }],
            },
        }),
    );
    assert_eq!(not["matches"], false);

    let failing = ws.call(
        "automation.test",
        json!({
            "task": key,
            "draft": {
                "trigger": { "on": "task_created" },
                "actions": [
                    { "kind": "add_label", "label": "triage" },
                    { "kind": "set_fields", "fields": { "Missing field": 3 } },
                ],
            },
        }),
    );
    assert_eq!(failing["ok"], false);
    assert_eq!(failing["failed_action"], 1);
}

#[test]
fn subtasks_and_linked_tasks_come_from_templates() {
    let ws = open();
    let source = ws.new_task(
        &app(),
        json!({ "project": "AU", "title": "Release checklist", "priority": "high" }),
    );
    ws.call(
        "template.create",
        json!({ "name": "Release", "project": "AU", "from_task": source }),
    );
    ws.rule(json!({
        "project": "AU",
        "name": "Plan releases",
        "trigger": { "on": "field_changed", "fields": ["labels"] },
        "condition": "label:urgent",
        "actions": [
            { "kind": "create_subtask", "template": "Release" },
            { "kind": "create_linked_task", "text": "Announce {title}", "link": "blocked_by" },
        ],
    }));
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Version 2" }));
    ws.call(
        "task.update",
        json!({ "key": key, "add_labels": ["urgent"] }),
    );
    let task = ws.task(&key);
    let subtasks = task["subtasks"].as_array().unwrap();
    assert_eq!(subtasks.len(), 1, "{task}");
    assert_eq!(subtasks[0]["title"], "Release checklist");
    let made = ws.task(subtasks[0]["key"].as_str().unwrap());
    assert_eq!(made["priority"], "high", "from the template");
    assert_eq!(
        made["created_by"],
        json!({ "kind": "automation", "name": "Plan releases" })
    );
    let relations = task["links"].to_string();
    assert!(relations.contains("blocked_by"), "{relations}");
}

#[test]
fn rules_are_checked_named_uniquely_switched_off_and_undone() {
    let ws = open();
    let bad = [
        (
            json!({ "on": "task_created", "status": "Done" }),
            json!([{ "kind": "add_label", "label": "triage" }]),
            "automation-field-not-used",
        ),
        (
            json!({ "on": "moved_to_status", "status": "Nowhere" }),
            json!([{ "kind": "add_label", "label": "triage" }]),
            "lookup-status-not-found",
        ),
        (
            json!({ "on": "task_created" }),
            json!([{ "kind": "add_label", "label": "nope" }]),
            "lookup-label-not-found",
        ),
        (
            json!({ "on": "task_created" }),
            json!([]),
            "automation-action-count",
        ),
        (
            json!({ "on": "field_changed", "fields": ["rank"] }),
            json!([{ "kind": "add_label", "label": "triage" }]),
            "automation-field-unknown",
        ),
        (
            json!({ "on": "schedule", "every": "weekly", "time": "25:00" }),
            json!([{ "kind": "add_label", "label": "triage" }]),
            "automation-time",
        ),
        (
            json!({ "on": "task_created" }),
            json!([{ "kind": "set_fields", "fields": { "priority": "loud" } }]),
            "automation-set-value",
        ),
    ];
    for (trigger, actions, id) in bad {
        let err = ws.fails(
            "automation.create",
            json!({ "project": "AU", "name": "Bad", "trigger": trigger, "condition": "is:open",
                    "actions": actions }),
        );
        assert_eq!(err.details["message_id"], id, "{err:?}");
    }
    let err = ws.fails(
        "automation.create",
        json!({ "project": "AU", "name": "Bad", "trigger": { "on": "task_created" },
                "condition": "priority:", "actions": [{ "kind": "add_label", "label": "triage" }] }),
    );
    assert_eq!(err.code, ErrorCode::Invalid);

    ws.rule(json!({
        "project": "AU",
        "name": "Label",
        "trigger": { "on": "task_created" },
        "actions": [{ "kind": "add_label", "label": "triage" }],
    }));
    let twice = ws.fails(
        "automation.create",
        json!({ "project": "AU", "name": "label", "trigger": { "on": "task_created" },
                "actions": [{ "kind": "add_label", "label": "triage" }] }),
    );
    assert_eq!(twice.code, ErrorCode::Conflict);

    ws.call(
        "automation.update",
        json!({ "rule": "Label", "project": "AU", "enabled": false }),
    );
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "Off" }));
    assert!(
        ws.labels(&key).is_empty(),
        "a rule switched off doesn't run"
    );
    ws.call("history.undo", json!({})); // the task
    ws.call("history.undo", json!({})); // switching it off
    let key = ws.new_task(&app(), json!({ "project": "AU", "title": "On" }));
    assert_eq!(ws.labels(&key), ["triage"]);

    let deleted = ws.call(
        "automation.delete",
        json!({ "rule": "Label", "project": "AU" }),
    );
    assert_eq!(deleted["deleted"], true);
    assert_eq!(
        ws.call("automation.list", json!({ "project": "AU" }))["total"],
        0
    );
    ws.call("history.undo", json!({}));
    let list = ws.call("automation.list", json!({ "project": "AU" }));
    assert_eq!(list["total"], 1);
    let rule = &list["items"][0];
    assert_eq!(rule["created_by"]["kind"], "user");
    assert_eq!(rule["last_run"]["result"], "done");
}

#[test]
fn renaming_a_label_rewrites_the_conditions_that_name_it() {
    let ws = open();
    ws.rule(json!({
        "project": "AU",
        "name": "Urgent ones",
        "trigger": { "on": "comment_added" },
        "condition": "label:urgent",
        "actions": [{ "kind": "set_fields", "fields": { "priority": "urgent" } }],
    }));
    ws.call(
        "project.update",
        json!({ "project": "AU", "labels": [{ "label": "urgent", "name": "hot" }] }),
    );
    let rule = ws.call(
        "automation.get",
        json!({ "rule": "Urgent ones", "project": "AU" }),
    );
    assert_eq!(rule["condition"], "label:hot");
}
