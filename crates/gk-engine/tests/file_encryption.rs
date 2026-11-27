// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The attached files, thumbnails and exports of an encrypted workspace:
//! sealed when the workspace is encrypted (backups too), read back through
//! the engine, exported encrypted and imported with the passphrase, given
//! a new key with a new passphrase, kept openable across a restore, and
//! plain again after decrypting.
//!
//! The passphrases a process knows are shared by its threads, so the whole
//! story is one test.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use gk_api::ErrorCode;
use gk_api::ops::common::Via;
use gk_engine::encryption::{self, Change};
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::{Passphrase, WorkspaceLayout};
use serde_json::{Value, json};
use tempfile::TempDir;

const START: i64 = 1_795_000_000_000;

/// The smallest PNG: one transparent pixel.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

fn config(clock: &Arc<AtomicI64>) -> EngineConfig {
    let tick = Arc::clone(clock);
    let mut config = EngineConfig::new(Role::App);
    config.clock = Arc::new(move || tick.fetch_add(1, Ordering::SeqCst));
    config
}

fn ctx() -> Context {
    Context::user(Via::App).with_confirmation(Confirmation::Given)
}

fn call(engine: &Engine, op: &str, input: Value) -> Value {
    engine
        .call(&ctx(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

/// Every stored file in the content-addressed folder `dir`.
fn blobs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for folder in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if folder.file_name().len() != 2 {
            continue;
        }
        for file in std::fs::read_dir(folder.path()).unwrap().flatten() {
            out.push(file.path());
        }
    }
    out.sort();
    out
}

fn sealed(path: &Path) -> bool {
    gk_store::looks_sealed(path).unwrap()
}

fn text_of(engine: &Engine, id: &str) -> String {
    call(engine, "file.read", json!({ "attachment": id }))["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Content that fills several chunks of a sealed file and a bit more.
fn large() -> Vec<u8> {
    (0..3_000_000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn attached_files_and_exports_of_an_encrypted_workspace() {
    if !gk_store::encryption_available() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(START));
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    let first = Passphrase::new("files first passphrase").unwrap();
    let second = Passphrase::new("files second passphrase").unwrap();

    // A plain workspace with a text file, an image, a large file and a backup.
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    call(
        &engine,
        "project.create",
        json!({ "name": "Files", "key": "ZZ" }),
    );
    call(
        &engine,
        "task.create",
        json!({ "project": "ZZ", "title": "One" }),
    );
    let note = call(
        &engine,
        "file.attach",
        json!({ "task": "ZZ-1", "name": "note.txt", "data_base64": "c2VjcmV0IG5vdGU=" }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let image = call(
        &engine,
        "file.attach",
        json!({ "task": "ZZ-1", "name": "dot.png", "data_base64": PNG }),
    )["file"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let big_path = dir.path().join("big.bin");
    std::fs::write(&big_path, large()).unwrap();
    let big = call(
        &engine,
        "file.attach",
        json!({ "task": "ZZ-1", "path": big_path.to_string_lossy() }),
    )["file"]
        .clone();
    let big_sha = big["sha256"].as_str().unwrap().to_owned();
    call(&engine, "file.thumbnail", json!({ "attachment": image }));
    let before = call(&engine, "backup.create", json!({}))["name"]
        .as_str()
        .unwrap()
        .to_owned();
    engine.close();
    let backups = gk_engine::backup::default_folder(&layout);
    assert_eq!(blobs(&layout.files_dir()).len(), 3);
    assert!(blobs(&layout.files_dir()).iter().all(|p| !sealed(p)));

    // Encrypting seals every stored file, the backups' files too, and the
    // backups keep the file key.
    let changed = encryption::change(
        &layout,
        None,
        &Change::Encrypt(first.clone()),
        START + 10_000,
        "t",
    )
    .unwrap();
    assert_eq!(changed.files_failed, 0);
    assert!(changed.files_converted >= 3, "{changed:?}");
    for blob in blobs(&layout.files_dir()) {
        assert!(sealed(&blob), "{blob:?}");
    }
    let backup_blobs = blobs(&backups.join("blobs"));
    assert_eq!(backup_blobs.len(), 3);
    for blob in &backup_blobs {
        assert!(sealed(blob), "{blob:?}");
    }
    assert!(!layout.thumbs_dir().exists());
    let before_db = backups.join(format!("{before}.db"));
    assert_eq!(gk_store::file_keys_of(&before_db).unwrap().len(), 1);

    // Through the engine everything reads as before.
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(text_of(&engine, &note), "secret note");
    let mut content = gk_store::open_file(&layout, &big_sha).unwrap();
    let mut read = Vec::new();
    std::io::Read::read_to_end(&mut content, &mut read).unwrap();
    assert!(read == large(), "the large file reads back");
    let thumb = call(&engine, "file.thumbnail", json!({ "attachment": image }));
    let thumb_path = PathBuf::from(thumb["path"].as_str().unwrap());
    assert!(sealed(&thumb_path));
    let checked = call(&engine, "backup.verify", json!({ "name": before }));
    assert_eq!(checked["ok"], true, "{checked}");
    // A new file is sealed as it is stored.
    let added = call(
        &engine,
        "file.attach",
        json!({ "task": "ZZ-1", "name": "later.txt", "data_base64": "bGF0ZXI=" }),
    )["file"]
        .clone();
    let added_path = gk_store::file_path(&layout, added["sha256"].as_str().unwrap()).unwrap();
    assert!(sealed(&added_path));
    assert_eq!(text_of(&engine, added["id"].as_str().unwrap()), "later");

    // An export into the exports folder is encrypted; one to a folder the
    // person chooses is not, unless asked, and then refused.
    let export = call(&engine, "transfer.export", json!({}));
    assert_eq!(export["encrypted"], true, "{export}");
    let export_path = PathBuf::from(export["path"].as_str().unwrap());
    assert!(export_path.starts_with(layout.exports_dir()));
    assert!(sealed(&export_path));
    let folder = dir.path().join("plain-export");
    let plain = call(
        &engine,
        "transfer.export",
        json!({ "path": folder.to_string_lossy(), "format": "folder" }),
    );
    assert!(plain.get("encrypted").is_none(), "{plain}");
    assert_eq!(
        std::fs::read(folder.join("files").join(&big_sha)).unwrap(),
        large()
    );
    let err = engine
        .call(
            &ctx(),
            "transfer.export",
            json!({ "path": dir.path().join("f2").to_string_lossy(), "format": "folder", "encrypt": true }),
        )
        .unwrap_err();
    assert_eq!(err.details["message_id"], "transfer-encrypt-folder");
    let wiki = call(&engine, "wiki.export", json!({ "project": "ZZ" }));
    assert_eq!(wiki["encrypted"], true, "{wiki}");
    engine.close();

    // Another workspace imports it only with the passphrase.
    let other = WorkspaceLayout::new(dir.path().join("Other"));
    let target = Engine::open(&other, config(&clock)).unwrap();
    gk_store::forget_passphrases();
    let import = json!({ "path": export_path.to_string_lossy() });
    let err = target
        .call(&ctx(), "transfer.import", import.clone())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden, "{err:?}");
    assert_eq!(err.details["passphrase"], "export");
    gk_store::remember_passphrase(&Passphrase::new("not the one").unwrap());
    let err = target
        .call(&ctx(), "transfer.import", import.clone())
        .unwrap_err();
    assert_eq!(err.details["passphrase"], "export");
    // A changed byte is found.
    let mut bytes = std::fs::read(&export_path).unwrap();
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0x20;
    let tampered = dir.path().join("tampered.zip");
    std::fs::write(&tampered, bytes).unwrap();
    gk_store::remember_passphrase(&first);
    let err = target
        .call(
            &ctx(),
            "transfer.import",
            json!({ "path": tampered.to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
    let imported = call(&target, "transfer.import", import);
    assert_eq!(imported["projects"][0]["key"], "ZZ", "{imported}");
    let other_blobs = blobs(&other.files_dir());
    assert_eq!(other_blobs.len(), 4);
    // The other workspace is plain: its files are too.
    assert!(other_blobs.iter().all(|p| !sealed(p)));
    target.close();

    // A new passphrase brings a new file key; older backups keep theirs.
    let key_before = gk_store::file_keys_of(&layout.database()).unwrap();
    encryption::change(
        &layout,
        None,
        &Change::Encrypt(second.clone()),
        START + 20_000,
        "t",
    )
    .unwrap();
    let key_after = gk_store::file_keys_of(&layout.database()).unwrap();
    assert_eq!(key_after.len(), 2);
    assert_ne!(key_after[0], key_before[0]);
    assert_eq!(key_after[1], key_before[0]);
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(text_of(&engine, &note), "secret note");
    let checked = call(&engine, "backup.verify", json!({ "name": before }));
    assert_eq!(checked["ok"], true, "{checked}");

    // Restoring the old backup: every file still opens.
    call(&engine, "backup.restore", json!({ "name": before }));
    drop(engine);
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(text_of(&engine, &note), "secret note");
    for blob in blobs(&layout.files_dir()) {
        assert!(sealed(&blob), "{blob:?}");
    }
    engine.close();

    // Decrypting makes every file plain and drops the keys.
    let changed = encryption::change(&layout, None, &Change::Decrypt, START + 30_000, "t").unwrap();
    assert_eq!(changed.files_failed, 0);
    for blob in blobs(&layout.files_dir()) {
        assert!(!sealed(&blob), "{blob:?}");
    }
    assert!(
        gk_store::file_keys_of(&layout.database())
            .unwrap()
            .is_empty()
    );
    let engine = Engine::open(&layout, config(&clock)).unwrap();
    assert_eq!(text_of(&engine, &note), "secret note");
    let last = dir.path().join("last.zip");
    let export = call(
        &engine,
        "transfer.export",
        json!({ "path": last.to_string_lossy() }),
    );
    assert!(export.get("encrypted").is_none(), "{export}");
    assert!(!sealed(&last));
    let err = engine
        .call(&ctx(), "transfer.export", json!({ "encrypt": true }))
        .unwrap_err();
    assert_eq!(
        err.details["message_id"],
        "transfer-encrypt-plain-workspace"
    );
    engine.close();
}
