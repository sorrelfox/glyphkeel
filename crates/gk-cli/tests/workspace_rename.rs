// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Renaming a workspace while other processes use it: a running agent
//! server and a command in another process keep it from moving, and say
//! why; once they let go, the rename goes through.

#![allow(clippy::unwrap_used)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{Agent, code, glyphkeel, stderr, stdout};
use gk_engine::{Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::json;

fn folder(home: &Path, name: &str) -> PathBuf {
    home.join("data")
        .join("glyphkeel")
        .join("workspaces")
        .join(name)
}

fn run(home: &Path, args: &[&str]) -> Output {
    glyphkeel(home).args(args).output().unwrap()
}

#[test]
fn a_workspace_in_use_by_the_agent_server_or_another_command_is_not_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let made = run(home, &["workspace", "create", "Work"]);
    assert_eq!(code(&made), 0, "{}", stderr(&made));
    let work = folder(home, "Work");

    // The agent server, as its own process, holds the workspace open.
    let mut agent = Agent::start(home, &work, "Helper");
    agent.ok("create_project", json!({ "name": "Team", "key": "TM" }));
    let refused = run(home, &["workspace", "rename", "Work", "Job"]);
    assert_eq!(code(&refused), 3, "{}", stdout(&refused));
    let why = stderr(&refused);
    assert!(why.contains("agent server"), "{why}");
    assert!(work.is_dir());
    assert!(!folder(home, "Job").exists(), "nothing moved");
    // The server goes on working in the folder it opened.
    agent.ok(
        "create_task",
        json!({ "project": "TM", "title": "Still here" }),
    );
    agent.finish();

    // A command that is still running elsewhere does the same.
    let engine = Engine::open(&WorkspaceLayout::new(&work), EngineConfig::new(Role::Cli)).unwrap();
    let refused = run(home, &["workspace", "rename", "Work", "Job"]);
    assert_eq!(code(&refused), 3, "{}", stderr(&refused));
    engine.close();

    // Nobody uses it any more: the rename goes through, with the tasks.
    let renamed = run(home, &["workspace", "rename", "Work", "Job"]);
    assert_eq!(code(&renamed), 0, "{}", stderr(&renamed));
    assert!(!work.exists());
    let listed = run(home, &["--json", "--workspace", "Job", "search", "Still"]);
    assert_eq!(code(&listed), 0, "{}", stderr(&listed));
    assert!(
        stdout(&listed).contains("Still here"),
        "{}",
        stdout(&listed)
    );
}

#[test]
fn a_workspace_open_in_the_app_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert_eq!(code(&run(home, &["workspace", "create", "Work"])), 0);
    let app = Engine::open(
        &WorkspaceLayout::new(folder(home, "Work")),
        EngineConfig::new(Role::App),
    )
    .unwrap();
    let refused = run(home, &["workspace", "rename", "Work", "Job"]);
    assert_eq!(code(&refused), 3);
    assert!(
        stderr(&refused).contains("open in the Glyphkeel app"),
        "{}",
        stderr(&refused)
    );
    app.close();
    assert_eq!(code(&run(home, &["workspace", "rename", "Work", "Job"])), 0);
}
