// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An encrypted workspace from the command line and the agent server:
//! `workspace encrypt` with the passphrase piped in, commands that need the
//! passphrase from the environment, the agent server opening it, and
//! `workspace decrypt`.

#![allow(clippy::unwrap_used)]

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use common::{Agent, code, glyphkeel, stderr, stdout};
use gk_store::{Encryption, encryption_of};
use serde_json::{Value, json};

const PASSPHRASE: &str = "a passphrase for the command line";

fn folder(home: &Path, name: &str) -> PathBuf {
    home.join("data")
        .join("glyphkeel")
        .join("workspaces")
        .join(name)
}

fn run(home: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    glyphkeel(home)
        .envs(env.iter().copied())
        .args(args)
        .output()
        .unwrap()
}

fn piped(home: &Path, args: &[&str], input: &str) -> Output {
    let mut child = glyphkeel(home)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn an_encrypted_workspace_from_the_command_line_and_the_agent_server() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let w = ["--workspace", "Work"];
    assert_eq!(code(&run(home, &[], &["workspace", "create", "Work"])), 0);
    let made = run(
        home,
        &[],
        &[
            w[0], w[1], "project", "create", "--name", "Team", "--key", "TM",
        ],
    );
    assert_eq!(code(&made), 0, "{}", stderr(&made));
    let database = folder(home, "Work").join(gk_store::DATABASE_FILE);

    // Encrypted with the passphrase piped in.
    let out = piped(
        home,
        &["--json", w[0], w[1], "workspace", "encrypt"],
        &format!("{PASSPHRASE}\n"),
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let value: Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(value["encrypted"], true, "{value}");
    assert_eq!(encryption_of(&database).unwrap(), Encryption::Encrypted);

    // Without the passphrase, and without a terminal to ask on, a command
    // fails with the reason and a hint.
    let refused = run(home, &[], &[w[0], w[1], "project", "list"]);
    assert_eq!(code(&refused), 3, "{}", stderr(&refused));
    assert!(
        stderr(&refused).contains("passphrase"),
        "{}",
        stderr(&refused)
    );
    let wrong = run(
        home,
        &[(gk_store::PASSPHRASE_ENV, "not it")],
        &[w[0], w[1], "project", "list"],
    );
    assert_eq!(code(&wrong), 3, "{}", stderr(&wrong));
    assert!(
        stderr(&wrong).contains("does not open"),
        "{}",
        stderr(&wrong)
    );

    // With it in the environment, commands work.
    let env = [(gk_store::PASSPHRASE_ENV, PASSPHRASE)];
    let task = run(
        home,
        &env,
        &[
            w[0],
            w[1],
            "task",
            "create",
            "--project",
            "TM",
            "--title",
            "Kept secret",
        ],
    );
    assert_eq!(code(&task), 0, "{}", stderr(&task));

    // The agent server opens it the same way.
    let mut agent = Agent::start_with(home, &folder(home, "Work"), "Helper", &env);
    let found = agent.ok("search", json!({ "text": "secret" }));
    assert!(found.to_string().contains("Kept secret"), "{found}");
    agent.finish();

    // Decrypting needs the passphrase too, and leaves a plain workspace.
    let out = run(home, &env, &[w[0], w[1], "workspace", "decrypt"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(encryption_of(&database).unwrap(), Encryption::Plain);
    let listed = run(home, &[], &["--json", w[0], w[1], "search", "secret"]);
    assert_eq!(code(&listed), 0, "{}", stderr(&listed));
    assert!(stdout(&listed).contains("Kept secret"));
}

#[test]
fn an_empty_passphrase_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    assert_eq!(code(&run(home, &[], &["workspace", "create", "Work"])), 0);
    let out = piped(home, &["--workspace", "Work", "workspace", "encrypt"], "\n");
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert_eq!(
        encryption_of(&folder(home, "Work").join(gk_store::DATABASE_FILE)).unwrap(),
        Encryption::Plain
    );
}
