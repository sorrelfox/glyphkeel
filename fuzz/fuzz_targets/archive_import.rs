// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any archive given to the import, checked without importing (a dry run):
//! it never panics, and what it reports names only paths inside the
//! archive. Three kinds of input: raw bytes as a `.zip` file, an archive of
//! generated entries (names with `..`, absolute paths and the like), and a
//! real export with some of its files changed.

#![no_main]

mod common;

use std::io::{Read as _, Write as _};
use std::sync::OnceLock;

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use serde_json::json;

/// The files of a real export of a small project, read once.
fn real_export() -> &'static [(String, Vec<u8>)] {
    static EXPORT: OnceLock<Vec<(String, Vec<u8>)>> = OnceLock::new();
    EXPORT.get_or_init(|| {
        let fixture = common::fixture();
        let ctx = common::person();
        for title in ["First", "Second **bold**", "Third [[Page]]"] {
            let _ = fixture.engine.call(
                &ctx,
                "task.create",
                json!({ "project": common::PROJECT, "title": title, "description": "Some *text* and a [link](https://example.org)." }),
            );
        }
        let path = common::input_path("export.zip");
        fixture
            .engine
            .call(
                &ctx,
                "transfer.export",
                json!({ "path": path.to_string_lossy(), "projects": [common::PROJECT] }),
            )
            .expect("an export");
        let file = std::fs::File::open(&path).expect("the export");
        let mut zip = zip::ZipArchive::new(file).expect("a zip file");
        let mut files = Vec::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).expect("an entry");
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("its bytes");
            let name = entry.name().expect("a name").into_owned();
            files.push((name, bytes));
        }
        common::remove(&path);
        files
    })
}

/// The real export with some files replaced or cut by the fuzzer.
fn changed_export(u: &mut Unstructured<'_>) -> arbitrary::Result<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in real_export() {
            let content: Vec<u8> = match u.int_in_range(0..=3u8)? {
                0 => u.arbitrary()?,
                1 => {
                    let cut = u.int_in_range(0..=bytes.len())?;
                    bytes[..cut].to_vec()
                }
                2 => continue,
                _ => bytes.clone(),
            };
            if zip.start_file(name.as_str(), options).is_ok() {
                let _ = zip.write_all(&content);
            }
        }
        let _ = zip.finish();
    }
    Ok(out.into_inner())
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut u = Unstructured::new(rest);
    let bytes = match mode % 3 {
        0 => rest.to_vec(),
        1 => match common::zip_from(&mut u) {
            Ok(bytes) => bytes,
            Err(_) => return,
        },
        _ => match changed_export(&mut u) {
            Ok(bytes) => bytes,
            Err(_) => return,
        },
    };
    let fixture = common::fixture();
    let path = common::write_input("archive.zip", &bytes);
    let result = fixture.engine.call(
        &common::person(),
        "transfer.import",
        json!({ "path": path.to_string_lossy(), "dry_run": true }),
    );
    common::remove(&path);
    match result {
        Ok(report) => common::assert_inside(&report, ""),
        Err(error) => assert!(!error.message.is_empty()),
    }
});
