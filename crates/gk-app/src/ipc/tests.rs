// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The page's calls over a real workspace: `op` through Tauri's IPC, typed
//! errors, change events to every window in order, and the confirmation of
//! permanent operations inside the call.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gk_api::catalog::Meta;
use gk_api::{Changed, ErrorCode, Exposure, OpDescriptor, OpKind, Operation, Safety};
use gk_engine::{Confirmation, EngineConfig, Failure, Registry, Role, WriteCtx};
use gk_store::WorkspaceLayout;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, Channel, InvokeBody, InvokeResponseBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};
use tempfile::TempDir;

use super::Workspace;

fn layout(dir: &TempDir) -> WorkspaceLayout {
    WorkspaceLayout::new(dir.path().join("Personal"))
}

fn open(dir: &TempDir, confirm: Confirmation) -> Workspace {
    Workspace::open(&layout(dir), EngineConfig::new(Role::App), confirm)
}

/// A channel like the page's, collecting what it receives.
fn channel() -> (Channel<Changed>, Arc<Mutex<Vec<Changed>>>) {
    let got = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&got);
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Json(json) = body else {
            panic!("events travel as JSON");
        };
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(&json).unwrap());
        Ok(())
    });
    (channel, got)
}

fn wait_for(got: &Mutex<Vec<Changed>>, count: usize) -> Vec<Changed> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let events = got.lock().unwrap().clone();
        if events.len() >= count || Instant::now() > deadline {
            return events;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_page_reads_the_workspace() {
    let dir = TempDir::new().unwrap();
    let workspace = open(&dir, Confirmation::Refuse);
    let info = workspace.call("workspace.get", Value::Null, None).unwrap();
    assert_eq!(info["name"], "Personal");
    assert_eq!(info["caller"]["via"], "app");
    assert!(dir.path().join("Personal/workspace.db").exists());
    workspace.close();
}

#[test]
fn errors_reach_the_page_typed() {
    let dir = TempDir::new().unwrap();
    let workspace = open(&dir, Confirmation::Refuse);

    let unknown = workspace.call("task.fly", json!({}), None).unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound);
    assert!(
        unknown.details["choices"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "workspace.get")
    );

    let invalid = workspace
        .call("settings.update", json!({ "hours_per_day": "many" }), None)
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::Invalid);
    assert_eq!(invalid.details["path"], "hours_per_day");

    let unknown_field = workspace
        .call("settings.update", json!({ "colour": "red" }), None)
        .unwrap_err();
    assert_eq!(unknown_field.code, ErrorCode::Invalid);
    workspace.close();
}

#[test]
fn a_workspace_that_cannot_open_answers_every_call_with_the_reason() {
    let dir = TempDir::new().unwrap();
    let first = open(&dir, Confirmation::Refuse);
    let second = open(&dir, Confirmation::Refuse);
    assert_eq!(second.failure().unwrap().code, ErrorCode::Busy);
    let err = second.call("workspace.get", Value::Null, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::Busy);
    first.close();
}

#[test]
fn change_events_reach_every_window_in_commit_order() {
    let dir = TempDir::new().unwrap();
    let workspace = open(&dir, Confirmation::Refuse);
    let (main, main_got) = channel();
    let (other, other_got) = channel();
    assert!(!workspace.subscribe("main", main));
    assert!(!workspace.subscribe("task-1", other));

    for hours in 1..=20 {
        workspace
            .call("settings.update", json!({ "hours_per_day": hours }), None)
            .unwrap();
    }
    // A change that changes nothing sends nothing.
    workspace
        .call("settings.update", json!({ "hours_per_day": 20 }), None)
        .unwrap();

    for got in [&main_got, &other_got] {
        let events = wait_for(got, 20);
        assert_eq!(events.len(), 20);
        assert!(events.windows(2).all(|pair| pair[0].seq < pair[1].seq));
        let change = &events[0].changes[0];
        assert_eq!(change.entity, "settings");
        assert_eq!(change.fields, ["hours_per_day"]);
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(main_got.lock().unwrap().len(), 20);

    // A reload subscribes again: the new channel replaces the old one.
    let (reloaded, reloaded_got) = channel();
    assert!(workspace.subscribe("main", reloaded));
    workspace
        .call("settings.update", json!({ "hours_per_day": 8 }), None)
        .unwrap();
    assert_eq!(wait_for(&reloaded_got, 1).len(), 1);
    assert_eq!(wait_for(&other_got, 21).len(), 21);
    assert_eq!(main_got.lock().unwrap().len(), 20);

    // A closed window gets nothing more.
    workspace.unsubscribe("task-1");
    workspace
        .call("settings.update", json!({ "hours_per_day": 7 }), None)
        .unwrap();
    assert_eq!(wait_for(&reloaded_got, 2).len(), 2);
    assert_eq!(other_got.lock().unwrap().len(), 21);
    workspace.close();
}

// A test-only permanent operation.

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WipeInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct WipeOutput {
    wiped: bool,
}

struct Wipe;
impl Operation for Wipe {
    const ID: &'static str = "test.wipe";
    type Input = WipeInput;
    type Output = WipeOutput;
}

static WIPE: OpDescriptor = OpDescriptor::of::<Wipe>(Meta {
    summary: "Wipe everything for good.",
    kind: OpKind::Command,
    safety: Safety::Permanent,
    exposure: Exposure::Data,
    cli: None,
    input_name: "WipeInput",
    output_name: "WipeOutput",
});

fn open_with_wipe(dir: &TempDir, confirm: Confirmation) -> Workspace {
    let mut registry = Registry::standard();
    registry.command::<Wipe, _>(&WIPE, |_: &mut WriteCtx<'_>, _| {
        Ok::<_, Failure>(WipeOutput { wiped: true })
    });
    let mut config = EngineConfig::new(Role::App);
    config.registry = registry;
    Workspace::open(&layout(dir), config, confirm)
}

#[test]
fn permanent_operations_wait_for_the_native_answer_inside_the_call() {
    let asked = Arc::new(AtomicUsize::new(0));
    let answer = Arc::new(Mutex::new(false));
    let confirm = {
        let (asked, answer) = (Arc::clone(&asked), Arc::clone(&answer));
        Confirmation::Ask(Arc::new(move |request| {
            assert_eq!(request.op.id, "test.wipe");
            asked.fetch_add(1, Ordering::SeqCst);
            *answer.lock().unwrap()
        }))
    };
    let dir = TempDir::new().unwrap();
    let workspace = open_with_wipe(&dir, confirm);

    let refused = workspace.call("test.wipe", json!({}), None).unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    // The page can't pass an answer of its own.
    let forged = workspace
        .call("test.wipe", json!({ "confirmed": true }), None)
        .unwrap_err();
    assert_eq!(forged.code, ErrorCode::Invalid);

    *answer.lock().unwrap() = true;
    let done = workspace.call("test.wipe", json!({}), None).unwrap();
    assert_eq!(done["wiped"], true);
    assert_eq!(asked.load(Ordering::SeqCst), 2);

    // Other writes never ask.
    workspace
        .call("settings.update", json!({ "trash_days": 9 }), None)
        .unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    workspace.close();
}

/// The embedded page's origin: Tauri's own scheme, which WebView2 on
/// Windows reaches as an `http` host instead. A request from any other
/// origin finds no command.
const PAGE_ORIGIN: &str = if cfg!(windows) {
    "http://tauri.localhost"
} else {
    "tauri://localhost"
};

fn request(cmd: &str, body: Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: PAGE_ORIGIN.parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    }
}

#[test]
fn the_op_command_takes_an_operation_and_its_input() {
    let dir = TempDir::new().unwrap();
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![super::op, super::subscribe])
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::new(open(&dir, Confirmation::Refuse)));
    let webview = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .unwrap();

    let info = get_ipc_response(&webview, request("op", json!({ "op": "workspace.get" })))
        .unwrap()
        .deserialize::<Value>()
        .unwrap();
    assert_eq!(info["name"], "Personal");

    let changed = get_ipc_response(
        &webview,
        request(
            "op",
            json!({ "op": "settings.update", "input": { "week_start": "sunday" }, "request_id": "r-1" }),
        ),
    )
    .unwrap()
    .deserialize::<Value>()
    .unwrap();
    assert_eq!(changed["changed"], json!(["week_start"]));

    // The engine's error, as the page receives it.
    let err = get_ipc_response(
        &webview,
        request(
            "op",
            json!({ "op": "settings.update", "input": { "trash_days": 0 } }),
        ),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalid");
    assert_eq!(err["details"]["path"], "trash_days");
    assert!(err.get("hint").is_none_or(Value::is_string));

    // A call Tauri itself rejects arrives as a plain string.
    let err = get_ipc_response(
        &webview,
        request("op", json!({ "operation": "workspace.get" })),
    )
    .unwrap_err();
    assert!(err.is_string());

    app.state::<Arc<Workspace>>().close();
}

#[test]
fn a_restore_opens_the_workspace_again_on_the_restored_data() {
    let dir = TempDir::new().unwrap();
    let mut config = EngineConfig::new(Role::App);
    config.backups = Some(dir.path().join("backups"));
    let workspace = Workspace::open(&layout(&dir), config, Confirmation::Given);
    let (main, got) = channel();
    workspace.subscribe("main", main);
    workspace
        .call(
            "project.create",
            json!({ "name": "Home", "key": "HOME" }),
            None,
        )
        .unwrap();
    let create = |title: &str| {
        workspace
            .call(
                "task.create",
                json!({ "project": "HOME", "title": title }),
                None,
            )
            .unwrap()
    };
    create("Before the backup");
    let backup = workspace.call("backup.create", json!({}), None).unwrap();
    create("After the backup");
    let before = wait_for(&got, 3).len();

    let restored = workspace
        .call("backup.restore", json!({ "name": backup["name"] }), None)
        .unwrap();
    assert_eq!(restored["restored"]["name"], backup["name"]);

    // The next call reaches a fresh engine, which sees the backup's data.
    let tasks = workspace
        .call("task.search", json!({ "project": "HOME" }), None)
        .unwrap();
    let titles: Vec<&str> = tasks["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|task| task["title"].as_str())
        .collect();
    assert!(titles.contains(&"Before the backup"), "{titles:?}");
    assert!(!titles.contains(&"After the backup"), "{titles:?}");
    // Every window hears that everything changed.
    let events = wait_for(&got, before + 1);
    assert!(
        events
            .iter()
            .any(|event| event.changes.iter().any(|c| c.entity == "workspace")),
        "{events:?}"
    );
    // And the reopened workspace keeps working.
    create("After the restore");
    workspace.close();
}

#[test]
fn deleting_a_backup_asks_first() {
    let dir = TempDir::new().unwrap();
    let mut config = EngineConfig::new(Role::App);
    config.backups = Some(dir.path().join("backups"));
    let workspace = Workspace::open(&layout(&dir), config, Confirmation::Refuse);
    let backup = workspace.call("backup.create", json!({}), None).unwrap();
    let err = workspace
        .call("backup.delete", json!({ "name": backup["name"] }), None)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
    workspace.close();
}

#[test]
fn raising_the_limits_on_agents_asks_natively_and_lowering_does_not() {
    let dir = TempDir::new().unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let answer = Arc::new(Mutex::new(false));
    let workspace = {
        let asked = Arc::clone(&asked);
        let answer = Arc::clone(&answer);
        open(
            &dir,
            Confirmation::Ask(Arc::new(move |request| {
                assert_eq!(request.op.id, "agent.limits.update");
                asked.fetch_add(1, Ordering::SeqCst);
                *answer.lock().unwrap()
            })),
        )
    };
    let limits = |workspace: &Workspace| {
        workspace.call("settings.get", Value::Null, None).unwrap()["agent_limits"].clone()
    };
    let set =
        |workspace: &Workspace, input: Value| workspace.call("agent.limits.update", input, None);

    // Lowering a limit, setting a limit where there was none, or the same
    // again, go through without a question.
    set(&workspace, json!({ "writes_per_minute": 60 })).unwrap();
    set(&workspace, json!({ "new_tasks_per_day": 100 })).unwrap();
    set(&workspace, json!({ "writes_per_minute": 60 })).unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 0);
    assert_eq!(limits(&workspace)["writes_per_minute"], 60);

    // A higher number, or no limit, asks; a refusal changes nothing.
    for input in [
        json!({ "writes_per_minute": 61 }),
        json!({ "no_limit": ["new_tasks_per_day"] }),
        json!({ "reset": ["writes_per_minute"] }),
    ] {
        let refused = set(&workspace, input.clone()).unwrap_err();
        assert_eq!(refused.code, ErrorCode::Forbidden, "{input}");
    }
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    assert_eq!(limits(&workspace)["writes_per_minute"], 60);
    assert_eq!(limits(&workspace)["new_tasks_per_day"], 100);

    *answer.lock().unwrap() = true;
    set(&workspace, json!({ "no_limit": ["new_tasks_per_day"] })).unwrap();
    assert!(limits(&workspace).get("new_tasks_per_day").is_none());
    assert_eq!(asked.load(Ordering::SeqCst), 4);
    workspace.close();
}

#[test]
fn raising_what_agents_may_do_asks_natively_and_lowering_does_not() {
    let dir = TempDir::new().unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let answer = Arc::new(Mutex::new(false));
    let workspace = {
        let asked = Arc::clone(&asked);
        let answer = Arc::clone(&answer);
        open(
            &dir,
            Confirmation::Ask(Arc::new(move |request| {
                assert_eq!(request.op.id, "agent.policy.update");
                asked.fetch_add(1, Ordering::SeqCst);
                *answer.lock().unwrap()
            })),
        )
    };
    let cap = |workspace: &Workspace| {
        workspace.call("settings.get", Value::Null, None).unwrap()["agent_max_profile"].clone()
    };
    assert_eq!(cap(&workspace), "standard");

    // Lowering, and setting the same again, go through without a question.
    workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "read-only" }),
            None,
        )
        .unwrap();
    assert_eq!(cap(&workspace), "read-only");
    workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "read-only" }),
            None,
        )
        .unwrap();
    assert_eq!(asked.load(Ordering::SeqCst), 0);

    // Raising asks; a refusal changes nothing.
    let refused = workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "full" }),
            None,
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert_eq!(cap(&workspace), "read-only");
    assert_eq!(asked.load(Ordering::SeqCst), 1);

    *answer.lock().unwrap() = true;
    workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "full" }),
            None,
        )
        .unwrap();
    assert_eq!(cap(&workspace), "full");
    assert_eq!(asked.load(Ordering::SeqCst), 2);

    // Turning off agents' questions to the person goes through; turning
    // them on again asks.
    let confirmations = |workspace: &Workspace| {
        workspace.call("settings.get", Value::Null, None).unwrap()["agent_confirmations"].clone()
    };
    workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "full", "confirmations": false }),
            None,
        )
        .unwrap();
    assert_eq!(confirmations(&workspace), false);
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    *answer.lock().unwrap() = false;
    let refused = workspace
        .call(
            "agent.policy.update",
            json!({ "max_profile": "full", "confirmations": true }),
            None,
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert_eq!(confirmations(&workspace), false);
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    workspace.close();
}

#[test]
fn the_first_start_on_a_workspace_is_known_as_such() {
    let dir = TempDir::new().unwrap();
    let first = open(&dir, Confirmation::Refuse);
    assert!(first.created());
    first.close();
    let again = open(&dir, Confirmation::Refuse);
    assert!(!again.created());
    again.close();
}
