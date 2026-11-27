// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Encrypted workspaces: encrypting, opening with and without the
//! passphrase, decrypting, snapshots and restores across the two kinds,
//! and two processes writing one encrypted workspace.
//!
//! Passphrases are kept per process, so every test uses its own.

#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

use gk_store::rusqlite::OptionalExtension;
use gk_store::{
    Encryption, Passphrase, Store, StoreConfig, StoreError, WorkspaceLayout, check_integrity,
    decrypt_workspace, encrypt_workspace, encryption_of, passphrase_opens, remember_passphrase,
    replace_database, vacuum_into,
};

const CHILD: &str = "GK_STORE_ENCRYPTED_CHILD";

fn pass(text: &str) -> Passphrase {
    Passphrase::new(text).unwrap()
}

fn set(store: &Store, key: &'static str, value: &'static str) {
    store
        .write("test", move |tx| {
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, 0)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

fn get(store: &Store, key: &'static str) -> Option<String> {
    store
        .read(|tx| {
            Ok::<_, StoreError>(
                tx.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()?,
            )
        })
        .unwrap()
}

fn workspace(dir: &tempfile::TempDir) -> WorkspaceLayout {
    let layout = WorkspaceLayout::new(dir.path().join("W"));
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    set(&store, "trash_days", "7");
    store.close();
    layout
}

#[test]
fn a_workspace_is_encrypted_and_decrypted_back() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );

    encrypt_workspace(&layout, &pass("round trip one")).unwrap();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Encrypted
    );
    let bytes = std::fs::read(layout.database()).unwrap();
    assert!(!bytes.windows(10).any(|w| w == b"trash_days"));
    assert!(
        check_integrity(&layout.database(), false)
            .unwrap()
            .is_empty()
    );
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    assert_eq!(get(&store, "trash_days").as_deref(), Some("7"));
    set(&store, "trash_days", "9");
    store.close();

    decrypt_workspace(&layout).unwrap();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    assert_eq!(get(&store, "trash_days").as_deref(), Some("9"));
    store.close();
    let leftovers: Vec<_> = std::fs::read_dir(layout.root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("converting"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn an_encrypted_workspace_needs_its_passphrase() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    encrypt_workspace(&layout, &pass("needs its own")).unwrap();
    let database = layout.database();
    assert!(!passphrase_opens(&database, &pass("not this one")).unwrap());
    assert!(passphrase_opens(&database, &pass("needs its own")).unwrap());
    // The changed passphrase opens it; the old one no longer does.
    encrypt_workspace(&layout, &pass("needs a new one")).unwrap();
    assert!(!passphrase_opens(&database, &pass("needs its own")).unwrap());
    assert!(passphrase_opens(&database, &pass("needs a new one")).unwrap());
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    assert_eq!(get(&store, "trash_days").as_deref(), Some("7"));
}

#[test]
fn a_process_without_the_passphrase_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    encrypt_workspace(&layout, &pass("only in the parent")).unwrap();
    // A process of its own knows no passphrase.
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_opens", "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("open|{}", layout.root().display()))
        .env_remove(gk_store::PASSPHRASE_ENV)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("result:required"), "{text}");
    // With a wrong one in the environment, it is the wrong one.
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_opens", "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("open|{}", layout.root().display()))
        .env(gk_store::PASSPHRASE_ENV, "a wrong one")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("result:wrong"), "{text}");
    assert!(!text.contains("ERROR CORE"), "{text}");
    // With the right one it opens.
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_opens", "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("open|{}", layout.root().display()))
        .env(gk_store::PASSPHRASE_ENV, "only in the parent")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("result:open:7"), "{text}");
}

/// The other process of the tests above and below.
#[test]
fn child_opens() {
    let Ok(spec) = std::env::var(CHILD) else {
        return;
    };
    let (what, root) = spec.split_once('|').unwrap();
    let layout = WorkspaceLayout::new(root);
    let result = gk_store::unlock_without_asking(&layout.database())
        .and_then(|_| Store::open(&layout, StoreConfig::default()));
    match (what, result) {
        ("open", Ok(store)) => println!("result:open:{}", get(&store, "trash_days").unwrap()),
        ("write", Ok(store)) => {
            for _ in 0..50 {
                store
                    .write("test", |tx| {
                        tx.execute(
                            "UPDATE settings SET value = CAST(value AS INTEGER) + 1
                             WHERE key = 'trash_days'",
                            [],
                        )?;
                        Ok::<_, StoreError>(())
                    })
                    .unwrap();
            }
            println!("result:written");
        }
        (_, Err(StoreError::PassphraseRequired(_))) => println!("result:required"),
        (_, Err(StoreError::WrongPassphrase(_))) => println!("result:wrong"),
        (_, Err(e)) => println!("result:error:{e}"),
        _ => unreachable!(),
    }
}

#[test]
fn two_processes_write_one_encrypted_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    encrypt_workspace(&layout, &pass("shared by two")).unwrap();
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    set(&store, "trash_days", "0");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_opens", "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("write|{}", layout.root().display()))
        .env(gk_store::PASSPHRASE_ENV, "shared by two")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..50 {
        store
            .write("test", |tx| {
                tx.execute(
                    "UPDATE settings SET value = CAST(value AS INTEGER) + 1
                     WHERE key = 'trash_days'",
                    [],
                )?;
                Ok::<_, StoreError>(())
            })
            .unwrap();
    }
    let lines: Vec<String> = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(Result::unwrap)
        .collect();
    assert!(child.wait().unwrap().success());
    assert!(
        lines.iter().any(|l| l.contains("result:written")),
        "{lines:?}"
    );
    assert_eq!(get(&store, "trash_days").as_deref(), Some("100"));
}

#[test]
fn converting_waits_for_the_workspace_to_be_closed() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    let refused = encrypt_workspace(&layout, &pass("while it is open"));
    assert!(matches!(refused, Err(StoreError::InUse(_))), "{refused:?}");
    // Nothing changed.
    assert_eq!(get(&store, "trash_days").as_deref(), Some("7"));
    set(&store, "trash_days", "8");
    store.close();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
}

#[test]
fn snapshots_of_an_encrypted_workspace_are_encrypted_and_restore_across_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    encrypt_workspace(&layout, &pass("snapshots too")).unwrap();
    let encrypted_copy = dir.path().join("encrypted.db");
    vacuum_into(&layout.database(), &encrypted_copy).unwrap();
    assert_eq!(
        encryption_of(&encrypted_copy).unwrap(),
        Encryption::Encrypted
    );
    assert!(check_integrity(&encrypted_copy, false).unwrap().is_empty());
    assert!(passphrase_opens(&encrypted_copy, &pass("snapshots too")).unwrap());

    // Back to plain; the encrypted snapshot still restores into it.
    decrypt_workspace(&layout).unwrap();
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    set(&store, "trash_days", "1");
    let plain_copy = dir.path().join("plain.db");
    vacuum_into(&layout.database(), &plain_copy).unwrap();
    replace_database(&layout, &encrypted_copy, Duration::from_secs(5)).unwrap();
    store.close();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Plain
    );
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    assert_eq!(get(&store, "trash_days").as_deref(), Some("7"));
    store.close();

    // Encrypted again with another passphrase; the plain snapshot restores
    // into it and the workspace stays encrypted.
    encrypt_workspace(&layout, &pass("snapshots again")).unwrap();
    replace_database(&layout, &plain_copy, Duration::from_secs(5)).unwrap();
    assert_eq!(
        encryption_of(&layout.database()).unwrap(),
        Encryption::Encrypted
    );
    assert!(passphrase_opens(&layout.database(), &pass("snapshots again")).unwrap());
    let store = Store::open(&layout, StoreConfig::default()).unwrap();
    assert_eq!(get(&store, "trash_days").as_deref(), Some("1"));
}

#[test]
fn a_remembered_passphrase_opens_copies_with_the_same_salt() {
    let dir = tempfile::tempdir().unwrap();
    let layout = workspace(&dir);
    let passphrase = pass("remembered once");
    encrypt_workspace(&layout, &passphrase).unwrap();
    remember_passphrase(&passphrase);
    let copy = dir.path().join("copy.db");
    vacuum_into(&layout.database(), &copy).unwrap();
    assert_eq!(
        gk_store::schema_version_of(&copy).unwrap(),
        gk_store::SCHEMA_VERSION
    );
}
