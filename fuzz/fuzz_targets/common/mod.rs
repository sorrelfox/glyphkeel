// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What the import targets share: one workspace in a temporary folder, with
//! a project to import into, opened once per fuzzing process, and archives
//! built from the fuzzer's bytes.

#![allow(dead_code)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use arbitrary::{Arbitrary, Unstructured};
use gk_api::ops::common::Via;
use gk_engine::{Context, Engine, EngineConfig, Role};
use gk_store::WorkspaceLayout;
use serde_json::{Value, json};

/// The project every import goes to.
pub const PROJECT: &str = "FZ";

pub struct Fixture {
    _dir: tempfile::TempDir,
    pub engine: Engine,
    /// Where the inputs are written, outside the workspace.
    pub inputs: PathBuf,
}

/// The workspace, opened on first use.
pub fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let layout = WorkspaceLayout::new(dir.path().join("workspace"));
        let engine = Engine::open(&layout, EngineConfig::new(Role::Cli)).expect("the workspace");
        engine
            .call(
                &person(),
                "project.create",
                json!({ "key": PROJECT, "name": "Fuzzing" }),
            )
            .expect("the project");
        let inputs = dir.path().join("inputs");
        std::fs::create_dir_all(&inputs).expect("the inputs folder");
        Fixture {
            _dir: dir,
            engine,
            inputs,
        }
    })
}

/// The person at the command line: permanent operations are refused.
pub fn person() -> Context {
    Context::user(Via::Cli)
}

/// A fresh path for an input named `name`.
pub fn input_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    fixture().inputs.join(format!("{n}-{name}"))
}

/// Writes `bytes` to a fresh input file named `name`.
pub fn write_input(name: &str, bytes: &[u8]) -> PathBuf {
    let path = input_path(name);
    std::fs::write(&path, bytes).expect("the input file");
    path
}

/// One entry of a generated archive: any name, including `..`, absolute
/// paths, backslashes, drive letters and control characters, and any
/// content.
#[derive(Debug, Arbitrary)]
pub struct Entry {
    pub name: String,
    pub content: Vec<u8>,
    pub dir: bool,
}

/// Names that path handling must cope with, mixed into the fuzzer's own.
const TRICKY: &[&str] = &[
    "../escape.md",
    "a/../../escape.md",
    "/etc/passwd",
    "C:\\Windows\\x.md",
    "a\\..\\..\\x.md",
    "./a.md",
    "a//b.md",
    "manifest.json",
    "project.json",
    "Page.md",
    "Page/Child.md",
    "img/../../../x.png",
    "\u{0}.md",
    "Ünïcödé.md",
];

/// The bytes of a `.zip` archive made of the entries `u` describes.
pub fn zip_from(u: &mut Unstructured<'_>) -> arbitrary::Result<Vec<u8>> {
    let entries: Vec<Entry> = u.arbitrary()?;
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (i, entry) in entries.iter().take(64).enumerate() {
            let name = if u.ratio(1, 3).unwrap_or(false) {
                TRICKY[i % TRICKY.len()].to_owned()
            } else {
                entry.name.clone()
            };
            if entry.dir {
                let _ = zip.add_directory(name, options);
            } else if zip.start_file(name, options).is_ok() {
                let _ = zip.write_all(&entry.content);
            }
        }
        let _ = zip.finish();
    }
    Ok(out.into_inner())
}

/// Every path a result names (`path` fields and lists of paths) stays a
/// relative path inside the import: no `..`, no absolute path.
pub fn assert_inside(value: &Value, key: &str) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                assert_inside(v, k);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_inside(item, key);
            }
        }
        Value::String(text) if matches!(key, "path" | "skipped" | "file") => {
            assert!(!text.starts_with('/'), "{key}: {text:?}");
            assert!(
                !text.split(['/', '\\']).any(|part| part == ".."),
                "{key}: {text:?}"
            );
        }
        _ => {}
    }
}

/// Removes an input when done with it, so the folder stays small.
pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}
