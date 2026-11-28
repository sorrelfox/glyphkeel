// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The text inside attached files: read in the background, found by global
//! search with a snippet and by task queries (`in:files`, `-in:files`,
//! `has:attachment-text`), kept right through the trash, read again on
//! request, and in an encrypted workspace kept only inside the encrypted
//! database.

#![allow(clippy::unwrap_used)]

#[path = "../../gk-extract/tests/fixtures/mod.rs"]
mod fixtures;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use gk_api::ops::common::Via;
use gk_engine::{Confirmation, Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Monday 23 November 2026, 10:00 UTC.
const NOW: i64 = 1_795_428_000_000;

fn config(role: Role) -> EngineConfig {
    let clock = Arc::new(AtomicI64::new(NOW));
    let mut config = EngineConfig::new(role);
    config.clock = Arc::new(move || clock.fetch_add(1000, Ordering::SeqCst));
    config.time_zone = Some("UTC".to_owned());
    config
}

fn me() -> Context {
    Context::user(Via::App).with_confirmation(Confirmation::Given)
}

fn call(e: &Engine, op: &str, input: Value) -> Value {
    e.call(&me(), op, input)
        .unwrap_or_else(|err| panic!("{op}: {err:?}"))
}

fn attach(e: &Engine, task: &str, name: &str, bytes: &[u8]) -> Value {
    call(
        e,
        "file.attach",
        json!({ "task": task, "name": name, "data_base64": gk_core::base64_encode(bytes) }),
    )["file"]
        .clone()
}

/// Runs background turns until nothing waits.
fn read_all(e: &Engine) {
    let started = Instant::now();
    loop {
        let turn = e.index_files(Duration::from_secs(2)).unwrap();
        if turn.waiting == 0 {
            return;
        }
        assert!(started.elapsed() < Duration::from_secs(60), "{turn:?}");
    }
}

fn outcome(e: &Engine, sha256: &str) -> (String, Option<String>, String) {
    e.store()
        .read(|tx| {
            Ok::<_, gk_store::StoreError>(tx.query_row(
                "SELECT outcome, reason, text FROM file_texts WHERE sha256 = ?1",
                [sha256],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap()
}

fn search(e: &Engine, text: &str) -> Vec<Value> {
    call(e, "search.query", json!({ "text": text }))["items"]
        .as_array()
        .unwrap()
        .clone()
}

fn keys(e: &Engine, query: &str) -> Vec<String> {
    call(e, "task.search", json!({ "query": query }))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].as_str().unwrap().to_owned())
        .collect()
}

fn workspace(dir: &TempDir, role: Role) -> Engine {
    let e = Engine::open(
        &WorkspaceLayout::new(dir.path().join("Personal")),
        config(role),
    )
    .unwrap();
    call(
        &e,
        "project.create",
        json!({ "name": "Files", "key": "ZZ" }),
    );
    for title in ["Contract", "Notes", "Other"] {
        call(
            &e,
            "task.create",
            json!({ "project": "ZZ", "title": title }),
        );
    }
    e
}

#[test]
fn every_kind_of_file_gives_its_text_or_a_clean_skip() {
    let dir = TempDir::new().unwrap();
    let e = workspace(&dir, Role::App);
    let files = [
        ("text.pdf", fixtures::text_pdf("The wombat clause applies")),
        ("scan.pdf", fixtures::scanned_pdf()),
        ("report.docx", fixtures::docx("Budget", "the platypus line")),
        ("sheet.xlsx", fixtures::xlsx(&["Revenue", "Aardvark Ltd"], "inline")),
        ("minutes.odt", fixtures::odt("Minutes", "the narwhal plan")),
        ("notes.txt", fixtures::utf16_text("Ünïcödé axolotl notes")),
        ("bomb.docx", fixtures::bomb_docx(8)),
        ("dot.png", gk_core::base64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==").unwrap()),
    ];
    let mut shas = Vec::new();
    for (name, bytes) in &files {
        shas.push(
            attach(&e, "ZZ-1", name, bytes)["sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let status = call(&e, "file.index_status", json!({}));
    assert_eq!(status["files"], 8, "{status}");
    assert_eq!(status["waiting"], 8, "{status}");
    read_all(&e);

    let (o, _, text) = outcome(&e, &shas[0]);
    assert_eq!(o, "text");
    assert!(text.contains("wombat clause"), "{text:?}");
    assert_eq!(
        outcome(&e, &shas[1]),
        ("none".into(), Some("no-text".into()), String::new())
    );
    assert_eq!(outcome(&e, &shas[2]).2, "Budget\nthe platypus line");
    assert_eq!(outcome(&e, &shas[3]).2, "Revenue\nAardvark Ltd\ninline");
    assert_eq!(outcome(&e, &shas[4]).2, "Minutes\nthe narwhal plan");
    assert_eq!(outcome(&e, &shas[5]).2, "Ünïcödé axolotl notes");
    assert_eq!(
        outcome(&e, &shas[6]),
        ("none".into(), Some("bomb".into()), String::new())
    );
    assert_eq!(outcome(&e, &shas[7]).0, "unsupported");

    let status = call(&e, "file.index_status", json!({}));
    assert_eq!(
        status,
        json!({ "files": 8, "read": 8, "waiting": 0, "with_text": 5, "skipped": 2 })
    );
}

#[test]
fn search_finds_a_word_that_is_only_inside_a_pdf() {
    let dir = TempDir::new().unwrap();
    let e = workspace(&dir, Role::App);
    let pdf = attach(
        &e,
        "ZZ-1",
        "contract.pdf",
        &fixtures::text_pdf("Liability of the quokka supplier"),
    );
    attach(&e, "ZZ-2", "plain.txt", b"nothing to see");
    // Not found before the file is read.
    assert!(search(&e, "quokka").is_empty());
    read_all(&e);

    let hits = search(&e, "quokka");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let hit = &hits[0];
    assert_eq!(hit["kind"], "file");
    assert_eq!(hit["matched_in"], "text");
    assert_eq!(hit["title"], "contract.pdf");
    assert_eq!(hit["task"], "ZZ-1");
    assert_eq!(hit["id"], pdf["id"]);
    let snippet = hit["snippet"].as_str().unwrap();
    assert!(snippet.contains("quokka"), "{snippet}");
    let m = &hit["snippet_matches"][0];
    let utf16: Vec<u16> = snippet.encode_utf16().collect();
    let from = usize::try_from(m["from"].as_u64().unwrap()).unwrap();
    let to = usize::try_from(m["to"].as_u64().unwrap()).unwrap();
    assert_eq!(String::from_utf16(&utf16[from..to]).unwrap(), "quokka");

    // Task queries: plain words include files, in:files only files,
    // -in:files none; has:attachment-text.
    assert_eq!(keys(&e, "quokka"), ["ZZ-1"]);
    assert_eq!(keys(&e, "quokka in:files"), ["ZZ-1"]);
    assert!(keys(&e, "quokka -in:files").is_empty());
    assert!(keys(&e, "Contract in:files").is_empty());
    assert_eq!(keys(&e, "Contract -in:files"), ["ZZ-1"]);
    assert_eq!(keys(&e, "has:attachment-text"), ["ZZ-1", "ZZ-2"]);
    assert_eq!(keys(&e, "-has:attachment-text"), ["ZZ-3"]);
    // A file's name is not its text.
    assert!(keys(&e, "contract.pdf in:files").is_empty());
}

#[test]
fn the_trash_takes_the_text_out_of_search_and_brings_it_back() {
    let dir = TempDir::new().unwrap();
    let e = workspace(&dir, Role::App);
    let file = attach(
        &e,
        "ZZ-1",
        "terms.docx",
        &fixtures::docx("Terms", "the dugong rule"),
    );
    let id = file["id"].as_str().unwrap().to_owned();
    read_all(&e);
    assert_eq!(search(&e, "dugong").len(), 1);

    call(&e, "file.delete", json!({ "attachment": id }));
    assert!(search(&e, "dugong").is_empty());
    assert!(keys(&e, "dugong").is_empty());
    let docs: i64 = e
        .store()
        .read(|tx| {
            Ok::<_, gk_store::StoreError>(tx.query_row(
                "SELECT count(*) FROM search_docs WHERE kind = 'file'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(docs, 0);

    let trash = call(&e, "trash.list", json!({}));
    let entry = trash["items"][0]["id"].as_str().unwrap().to_owned();
    call(&e, "trash.restore", json!({ "items": [entry] }));
    // Back at once, without reading the file again.
    let hits = search(&e, "dugong");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0]["id"], id.as_str());
    assert_eq!(keys(&e, "dugong in:files"), ["ZZ-1"]);

    // The same content attached again is found at once too.
    attach(
        &e,
        "ZZ-2",
        "copy.docx",
        &fixtures::docx("Terms", "the dugong rule"),
    );
    assert_eq!(search(&e, "dugong").len(), 2);
}

#[test]
fn reindexing_reads_every_file_again_and_keeps_the_text_meanwhile() {
    let dir = TempDir::new().unwrap();
    let e = workspace(&dir, Role::App);
    attach(&e, "ZZ-1", "a.txt", b"the first tapir");
    attach(&e, "ZZ-2", "b.md", b"# the second tapir");
    read_all(&e);
    let history = |e: &Engine| -> i64 {
        e.store()
            .read(|tx| {
                Ok::<_, gk_store::StoreError>(tx.query_row(
                    "SELECT count(*) FROM activity",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap()
    };
    let before = history(&e);
    let status = call(&e, "file.reindex", json!({}));
    assert_eq!(status["waiting"], 2, "{status}");
    assert_eq!(search(&e, "tapir").len(), 2);
    // A turn with no time left reads nothing; one with time reads it all.
    let turn = e.index_files(Duration::ZERO).unwrap();
    assert_eq!(turn.waiting, 2);
    read_all(&e);
    assert_eq!(call(&e, "file.index_status", json!({}))["read"], 2);
    // Reading is not anyone's change: it leaves no history.
    assert_eq!(history(&e), before);
}

#[test]
fn the_command_line_reads_the_file_it_attached_before_it_exits() {
    let dir = TempDir::new().unwrap();
    let e = workspace(&dir, Role::Cli);
    attach(&e, "ZZ-1", "brief.txt", b"bring the okapi report");
    assert_eq!(call(&e, "file.index_status", json!({}))["waiting"], 0);
    assert_eq!(search(&e, "okapi").len(), 1);
}

#[test]
fn a_pdf_that_runs_out_of_time_is_not_retried_and_the_next_pdf_waits_for_it() {
    let dir = TempDir::new().unwrap();
    let mut cfg = config(Role::App);
    cfg.file_text_limits = gk_extract::Limits {
        max_time: Duration::from_millis(20),
        ..gk_extract::Limits::default()
    };
    let e = Engine::open(&WorkspaceLayout::new(dir.path().join("Personal")), cfg).unwrap();
    call(
        &e,
        "project.create",
        json!({ "name": "Files", "key": "ZZ" }),
    );
    call(
        &e,
        "task.create",
        json!({ "project": "ZZ", "title": "Slow" }),
    );
    // Attached first, so it is read last (newest first).
    let other = attach(
        &e,
        "ZZ-1",
        "other.pdf",
        &fixtures::text_pdf("the gecko memo"),
    );
    // One page of a million words: one call into the PDF reader that the
    // time limit can't interrupt.
    let words = "(word) Tj ".repeat(1_000_000);
    let slow = attach(
        &e,
        "ZZ-1",
        "slow.pdf",
        &fixtures::text_pdf(&format!("x) Tj {words} (y")),
    );
    let (slow, other) = (
        slow["sha256"].as_str().unwrap().to_owned(),
        other["sha256"].as_str().unwrap().to_owned(),
    );
    let read = |sha: &str| -> Option<(String, Option<String>)> {
        e.store()
            .read(|tx| {
                Ok::<_, gk_store::StoreError>(
                    tx.query_row(
                        "SELECT outcome, reason FROM file_texts WHERE sha256 = ?1",
                        [sha],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok(),
                )
            })
            .unwrap()
    };

    // The slow one is left running past its limit and recorded as timed
    // out; the other PDF is not started while it runs.
    let turn = e.index_files(Duration::from_millis(150)).unwrap();
    assert_eq!(read(&slow), Some(("none".into(), Some("timed-out".into()))));
    assert_eq!(read(&other), None, "{turn:?}");
    assert_eq!(turn.waiting, 1);

    // Once it has ended, the other is read; the slow one is not tried again.
    read_all(&e);
    assert_eq!(read(&other).map(|o| o.0).as_deref(), Some("text"));
    assert_eq!(search(&e, "gecko").len(), 1);
    let at = |sha: &str| -> i64 {
        e.store()
            .read(|tx| {
                Ok::<_, gk_store::StoreError>(tx.query_row(
                    "SELECT extracted_at FROM file_texts WHERE sha256 = ?1",
                    [sha],
                    |r| r.get(0),
                )?)
            })
            .unwrap()
    };
    let first = at(&slow);
    for _ in 0..3 {
        assert_eq!(e.index_files(Duration::from_millis(50)).unwrap().waiting, 0);
    }
    assert_eq!(at(&slow), first);

    // Read again in Preferences tries it once more.
    call(&e, "file.reindex", json!({}));
    read_all(&e);
    assert_ne!(at(&slow), first);
    assert_eq!(read(&slow), Some(("none".into(), Some("timed-out".into()))));
}

/// Every file under `dir` that holds `needle`.
fn holding(dir: &Path, needle: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path)
                && bytes.windows(needle.len()).any(|w| w == needle)
            {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    out
}

#[test]
fn an_encrypted_workspace_keeps_the_text_inside_its_encrypted_database() {
    if !gk_store::encryption_available() {
        return;
    }
    // The word is written in a way only extraction turns into plain UTF-8:
    // UTF-16 in the file, so a plain file store doesn't hold it either.
    let word = "capybarafingerprint";
    let control = TempDir::new().unwrap();
    {
        let e = workspace(&control, Role::App);
        attach(&e, "ZZ-1", "notes.txt", &fixtures::utf16_text(word));
        read_all(&e);
        assert_eq!(search(&e, word).len(), 1);
        e.close();
    }
    // The test can see the text where it is plain.
    assert!(!holding(control.path(), word.as_bytes()).is_empty());

    let dir = TempDir::new().unwrap();
    let layout = WorkspaceLayout::new(dir.path().join("Personal"));
    workspace(&dir, Role::App).close();
    let pass = gk_store::Passphrase::new("search inside files passphrase").unwrap();
    gk_engine::encryption::change(
        &layout,
        None,
        &gk_engine::encryption::Change::Encrypt(pass),
        NOW,
        "t",
    )
    .unwrap();
    let e = Engine::open(&layout, config(Role::App)).unwrap();
    attach(&e, "ZZ-1", "notes.txt", &fixtures::utf16_text(word));
    attach(&e, "ZZ-2", "terms.pdf", &fixtures::text_pdf(word));
    read_all(&e);
    assert_eq!(search(&e, word).len(), 2);
    e.close();
    let found = holding(dir.path(), word.as_bytes());
    assert!(found.is_empty(), "plain text in {found:?}");
}

/// Data set L: run with `cargo test -p gk-engine --release --test file_text -- --ignored`.
#[test]
#[ignore = "a timing run on the large data set"]
fn data_set_l_is_read_and_searched_in_reasonable_time() {
    let dir = TempDir::new().unwrap();
    let archive = dir.path().join("seed-L");
    gk_seed::generate(gk_seed::DataSet::Large, 1)
        .unwrap()
        .write_to(&archive)
        .unwrap();
    let e = Engine::open(
        &WorkspaceLayout::new(dir.path().join("L")),
        config(Role::App),
    )
    .unwrap();
    let input = gk_api::ops::transfer::ImportInput {
        path: archive.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let progress: gk_engine::transfer::Progress = Arc::new(|_| {});
    e.import_archive(&me(), &input, &progress).unwrap();
    let before = call(&e, "file.index_status", json!({}));
    println!("before: {before}");
    let started = Instant::now();
    let mut turns = 0;
    loop {
        turns += 1;
        let turn = e.index_files(Duration::from_secs(3)).unwrap();
        if turn.waiting == 0 {
            break;
        }
    }
    let took = started.elapsed();
    let after = call(&e, "file.index_status", json!({}));
    println!("read in {took:.2?} over {turns} turns: {after}");
    assert_eq!(after["waiting"], 0);
    assert!(took < Duration::from_secs(120), "{took:?}");
    for text in ["error", "timeout", "the attached file"] {
        let started = Instant::now();
        let hits = call(&e, "search.query", json!({ "text": text }));
        let took = started.elapsed();
        println!("search {text:?}: {} hits in {took:.2?}", hits["total"]);
        assert!(took < Duration::from_millis(500), "{text}: {took:?}");
        let started = Instant::now();
        let tasks = call(
            &e,
            "task.search",
            json!({ "query": format!("{text} in:files") }),
        );
        let took = started.elapsed();
        println!(
            "query {text:?} in:files: {} tasks in {took:.2?}",
            tasks["total"]
        );
        assert!(took < Duration::from_millis(500), "{text}: {took:?}");
    }
}
