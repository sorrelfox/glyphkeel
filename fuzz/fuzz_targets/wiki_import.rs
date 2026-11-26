// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any folder of Markdown files given to the wiki import, as a `.zip` file
//! of generated entries: names with `..`, absolute paths, backslashes and
//! the like, and texts whose links and images point anywhere (`../../`,
//! absolute paths, `%2e%2e`). Checked without importing (a dry run): it
//! never panics, and what it reports (skipped files, renamed pages) names
//! only paths inside the folder.

#![no_main]

mod common;

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use serde_json::json;

/// Links a page may hold, mixed into the fuzzer's own text.
const LINKS: &[&str] = &[
    "![x](../../../../etc/passwd)",
    "![x](/etc/passwd)",
    "[x](%2e%2e/%2e%2e/secret.md)",
    "[x](..\\..\\x.md)",
    "![x](file:///etc/passwd)",
    "[x](Child.md#heading)",
    "![x](img/a.png)",
    "[[Page]]",
    "[x](./Page.md)",
];

#[derive(Debug, Arbitrary)]
struct Page {
    name: String,
    text: String,
    links: Vec<u8>,
}

fn folder(u: &mut Unstructured<'_>) -> arbitrary::Result<Vec<u8>> {
    if u.ratio(1, 2)? {
        return common::zip_from(u);
    }
    let pages: Vec<Page> = u.arbitrary()?;
    let mut out = std::io::Cursor::new(Vec::new());
    {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for page in pages.iter().take(32) {
            let mut text = page.text.clone();
            for &l in &page.links {
                text.push('\n');
                text.push_str(LINKS[usize::from(l) % LINKS.len()]);
            }
            let name = format!("{}.md", page.name);
            if zip.start_file(name, options).is_ok() {
                let _ = zip.write_all(text.as_bytes());
            }
        }
        let _ = zip.finish();
    }
    Ok(out.into_inner())
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(bytes) = folder(&mut u) else {
        return;
    };
    let fixture = common::fixture();
    let path = common::write_input("wiki.zip", &bytes);
    let result = fixture.engine.call(
        &common::person(),
        "wiki.import",
        json!({ "project": common::PROJECT, "path": path.to_string_lossy(), "dry_run": true }),
    );
    common::remove(&path);
    match result {
        Ok(report) => common::assert_inside(&report, ""),
        Err(error) => assert!(!error.message.is_empty()),
    }
});
