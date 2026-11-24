// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The query corpus over data set S through all three doors: the app's
//! operation (`task.search` on the engine), `glyphkeel task list` and the
//! agent server's `search_tasks`. Each query must find the same tasks in
//! the same order everywhere. The expected results themselves are checked
//! in the engine's own corpus test.

#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// The zone every door uses: "today" must be the same day for all three.
const ZONE: &str = "UTC";

fn corpus() -> Vec<String> {
    include_str!("../../gk-engine/tests/corpus/queries.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn workspace(dir: &Path) -> PathBuf {
    dir.join("Personal")
}

/// The program, with its data and settings inside `home`.
fn glyphkeel(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_glyphkeel"));
    cmd.env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env_remove("GLYPHKEEL_WORKSPACE")
        .env_remove("RUST_LOG");
    cmd
}

fn keys(list: &Value) -> Vec<String> {
    assert!(list.get("next_cursor").is_none(), "one page: {list}");
    list["items"]
        .as_array()
        .unwrap_or_else(|| panic!("a list: {list}"))
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

/// The agent server as its own process, spoken to over standard input
/// and output.
struct Agent {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Agent {
    fn start(home: &Path) -> Self {
        let mut child = glyphkeel(home)
            .arg("--workspace")
            .arg(workspace(home))
            .args(["--tz", ZONE, "mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut agent = Self {
            child,
            input,
            output,
            next_id: 1,
        };
        agent.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "Corpus check", "version": "1.0" },
            }),
        );
        agent.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        agent
    }

    fn send(&mut self, message: &Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.input.write_all(line.as_bytes()).unwrap();
        self.input.flush().unwrap();
    }

    /// Sends a request and waits for its answer, skipping notifications.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "the server closed"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }

    fn search_tasks(&mut self, query: &str) -> Vec<String> {
        let result = self.request(
            "tools/call",
            json!({ "name": "search_tasks", "arguments": { "query": query, "limit": 200 } }),
        );
        assert_eq!(result["isError"], false, "{query}: {result}");
        keys(&result["structuredContent"])
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_corpus_gives_the_same_results_through_every_door() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let layout = WorkspaceLayout::new(workspace(home));
    let archive = home.join("seed-S");
    gk_seed::generate(gk_seed::DataSet::Small, 1)
        .unwrap()
        .write_to(&archive)
        .unwrap();
    let mut config = EngineConfig::new(Role::Cli);
    config.time_zone = Some(ZONE.to_owned());
    let engine = Engine::open(&layout, config).unwrap();
    let app = Context::user(Via::App);
    engine
        .call(
            &app,
            "transfer.import",
            json!({ "path": archive.to_string_lossy() }),
        )
        .unwrap();

    let in_app = |query: &str| {
        keys(
            &engine
                .call(&app, "task.search", json!({ "query": query, "limit": 200 }))
                .unwrap_or_else(|e| panic!("{query}: {e:?}")),
        )
    };
    let in_cli = |query: &str| {
        let out = glyphkeel(home)
            .arg("--workspace")
            .arg(workspace(home))
            .args(["--json", "--tz", ZONE, "task", "list", "--limit", "200"])
            // `=` keeps a query that starts with `-` a value.
            .arg(format!("--query={query}"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{query}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        keys(&serde_json::from_slice(&out.stdout).unwrap())
    };
    let mut agent = Agent::start(home);

    let queries = corpus();
    assert!(queries.len() >= 50);
    let mut found_any = 0;
    for query in &queries {
        // The doors run one after the other on the real clock; should the
        // day change in between, the query runs once more.
        let mut agree = false;
        for _ in 0..2 {
            let app_keys = in_app(query);
            let cli_keys = in_cli(query);
            let agent_keys = agent.search_tasks(query);
            if app_keys == cli_keys && app_keys == agent_keys {
                found_any += usize::from(!app_keys.is_empty());
                agree = true;
                break;
            }
            eprintln!("{query}: app {app_keys:?}, cli {cli_keys:?}, agent {agent_keys:?}");
        }
        assert!(agree, "{query}: the doors disagree");
    }
    // Most of the corpus finds something, so agreeing means something.
    assert!(
        found_any * 2 > queries.len(),
        "{found_any} queries found tasks"
    );
    drop(agent);
    engine.close();
}
