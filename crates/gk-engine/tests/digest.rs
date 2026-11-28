// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The digest: what agents, automation rules, reminders, due dates and the
//! command line did since the person last looked, grouped; reverting one
//! session from it; the person's marker, which moves when they dismiss the
//! digest and stays across a restart; a caller limited to a project; and
//! its speed on a long history.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::Profile;
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role, SessionStart, rfc3339};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 08:00 UTC.
const START: i64 = 1_795_420_800_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

struct Ws {
    engine: Engine,
    clock: Arc<AtomicI64>,
}

fn open_at(dir: &Path, clock: &Arc<AtomicI64>) -> Ws {
    let tick = Arc::clone(clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    config.time_zone = Some("UTC".into());
    let engine = Engine::open(&WorkspaceLayout::new(dir.join("Personal")), config).unwrap();
    Ws {
        engine,
        clock: Arc::clone(clock),
    }
}

fn me() -> Context {
    Context::user(Via::App)
}

fn cli() -> Context {
    Context::user(Via::Cli)
}

impl Ws {
    fn call_as(&self, ctx: &Context, op: &str, input: Value) -> Value {
        self.engine
            .call(ctx, op, input)
            .unwrap_or_else(|err| panic!("{op}: {err:?}"))
    }

    fn call(&self, op: &str, input: Value) -> Value {
        self.call_as(&me(), op, input)
    }

    fn task(&self, ctx: &Context, project: &str, title: &str) -> String {
        self.call_as(
            ctx,
            "task.create",
            json!({ "project": project, "title": title }),
        )["task"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn agent(&self, name: &str, client: Option<&str>) -> (Context, String) {
        let session = self
            .engine
            .start_session(&SessionStart {
                name: name.into(),
                client_id: client.map(Into::into),
                client_version: None,
                profile: Profile::Standard,
            })
            .unwrap();
        (
            Context::agent(name, session, Profile::Standard),
            session.to_string(),
        )
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn now(&self) -> i64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn digest(&self) -> Value {
        self.call("digest.get", json!({}))
    }
}

fn keys(tasks: &Value) -> Vec<String> {
    let mut keys: Vec<String> = tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t["key"]
                .as_str()
                .or_else(|| t["task"]["key"].as_str())
                .unwrap()
                .to_owned()
        })
        .collect();
    keys.sort();
    keys
}

fn names(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect()
}

/// Two projects, a rule that marks commented tasks urgent, and tasks
/// before the marker.
fn workspace(ws: &Ws) {
    ws.call("project.create", json!({ "name": "Team", "key": "TM" }));
    ws.call("project.create", json!({ "name": "Yard", "key": "ZZ" }));
    ws.call(
        "automation.create",
        json!({
            "project": "TM",
            "name": "Escalate",
            "trigger": { "on": "comment_added" },
            "actions": [{ "kind": "set_fields", "fields": { "priority": "urgent" } }],
        }),
    );
    for title in ["Existing one", "Existing two", "Existing three"] {
        ws.task(&me(), "TM", title);
    }
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Due tomorrow", "due_date": "tomorrow" }),
    );
}

#[test]
fn changes_after_the_marker_land_in_their_groups_and_earlier_ones_stay_out() {
    let dir = TempDir::new().unwrap();
    let ws = open_at(dir.path(), &Arc::new(AtomicI64::new(START)));
    workspace(&ws);

    // Before the marker: an agent, the rule (set off by a comment) and the
    // command line, and a reminder that came due.
    let (early, _) = ws.agent("Early bird", None);
    ws.task(&early, "TM", "Made before");
    ws.call_as(
        &early,
        "comment.add",
        json!({ "key": "TM-1", "body": "early" }),
    );
    ws.task(&cli(), "TM", "Scripted before");
    ws.call(
        "reminder.set",
        json!({ "key": "TM-2", "at": rfc3339(ws.now() + 10 * MINUTE) }),
    );
    ws.advance(HOUR);
    let first = ws.digest();
    assert_eq!(first["since_from"], "last_day");
    assert_eq!(names(&first["agents"]), ["Early bird"]);
    let dismissed = ws.call("digest.dismiss", json!({ "at": first["until"] }));
    assert_eq!(dismissed["marker"], first["until"]);
    // The next day: TM-4 comes due.
    ws.advance(DAY);

    // After it: two agents, the rule, the command line, the person in the
    // app (left out) and a reminder.
    let (scout, scout_id) = ws.agent("Scout", Some("claude-code"));
    let made_a = ws.task(&scout, "TM", "Scout's first");
    let made_b = ws.task(&scout, "TM", "Scout's second");
    ws.call_as(
        &scout,
        "comment.add",
        json!({ "key": "TM-3", "body": "Looked at it." }),
    );
    ws.advance(MINUTE);
    let (builder, builder_id) = ws.agent("Builder", None);
    ws.call_as(
        &builder,
        "task.update",
        json!({ "key": "TM-2", "title": "Existing two, renamed" }),
    );
    ws.call_as(
        &cli(),
        "task.update",
        json!({ "key": "TM-1", "title": "Scripted rename" }),
    );
    ws.call("task.update", json!({ "key": "TM-4", "title": "By me" }));
    ws.call(
        "reminder.set",
        json!({ "key": "TM-1", "at": rfc3339(ws.now() + 5 * MINUTE), "note": "Look" }),
    );
    ws.advance(10 * MINUTE);

    let d = ws.digest();
    assert_eq!(d["since_from"], "marker");
    assert_eq!(d["since"], first["until"]);
    // The latest session first; the early one is before the marker.
    assert_eq!(names(&d["agents"]), ["Builder", "Scout"]);
    let scout_entry = &d["agents"][1];
    assert_eq!(scout_entry["id"], scout_id.as_str());
    assert_eq!(scout_entry["client"], "claude-code");
    assert_eq!(scout_entry["running"], true);
    assert_eq!(scout_entry["groups"], 3);
    assert_eq!(scout_entry["reverted"], 0);
    let mut expected = vec![made_a.clone(), made_b.clone(), "TM-3".to_owned()];
    expected.sort();
    assert_eq!(keys(&scout_entry["tasks"]), expected);
    // The most recently touched first: the comment came last.
    assert_eq!(scout_entry["tasks"][0]["key"], "TM-3");
    assert_eq!(scout_entry["task_count"], 3);
    assert_eq!(d["agents"][0]["id"], builder_id.as_str());
    assert_eq!(keys(&d["agents"][0]["tasks"]), ["TM-2"]);
    assert_eq!(d["agents"][0]["changes"], 1);

    // The rule ran once since the marker, on Scout's comment.
    assert_eq!(names(&d["automation"]), ["Escalate"]);
    let rule = &d["automation"][0];
    assert_eq!(rule["project"], "TM");
    assert_eq!(rule["runs"], 1);
    assert_eq!(rule["failures"], 0);
    assert_eq!(keys(&rule["tasks"]), ["TM-3"]);
    assert_eq!(rule["recent"].as_array().unwrap().len(), 1);
    assert_eq!(rule["groups"].as_array().unwrap().len(), 1);

    // The reminder set after the marker; the earlier one is left out.
    let reminders = d["reminders"].as_array().unwrap();
    assert_eq!(reminders.len(), 1, "{reminders:?}");
    assert_eq!(reminders[0]["task"]["key"], "TM-1");
    assert_eq!(reminders[0]["note"], "Look");
    assert_eq!(keys(&d["became_due"]), ["TM-4"]);
    assert_eq!(d["became_due"][0]["overdue"], false);

    // The command line: one command, one task; the person's own change in
    // the app isn't listed anywhere.
    assert_eq!(d["cli"]["group_count"], 1);
    assert_eq!(keys(&d["cli"]["tasks"]), ["TM-1"]);
    assert_eq!(d["counts"]["sessions"], 2);
    assert_eq!(d["counts"]["rules"], 1);
    assert_eq!(d["counts"]["runs"], 1);
    assert_eq!(d["counts"]["reminders"], 1);
    assert_eq!(d["counts"]["became_due"], 1);
    assert_eq!(d["counts"]["items"], 2 + 1 + 1 + 1 + 1);

    // Reverting Scout's session from the digest reverts exactly it.
    let reverted = ws.call("history.revert", json!({ "session": scout_entry["id"] }));
    assert_eq!(reverted["skipped"], json!([]), "{reverted}");
    assert_eq!(reverted["reverted"].as_array().unwrap().len(), 3);
    let after = ws.digest();
    let scout_after = after["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Scout")
        .unwrap()
        .clone();
    assert_eq!(scout_after["reverted"], scout_after["changes"]);
    let builder_after = &after["agents"][0];
    assert_eq!(builder_after["name"], "Builder");
    assert_eq!(builder_after["reverted"], 0);
    let tasks = ws.call(
        "task.get",
        json!({ "keys": ["TM-1", "TM-2", "TM-3"], "include": ["comments"] }),
    );
    assert_eq!(tasks["items"][0]["title"], "Scripted rename");
    assert_eq!(tasks["items"][1]["title"], "Existing two, renamed");
    assert_eq!(
        tasks["items"][2]["comment_count"], 0,
        "Scout's comment is gone"
    );
    assert_ne!(
        tasks["items"][2]["priority"], "urgent",
        "and the run it set off"
    );
    assert_eq!(
        tasks["items"][0]["priority"], "urgent",
        "the early run stays"
    );
    for key in [&made_a, &made_b] {
        let gone = ws
            .engine
            .call(&me(), "task.get", json!({ "keys": [key] }))
            .unwrap_or_else(|e| json!({ "error": e.code }));
        assert_ne!(
            gone["items"][0]["title"].as_str(),
            Some("Scout's first"),
            "{gone}"
        );
        assert!(
            gone["items"].as_array().is_none_or(Vec::is_empty) || gone.get("error").is_some(),
            "{key} is in the trash: {gone}"
        );
    }
}

#[test]
fn dismissing_moves_the_marker_forward_only_and_a_restart_keeps_it() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let marker = {
        let ws = open_at(dir.path(), &clock);
        workspace(&ws);
        assert_eq!(ws.digest()["since_from"], "last_day");
        ws.advance(HOUR);
        let until = ws.digest()["until"].clone();
        ws.advance(MINUTE);
        let moved = ws.call("digest.dismiss", json!({ "at": until }));
        assert_eq!(moved["marker"], until);
        // An older moment doesn't move it back; a later one than now is now.
        let kept = ws.call("digest.dismiss", json!({ "at": rfc3339(START) }));
        assert_eq!(kept["marker"], until);
        let bad = ws
            .engine
            .call(&me(), "digest.dismiss", json!({ "at": "soon-ish" }))
            .unwrap_err();
        assert_eq!(bad.code, gk_api::ErrorCode::Invalid);
        // Dismissing is not history.
        let feed = ws.call("history.feed", json!({ "limit": 1 }));
        assert_ne!(feed["items"][0]["op"], "digest.dismiss");
        ws.engine.close();
        until
    };
    clock.fetch_add(DAY, Ordering::SeqCst);
    let ws = open_at(dir.path(), &clock);
    let d = ws.digest();
    assert_eq!(d["since_from"], "marker");
    assert_eq!(d["since"], marker);
    // A given start wins; days in words start at midnight.
    let given = ws.call("digest.get", json!({ "since": "yesterday" }));
    assert_eq!(given["since_from"], "given");
    assert_eq!(given["since"], "2026-11-23T00:00:00.000Z");
}

#[test]
fn a_caller_limited_to_a_project_sees_only_its_changes() {
    let dir = TempDir::new().unwrap();
    let ws = open_at(dir.path(), &Arc::new(AtomicI64::new(START)));
    workspace(&ws);
    ws.advance(HOUR);
    let (inside, _) = ws.agent("Inside", None);
    ws.task(&inside, "TM", "In the team");
    let (outside, _) = ws.agent("Outside", None);
    ws.task(&outside, "ZZ", "In the yard");
    ws.task(&cli(), "ZZ", "Scripted in the yard");
    ws.call(
        "task.create",
        json!({ "project": "ZZ", "title": "Yard due", "due_date": "today" }),
    );
    ws.call(
        "task.create",
        json!({ "project": "TM", "title": "Team due", "due_date": "today" }),
    );
    ws.advance(MINUTE);

    let all = ws.call("digest.get", json!({ "since": "today" }));
    assert_eq!(names(&all["agents"]), ["Outside", "Inside"]);
    assert!(all["counts"]["cli_changes"].as_u64().unwrap() > 0);

    let (limited, _) = ws.agent("Limited", None);
    let limited = limited.within_project("TM");
    let d = ws.call_as(&limited, "digest.get", json!({ "since": "today" }));
    assert_eq!(names(&d["agents"]), ["Inside"]);
    assert_eq!(d["cli"]["changes"], 0);
    assert_eq!(keys(&d["became_due"]), ["TM-6"]);
    let refused = ws
        .engine
        .call(&limited, "digest.get", json!({ "project": "ZZ" }))
        .unwrap_err();
    assert_eq!(refused.code, gk_api::ErrorCode::NotFound);

    // An agent's own start is the end of its previous session of that name;
    // it sees the person's changes in the app, and not its own session.
    let (again, _) = ws.agent("Inside", None);
    ws.task(&again, "TM", "By the second Inside");
    let d = ws.call_as(&again, "digest.get", json!({}));
    assert_eq!(d["since_from"], "previous_session");
    assert_eq!(names(&d["agents"]), ["Outside"]);
    assert_eq!(keys(&d["app"]["tasks"]), ["TM-6", "ZZ-3"]);
    assert_eq!(ws.digest()["app"]["changes"], 0, "the person saw their own");
}

/// A long history: 200,000 changes over 60 days by 50 agent sessions, the
/// command line and the person, on 400 tasks. The digest of the last day
/// and of the whole history stay within a bound: generous in a debug
/// build, where it runs with the other tests, and tight in a release
/// build.
#[test]
fn the_digest_of_a_long_history_stays_within_a_bound() {
    use gk_store::rusqlite::params;
    let dir = TempDir::new().unwrap();
    let ws = open_at(dir.path(), &Arc::new(AtomicI64::new(START)));
    ws.call("project.create", json!({ "name": "Big", "key": "BIG" }));
    for i in 0..400 {
        ws.task(&me(), "BIG", &format!("Task {i}"));
    }
    let now = ws.now() + 60 * DAY;
    ws.engine
        .store()
        .write("test.long_history", move |tx| {
            let project: Vec<u8> =
                tx.query_row("SELECT id FROM projects WHERE key = 'BIG'", [], |r| {
                    r.get(0)
                })?;
            let tasks: Vec<Vec<u8>> = {
                let mut stmt = tx.prepare("SELECT id FROM tasks")?;
                stmt.query_map([], |r| r.get(0))?
                    .collect::<Result<_, _>>()?
            };
            let mut sessions = Vec::new();
            for s in 0..50u8 {
                let id = vec![s + 1; 16];
                tx.execute(
                    "INSERT INTO agent_sessions (id, display_name, profile, started_at,
                                                 last_seen_at)
                     VALUES (?1, ?2, 'standard', ?3, ?3)",
                    params![id, format!("Agent {s}"), START],
                )?;
                sessions.push(id);
            }
            let mut insert = tx.prepare(
                "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via,
                                       session_id, project_id, entity_kind, entity_id, changes)
                 VALUES (?1, ?2, 'task.update', ?3, ?4, ?5, ?6, ?7, 'task', ?8, '{}')",
            )?;
            let total = 200_000i64;
            for i in 0..total {
                let at = START + (now - START) * i / total;
                let index = usize::try_from(i).unwrap();
                let (kind, via, session) = match i % 4 {
                    0 | 1 => ("agent", "mcp", Some(sessions[index % 50].clone())),
                    2 => ("user", "cli", None),
                    _ => ("user", "app", None),
                };
                let group = (i / 3).to_be_bytes().repeat(2);
                insert.execute(params![
                    group,
                    at,
                    kind,
                    "Someone",
                    via,
                    session,
                    project,
                    tasks[index % tasks.len()],
                ])?;
            }
            Ok::<_, gk_engine::Failure>(())
        })
        .unwrap();
    ws.clock.store(now, Ordering::SeqCst);
    let timed = |since: &str| {
        ws.call("digest.get", json!({ "since": since }));
        (0..3)
            .map(|_| {
                let start = std::time::Instant::now();
                let out = ws.call("digest.get", json!({ "since": since }));
                (start.elapsed(), out)
            })
            .min_by_key(|(t, _)| *t)
            .unwrap()
    };
    let (day, out) = timed("-1d");
    assert!(out["counts"]["sessions"].as_u64().unwrap() > 0);
    let (all, out) = timed("2026-11-01");
    assert_eq!(out["counts"]["sessions"], 50);
    println!("digest of a long history: last day {day:?}, everything {all:?}");
    let (day_bound, all_bound) = if cfg!(debug_assertions) {
        (
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(20),
        )
    } else {
        (
            std::time::Duration::from_millis(100),
            std::time::Duration::from_secs(2),
        )
    };
    assert!(day < day_bound, "the last day took {day:?}");
    assert!(all < all_bound, "everything took {all:?}");
}
