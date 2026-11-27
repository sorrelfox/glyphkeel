// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! One file format for encrypted workspaces on every system.
//!
//! The Linux build encrypts with the system's OpenSSL, the Windows build
//! with LibTomCrypt. The files in `cipher/` were encrypted once, one by
//! each build, and are never regenerated: every build must open both, with
//! the passphrase as SQLCipher derives the key and with the key this
//! program derives itself, and read back what was written.
//!
//! A new sample is written by the ignored test at the end:
//! `GK_CIPHER_SAMPLE=<file> cargo test -p gk-store --test cipher_format -- --ignored`.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use gk_store::rusqlite::Connection;
use gk_store::{Encryption, Passphrase, encryption_of, open_read_only, passphrase_opens};

/// The passphrase of every sample.
const PASSPHRASE: &str = "Glyphkeel sample passphrase";

/// The text in a sample's `filler` row, long enough to span pages.
fn filler() -> String {
    "the quick brown fox jumps over the lazy dog. ".repeat(400)
}

const SAMPLES: [(&str, &[u8]); 2] = [
    ("linux", include_bytes!("cipher/linux.db")),
    ("windows", include_bytes!("cipher/windows.db")),
];

/// The sample `bytes` copied into a new folder.
fn copy(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.db");
    std::fs::write(&path, bytes).unwrap();
    (dir, path)
}

/// Checks what `write_sample` wrote.
fn check_contents(conn: &Connection, system: &str) {
    let made_on: String = conn
        .query_row("SELECT value FROM sample WHERE key = 'made_on'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(made_on, system);
    let text: String = conn
        .query_row("SELECT value FROM sample WHERE key = 'filler'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(text, filler());
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

#[test]
fn files_encrypted_on_each_system_open_with_sqlciphers_key() {
    if !gk_store::encryption_available() {
        return;
    }
    for (system, bytes) in SAMPLES {
        let (_dir, path) = copy(bytes);
        assert_eq!(encryption_of(&path).unwrap(), Encryption::Encrypted);
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "key", PASSPHRASE).unwrap();
        check_contents(&conn, system);
    }
}

#[test]
fn files_encrypted_on_each_system_open_with_this_programs_key() {
    if !gk_store::encryption_available() {
        return;
    }
    for (system, bytes) in SAMPLES {
        let (_dir, path) = copy(bytes);
        let wrong = Passphrase::new("not the sample passphrase").unwrap();
        assert!(!passphrase_opens(&path, &wrong).unwrap(), "{system}");
        let right = Passphrase::new(PASSPHRASE).unwrap();
        assert!(passphrase_opens(&path, &right).unwrap(), "{system}");
        check_contents(&open_read_only(&path).unwrap(), system);
    }
}

/// Writes a new sample to the file `GK_CIPHER_SAMPLE` names, made on this
/// system with SQLCipher's defaults.
#[test]
#[ignore = "writes a new sample file; run by hand"]
fn write_sample() {
    let target = std::env::var_os("GK_CIPHER_SAMPLE").expect("GK_CIPHER_SAMPLE names the file");
    let target = Path::new(&target);
    let _ = std::fs::remove_file(target);
    let system = if cfg!(windows) { "windows" } else { "linux" };
    let conn = Connection::open(target).unwrap();
    conn.pragma_update(None, "key", PASSPHRASE).unwrap();
    conn.execute_batch("CREATE TABLE sample (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
        .unwrap();
    conn.execute(
        "INSERT INTO sample (key, value) VALUES ('made_on', ?1), ('filler', ?2)",
        [system.to_owned(), filler()],
    )
    .unwrap();
    drop(conn);
    assert_eq!(encryption_of(target).unwrap(), Encryption::Encrypted);
}
