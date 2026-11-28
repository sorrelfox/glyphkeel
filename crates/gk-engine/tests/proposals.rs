// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Changes agents propose for the person's approval: kept with a preview
//! instead of made, approved once as the agent with history saying so,
//! rejected with a reason the agent reads, stale ones showing what changed
//! since, expiry, Undo of an approved change, and how the setting fits the
//! profiles.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::Profile;
use gk_api::ops::common::{ActorKind, Via};
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;
const DAY: i64 = 24 * 60 * 60 * 1000;

struct Ws {
    _dir: TempDir,
    engine: Engine,
    clock: Arc<AtomicI64>,
}

fn open() -> Ws {
    let dir = TempDir::new().unwrap();
    let mut config = EngineConfig::new(Role::App);
    let clock = Arc::new(AtomicI64::new(NOW));
    let ticks = Arc::clone(&clock);
    config.clock = Arc::new(move || ticks.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    let engine = Engine::open(&WorkspaceLayout::new(dir.path().join("W")), config).unwrap();
    let ws = Ws {
        _dir: dir,
        engine,
        clock,
    };
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    ws.call("task.create", json!({ "project": "TM", "title": "First" }));
    ws.call("task.create", json!({ "project": "TM", "title": "Second" }));
    ws
}

fn person() -> Context {
    Context::user(Via::App)
}

impl Ws {
    fn call(&self, op: &str, input: Value) -> Value {
        self.engine
            .call(&person(), op, input)
            .unwrap_or_else(|e| panic!("{op}: {e:?}"))
    }

    fn agent(&self, profile: Profile) -> Context {
        let session = self
            .engine
            .start_session(&SessionStart {
                name: "Helper".into(),
                client_id: Some("helper".into()),
                client_version: None,
                profile,
            })
            .unwrap();
        Context::agent("Helper", session, profile)
    }

    fn approvals(&self, read_only: &str, standard: &str, full: &str, days: u32) {
        self.call(
            "agent.policy.update",
            json!({
                "max_profile": "full",
                "approvals": {
                    "read_only": read_only, "standard": standard, "full": full,
                    "expire_days": days
                }
            }),
        );
    }

    fn title(&self, key: &str) -> String {
        self.call("task.get", json!({ "keys": [key] }))["items"][0]["title"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Proposes `op` as `agent` and returns the proposal's ID.
    fn propose(&self, agent: &Context, op: &str, input: Value) -> String {
        let e = self.engine.call(agent, op, input).unwrap_err();
        assert_eq!(e.code, ErrorCode::PendingApproval, "{e:?}");
        e.details["proposal"].as_str().unwrap().to_owned()
    }

    fn proposal(&self, ctx: &Context, id: &str) -> Value {
        self.engine
            .call(ctx, "proposal.get", json!({ "id": id }))
            .unwrap()
    }
}

#[test]
fn an_approved_proposal_runs_once_as_the_agent_and_history_says_the_person_approved() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);

    let id = ws.propose(
        &agent,
        "task.update",
        json!({ "key": "TM-1", "title": "Renamed" }),
    );
    assert_eq!(ws.title("TM-1"), "First", "nothing changes before approval");

    // The person sees it with its preview; the agent sees its own.
    let listed = ws.call("proposal.list", json!({}));
    assert_eq!(listed["pending"], 1);
    let item = &listed["items"][0];
    assert_eq!(item["status"], "pending");
    assert_eq!(item["op"], "task.update");
    assert_eq!(item["agent"]["name"], "Helper");
    assert_eq!(item["task"]["key"], "TM-1");
    let change = &item["changes"][0];
    assert_eq!(change["entity"], "task");
    assert_eq!(change["fields"]["title"]["before"], "First");
    assert_eq!(change["fields"]["title"]["after"], "Renamed");
    let own = ws
        .engine
        .call(&agent, "proposal.list", json!({ "session": "current" }))
        .unwrap();
    assert_eq!(own["items"][0]["id"], id.as_str());

    let result = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(result["approved"], 1, "{result}");
    assert_eq!(ws.title("TM-1"), "Renamed");
    let after = ws.proposal(&agent, &id);
    assert_eq!(after["status"], "approved");
    assert_eq!(after["decided_by"]["kind"], "user");
    let group = after["group"].as_str().unwrap().to_owned();

    // The change is the agent's, in its session, approved by the person.
    let feed = ws.call("history.feed", json!({ "group": group }));
    let rows: Vec<&Value> = feed["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["entity"] == "task")
        .collect();
    assert_eq!(rows.len(), 1, "{feed}");
    assert_eq!(rows[0]["actor"]["kind"], "agent");
    assert_eq!(rows[0]["actor"]["name"], "Helper");
    assert_eq!(rows[0]["via"], "mcp");
    assert_eq!(rows[0]["session"], agent.session.unwrap().to_string());
    assert_eq!(rows[0]["approved_by_person"], true);

    // Approving again runs nothing.
    let again = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(again["approved"], 0);
    assert_eq!(again["items"][0]["error"]["code"], "conflict");
    let titles = ws.call("history.feed", json!({ "task": "TM-1", "limit": 50 }));
    let renames = titles["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["fields"]["title"]["after"] == "Renamed")
        .count();
    assert_eq!(renames, 1, "applied exactly once");
}

#[test]
fn a_rejection_keeps_its_reason_for_the_agent() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);
    let id = ws.propose(
        &agent,
        "comment.add",
        json!({ "key": "TM-2", "body": "Looks done to me." }),
    );
    let preview = ws.proposal(&person(), &id);
    assert_eq!(preview["task"]["key"], "TM-2", "{preview}");

    let rejected = ws.call(
        "proposal.reject",
        json!({ "ids": [id], "reason": "Not yet: QA is still testing." }),
    );
    assert_eq!(rejected["items"][0]["status"], "rejected");

    let seen = ws.proposal(&agent, &id);
    assert_eq!(seen["status"], "rejected");
    assert_eq!(seen["reason"], "Not yet: QA is still testing.");
    let comments = ws.call("comment.list", json!({ "key": "TM-2" }));
    assert_eq!(comments["items"].as_array().unwrap().len(), 0);

    let approve = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(approve["approved"], 0);
    let reject = ws
        .engine
        .call(&person(), "proposal.reject", json!({ "ids": [id] }))
        .unwrap_err();
    assert_eq!(reject.code, ErrorCode::Conflict);

    // Agents can't decide.
    for op in ["proposal.approve", "proposal.reject"] {
        let e = ws
            .engine
            .call(&agent, op, json!({ "ids": [id] }))
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::Forbidden, "{op}");
    }
}

#[test]
fn a_stale_proposal_shows_the_conflict_and_runs_only_when_forced() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);
    let id = ws.propose(
        &agent,
        "task.update",
        json!({ "key": "TM-1", "title": "Agent's title" }),
    );
    ws.call(
        "task.update",
        json!({ "key": "TM-1", "title": "Person's title" }),
    );

    let shown = ws.proposal(&person(), &id);
    let conflict = &shown["conflicts"][0];
    assert_eq!(conflict["field"], "title", "{shown}");
    assert_eq!(conflict["then"], "First");
    assert_eq!(conflict["now"], "Person's title");

    let refused = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(refused["approved"], 0);
    assert_eq!(refused["items"][0]["error"]["code"], "conflict");
    assert_eq!(ws.title("TM-1"), "Person's title");

    let forced = ws.call("proposal.approve", json!({ "ids": [id], "force": true }));
    assert_eq!(forced["approved"], 1);
    assert_eq!(ws.title("TM-1"), "Agent's title");
}

#[test]
fn proposals_expire_after_the_configured_days_and_stay_in_the_record() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);
    let id = ws.propose(
        &agent,
        "task.update",
        json!({ "key": "TM-1", "title": "Late" }),
    );
    ws.clock.fetch_add(15 * DAY, Ordering::SeqCst);
    assert_eq!(ws.proposal(&person(), &id)["status"], "expired");
    let listed = ws.call("proposal.list", json!({ "status": "expired" }));
    assert_eq!(listed["total"], 1);
    assert_eq!(ws.call("proposal.list", json!({}))["pending"], 0);
    let e = ws
        .engine
        .call(&person(), "proposal.approve", json!({ "ids": [id] }))
        .unwrap();
    assert_eq!(e["items"][0]["error"]["code"], "conflict");
    assert_eq!(ws.title("TM-1"), "First");

    // A longer wait set later applies to waiting proposals too.
    ws.approvals("off", "all_writes", "off", 30);
    assert_eq!(ws.proposal(&person(), &id)["status"], "pending");
    let bad = ws
        .engine
        .call(
            &person(),
            "agent.policy.update",
            json!({ "max_profile": "full", "approvals": {
                "read_only": "off", "standard": "off", "full": "off", "expire_days": 0 } }),
        )
        .unwrap_err();
    assert_eq!(bad.code, ErrorCode::Invalid);
}

#[test]
fn undo_takes_back_an_approved_change_and_the_proposal_stays_approved() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);
    let first = ws.propose(
        &agent,
        "task.update",
        json!({ "key": "TM-1", "title": "One" }),
    );
    let second = ws.propose(
        &agent,
        "task.update",
        json!({ "key": "TM-2", "title": "Two" }),
    );
    // Approve all in the session, oldest first.
    let session = agent.session.unwrap().to_string();
    let all = ws.call("proposal.approve", json!({ "session": session }));
    assert_eq!(all["approved"], 2, "{all}");
    assert_eq!(all["items"][0]["proposal"]["id"], first.as_str());
    assert_eq!(
        (ws.title("TM-1"), ws.title("TM-2")),
        ("One".into(), "Two".into())
    );

    ws.call("history.undo", json!({}));
    assert_eq!(ws.title("TM-2"), "Second");
    assert_eq!(ws.title("TM-1"), "One");
    assert_eq!(ws.proposal(&person(), &second)["status"], "approved");

    // Reverting the agent's session takes back its changes only.
    let reverted = ws.call(
        "history.revert",
        json!({ "session": agent.session.unwrap().to_string() }),
    );
    assert!(
        reverted["skipped"].as_array().unwrap().is_empty(),
        "{reverted}"
    );
    assert_eq!(ws.title("TM-1"), "First");
    assert_eq!(ws.proposal(&person(), &first)["status"], "approved");
}

#[test]
fn the_setting_fits_each_profile() {
    let ws = open();
    // Destructive only: changes run, moving to the trash is proposed.
    ws.approvals("off", "destructive", "off", 14);
    let standard = ws.agent(Profile::Standard);
    ws.engine
        .call(
            &standard,
            "task.update",
            json!({ "key": "TM-1", "title": "Direct" }),
        )
        .unwrap();
    assert_eq!(ws.title("TM-1"), "Direct");
    let id = ws.propose(&standard, "task.delete", json!({ "keys": ["TM-2"] }));
    assert_eq!(ws.title("TM-2"), "Second");
    let preview = ws.proposal(&person(), &id);
    assert!(preview["changes"].as_array().is_some_and(|c| !c.is_empty()));

    // Full agents follow their own mode; off means their changes run.
    let full = ws.agent(Profile::Full);
    ws.engine
        .call(&full, "task.delete", json!({ "keys": ["TM-2"] }))
        .unwrap();

    // A read-only agent is refused unless it may propose; then it proposes
    // what a standard agent may do, but nothing permanent.
    let reader = ws.agent(Profile::ReadOnly);
    let e = ws
        .engine
        .call(
            &reader,
            "task.update",
            json!({ "key": "TM-1", "title": "X" }),
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    ws.approvals("all_writes", "off", "off", 14);
    let id = ws.propose(
        &reader,
        "task.update",
        json!({ "key": "TM-1", "title": "From the reader" }),
    );
    let e = ws
        .engine
        .call(&reader, "trash.purge", json!({ "all": true }))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    let done = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(done["approved"], 1, "{done}");
    assert_eq!(ws.title("TM-1"), "From the reader");

    // Read-only agents propose all changes or none.
    let bad = ws
        .engine
        .call(
            &person(),
            "agent.policy.update",
            json!({ "max_profile": "full", "approvals": {
                "read_only": "destructive", "standard": "off", "full": "off", "expire_days": 14 } }),
        )
        .unwrap_err();
    assert_eq!(bad.code, ErrorCode::Invalid);

    // A change the person allowed when asked runs at once.
    ws.approvals("off", "all_writes", "off", 14);
    ws.engine
        .call(
            &standard.clone().confirmed_by_person(),
            "task.update",
            json!({ "key": "TM-1", "title": "Allowed" }),
        )
        .unwrap();
    assert_eq!(ws.title("TM-1"), "Allowed");

    // The agent learns its mode from the workspace overview.
    let info = ws
        .engine
        .call(&standard, "workspace.get", json!({}))
        .unwrap();
    assert_eq!(info["caller"]["approvals"], "all_writes");
    assert!(ws.call("workspace.get", json!({}))["caller"]["approvals"].is_null());
}

#[test]
fn input_the_change_refuses_is_refused_when_proposed() {
    let ws = open();
    ws.approvals("off", "all_writes", "off", 14);
    let agent = ws.agent(Profile::Standard);
    let e = ws
        .engine
        .call(
            &agent,
            "task.update",
            json!({ "key": "TM-99", "title": "X" }),
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(ws.call("proposal.list", json!({}))["total"], 0);

    // A repeated request finds the first proposal.
    let first = ws.propose(
        &agent,
        "comment.add",
        json!({ "key": "TM-1", "body": "Once", "client_request_id": "r-1" }),
    );
    let again = ws.propose(
        &agent,
        "comment.add",
        json!({ "key": "TM-1", "body": "Once", "client_request_id": "r-1" }),
    );
    assert_eq!(first, again);
    assert_eq!(ws.call("proposal.list", json!({}))["total"], 1);
    let page = ws
        .engine
        .run::<gk_api::ops::history::HistoryFeed>(&person(), &Default::default())
        .unwrap();
    assert!(
        page.items
            .iter()
            .any(|e| e.entity == "proposal" && e.actor.kind == ActorKind::Agent)
    );
}

/// Two programs with the workspace open approve the same proposal of an
/// operation that runs outside one transaction at the same moment: one
/// runs it, the other is told it is being (or was) decided.
#[test]
fn two_programs_approving_at_once_run_it_once() {
    let dir = TempDir::new().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("W"));
    let program = || {
        let mut config = EngineConfig::new(Role::Cli);
        config.time_zone = Some("UTC".to_owned());
        Engine::open(&layout, config).unwrap()
    };
    let first = program();
    let person = Context::user(Via::Cli);
    let call = |e: &Engine, op: &str, input: Value| e.call(&person, op, input).unwrap();
    call(
        &first,
        "project.create",
        json!({ "name": "Team", "key": "TM" }),
    );
    call(
        &first,
        "task.create",
        json!({ "project": "TM", "title": "First" }),
    );
    let session = first
        .start_session(&SessionStart {
            name: "Helper".into(),
            client_id: None,
            client_version: None,
            profile: Profile::Standard,
        })
        .unwrap();
    let agent = Context::agent("Helper", session, Profile::Standard);
    let changed = first
        .call(
            &agent,
            "task.update",
            json!({ "key": "TM-1", "title": "Renamed" }),
        )
        .unwrap();
    let group = changed["group"].as_str().unwrap().to_owned();
    call(
        &first,
        "agent.policy.update",
        json!({ "max_profile": "standard", "approvals": {
            "read_only": "off", "standard": "all_writes", "full": "off", "expire_days": 14 } }),
    );
    let e = first
        .call(&agent, "history.revert", json!({ "groups": [group] }))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::PendingApproval, "{e:?}");
    let id = e.details["proposal"].as_str().unwrap().to_owned();

    let second = program();
    let start = std::sync::Barrier::new(2);
    let outcomes: Vec<Value> = std::thread::scope(|s| {
        let runs: Vec<_> = [&first, &second]
            .into_iter()
            .map(|engine| {
                let (start, id, person) = (&start, &id, &person);
                s.spawn(move || {
                    start.wait();
                    engine
                        .call(person, "proposal.approve", json!({ "ids": [id] }))
                        .unwrap()
                })
            })
            .collect();
        runs.into_iter().map(|r| r.join().unwrap()).collect()
    });
    let approved: Vec<u64> = outcomes
        .iter()
        .map(|o| o["approved"].as_u64().unwrap())
        .collect();
    assert_eq!(approved.iter().sum::<u64>(), 1, "{outcomes:?}");
    let refused = outcomes.iter().find(|o| o["approved"] == 0).unwrap();
    assert_eq!(
        refused["items"][0]["error"]["code"], "conflict",
        "{refused}"
    );

    let reverts = call(
        &first,
        "history.feed",
        json!({ "task": "TM-1", "limit": 50 }),
    )["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["op"] == "history.revert")
        .count();
    assert_eq!(reverts, 1, "reverted exactly once");
    let shown = call(&first, "proposal.get", json!({ "id": id }));
    assert_eq!(shown["status"], "approved");
    assert!(shown.get("claimed_at").is_none());
    second.close();
    first.close();
}

/// A program that stopped while it ran an approved proposal leaves it
/// being approved: shown as interrupted, never run again by itself, and
/// the person can set it aside.
#[test]
fn an_interrupted_approval_is_shown_and_never_runs_again() {
    let ws = open();
    ws.approvals("off", "off", "all_writes", 14);
    let agent = ws.agent(Profile::Full);
    let id = ws.propose(&agent, "trash.purge", json!({ "all": true }));
    // A failed run leaves it waiting with the error: the person lowered
    // the cap meanwhile, so the agent may no longer purge.
    ws.call(
        "agent.policy.update",
        json!({ "max_profile": "standard", "approvals": {
            "read_only": "off", "standard": "off", "full": "all_writes", "expire_days": 14 } }),
    );
    let failed = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(failed["approved"], 0, "{failed}");
    let back = ws.proposal(&person(), &id);
    assert_eq!(back["status"], "pending", "{back}");
    assert!(back["last_error"].is_string(), "{back}");

    // The program stopped mid-run, an hour ago.
    let db = gk_store::rusqlite::Connection::open(
        WorkspaceLayout::new(ws._dir.path().join("W")).database(),
    )
    .unwrap();
    db.execute(
        "UPDATE proposals SET status = 'approving', claimed_at = ?1",
        [NOW - 60 * 60 * 1000],
    )
    .unwrap();
    let shown = ws.proposal(&person(), &id);
    assert_eq!(shown["status"], "approving");
    assert_eq!(shown["interrupted"], true);
    let again = ws.call("proposal.approve", json!({ "ids": [id] }));
    assert_eq!(again["approved"], 0);
    assert_eq!(again["items"][0]["error"]["code"], "conflict");
    let set_aside = ws.call(
        "proposal.reject",
        json!({ "ids": [id], "reason": "Stopped while it ran." }),
    );
    assert_eq!(set_aside["items"][0]["status"], "rejected");
}
