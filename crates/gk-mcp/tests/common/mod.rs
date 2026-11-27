// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Shared set-up: a temporary workspace, the server running in this
//! process, and an `rmcp` client talking to it over an in-memory pipe.

#![allow(dead_code, unreachable_pub, clippy::unwrap_used)]

use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_mcp::Options;
use gk_store::WorkspaceLayout;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    ProtocolVersion,
};
use rmcp::service::RunningService;
use rmcp::{ClientLifecycleMode, ClientServiceExt, RoleClient};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::task::JoinHandle;

/// A temporary workspace and the options that open it.
pub fn workspace() -> (TempDir, Options) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Test");
    let mut options = Options::with_workspace(path.to_str().unwrap());
    // Never the developer's own personal settings.
    options.settings_file = Some(dir.path().join("settings.json"));
    (dir, options)
}

/// A running server and a client connected to it.
pub struct Connection {
    pub client: RunningService<RoleClient, ClientConfig>,
    pub server: JoinHandle<()>,
}

/// The lifecycles the contract runs in: label, client mode, the version the
/// client asks for, and the version it must end up with.
pub fn lifecycles() -> Vec<(
    &'static str,
    ClientLifecycleMode,
    ProtocolVersion,
    ProtocolVersion,
)> {
    vec![
        (
            "initialize 2025-11-25",
            ClientLifecycleMode::Initialize,
            ProtocolVersion::V_2025_11_25,
            ProtocolVersion::V_2025_11_25,
        ),
        (
            "discover 2026-07-28",
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
            ProtocolVersion::V_2026_07_28,
            ProtocolVersion::V_2026_07_28,
        ),
    ]
}

/// Starts the server for `options` in this process and connects a client
/// that calls itself `client_name` 2.1.330.
pub async fn connect(
    options: &Options,
    mode: ClientLifecycleMode,
    version: ProtocolVersion,
    client_name: &str,
) -> Connection {
    let server = gk_mcp::open(options).unwrap();
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let server = tokio::spawn(gk_mcp::serve(server, server_io));
    let mut config = ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new(client_name, "2.1.330"),
    );
    config.protocol_version = version;
    let client = config.serve_with_lifecycle(client_io, mode).await.unwrap();
    Connection { client, server }
}

impl Connection {
    /// Calls `tool` with `args`.
    pub async fn call(&self, tool: &str, args: Value) -> CallToolResult {
        let args = args.as_object().cloned().unwrap_or_default();
        let request = CallToolRequestParams::new(tool.to_owned()).with_arguments(args);
        self.client.call_tool(request).await.unwrap()
    }

    /// The names `tools/list` returns, in order.
    pub async fn tool_names(&self) -> Vec<String> {
        let tools = self.client.list_all_tools().await.unwrap();
        tools.iter().map(|t| t.name.to_string()).collect()
    }

    /// Closes the client and waits until the server has ended the session
    /// and closed the workspace.
    pub async fn close(self) {
        self.client.cancel().await.unwrap();
        self.server.await.unwrap();
    }
}

/// The text of a result's only content block.
pub fn text(result: &CallToolResult) -> String {
    assert_eq!(result.content.len(), 1, "one text block: {result:?}");
    result.content[0].as_text().unwrap().text.clone()
}

/// Opens the workspace as the person through the command line, for checks
/// after the server has closed.
pub fn open_as_person(options: &Options) -> (Engine, Context) {
    let layout: WorkspaceLayout = options.layout().unwrap();
    let engine = Engine::open(&layout, EngineConfig::new(Role::Cli)).unwrap();
    (engine, Context::user(Via::App))
}

/// Sets the workspace's "most an agent may do".
pub fn set_agent_cap(engine: &Engine, cap: &str) {
    engine
        .call(
            &Context::user(Via::App),
            "agent.policy.update",
            json!({ "max_profile": cap }),
        )
        .unwrap();
}

/// The golden tool list checked in with the catalog.
pub fn golden() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/reference/mcp-tools.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The tool names the golden list gives `profile` with the default toolsets.
pub fn golden_names(profile: &str) -> Vec<String> {
    golden()["profiles"][profile]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}
