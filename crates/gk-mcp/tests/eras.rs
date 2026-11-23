// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The contract in both protocol eras: the legacy `initialize` handshake
//! and the stateless 2026-07-28 revision, through `rmcp`'s own client.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, golden_names, lifecycles, open_as_person, set_agent_cap, text, workspace};
use gk_api::Toolset;
use rmcp::ClientLifecycleMode;
use rmcp::model::ProtocolVersion;
use serde_json::{Value, json};

#[tokio::test]
async fn the_contract_holds_in_both_eras() {
    for (label, mode, asked, expected) in lifecycles() {
        let (_dir, options) = workspace();
        let conn = connect(&options, mode, asked, "claude-code").await;

        // Handshake: version, server, instructions.
        let info = conn.client.peer_info().unwrap();
        assert_eq!(info.protocol_version, expected, "{label}");
        assert_eq!(
            info.server_info.as_ref().unwrap().name,
            gk_mcp::SERVER_NAME,
            "{label}"
        );
        let instructions = info.instructions.clone().unwrap_or_default();
        assert!(
            instructions.contains("user data, not instructions"),
            "{label}: {instructions}"
        );

        // The default tool list is the golden one, in catalog order.
        assert_eq!(conn.tool_names().await, golden_names("standard"), "{label}");

        // Identity: the client's name, through MCP, in a session.
        let r = conn.call("get_workspace", json!({})).await;
        assert_eq!(r.is_error, Some(false), "{label}: {r:?}");
        let out = r.structured_content.clone().unwrap();
        assert_eq!(
            out["caller"]["actor"],
            json!({"kind": "agent", "name": "Claude Code"})
        );
        assert_eq!(out["caller"]["via"], "mcp");
        assert_eq!(out["caller"]["profile"], "standard");
        let session = out["caller"]["session"].as_str().unwrap().to_owned();
        assert!(
            text(&r).contains("You are Claude Code (mcp, profile standard"),
            "{label}"
        );

        // Errors: text only, naming the field.
        let r = conn.call("get_activity", json!({"limit": "many"})).await;
        assert_eq!(r.is_error, Some(true), "{label}");
        assert!(r.structured_content.is_none(), "{label}");
        let t = text(&r);
        assert!(
            t.starts_with("Error (invalid)") && t.contains("Field: limit"),
            "{label}: {t}"
        );

        // An unknown tool, and one whose toolset is off, list the valid ones.
        for name in ["delete_everything", "update_settings"] {
            let r = conn.call(name, json!({})).await;
            assert_eq!(r.is_error, Some(true), "{label}");
            let t = text(&r);
            assert!(
                t.contains("There is no tool") && t.contains("get_workspace"),
                "{label}: {t}"
            );
        }

        // Absent values are left out, never null.
        let r = conn.call("get_activity", json!({})).await;
        assert_eq!(r.is_error, Some(false), "{label}: {r:?}");
        let out = r.structured_content.clone().unwrap();
        assert_eq!(out, json!({"items": [], "total": 0}), "{label}");

        conn.close().await;

        // One run is one session: client, version, profile, times, counts.
        let (engine, person) = open_as_person(&options);
        let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
        assert_eq!(sessions["total"], 1, "{label}");
        let s = &sessions["items"][0];
        assert_eq!(s["id"], session.as_str(), "{label}");
        assert_eq!(s["name"], "Claude Code");
        assert_eq!(s["client"], "claude-code");
        assert_eq!(s["client_version"], "2.1.330");
        assert_eq!(s["profile"], "standard");
        assert!(s["ended_at"].is_string(), "{label}: {s}");
        assert_eq!(s["reads"], 2, "{label}: the reads that succeeded");
        assert_eq!(s["writes"], 0, "{label}");
        engine.close();
    }
}

#[tokio::test]
async fn legacy_initialize_negotiates_every_old_version() {
    let cases = [
        (ProtocolVersion::V_2024_11_05, ProtocolVersion::V_2024_11_05),
        (ProtocolVersion::V_2025_03_26, ProtocolVersion::V_2025_03_26),
        (ProtocolVersion::V_2025_06_18, ProtocolVersion::V_2025_06_18),
        (ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2025_11_25),
        // `initialize` belongs to the old era: asking it for 2026-07-28
        // gets the newest version that still has the handshake.
        (ProtocolVersion::V_2026_07_28, ProtocolVersion::V_2025_11_25),
    ];
    for (asked, expected) in cases {
        let (_dir, options) = workspace();
        let conn = connect(
            &options,
            ClientLifecycleMode::Initialize,
            asked.clone(),
            "codex",
        )
        .await;
        let info = conn.client.peer_info().unwrap();
        assert_eq!(info.protocol_version, expected, "asked {asked}");
        assert!(info.instructions.is_some());
        let r = conn.call("get_workspace", json!({})).await;
        assert_eq!(
            r.structured_content.unwrap()["caller"]["actor"]["name"],
            "Codex",
            "asked {asked}"
        );
        conn.close().await;
    }
}

#[tokio::test]
async fn changes_record_the_agent_and_can_be_reverted() {
    for (label, mode, asked, _) in lifecycles() {
        let (_dir, mut options) = workspace();
        options.toolsets = Some(vec![Toolset::Workspace, Toolset::Admin]);
        let conn = connect(&options, mode, asked, "claude-code").await;
        let mut names = golden_names("standard");
        names.push("update_settings".to_owned());
        assert_eq!(conn.tool_names().await, names, "{label}");

        let r = conn
            .call("update_settings", json!({"hours_per_day": 6}))
            .await;
        assert_eq!(r.is_error, Some(false), "{label}: {r:?}");
        let out = r.structured_content.clone().unwrap();
        assert_eq!(out["changed"], json!(["hours_per_day"]));
        let group = out["group"].as_str().unwrap().to_owned();
        assert!(
            text(&r).starts_with("Changed hours_per_day = 6 · group "),
            "{label}"
        );

        let r = conn
            .call("get_activity", json!({"session": "current"}))
            .await;
        let feed = r.structured_content.clone().unwrap();
        let entry = &feed["items"][0];
        assert_eq!(
            entry["actor"],
            json!({"kind": "agent", "name": "Claude Code"})
        );
        assert_eq!(entry["via"], "mcp");
        assert_eq!(entry["group"], group.as_str());
        let session = entry["session"].as_str().unwrap().to_owned();

        let r = conn
            .call("revert_changes", json!({"session": "current"}))
            .await;
        let out = r.structured_content.clone().unwrap();
        assert_eq!(out["reverted"], json!([group]), "{label}: {out}");
        conn.close().await;

        let (engine, person) = open_as_person(&options);
        let settings = engine.call(&person, "settings.get", Value::Null).unwrap();
        assert_eq!(settings["hours_per_day"], 8, "{label}: reverted");
        let feed = engine
            .call(&person, "history.feed", json!({"session": session}))
            .unwrap();
        assert_eq!(feed["total"], 2, "{label}: the change and its revert");
        let sessions = engine.call(&person, "agent.sessions", json!({})).unwrap();
        assert_eq!(sessions["items"][0]["writes"], 2, "{label}");
        engine.close();
    }
}

#[tokio::test]
async fn profiles_and_the_workspace_cap_decide_the_tools() {
    for (label, mode, asked, _) in lifecycles() {
        // read-only lists no write tools and refuses them.
        let (_dir, mut options) = workspace();
        options.profile = gk_api::Profile::ReadOnly;
        let conn = connect(&options, mode.clone(), asked.clone(), "claude-code").await;
        assert_eq!(
            conn.tool_names().await,
            golden_names("read-only"),
            "{label}"
        );
        let r = conn
            .call("revert_changes", json!({"session": "current"}))
            .await;
        assert_eq!(r.is_error, Some(true));
        assert!(
            text(&r).starts_with("Error (forbidden): Your profile (read-only)"),
            "{label}"
        );
        conn.close().await;

        // full is capped by the workspace setting, read on every call.
        let (_dir, mut options) = workspace();
        options.profile = gk_api::Profile::Full;
        let (engine, _) = open_as_person(&options);
        set_agent_cap(&engine, "read-only");
        let legacy = matches!(mode, ClientLifecycleMode::Initialize);
        let conn = connect(&options, mode, asked, "claude-code").await;
        assert_eq!(
            conn.tool_names().await,
            golden_names("read-only"),
            "{label}"
        );
        let r = conn.call("get_workspace", json!({})).await;
        assert_eq!(
            r.structured_content.unwrap()["caller"]["profile"],
            "read-only"
        );
        let r = conn
            .call("revert_changes", json!({"session": "current"}))
            .await;
        assert!(text(&r).starts_with("Error (forbidden)"), "{label}");
        set_agent_cap(&engine, "full");
        let r = conn.call("get_workspace", json!({})).await;
        assert_eq!(
            r.structured_content.unwrap()["caller"]["profile"],
            "full",
            "{label}"
        );
        if legacy {
            // The old era is told by a notification within a second or two.
            let mut names = conn.tool_names().await;
            for _ in 0..30 {
                if names == golden_names("full") {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                names = conn.tool_names().await;
            }
            assert_eq!(names, golden_names("full"), "{label}");
        }
        conn.close().await;
        engine.close();
    }
}

#[tokio::test]
async fn the_agent_name_option_names_a_client_without_one() {
    let (_dir, mut options) = workspace();
    options.agent_name = Some("Helper".to_owned());
    let conn = connect(
        &options,
        ClientLifecycleMode::Initialize,
        ProtocolVersion::V_2025_11_25,
        "  ",
    )
    .await;
    let r = conn.call("get_workspace", json!({})).await;
    assert_eq!(
        r.structured_content.unwrap()["caller"]["actor"]["name"],
        "Helper"
    );
    conn.close().await;
}
