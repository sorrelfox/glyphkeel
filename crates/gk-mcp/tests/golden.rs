// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The tool list the server serves is the checked-in golden list
//! (`docs/reference/mcp-tools.json`): the same tools per profile, in the same
//! order, with the same names, descriptions, schemas and annotations.

#![allow(clippy::unwrap_used)]

mod common;

use common::{connect, golden, golden_names, workspace};
use gk_api::{Profile, Toolset};
use rmcp::ClientLifecycleMode;
use rmcp::model::ProtocolVersion;
use serde_json::Value;

/// The golden entry of tool `name`, without the fields only the golden
/// list has.
fn golden_tool(golden: &Value, name: &str) -> Value {
    let mut tool = golden["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} is not in the golden list"))
        .clone();
    let object = tool.as_object_mut().unwrap();
    object.remove("toolset");
    object.remove("covers");
    tool
}

#[tokio::test]
async fn the_served_tool_list_matches_the_golden_list() {
    let golden = golden();
    for profile in Profile::ALL {
        let (_dir, mut options) = workspace();
        options.profile = profile;
        let (engine, _) = common::open_as_person(&options);
        common::set_agent_cap(&engine, "full");
        engine.close();

        let conn = connect(
            &options,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
            ProtocolVersion::V_2026_07_28,
            "golden-check",
        )
        .await;
        let tools = conn.client.list_all_tools().await.unwrap();
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert_eq!(names, golden_names(profile.as_str()), "{profile}");
        for tool in &tools {
            let served = serde_json::to_value(tool).unwrap();
            assert_eq!(
                served,
                golden_tool(&golden, &tool.name),
                "{profile}: {}",
                tool.name
            );
        }
        conn.close().await;
    }
}

#[tokio::test]
async fn every_toolset_lists_every_golden_tool_in_order() {
    let golden = golden();
    let (_dir, mut options) = workspace();
    options.profile = Profile::Full;
    options.toolsets = Some(Toolset::ALL.to_vec());
    let (engine, _) = common::open_as_person(&options);
    common::set_agent_cap(&engine, "full");
    engine.close();
    let conn = connect(
        &options,
        ClientLifecycleMode::Initialize,
        ProtocolVersion::V_2025_11_25,
        "golden-check",
    )
    .await;
    let tools = conn.client.list_all_tools().await.unwrap();
    let served: Vec<Value> = tools
        .iter()
        .map(|t| serde_json::to_value(t).unwrap())
        .collect();
    let all: Vec<Value> = golden["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| golden_tool(&golden, t["name"].as_str().unwrap()))
        .collect();
    assert_eq!(served, all);
    conn.close().await;
}

#[test]
fn the_golden_list_is_the_catalogs() {
    // Guards against a stale checked-in file; `cargo xtask gen` rewrites it.
    assert_eq!(golden(), gk_api::mcp::golden_document());
}
