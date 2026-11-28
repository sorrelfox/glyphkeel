// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The prompts in both protocol eras: the served list is the checked-in
//! golden list (`docs/reference/mcp-prompts.json`) for every profile, each
//! prompt's text is the one the reference page shows, every tool call a
//! text gives as an example runs, arguments are checked and complete, and
//! the session's project limit holds.

#![allow(clippy::unwrap_used)]

mod common;

use std::collections::HashMap;
use std::time::Duration;

use common::{connect, golden, lifecycles, open_as_person, set_agent_cap, workspace};
use gk_api::{Profile, Toolset};
use gk_mcp::Options;
use rmcp::ClientLifecycleMode;
use rmcp::model::{CompletionContext, GetPromptRequestParams, ProtocolVersion};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The golden prompt list.
fn golden_prompts() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/reference/mcp-prompts.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The prompt texts the reference page shows, by prompt name.
fn reference_texts() -> HashMap<String, String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/reference/mcp-prompts.md"
    );
    let page = std::fs::read_to_string(path).unwrap();
    let mut texts = HashMap::new();
    for section in page.split("\n## `").skip(1) {
        let name = section.split('`').next().unwrap().to_owned();
        let start = section.find("```text\n").unwrap() + "```text\n".len();
        let end = start + section[start..].find("\n```").unwrap();
        texts.insert(name, section[start..end].to_owned());
    }
    texts
}

/// A workspace with projects TM (with sub-project DOC) and OT, version 1.2
/// and 1.3 beta in TM, tasks TM-1 to TM-41 and OT-1.
fn seeded(options: &Options) {
    let (engine, person) = open_as_person(options);
    let call = |op: &str, input: Value| engine.call(&person, op, input).unwrap();
    call(
        "project.create",
        json!({ "name": "Task manager", "key": "TM" }),
    );
    call(
        "project.create",
        json!({ "name": "Docs", "key": "DOC", "parent": "TM" }),
    );
    call("project.create", json!({ "name": "Other", "key": "OT" }));
    call("version.create", json!({ "project": "TM", "name": "1.2" }));
    call(
        "version.create",
        json!({ "project": "TM", "name": "1.3 beta" }),
    );
    for n in 1..=40 {
        call(
            "task.create",
            json!({ "project": "TM", "title": format!("Task number {n}") }),
        );
    }
    call(
        "task.create",
        json!({ "project": "TM", "title": "Write the release notes", "fix_version": "1.2" }),
    );
    call(
        "task.create",
        json!({ "project": "OT", "title": "Elsewhere" }),
    );
    set_agent_cap(&engine, "full");
    engine.close();
}

fn args(pairs: &[(&str, &str)]) -> serde_json::Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), json!(v)))
        .collect()
}

async fn prompt_text(
    conn: &common::Connection,
    name: &str,
    arguments: &[(&str, &str)],
) -> Result<String, String> {
    let request = GetPromptRequestParams::new(name).with_arguments(args(arguments));
    match conn.client.get_prompt(request).await {
        Ok(r) => {
            assert_eq!(r.messages.len(), 1, "{name}");
            Ok(r.messages[0].content.as_text().unwrap().text.clone())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The `tool {json}` examples in `text`: each must be JSON.
fn examples(text: &str) -> Vec<(String, Value)> {
    let names: Vec<String> = golden()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    let mut found = Vec::new();
    for name in names {
        let marker = format!("{name} {{");
        let mut from = 0;
        while let Some(at) = text[from..].find(&marker) {
            let start = from + at + name.len() + 1;
            let before = text[..from + at].chars().last();
            from = start;
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let value = serde_json::Deserializer::from_str(&text[start..])
                .into_iter::<Value>()
                .next()
                .unwrap()
                .unwrap_or_else(|e| panic!("{name}'s example isn't JSON: {e}"));
            found.push((name.clone(), value));
        }
    }
    found
}

#[tokio::test]
async fn the_served_prompt_list_matches_the_golden_list_in_both_eras() {
    let golden = golden_prompts();
    for (label, mode, asked, _) in lifecycles() {
        for profile in Profile::ALL {
            let (_dir, mut options) = workspace();
            options.profile = profile;
            seeded(&options);
            let conn = connect(&options, mode.clone(), asked.clone(), "golden-check").await;
            let info = conn.client.peer_info().unwrap();
            let prompts_cap = info.capabilities.prompts.clone().unwrap();
            assert_eq!(prompts_cap.list_changed, Some(true), "{label}");
            assert!(info.capabilities.completions.is_some(), "{label}");

            let served = conn.client.list_all_prompts().await.unwrap();
            let names: Vec<&str> = served.iter().map(|p| p.name.as_str()).collect();
            let expected: Vec<&str> = golden["profiles"][profile.as_str()]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(names, expected, "{label} {profile}");
            for p in &served {
                let mut want = golden["prompts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|g| g["name"] == p.name.as_str())
                    .unwrap()
                    .clone();
                let object = want.as_object_mut().unwrap();
                object.remove("tools");
                object.remove("completions");
                assert_eq!(serde_json::to_value(p).unwrap(), want, "{label} {}", p.name);
            }
            conn.close().await;
        }
    }
}

#[tokio::test]
async fn every_prompt_gives_the_reference_text_and_its_examples_run() {
    let reference = reference_texts();
    let prompts = golden_prompts();
    let read_only: Vec<String> = golden()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["annotations"]["readOnlyHint"] == true)
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, mut options) = workspace();
        options.toolsets = Some(Toolset::ALL.to_vec());
        seeded(&options);
        let conn = connect(&options, mode, asked, "claude-code").await;
        for p in prompts["prompts"].as_array().unwrap() {
            let name = p["name"].as_str().unwrap();
            let completions = p["completions"].as_object().unwrap();
            let required: Vec<(&str, &str)> = p["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| a["required"] == true)
                .map(|a| {
                    let arg = a["name"].as_str().unwrap();
                    // Lower case: the text gets the canonical key.
                    let value = match completions[arg].as_str().unwrap() {
                        "task" => "tm-41",
                        "version" => "1.2",
                        _ => "tm",
                    };
                    (arg, value)
                })
                .collect();
            let text = prompt_text(&conn, name, &required).await.unwrap();
            assert_eq!(&text, &reference[name], "{label}: {name}");

            let mut texts = vec![text];
            let optional_project = p["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["name"] == "project" && a["required"] == false);
            if optional_project {
                let mut with = required.clone();
                with.push(("project", "TM"));
                let t = prompt_text(&conn, name, &with).await.unwrap();
                assert!(t.contains("TM"), "{label}: {name}");
                texts.push(t);
            }
            for text in texts {
                let found = examples(&text);
                assert!(!found.is_empty(), "{name} gives no example call");
                for (tool, input) in found {
                    if !read_only.contains(&tool) {
                        continue;
                    }
                    let r = conn.call(&tool, input.clone()).await;
                    assert_eq!(
                        r.is_error,
                        Some(false),
                        "{label}: {name}: {tool} {input} -> {r:?}"
                    );
                }
            }
        }
        conn.close().await;
    }
}

/// A prompt, its arguments and a part of the error it gives.
type Case = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static str,
);

#[tokio::test]
async fn arguments_are_checked() {
    let (_dir, mut options) = workspace();
    options.profile = Profile::ReadOnly;
    seeded(&options);
    let (_, mode, asked, _) = lifecycles().remove(1);
    let conn = connect(&options, mode, asked, "claude-code").await;
    let cases: [Case; 7] = [
        (
            "no_such_prompt",
            &[],
            "There is no prompt \"no_such_prompt\"",
        ),
        ("write_status_update", &[], "needs the argument \"project\""),
        (
            "write_status_update",
            &[("project", "TM"), ("colour", "red")],
            "has no argument \"colour\"",
        ),
        ("write_status_update", &[("project", "NOPE")], "not_found"),
        ("summarise_task_history", &[("task", "TM-999")], "not_found"),
        (
            "write_release_notes",
            &[("project", "TM"), ("version", "9.9")],
            "not_found",
        ),
        (
            "break_down_task",
            &[("task", "TM-1")],
            "Your profile (read-only) doesn't allow the prompt break_down_task",
        ),
    ];
    for (name, arguments, expected) in cases {
        let e = prompt_text(&conn, name, arguments).await.unwrap_err();
        assert!(e.contains(expected), "{name} {arguments:?}: {e}");
        assert!(e.contains("-32602"), "invalid params: {e}");
    }
    // A version's name reaches the text as a JSON string.
    let t = prompt_text(
        &conn,
        "write_release_notes",
        &[("project", "TM"), ("version", "1.3 beta")],
    )
    .await
    .unwrap();
    assert!(
        t.contains(r#"get_version {"project": "TM", "version": "1.3 beta"}"#),
        "{t}"
    );
    conn.close().await;
}

#[tokio::test]
async fn arguments_complete_in_both_eras() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, options) = workspace();
        seeded(&options);
        let conn = connect(&options, mode, asked, "claude-code").await;
        let complete = |prompt: &'static str, arg: &'static str, value: &'static str| {
            conn.client
                .complete_prompt_argument(prompt, arg, value, None)
        };
        let projects = complete("write_status_update", "project", "")
            .await
            .unwrap();
        assert_eq!(projects.values, vec!["TM", "DOC", "OT"], "{label}");
        let projects = complete("write_status_update", "project", "o")
            .await
            .unwrap();
        assert_eq!(
            projects.values,
            vec!["OT", "DOC"],
            "{label}: key first, then the name"
        );

        let tasks = complete("break_down_task", "task", "tm-4").await.unwrap();
        let mut values = tasks.values.clone();
        values.sort();
        assert_eq!(values, vec!["TM-4", "TM-40", "TM-41"], "{label}");
        let tasks = complete("summarise_task_history", "task", "release")
            .await
            .unwrap();
        assert_eq!(tasks.values, vec!["TM-41"], "{label}");
        let tasks = complete("summarise_task_history", "task", "TM")
            .await
            .unwrap();
        assert_eq!(tasks.values.len(), 41, "{label}");
        assert_eq!(tasks.has_more, Some(false), "{label}");

        let context =
            CompletionContext::with_arguments([("project".to_owned(), "TM".to_owned())].into());
        let versions = conn
            .client
            .complete_prompt_argument("write_release_notes", "version", "1.", Some(context))
            .await
            .unwrap();
        assert_eq!(versions.values, vec!["1.2", "1.3 beta"], "{label}");
        // Without a project there is nothing to complete a version in.
        let versions = complete("write_release_notes", "version", "")
            .await
            .unwrap();
        assert!(versions.values.is_empty(), "{label}");

        let e = complete("write_release_notes", "colour", "")
            .await
            .unwrap_err();
        assert!(e.to_string().contains("has no argument"), "{label}: {e}");
        conn.close().await;
    }
}

#[tokio::test]
async fn a_session_limited_to_a_project_completes_and_names_only_that_project() {
    let (_dir, mut options) = workspace();
    seeded(&options);
    options.project = Some("TM".to_owned());
    let (_, mode, asked, _) = lifecycles().remove(0);
    let conn = connect(&options, mode, asked, "claude-code").await;

    let projects = conn
        .client
        .complete_prompt_argument("write_status_update", "project", "", None)
        .await
        .unwrap();
    assert_eq!(projects.values, vec!["TM", "DOC"]);
    let tasks = conn
        .client
        .complete_prompt_argument("summarise_task_history", "task", "elsewhere", None)
        .await
        .unwrap();
    assert!(tasks.values.is_empty(), "{tasks:?}");
    // The version completes in the session's project without naming it.
    let versions = conn
        .client
        .complete_prompt_argument("write_release_notes", "version", "", None)
        .await
        .unwrap();
    assert_eq!(versions.values, vec!["1.2", "1.3 beta"]);

    // Left out, the project is the session's; another one is refused.
    let t = prompt_text(&conn, "write_status_update", &[])
        .await
        .unwrap();
    assert!(
        t.starts_with("Write a status update for project TM "),
        "{t}"
    );
    let t = prompt_text(&conn, "plan_my_day", &[]).await.unwrap();
    assert!(t.contains("Look only at project TM"), "{t}");
    let e = prompt_text(&conn, "write_status_update", &[("project", "OT")])
        .await
        .unwrap_err();
    assert!(e.contains("not_found"), "{e}");
    let e = prompt_text(&conn, "summarise_task_history", &[("task", "OT-1")])
        .await
        .unwrap_err();
    assert!(e.contains("not_found"), "{e}");
    conn.close().await;
}

/// `glyphkeel mcp` with no options but the workspace serves every prompt:
/// the standard profile with the default toolsets.
#[tokio::test]
async fn the_default_command_line_lists_every_prompt() {
    let (_dir, defaults) = workspace();
    let workspace = defaults.workspace.clone().unwrap();
    let options = Options::parse_args(&["--workspace".into(), workspace.into()]).unwrap();
    assert_eq!(options.profile, Profile::Standard);
    assert_eq!(options.toolsets(), Toolset::defaults());
    seeded(&options);
    let conn = connect(
        &options,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
        ProtocolVersion::V_2026_07_28,
        "default-check",
    )
    .await;
    let names: Vec<String> = conn
        .client
        .list_all_prompts()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let all: Vec<String> = golden_prompts()["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(all.len(), 9);
    assert_eq!(names, all);
    conn.close().await;
}

#[tokio::test]
async fn the_prompt_list_follows_the_toolsets() {
    let (_dir, mut options) = workspace();
    options.toolsets = Some(vec![Toolset::Workspace, Toolset::Tasks]);
    let conn = connect(
        &options,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
        ProtocolVersion::V_2026_07_28,
        "claude-code",
    )
    .await;
    // Only the task history needs nothing beyond the task and the activity
    // tools; the others need projects, views, time or schedule tools.
    let names: Vec<String> = conn
        .client
        .list_all_prompts()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["summarise_task_history"]);
    conn.close().await;
}

/// The old era learns of a changed prompt list by a notification.
#[tokio::test]
async fn a_lowered_cap_sends_a_prompt_list_notification() {
    let (_dir, mut options) = workspace();
    options.profile = Profile::Full;
    seeded(&options);
    let server = gk_mcp::open(&options).unwrap();
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server = tokio::spawn(gk_mcp::serve(server, server_io));
    let (reader, mut writer) = tokio::io::split(client_io);
    let mut reader = BufReader::new(reader);
    for message in [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                          "clientInfo": {"name": "claude-code", "version": "2.1.330"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "prompts/list"}),
    ] {
        let mut line = message.to_string();
        line.push('\n');
        writer.write_all(line.as_bytes()).await.unwrap();
    }

    let mut lowered = false;
    let found = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).await.unwrap() > 0);
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == 2 {
                assert!(
                    message["result"]["prompts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["name"] == "break_down_task"),
                    "{message}"
                );
                let (engine, _) = open_as_person(&options);
                set_agent_cap(&engine, "read-only");
                engine.close();
                lowered = true;
            }
            if message["method"] == "notifications/prompts/list_changed" {
                return lowered;
            }
        }
    })
    .await;
    assert_eq!(found, Ok(true), "the notification came after the change");
    drop(writer);
    drop(reader);
    server.await.unwrap();
}
