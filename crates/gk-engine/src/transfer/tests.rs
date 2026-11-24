// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Hostile archives: every kind of damaged or crafted archive is refused
//! with its own message, and the workspace it was imported into is left
//! exactly as it was (its records and its stored files).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_api::ops::common::Via;
use gk_api::ops::transfer::ImportInput;
use gk_store::WorkspaceLayout;
use gk_store::rusqlite::types::ValueRef;
use serde_json::{Value, json};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

use super::{Limits, import, no_progress};
use crate::context::Context;
use crate::engine::{Engine, EngineConfig, Role};

fn me() -> Context {
    Context::user(Via::Cli)
}

fn open(dir: &Path, name: &str) -> Engine {
    Engine::open(
        &WorkspaceLayout::new(dir.join(name)),
        EngineConfig::new(Role::Cli),
    )
    .unwrap()
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

/// A workspace with one project of every kind of record, exported as a
/// folder; and a second workspace with data of its own to import into.
struct Setup {
    dir: TempDir,
    archive: PathBuf,
    target: Engine,
}

fn setup() -> Setup {
    let dir = TempDir::new().unwrap();
    let source = open(dir.path(), "source");
    call(
        &source,
        "project.create",
        json!({ "name": "Tasks", "key": "TM" }),
    );
    call(
        &source,
        "task.create",
        json!({ "project": "TM", "title": "First", "description": "Some **text**." }),
    );
    call(
        &source,
        "task.create",
        json!({ "project": "TM", "title": "Second", "parent": "TM-1" }),
    );
    call(
        &source,
        "comment.add",
        json!({ "key": "TM-1", "body": "A comment" }),
    );
    call(
        &source,
        "time.log",
        json!({ "key": "TM-1", "duration": "30m" }),
    );
    let file = dir.path().join("note.txt");
    fs::write(&file, b"attached content").unwrap();
    call(
        &source,
        "file.attach",
        json!({ "task": "TM-1", "path": file.to_string_lossy() }),
    );
    call(
        &source,
        "wiki.create",
        json!({ "project": "TM", "title": "Home", "body": "Welcome" }),
    );
    call(
        &source,
        "wiki.create",
        json!({ "project": "TM", "title": "Notes", "body": "More" }),
    );
    let archive = dir.path().join("archive");
    call(
        &source,
        "transfer.export",
        json!({ "path": archive.to_string_lossy(), "format": "folder" }),
    );
    drop(source);

    let target = open(dir.path(), "target");
    call(
        &target,
        "project.create",
        json!({ "name": "Mine", "key": "MY" }),
    );
    call(
        &target,
        "task.create",
        json!({ "project": "MY", "title": "Mine" }),
    );
    // The archive is good: a dry run accepts it.
    let check = call(
        &target,
        "transfer.import",
        json!({ "path": archive.to_string_lossy(), "dry_run": true }),
    );
    assert_eq!(check["counts"]["tasks"], 2, "{check}");
    Setup {
        dir,
        archive,
        target,
    }
}

/// Every row of every table and every stored file of `e`.
fn state(e: &Engine) -> Vec<String> {
    let mut out = e
        .store()
        .read(|tx| {
            let tables: Vec<String> = tx
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type = 'table'
                      AND name NOT LIKE 'sqlite_%' ORDER BY name",
                )?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            let mut out = Vec::new();
            for table in tables {
                let mut stmt = tx.prepare(&format!("SELECT * FROM \"{table}\""))?;
                let columns = stmt.column_count();
                let mut rows = stmt.query([])?;
                let mut lines = Vec::new();
                while let Some(r) = rows.next()? {
                    let mut cells = Vec::with_capacity(columns);
                    for i in 0..columns {
                        cells.push(match r.get_ref(i)? {
                            ValueRef::Null => "-".to_owned(),
                            ValueRef::Integer(n) => n.to_string(),
                            ValueRef::Real(f) => f.to_string(),
                            ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
                            ValueRef::Blob(b) => format!("{b:02x?}"),
                        });
                    }
                    lines.push(format!("{table}: {}", cells.join("|")));
                }
                lines.sort();
                out.extend(lines);
            }
            Ok::<_, gk_store::StoreError>(out)
        })
        .unwrap();
    let mut stack = vec![e.store().layout().files_dir()];
    while let Some(dir) = stack.pop() {
        let Ok(list) = fs::read_dir(&dir) else {
            continue;
        };
        for item in list.flatten() {
            if item.file_type().unwrap().is_dir() {
                stack.push(item.path());
            } else {
                out.push(format!("file: {}", item.path().display()));
            }
        }
    }
    out
}

/// Imports `path` into the target with `limits`, expects it refused with
/// message `id`, and checks that nothing changed.
fn refused_with(s: &Setup, path: &Path, limits: &Limits, id: &str) -> OpError {
    let before = state(&s.target);
    let input = ImportInput {
        path: path.to_string_lossy().into_owned(),
        ..ImportInput::default()
    };
    let err = import::import(&s.target, &me(), &input, path, limits, &no_progress())
        .expect_err("the archive was imported");
    assert_eq!(err.details["message_id"], id, "{err:?}");
    assert_eq!(err.details["hint_id"], "transfer-rejected-hint", "{err:?}");
    assert!(!err.message.contains("transfer-"), "{}", err.message);
    assert_eq!(state(&s.target), before, "the workspace changed");
    err
}

/// Like [`refused`], with one of the messages `ids`.
fn refused_any(s: &Setup, path: &Path, ids: &[&str]) -> OpError {
    let before = state(&s.target);
    let input = ImportInput {
        path: path.to_string_lossy().into_owned(),
        ..ImportInput::default()
    };
    let err = import::import(
        &s.target,
        &me(),
        &input,
        path,
        &Limits::default(),
        &no_progress(),
    )
    .expect_err("the archive was imported");
    let id = err.details["message_id"].as_str().unwrap_or_default();
    assert!(ids.contains(&id), "{err:?}");
    assert_eq!(state(&s.target), before, "the workspace changed");
    err
}

fn refused(s: &Setup, path: &Path, id: &str) -> OpError {
    refused_with(s, path, &Limits::default(), id)
}

/// A copy of the good archive, changed by `change`.
fn variant(s: &Setup, name: &str, change: impl FnOnce(&Path)) -> PathBuf {
    let to = s.dir.path().join(name);
    copy_dir(&s.archive, &to);
    change(&to);
    to
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for item in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(item.file_name());
        if item.file_type().unwrap().is_dir() {
            copy_dir(&item.path(), &target);
        } else {
            fs::copy(item.path(), target).unwrap();
        }
    }
}

/// Every file of the folder archive `root`, by its archive path.
fn entries(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        for item in fs::read_dir(&dir).unwrap().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if item.file_type().unwrap().is_dir() {
                stack.push((item.path(), rel));
            } else {
                out.push((rel, fs::read(item.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap()
}

fn write_manifest(root: &Path, m: &Value) {
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(m).unwrap(),
    )
    .unwrap();
}

/// Lists every file of the folder in the manifest with its right hash.
fn rehash(root: &Path) {
    let mut m = manifest(root);
    let mut hashes = serde_json::Map::new();
    for (path, bytes) in entries(root) {
        if path != "manifest.json" {
            hashes.insert(path, Value::from(gk_store::sha256_hex(&bytes)));
        }
    }
    m["hashes"] = Value::Object(hashes);
    write_manifest(root, &m);
}

/// Changes line `line` (from 1) of the `.jsonl` file `rel`, then rehashes.
fn edit_line(root: &Path, rel: &str, line: usize, change: impl FnOnce(&mut Value)) {
    let path = root.join(rel);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut v: Value = serde_json::from_str(&lines[line - 1]).unwrap();
    change(&mut v);
    lines[line - 1] = serde_json::to_string(&v).unwrap();
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    rehash(root);
}

fn stored() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
}

/// The folder archive `root` as a `.zip` file, with `extra` entries
/// added after its own.
fn zip_of(root: &Path, to: &Path, extra: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(to).unwrap());
    for (path, bytes) in entries(root) {
        zip.start_file(path, stored()).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    for (path, bytes) in extra {
        zip.start_file(*path, stored()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// Replaces every `from` in the bytes of file `path` with `to` (the same
/// length): how names and sizes the zip writer won't write are made.
fn patch(path: &Path, from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut hits = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            hits += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert!(hits > 0, "nothing to patch");
    fs::write(path, bytes).unwrap();
}

/// Sets the uncompressed size the central directory gives for entry
/// `name` of the zip file `path`.
fn declare_size(path: &Path, name: &str, size: u32) {
    let mut bytes = fs::read(path).unwrap();
    let mut i = 0;
    let mut found = false;
    while i + 46 <= bytes.len() {
        if bytes[i..i + 4] == [0x50, 0x4b, 0x01, 0x02] {
            let len = usize::from(u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]));
            if bytes.get(i + 46..i + 46 + len) == Some(name.as_bytes()) {
                bytes[i + 24..i + 28].copy_from_slice(&size.to_le_bytes());
                found = true;
            }
        }
        i += 1;
    }
    assert!(found, "no entry {name}");
    fs::write(path, bytes).unwrap();
}

/// A change to one record.
type Change = Box<dyn Fn(&mut Value)>;

fn project_file(rel: &str) -> String {
    format!("projects/TM/{rel}")
}

// ---------------------------------------------------------------------
// The container: paths, links, entries, sizes

#[test]
fn unsafe_paths_are_refused() {
    let s = setup();
    for (i, name) in [
        "../escape.txt",
        "/etc/escape.txt",
        "projects/../../escape.txt",
    ]
    .into_iter()
    .enumerate()
    {
        let zip = s.dir.path().join(format!("path-{i}.zip"));
        zip_of(&s.archive, &zip, &[]);
        // Written under a harmless name of the same length, then renamed in
        // the bytes, so the writer can't normalise it.
        let harmless = "x".repeat(name.len());
        let mut z = zip::ZipWriter::new_append(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&zip)
                .unwrap(),
        )
        .unwrap();
        z.start_file(harmless.as_str(), stored()).unwrap();
        z.write_all(b"evil").unwrap();
        z.finish().unwrap();
        patch(&zip, harmless.as_bytes(), name.as_bytes());
        refused(&s, &zip, "transfer-bad-path");
    }
    // A backslash in a folder archive's file name.
    let folder = variant(&s, "backslash", |root| {
        fs::write(root.join("a\\b.txt"), b"evil").unwrap();
    });
    refused(&s, &folder, "transfer-bad-path");
    // A path in the manifest that leaves the archive.
    let folder = variant(&s, "manifest-path", |root| {
        let mut m = manifest(root);
        m["hashes"]["../outside"] = json!("00");
        write_manifest(root, &m);
    });
    refused(&s, &folder, "transfer-bad-path");
    let folder = variant(&s, "project-path", |root| {
        let mut m = manifest(root);
        m["projects"][0]["path"] = json!("../TM");
        write_manifest(root, &m);
    });
    refused(&s, &folder, "transfer-bad-path");
}

#[cfg(unix)]
#[test]
fn links_are_refused() {
    let s = setup();
    let folder = variant(&s, "link", |root| {
        std::os::unix::fs::symlink("/etc/passwd", root.join("projects/TM/link.md")).unwrap();
    });
    refused(&s, &folder, "transfer-link");
    let zip = s.dir.path().join("link.zip");
    zip_of(&s.archive, &zip, &[]);
    let mut z = zip::ZipWriter::new_append(
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&zip)
            .unwrap(),
    )
    .unwrap();
    z.add_symlink("files/link", "/etc/passwd", stored())
        .unwrap();
    z.finish().unwrap();
    refused(&s, &zip, "transfer-link");
    // The archive itself given as a link.
    let link = s.dir.path().join("archive-link");
    std::os::unix::fs::symlink(&s.archive, &link).unwrap();
    refused(&s, &link, "transfer-link");
}

#[test]
fn duplicate_entries_are_refused() {
    let s = setup();
    let zip = s.dir.path().join("dup.zip");
    zip_of(
        &s.archive,
        &zip,
        &[("duplicate-entry-A", b"one"), ("duplicate-entry-B", b"two")],
    );
    patch(&zip, b"duplicate-entry-B", b"duplicate-entry-A");
    refused(&s, &zip, "transfer-duplicate-entries");
    // The same project twice in the manifest.
    let folder = variant(&s, "dup-project", |root| {
        let mut m = manifest(root);
        let first = m["projects"][0].clone();
        m["projects"].as_array_mut().unwrap().push(first);
        write_manifest(root, &m);
    });
    refused(&s, &folder, "transfer-duplicate");
}

#[test]
fn too_many_entries_or_bytes_are_refused() {
    let s = setup();
    let count = entries(&s.archive).len();
    let zip = s.dir.path().join("all.zip");
    zip_of(&s.archive, &zip, &[]);
    for path in [&s.archive, &zip] {
        let few = Limits {
            max_entries: count - 1,
            ..Limits::default()
        };
        refused_with(&s, path, &few, "transfer-too-many-entries");
        let small = Limits {
            max_total: 100,
            ..Limits::default()
        };
        refused_with(&s, path, &small, "transfer-too-large");
    }
    let one = Limits {
        max_manifest: 16,
        ..Limits::default()
    };
    refused_with(&s, &zip, &one, "transfer-entry-too-large");
    let records = Limits {
        max_records: 2,
        ..Limits::default()
    };
    refused_with(&s, &zip, &records, "transfer-too-many-records");
}

#[test]
fn an_entry_that_expands_too_far_is_refused() {
    let s = setup();
    let zip = s.dir.path().join("bomb.zip");
    zip_of(&s.archive, &zip, &[]);
    let mut z = zip::ZipWriter::new_append(
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&zip)
            .unwrap(),
    )
    .unwrap();
    z.start_file(
        "files/zeros",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    z.write_all(&vec![0u8; 8 << 20]).unwrap();
    z.finish().unwrap();
    refused(&s, &zip, "transfer-bomb");
}

#[test]
fn an_entry_that_lies_about_its_size_is_refused() {
    let s = setup();
    let zip = s.dir.path().join("lie.zip");
    zip_of(&s.archive, &zip, &[]);
    let real = u32::try_from(fs::metadata(s.archive.join("manifest.json")).unwrap().len()).unwrap();
    // Smaller than it is: the zip reader notices first. Larger: the read
    // stops short of the declared size.
    declare_size(&zip, "manifest.json", real - 10);
    let err = refused_any(&s, &zip, &["transfer-size-mismatch", "transfer-unreadable"]);
    assert!(err.message.contains("manifest.json"), "{}", err.message);
    declare_size(&zip, "manifest.json", real + 10);
    refused_any(&s, &zip, &["transfer-size-mismatch", "transfer-unreadable"]);
}

#[test]
fn something_else_than_an_archive_is_refused() {
    let s = setup();
    let junk = s.dir.path().join("junk.zip");
    fs::write(&junk, b"this is not a zip file at all").unwrap();
    refused(&s, &junk, "transfer-not-an-archive");
    refused(&s, &s.dir.path().join("nowhere.zip"), "transfer-unreadable");
}

// ---------------------------------------------------------------------
// The manifest: listing, format, hashes, counts

#[test]
fn unlisted_missing_and_unexpected_files_are_refused() {
    let s = setup();
    let folder = variant(&s, "unlisted", |root| {
        fs::write(root.join("projects/TM/extra.txt"), b"x").unwrap();
    });
    refused(&s, &folder, "transfer-unlisted");
    let folder = variant(&s, "missing", |root| {
        fs::remove_file(root.join(project_file("comments.jsonl"))).unwrap();
    });
    refused(&s, &folder, "transfer-missing");
    let folder = variant(&s, "no-manifest", |root| {
        fs::remove_file(root.join("manifest.json")).unwrap();
    });
    refused(&s, &folder, "transfer-missing");
    let folder = variant(&s, "unexpected", |root| {
        fs::write(root.join("projects/TM/run.sh"), b"#!/bin/sh").unwrap();
        rehash(root);
    });
    refused(&s, &folder, "transfer-unexpected");
}

#[test]
fn a_wrong_format_or_version_is_refused() {
    let s = setup();
    let folder = variant(&s, "format", |root| {
        let mut m = manifest(root);
        m["format"] = json!("something-else");
        write_manifest(root, &m);
    });
    let err = refused(&s, &folder, "transfer-format");
    assert!(err.message.contains("something-else"), "{}", err.message);
    let folder = variant(&s, "newer", |root| {
        let mut m = manifest(root);
        m["format_version"] = json!(2);
        // A newer version may have fields this one doesn't know.
        m["future"] = json!(true);
        write_manifest(root, &m);
    });
    refused(&s, &folder, "transfer-version-newer");
    let folder = variant(&s, "older", |root| {
        let mut m = manifest(root);
        m["format_version"] = json!(0);
        write_manifest(root, &m);
    });
    refused(&s, &folder, "transfer-version-unknown");
}

#[test]
fn content_that_does_not_match_its_hash_is_refused() {
    let s = setup();
    let folder = variant(&s, "hash-text", |root| {
        let path = root.join(project_file("tasks.jsonl"));
        let text = fs::read_to_string(&path).unwrap().replace("First", "Fhrst");
        fs::write(path, text).unwrap();
    });
    refused(&s, &folder, "transfer-hash");
    let folder = variant(&s, "hash-file", |root| {
        let (path, _) = entries(root)
            .into_iter()
            .find(|(p, _)| p.starts_with("files/"))
            .unwrap();
        fs::write(root.join(path), b"other content").unwrap();
        rehash(root);
    });
    refused(&s, &folder, "transfer-hash");
}

#[test]
fn counts_that_differ_from_the_manifest_are_refused() {
    let s = setup();
    let folder = variant(&s, "counts", |root| {
        let mut m = manifest(root);
        m["counts"]["tasks"] = json!(3);
        write_manifest(root, &m);
    });
    let err = refused(&s, &folder, "transfer-counts");
    assert_eq!(err.details["args"]["what"], "tasks");
}

// ---------------------------------------------------------------------
// The records

#[test]
fn overlong_lines_and_broken_records_are_refused() {
    let s = setup();
    let short = Limits {
        max_line: 64,
        ..Limits::default()
    };
    let err = refused_with(&s, &s.archive, &short, "transfer-line-too-long");
    assert_eq!(err.details["line"], "1");
    let folder = variant(&s, "unknown-field", |root| {
        edit_line(root, &project_file("tasks.jsonl"), 1, |t| {
            t["surprise"] = json!(1);
        });
    });
    let err = refused(&s, &folder, "transfer-json");
    assert_eq!(err.details["path"], project_file("tasks.jsonl"));
    let folder = variant(&s, "not-json", |root| {
        fs::write(root.join(project_file("time.jsonl")), b"{ not json\n").unwrap();
        rehash(root);
    });
    refused(&s, &folder, "transfer-json");
    let folder = variant(&s, "bad-id", |root| {
        edit_line(root, &project_file("comments.jsonl"), 1, |c| {
            c["id"] = json!("not-an-id");
        });
    });
    refused(&s, &folder, "transfer-invalid-id");
}

#[test]
fn records_that_point_nowhere_are_refused() {
    let s = setup();
    let elsewhere = gk_core::Id::generate().to_string();
    let cases: Vec<(&str, &str, Change)> = vec![
        ("comments.jsonl", "transfer-invalid-comment-task", {
            let id = elsewhere.clone();
            Box::new(move |c| c["task_id"] = json!(id))
        }),
        ("time.jsonl", "transfer-invalid-time-task", {
            let id = elsewhere.clone();
            Box::new(move |e| e["task_id"] = json!(id))
        }),
        ("files.jsonl", "transfer-invalid-file-owner", {
            let id = elsewhere.clone();
            Box::new(move |a| a["owner_id"] = json!(id))
        }),
        (
            "files.jsonl",
            "transfer-invalid-file-missing",
            Box::new(|a| a["sha256"] = json!("0".repeat(64))),
        ),
        ("tasks.jsonl", "transfer-invalid-task-status", {
            let id = elsewhere.clone();
            Box::new(move |t| t["status_id"] = json!(id))
        }),
        (
            "tasks.jsonl",
            "transfer-invalid-task-label",
            Box::new(|t| t["labels"] = json!(["missing"])),
        ),
        ("tasks.jsonl", "transfer-invalid-task-project", {
            let id = elsewhere.clone();
            Box::new(move |t| t["project_id"] = json!(id))
        }),
        ("wiki.jsonl", "transfer-invalid-page-parent", {
            let id = elsewhere.clone();
            Box::new(move |w| w["parent_id"] = json!(id))
        }),
    ];
    for (i, (file, id, change)) in cases.into_iter().enumerate() {
        let folder = variant(&s, &format!("ref-{i}"), |root| {
            edit_line(root, &project_file(file), 1, |v| change(v));
        });
        refused(&s, &folder, id);
    }
    // A subtask whose parent is itself.
    let folder = variant(&s, "loop", |root| {
        let tasks = fs::read_to_string(root.join(project_file("tasks.jsonl"))).unwrap();
        let line = tasks
            .lines()
            .position(|l| l.contains("\"parent_id\":\""))
            .unwrap();
        edit_line(root, &project_file("tasks.jsonl"), line + 1, |t| {
            t["parent_id"] = t["id"].clone();
        });
    });
    refused(&s, &folder, "transfer-invalid-task-nesting");
    // Two pages of instructions for agents in one project.
    let folder = variant(&s, "briefs", |root| {
        for line in [1, 2] {
            edit_line(root, &project_file("wiki.jsonl"), line, |w| {
                w["is_agent_brief"] = json!(true);
            });
        }
    });
    refused(&s, &folder, "transfer-invalid-two-briefs");
    // The same ID twice.
    let folder = variant(&s, "twice", |root| {
        let tasks = fs::read_to_string(root.join(project_file("tasks.jsonl"))).unwrap();
        let first: Value = serde_json::from_str(tasks.lines().next().unwrap()).unwrap();
        edit_line(root, &project_file("comments.jsonl"), 1, |c| {
            c["id"] = first["id"].clone();
        });
    });
    refused(&s, &folder, "transfer-invalid-id-twice");
}

#[test]
fn messages_of_refused_archives_are_complete_sentences() {
    for id in [
        "transfer-bad-path",
        "transfer-counts",
        "transfer-invalid-task-label",
        "transfer-keys-taken",
        "transfer-rejected-hint",
    ] {
        let shown = crate::messages::text(
            id,
            &[
                ("path", "p".into()),
                ("line", "1".into()),
                ("label", "l".into()),
                ("what", "tasks".into()),
                ("said", "1".into()),
                ("found", "2".into()),
                ("keys", "TM".into()),
                ("count", "1".into()),
            ],
        );
        assert_ne!(shown, id);
        assert!(!shown.contains('{'), "{shown}");
    }
}
