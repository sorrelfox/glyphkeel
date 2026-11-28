// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any file read as an Office Open XML or OpenDocument file for search:
//! the ZIP directory and its limits (entries, sizes, ratio), inflating the
//! parts, and the XML text nodes with their references. The first byte
//! picks the kind of name, so both layouts and every part are reached. It
//! never panics, keeps to the text limit, and is done within a bounded
//! time.

#![no_main]

use std::time::{Duration, Instant};

use gk_extract::{Extracted, Limits, extract};
use libfuzzer_sys::fuzz_target;

const NAMES: &[(&str, &str)] = &[
    (
        "a.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        "a.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        "a.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("a.odt", "application/vnd.oasis.opendocument.text"),
];

fuzz_target!(|data: &[u8]| {
    let Some((pick, bytes)) = data.split_first() else {
        return;
    };
    let (name, mime) = NAMES[usize::from(*pick) % NAMES.len()];
    let limits = Limits {
        max_text: 64 * 1024,
        max_unpacked: 16 * 1024 * 1024,
        max_entry: 8 * 1024 * 1024,
        ..Limits::default()
    };
    let started = Instant::now();
    let found = extract(bytes, mime, name, &limits);
    if let Extracted::Text { text, .. } = &found {
        assert!(text.len() <= limits.max_text);
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{} bytes took {:?}",
        bytes.len(),
        started.elapsed()
    );
});
