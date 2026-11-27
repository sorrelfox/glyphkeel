// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Shared set-up for tests that run `glyphkeel` as a program: a temporary
//! workspace, commands against it, and the agent server as its own process
//! spoken to over standard input and output.

#![allow(dead_code, unreachable_pub, clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use gk_engine::{Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// The zone every command uses, so "today" is the same day for all.
pub const ZONE: &str = "UTC";

/// The program, with its data folder inside `home` so a test never touches
/// the real one.
pub fn glyphkeel(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_glyphkeel"));
    cmd.env(gk_store::DATA_DIR_ENV, home.join("data").join("glyphkeel"))
        .env(
            gk_store::CONFIG_DIR_ENV,
            home.join("config").join("glyphkeel"),
        )
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env_remove("GLYPHKEEL_WORKSPACE")
        .env_remove(gk_store::PASSPHRASE_ENV)
        .env_remove("RUST_LOG");
    cmd
}

pub fn code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

/// A new, empty workspace in a temporary folder.
pub struct Workspace {
    pub dir: TempDir,
}

impl Workspace {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            &WorkspaceLayout::new(dir.path().join("Personal")),
            EngineConfig::new(Role::Cli),
        )
        .unwrap();
        engine.close();
        Self { dir }
    }

    pub fn home(&self) -> &Path {
        self.dir.path()
    }

    pub fn path(&self) -> PathBuf {
        self.dir.path().join("Personal")
    }

    /// Runs `glyphkeel --workspace <this> --tz UTC <args>`.
    pub fn run(&self, args: &[&str]) -> Output {
        let mut cmd = glyphkeel(self.home());
        cmd.arg("--workspace")
            .arg(self.path())
            .args(["--tz", ZONE])
            .args(args);
        cmd.output().unwrap()
    }

    /// Runs a command that must succeed and returns its standard output.
    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert_eq!(code(&out), 0, "{args:?}: {}", stderr(&out));
        stdout(&out)
    }

    /// Runs a command that must succeed and reads its JSON output.
    pub fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        serde_json::from_str(&self.ok(&all)).unwrap()
    }

    /// Runs a command that must fail with `expected` as its exit code and
    /// returns its standard error.
    pub fn fails(&self, expected: i32, args: &[&str]) -> String {
        let out = self.run(args);
        assert_eq!(code(&out), expected, "{args:?}: {}", stderr(&out));
        stderr(&out)
    }
}

/// The keys of a list's items, in order.
pub fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("a list: {list}"))
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

/// The titles of a list's items, in order.
pub fn titles(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("a list: {list}"))
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

/// A schedule's problems as (kind, task) pairs.
pub fn problems(schedule: &Value) -> Vec<(String, String)> {
    schedule["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["kind"].as_str().unwrap().to_owned(),
                p["task"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// A schedule row's dates.
pub fn dates(schedule: &Value, key: &str) -> (Value, Value) {
    let row = schedule["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == key)
        .unwrap_or_else(|| panic!("{key} in {schedule}"));
    (row["start_date"].clone(), row["due_date"].clone())
}

/// The agent server as its own process, `glyphkeel mcp`.
pub struct Agent {
    child: Child,
    input: Option<ChildStdin>,
    /// The server's messages, one a line, read on a thread of their own so
    /// that waiting for one can time out.
    output: Receiver<Value>,
    /// Notifications read while waiting for an answer.
    notifications: Vec<Value>,
    next_id: u64,
}

/// How long an answer may take before the test fails instead of hanging.
const ANSWER_TIME: Duration = Duration::from_secs(120);

impl Agent {
    /// Starts the server on `workspace` and goes through the handshake as
    /// a client called `name`.
    pub fn start(home: &Path, workspace: &Path, name: &str) -> Self {
        Self::start_with(home, workspace, name, &[])
    }

    /// As [`Agent::start`], with these environment variables set.
    pub fn start_with(home: &Path, workspace: &Path, name: &str, env: &[(&str, &str)]) -> Self {
        let mut child = glyphkeel(home)
            .envs(env.iter().copied())
            .arg("--workspace")
            .arg(workspace)
            .args(["--tz", ZONE, "mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (sink, output) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                let Ok(line) = line else { return };
                let message: Value = serde_json::from_str(&line).unwrap();
                if sink.send(message).is_err() {
                    return;
                }
            }
        });
        let mut agent = Self {
            child,
            input: Some(input),
            output,
            notifications: Vec::new(),
            next_id: 1,
        };
        agent.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": name, "version": "1.0" },
            }),
        );
        agent.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        agent
    }

    fn send(&mut self, message: &Value) {
        let mut line = message.to_string();
        line.push('\n');
        let input = self.input.as_mut().unwrap();
        input.write_all(line.as_bytes()).unwrap();
        input.flush().unwrap();
    }

    /// Sends a request and waits for its answer, skipping notifications.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let message = match self.output.recv_timeout(ANSWER_TIME) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => panic!("no answer to {method}"),
                Err(RecvTimeoutError::Disconnected) => panic!("the server closed"),
            };
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
            if message.get("id").is_none() {
                self.notifications.push(message);
            }
        }
    }

    /// The next notification called `method` within `wait`, if one comes.
    pub fn notification(&mut self, method: &str, wait: Duration) -> Option<Value> {
        if let Some(at) = self
            .notifications
            .iter()
            .position(|n| n["method"] == method)
        {
            return Some(self.notifications.remove(at));
        }
        let until = Instant::now() + wait;
        loop {
            let left = until.checked_duration_since(Instant::now())?;
            let message = self.output.recv_timeout(left).ok()?;
            if message["method"] == method {
                return Some(message);
            }
            if message.get("id").is_none() {
                self.notifications.push(message);
            }
        }
    }

    /// Calls a tool and returns the whole result, errors included.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }

    /// Calls a tool that must succeed and returns its structured output.
    pub fn ok(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.call(tool, arguments.clone());
        assert_eq!(result["isError"], false, "{tool} {arguments}: {result}");
        result["structuredContent"].clone()
    }

    /// Closes standard input and waits for the server to end.
    pub fn finish(mut self) {
        drop(self.input.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "the server ended with {status}");
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
