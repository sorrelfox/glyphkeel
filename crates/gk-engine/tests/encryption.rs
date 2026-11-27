// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An encrypted workspace through the engine: encrypting with a backup
//! first, the backups encrypted too, opening without and with the
//! passphrase, changing it, restoring a backup that needs another one, and
//! decrypting.
//!
//! The passphrases a process knows are shared by its threads, so the whole
//! story is one test.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::encryption::{self, Change};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::{Encryption, Passphrase, WorkspaceLayout, encryption_of};
use serde_json::{Value, json};
use tempfile::TempDir;

const START: i64 = 1_795_000_000_000;

fn config(clock: &Arc<AtomicI64>) -> EngineConfig {
    let tick = Arc::clone(clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    config
}

fn call(engine: &Engine, op: &str, input: Value) -> Value {
    engine
        .call(
            &Context::user(Via::Cli).with_confirmation(Confirmation::Given),
            op,
            input,
        )
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn title(engine: &Engine) -> String {
    call(engine, "task.get", json!({ "keys": ["TM-1"] }))["items"][0]["title"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn backup_files(layout: &WorkspaceLayout) -> Vec<std::path::PathBuf> {
    let folder = gk_engine::backup::default_folder(layout);
    std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "db"))
        .collect()
}

#[test]
fn an_encrypted_workspace_through_the_engine() {
    if !gk_store::encryption_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    let first = Passphrase::new("engine first passphrase").unwrap();
    let second = Passphrase::new("engine second passphrase").unwrap();

    // A plain workspace with a backup.
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    call(
        &engine,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(
        &engine,
        "task.create",
        json!({ "project": "TM", "title": "Before" }),
    );
    let before = call(&engine, "backup.create", json!({}))["name"]
        .as_str()
        .unwrap()
        .to_owned();
    let status = encryption::status(&layout).unwrap();
    assert!(status.available && !status.encrypted);
    engine.close();

    // Encrypting makes a backup first and encrypts every backup.
    let changed = encryption::change(
        &layout,
        None,
        &Change::Encrypt(first.clone()),
        START + 10_000,
        "test",
    )
    .unwrap();
    assert_eq!(changed.backups_left_plain, 0);
    assert_eq!(changed.backup.kind.as_str(), "safety");
    assert!(encryption::status(&layout).unwrap().encrypted);
    let files = backup_files(&layout);
    assert_eq!(files.len(), 2, "{files:?}");
    for file in &files {
        assert_eq!(
            encryption_of(file).unwrap(),
            Encryption::Encrypted,
            "{file:?}"
        );
    }

    // Without a passphrase it does not open; the error says why.
    gk_store::forget_passphrases();
    let err = Engine::open(&layout, config(&clock)).unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden, "{err:?}");
    assert_eq!(err.details["passphrase"], "required");
    assert_eq!(err.details["message_id"], "storage-passphrase-required");
    gk_store::remember_passphrase(&Passphrase::new("not the passphrase").unwrap());
    let err = Engine::open(&layout, config(&clock)).unwrap_err();
    assert_eq!(err.details["passphrase"], "wrong");

    // With it, everything is there and works.
    gk_store::remember_passphrase(&first);
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(title(&engine), "Before");
    call(
        &engine,
        "task.update",
        json!({ "key": "TM-1", "title": "After" }),
    );
    let made = call(&engine, "backup.create", json!({}));
    let name = made["name"].as_str().unwrap();
    let (path, _) = (engine.backups().root().join(format!("{name}.db")), ());
    assert_eq!(encryption_of(&path).unwrap(), Encryption::Encrypted);
    let checked = call(&engine, "backup.verify", json!({ "name": name }));
    assert_eq!(checked["ok"], true, "{checked}");
    engine.close();

    // A new passphrase; the older backups keep the one they were made with.
    encryption::change(
        &layout,
        None,
        &Change::Encrypt(second.clone()),
        START + 20_000,
        "test",
    )
    .unwrap();
    gk_store::forget_passphrases();
    gk_store::remember_passphrase(&second);
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(title(&engine), "After");
    // Restoring a backup made with the first passphrase asks for it.
    let err = engine
        .call(
            &Context::user(Via::Cli).with_confirmation(Confirmation::Given),
            "backup.restore",
            json!({ "name": before }),
        )
        .unwrap_err();
    assert_eq!(err.details["passphrase"], "wrong", "{err:?}");
    gk_store::remember_passphrase(&first);
    call(&engine, "backup.restore", json!({ "name": before }));
    drop(engine);
    // The restored workspace stays encrypted with the second passphrase.
    assert!(gk_store::passphrase_opens(&layout.database(), &second).unwrap());
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(title(&engine), "Before");
    engine.close();

    // Decrypting: plain again, and an encrypted backup still restores.
    encryption::change(&layout, None, &Change::Decrypt, START + 30_000, "test").unwrap();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    call(&engine, "backup.restore", json!({ "name": name }));
    drop(engine);
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
    // A plain workspace has nothing to decrypt.
    let err =
        encryption::change(&layout, None, &Change::Decrypt, START + 40_000, "test").unwrap_err();
    assert_eq!(err.details["message_id"], "encryption-not-encrypted");
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(title(&engine), "After");
}

#[test]
fn encrypting_refuses_while_the_workspace_is_open() {
    if !gk_store::encryption_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let layout = WorkspaceLayout::new(dir.path().join("Open"));
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    let err = encryption::change(
        &layout,
        None,
        &Change::Encrypt(Passphrase::new("refused while open").unwrap()),
        START + 1,
        "test",
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Busy, "{err:?}");
    assert_eq!(err.details["message_id"], "storage-in-use");
    drop(engine);
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
}

/// A build without SQLCipher says so, refuses to encrypt and
/// keeps working on plain workspaces.
#[test]
fn a_build_that_cannot_encrypt_says_so() {
    if gk_store::encryption_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let layout = WorkspaceLayout::new(dir.path().join("Plain"));
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    call(
        &engine,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    engine.close();
    let status = encryption::status(&layout).unwrap();
    assert!(!status.available && !status.encrypted);
    let err = encryption::change(
        &layout,
        None,
        &Change::Encrypt(Passphrase::new("not on this build").unwrap()),
        START + 1,
        "test",
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Storage, "{err:?}");
    assert_eq!(err.details["message_id"], "storage-encryption-unavailable");
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    call(
        &engine,
        "task.create",
        json!({ "project": "TM", "title": "Still plain" }),
    );
    engine.close();
}
