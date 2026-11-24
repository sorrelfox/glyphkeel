// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The operation pipeline over a real workspace: decoding, authorization,
//! confirmation, transactions with history, events, revert, Undo and Redo,
//! the schema guard and the locks.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gk_api::catalog::Meta;
use gk_api::ops::agent::{AgentPolicyPatch, AgentPolicyUpdate, AgentSessions, AgentSessionsInput};
use gk_api::ops::common::{ActorKind, Via};
use gk_api::ops::history::{ActivityQuery, HistoryFeed, HistoryRevert, RevertInput};
use gk_api::ops::settings::{SettingsGet, SettingsPatch, SettingsUpdate};
use gk_api::ops::workspace::WorkspaceGet;
use gk_api::{ErrorCode, Exposure, OpDescriptor, OpError, OpKind, Operation, Profile, Safety};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Registry, Role, SessionStart};
use gk_store::WorkspaceLayout;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tempfile::TempDir;

fn layout(dir: &TempDir) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.path().join("Personal"))
}

fn open(dir: &TempDir, role: Role) -> Engine {
    Engine::open(&layout(dir), EngineConfig::new(role)).unwrap()
}

fn open_with(dir: &TempDir, role: Role, registry: Registry) -> Engine {
    let mut config = EngineConfig::new(role);
    config.registry = registry;
    Engine::open(&layout(dir), config).unwrap()
}

fn app() -> Context {
    Context::user(Via::App)
}

fn agent(engine: &Engine, profile: Profile) -> Context {
    let session = engine
        .start_session(&SessionStart {
            name: "Test Agent".into(),
            client_id: Some("test-agent".into()),
            client_version: Some("1.0".into()),
            profile,
        })
        .unwrap();
    Context::agent("Test Agent", session, profile)
}

fn hours(n: u32) -> SettingsPatch {
    SettingsPatch {
        hours_per_day: Some(n),
        ..SettingsPatch::default()
    }
}

fn hours_now(engine: &Engine) -> u32 {
    engine
        .run::<SettingsGet>(&app(), &Default::default())
        .unwrap()
        .hours_per_day
}

// A test-only operation that can be made permanent, takes a request ID and
// can fail after writing.

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BumpInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_request_id: Option<String>,
    #[serde(default)]
    fail_after_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct BumpOutput {
    trash_days: u32,
}

struct Bump;
impl Operation for Bump {
    const ID: &'static str = "test.bump";
    type Input = BumpInput;
    type Output = BumpOutput;
}

struct Purge;
impl Operation for Purge {
    const ID: &'static str = "test.purge";
    type Input = BumpInput;
    type Output = BumpOutput;
}

const fn meta(safety: Safety) -> Meta {
    Meta {
        summary: "Test operation.",
        kind: OpKind::Command,
        safety,
        exposure: Exposure::Data,
        cli: None,
        input_name: "BumpInput",
        output_name: "BumpOutput",
    }
}

static BUMP: OpDescriptor = OpDescriptor::of::<Bump>(meta(Safety::Write));
static PURGE: OpDescriptor = OpDescriptor::of::<Purge>(meta(Safety::Permanent));

fn bump_registry() -> Registry {
    fn bump(
        w: &mut gk_engine::WriteCtx<'_>,
        input: BumpInput,
    ) -> Result<BumpOutput, gk_engine::Failure> {
        let days: u32 = w
            .tx()
            .query_row(
                "SELECT coalesce((SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'trash_days'), 30)",
                [],
                |r| r.get(0),
            )
            .map_err(gk_engine::Failure::from)?;
        let new = days + 1;
        w.tx()
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('trash_days', ?1, 0)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [new.to_string()],
            )
            .map_err(gk_engine::Failure::from)?;
        let id = w.workspace_id();
        w.record_field(
            "settings",
            id,
            None,
            "trash_days",
            Some(json!(days)),
            Some(json!(new)),
        );
        if input.fail_after_write {
            return Err(OpError::invalid("asked to fail").into());
        }
        Ok(BumpOutput { trash_days: new })
    }
    let mut r = Registry::standard();
    r.command::<Bump, _>(&BUMP, bump);
    r.command::<Purge, _>(&PURGE, bump);
    r
}

fn activity_count(engine: &Engine) -> i64 {
    engine
        .store()
        .read(|tx| {
            tx.query_row("SELECT count(*) FROM activity", [], |r| r.get(0))
                .map_err(gk_store::StoreError::from)
        })
        .unwrap()
}

#[test]
fn workspace_get_describes_the_workspace_and_caller() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Cli);
    let info = engine
        .run::<WorkspaceGet>(&Context::user(Via::Cli), &Default::default())
        .unwrap();
    assert_eq!(info.name, "Personal");
    assert_eq!(info.id, engine.store().workspace_id().to_string());
    assert_eq!(info.caller.via, Via::Cli);
    assert_eq!(info.caller.actor.kind, ActorKind::User);
    assert!(info.caller.profile.is_none());
    assert_eq!(info.settings.hours_per_day, 8);

    let ctx = agent(&engine, Profile::Full);
    let info = engine
        .run::<WorkspaceGet>(&ctx, &Default::default())
        .unwrap();
    // The workspace caps agents at standard by default.
    assert_eq!(info.caller.profile, Some(Profile::Standard));
    assert!(info.caller.session.is_some());
}

#[test]
fn unknown_operation_lists_the_valid_ones() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Cli);
    let e = engine
        .call(&app(), "task.frobnicate", json!({}))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    let choices = e.details["choices"].as_array().unwrap();
    assert!(choices.contains(&json!("settings.update")));
}

#[test]
fn invalid_input_names_the_field() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Cli);
    let e = engine
        .call(&app(), "settings.update", json!({ "colour": "red" }))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert!(
        e.message.contains("unknown field `colour`"),
        "{}",
        e.message
    );

    let e = engine
        .call(&app(), "settings.update", json!({ "trash_days": "many" }))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert_eq!(e.details["path"], json!("trash_days"));

    let e = engine
        .call(&app(), "settings.update", json!({ "hours_per_day": 25 }))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert_eq!(e.details["max"], json!(24));

    // `null` input counts as an empty object.
    engine.call(&app(), "settings.get", Value::Null).unwrap();
    assert_eq!(activity_count(&engine), 0);
}

#[test]
fn a_command_commits_with_history_and_one_event() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::App);
    let events = engine.subscribe();

    let change = engine.run::<SettingsUpdate>(&app(), &hours(6)).unwrap();
    assert_eq!(change.settings.hours_per_day, 6);
    assert_eq!(change.changed, ["hours_per_day"]);
    let group = change.group.unwrap();

    let event = events.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(event.group, group);
    assert_eq!(event.origin.via, Via::App);
    assert_eq!(event.changes.len(), 1);
    assert_eq!(event.changes[0].entity, "settings");
    assert_eq!(event.changes[0].fields, ["hours_per_day"]);

    let feed = engine
        .run::<HistoryFeed>(&app(), &ActivityQuery::default())
        .unwrap();
    assert_eq!(feed.total, 1);
    let entry = &feed.items[0];
    assert_eq!(entry.group, group);
    assert_eq!(entry.seq, event.seq);
    assert_eq!(entry.op, "settings.update");
    assert_eq!(entry.fields["hours_per_day"].before, None);
    assert_eq!(entry.fields["hours_per_day"].after, Some(json!(6)));

    // The same value again changes nothing: no group, no row, no event.
    let again = engine.run::<SettingsUpdate>(&app(), &hours(6)).unwrap();
    assert!(again.group.is_none());
    assert!(again.changed.is_empty());
    assert_eq!(activity_count(&engine), 1);
    assert!(events.recv_timeout(Duration::from_millis(50)).is_err());
}

#[test]
fn a_failing_command_keeps_nothing() {
    let dir = TempDir::new().unwrap();
    let engine = open_with(&dir, Role::Cli, bump_registry());
    let events = engine.subscribe();
    let e = engine
        .call(&app(), "test.bump", json!({ "fail_after_write": true }))
        .unwrap_err();
    assert_eq!(e.message, "asked to fail");
    assert_eq!(activity_count(&engine), 0);
    assert_eq!(
        engine
            .run::<SettingsGet>(&app(), &Default::default())
            .unwrap()
            .trash_days,
        30
    );
    assert!(events.try_recv().is_err());
}

#[test]
fn profiles_and_the_workspace_cap() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);

    let reader = agent(&engine, Profile::ReadOnly);
    engine
        .run::<WorkspaceGet>(&reader, &Default::default())
        .unwrap();
    let e = engine
        .run::<SettingsUpdate>(&reader, &hours(6))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    assert_eq!(e.details["profile"], json!("read-only"));

    let standard = agent(&engine, Profile::Standard);
    engine.run::<SettingsUpdate>(&standard, &hours(6)).unwrap();

    // The person lowers the cap; the standard agent becomes read-only.
    engine
        .run::<AgentPolicyUpdate>(
            &app(),
            &AgentPolicyPatch {
                max_profile: Profile::ReadOnly,
            },
        )
        .unwrap();
    let e = engine
        .run::<SettingsUpdate>(&standard, &hours(7))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    assert_eq!(hours_now(&engine), 6);
}

#[test]
fn app_operations_are_kept_from_agents_and_mcp() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);
    let full = agent(&engine, Profile::Full);
    let policy = json!({ "max_profile": "full" });

    let e = engine
        .call(&full, "agent.policy.update", policy.clone())
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    let e = engine
        .call(
            &Context::user(Via::Mcp),
            "agent.policy.update",
            policy.clone(),
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    // The command line has `agents policy`, but no Undo.
    engine
        .call(&Context::user(Via::Cli), "agent.policy.update", policy)
        .unwrap();
    let e = engine
        .call(&Context::user(Via::Cli), "history.undo", json!({}))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
}

#[test]
fn permanent_operations_need_a_confirmation_inside_the_call() {
    let dir = TempDir::new().unwrap();
    let engine = open_with(&dir, Role::App, bump_registry());

    let e = engine.call(&app(), "test.purge", json!({})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);

    let asked = Arc::new(AtomicUsize::new(0));
    let answer = |yes: bool| {
        let asked = Arc::clone(&asked);
        Confirmation::Ask(Arc::new(move |req| {
            assert_eq!(req.op.id, "test.purge");
            asked.fetch_add(1, Ordering::SeqCst);
            yes
        }))
    };
    let e = engine
        .call(
            &app().with_confirmation(answer(false)),
            "test.purge",
            json!({}),
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Forbidden);
    engine
        .call(
            &app().with_confirmation(answer(true)),
            "test.purge",
            json!({}),
        )
        .unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    // Ordinary writes never ask.
    engine
        .call(
            &app().with_confirmation(answer(true)),
            "test.bump",
            json!({}),
        )
        .unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    // `--yes`.
    engine
        .call(
            &Context::user(Via::Cli).with_confirmation(Confirmation::Given),
            "test.purge",
            json!({}),
        )
        .unwrap();

    // Agents are governed by their profile, not by a dialog.
    engine
        .run::<AgentPolicyUpdate>(
            &app(),
            &AgentPolicyPatch {
                max_profile: Profile::Full,
            },
        )
        .unwrap();
    let standard = agent(&engine, Profile::Standard);
    assert_eq!(
        engine
            .call(&standard, "test.purge", json!({}))
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    let full = agent(&engine, Profile::Full);
    engine.call(&full, "test.purge", json!({})).unwrap();
}

#[test]
fn a_repeated_request_id_returns_the_first_answer() {
    let dir = TempDir::new().unwrap();
    let engine = open_with(&dir, Role::Cli, bump_registry());
    let ctx = Context::user(Via::Cli);
    let first = engine
        .call(&ctx, "test.bump", json!({ "client_request_id": "r1" }))
        .unwrap();
    let again = engine
        .call(&ctx, "test.bump", json!({ "client_request_id": "r1" }))
        .unwrap();
    assert_eq!(first, again);
    assert_eq!(first["trash_days"], json!(31));
    assert_eq!(activity_count(&engine), 1);
    let other = engine
        .call(&ctx, "test.bump", json!({ "client_request_id": "r2" }))
        .unwrap();
    assert_eq!(other["trash_days"], json!(32));
}

#[test]
fn undo_and_redo_walk_the_apps_own_changes() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::App);
    let first = engine
        .run::<SettingsUpdate>(&app(), &hours(6))
        .unwrap()
        .group
        .unwrap();
    engine.run::<SettingsUpdate>(&app(), &hours(7)).unwrap();

    let undone = engine.call(&app(), "history.undo", json!({})).unwrap();
    assert_eq!(undone["op"], json!("settings.update"));
    assert_eq!(hours_now(&engine), 6);
    let undone = engine.call(&app(), "history.undo", json!({})).unwrap();
    assert_eq!(undone["target"], json!(first));
    assert_eq!(hours_now(&engine), 8);
    let e = engine.call(&app(), "history.undo", json!({})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);

    engine.call(&app(), "history.redo", json!({})).unwrap();
    assert_eq!(hours_now(&engine), 6);
    engine.call(&app(), "history.redo", json!({})).unwrap();
    assert_eq!(hours_now(&engine), 7);
    engine.call(&app(), "history.undo", json!({})).unwrap();
    assert_eq!(hours_now(&engine), 6);

    // A new change clears Redo.
    engine.run::<SettingsUpdate>(&app(), &hours(5)).unwrap();
    let e = engine.call(&app(), "history.redo", json!({})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn undo_never_overwrites_a_newer_change() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::App);
    engine.run::<SettingsUpdate>(&app(), &hours(6)).unwrap();
    // Someone else (the command line) changes the same setting.
    engine
        .run::<SettingsUpdate>(&Context::user(Via::Cli), &hours(4))
        .unwrap();
    let e = engine.call(&app(), "history.undo", json!({})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(e.message.contains("changed again"), "{}", e.message);
    assert_eq!(hours_now(&engine), 4);
}

#[test]
fn revert_a_session_skips_what_changed_since() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);
    let ctx = agent(&engine, Profile::Standard);
    engine.run::<SettingsUpdate>(&ctx, &hours(6)).unwrap();
    engine
        .run::<SettingsUpdate>(
            &ctx,
            &SettingsPatch {
                trash_days: Some(10),
                ..Default::default()
            },
        )
        .unwrap();
    // The person changes trash_days again afterwards.
    engine
        .run::<SettingsUpdate>(
            &app(),
            &SettingsPatch {
                trash_days: Some(12),
                ..Default::default()
            },
        )
        .unwrap();

    let input = RevertInput {
        session: Some("current".into()),
        dry_run: true,
        ..Default::default()
    };
    let dry = engine.run::<HistoryRevert>(&ctx, &input).unwrap();
    assert_eq!(dry.reverted.len(), 1);
    assert_eq!(dry.skipped.len(), 1);
    assert!(dry.group.is_none());
    assert_eq!(hours_now(&engine), 6);

    let done = engine
        .run::<HistoryRevert>(
            &ctx,
            &RevertInput {
                dry_run: false,
                ..input
            },
        )
        .unwrap();
    assert_eq!(done.reverted, dry.reverted);
    assert!(done.group.is_some());
    assert_eq!(hours_now(&engine), 8);
    let settings = engine
        .run::<SettingsGet>(&app(), &Default::default())
        .unwrap();
    assert_eq!(settings.trash_days, 12);

    // Reverting the same group again is skipped with the reason.
    let again = engine
        .run::<HistoryRevert>(
            &ctx,
            &RevertInput {
                groups: done.reverted.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(again.reverted.is_empty());
    assert_eq!(again.skipped[0].reason, "It was already reverted.");

    let feed = engine
        .run::<HistoryFeed>(
            &app(),
            &ActivityQuery {
                group: done.reverted.first().cloned(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(feed.items[0].reverted_by, done.group);
}

#[test]
fn feed_pages_and_filters() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);
    let ctx = agent(&engine, Profile::Standard);
    for n in 1..=5 {
        engine.run::<SettingsUpdate>(&app(), &hours(n)).unwrap();
    }
    engine.run::<SettingsUpdate>(&ctx, &hours(9)).unwrap();

    let page = engine
        .run::<HistoryFeed>(
            &app(),
            &ActivityQuery {
                limit: Some(4),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 6);
    assert_eq!(page.items.len(), 4);
    let next = engine
        .run::<HistoryFeed>(
            &app(),
            &ActivityQuery {
                limit: Some(4),
                cursor: page.next_cursor.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(next.items.len(), 2);
    assert!(next.next_cursor.is_none());
    assert!(next.items[0].seq < page.items[3].seq);

    let mine = engine
        .run::<HistoryFeed>(
            &ctx,
            &ActivityQuery {
                session: Some("current".into()),
                sessions: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(mine.total, 1);
    assert_eq!(mine.items[0].actor.name, "Test Agent");
    assert_eq!(mine.items[0].via, Via::Mcp);
    assert_eq!(mine.sessions.unwrap().len(), 1);

    let by_actor = engine
        .run::<HistoryFeed>(
            &app(),
            &ActivityQuery {
                actor: Some("test agent".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(by_actor.total, 1);

    let e = engine
        .call(&app(), "history.feed", json!({ "limit": 500 }))
        .unwrap_err();
    assert_eq!(e.details["path"], json!("limit"));
}

#[test]
fn sessions_count_reads_and_writes() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);
    let ctx = agent(&engine, Profile::Standard);
    engine
        .run::<WorkspaceGet>(&ctx, &Default::default())
        .unwrap();
    engine
        .run::<SettingsGet>(&ctx, &Default::default())
        .unwrap();
    engine.run::<SettingsUpdate>(&ctx, &hours(6)).unwrap();
    engine
        .run::<SettingsGet>(&ctx, &Default::default())
        .unwrap();
    engine.end_session(ctx.session.unwrap()).unwrap();

    let list = engine
        .run::<AgentSessions>(&app(), &AgentSessionsInput::default())
        .unwrap();
    assert_eq!(list.total, 1);
    let s = &list.items[0];
    assert_eq!(s.client.as_deref(), Some("test-agent"));
    assert_eq!((s.reads, s.writes), (3, 1));
    assert!(s.ended_at.is_some());
    let running = engine
        .run::<AgentSessions>(
            &app(),
            &AgentSessionsInput {
                running_only: true,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(running.total, 0);
}

#[test]
fn other_processes_changes_arrive_as_events() {
    let dir = TempDir::new().unwrap();
    let watcher_side = open(&dir, Role::Cli);
    let writer_side = open(&dir, Role::Cli);
    let events = watcher_side.subscribe();
    let _watch = watcher_side
        .watch_other_processes(Duration::from_millis(10))
        .unwrap();

    // Its own change is published once, at commit.
    let own = watcher_side
        .run::<SettingsUpdate>(&app(), &hours(3))
        .unwrap()
        .group
        .unwrap();
    let event = events.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(event.group, own);

    let other = writer_side
        .run::<SettingsUpdate>(&Context::user(Via::Cli), &hours(6))
        .unwrap()
        .group
        .unwrap();
    let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(event.group, other);
    assert_eq!(event.origin.via, Via::Cli);
    assert_eq!(event.changes[0].fields, ["hours_per_day"]);
    assert!(
        events.recv_timeout(Duration::from_millis(100)).is_err(),
        "no duplicate"
    );
}

#[test]
fn schema_guard_refuses_writes_after_an_upgrade() {
    let dir = TempDir::new().unwrap();
    let engine = open(&dir, Role::Mcp);
    engine.run::<SettingsUpdate>(&app(), &hours(6)).unwrap();

    let raw = gk_store::open_raw(&layout(&dir).database()).unwrap();
    raw.pragma_update(None, "user_version", 99).unwrap();
    drop(raw);

    let e = engine.run::<SettingsUpdate>(&app(), &hours(7)).unwrap_err();
    assert_eq!(e.code, ErrorCode::WorkspaceUpgraded);
    assert!(engine.must_exit());
    // Every later call fails the same way.
    let e = engine
        .run::<SettingsGet>(&app(), &Default::default())
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::WorkspaceUpgraded);
    engine.close();

    // A newer database is refused on open, naming the version.
    let e = Engine::open(&layout(&dir), EngineConfig::new(Role::Cli)).unwrap_err();
    assert_eq!(e.code, ErrorCode::WorkspaceTooNew);
    assert_eq!(e.details["found"], json!(99));
    assert_eq!(e.code.exit_code(), 4);
}

#[test]
fn locks_per_role() {
    let dir = TempDir::new().unwrap();
    let app_engine = open(&dir, Role::App);
    assert!(app_engine.holds_jobs_lock());
    let e = Engine::open(&layout(&dir), EngineConfig::new(Role::App)).unwrap_err();
    assert_eq!(e.code, ErrorCode::Busy);

    let mcp = open(&dir, Role::Mcp);
    assert!(!mcp.holds_jobs_lock());
    let cli = open(&dir, Role::Cli);
    assert!(!cli.holds_jobs_lock());

    // The jobs lock passes on when its holder ends.
    app_engine.close();
    mcp.jobs_tick().unwrap();
    assert!(mcp.holds_jobs_lock());
    cli.jobs_tick().unwrap();
    assert!(!cli.holds_jobs_lock());
}

#[test]
fn every_catalog_operation_has_a_handler() {
    let registry = Registry::standard();
    for d in gk_api::CATALOG {
        assert!(registry.contains(d.id), "{} has no handler", d.id);
    }
    assert_eq!(registry.names().len(), gk_api::CATALOG.len());
}
