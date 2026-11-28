// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any file read as a PDF for search: the document's structure, its
//! streams and filters, fonts and encodings, and the text of every page.
//! It never panics, keeps to the text limit, and gives up within a bounded
//! time.

#![no_main]

use std::time::{Duration, Instant};

use gk_extract::{Extracted, Limits, extract};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = Limits {
        max_text: 64 * 1024,
        max_pdf_stream: 4 * 1024 * 1024,
        ..Limits::default()
    };
    let started = Instant::now();
    let found = extract(data, "application/pdf", "file.pdf", &limits);
    if let Extracted::Text { text, .. } = &found {
        assert!(text.len() <= limits.max_text);
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{} bytes took {:?}",
        data.len(),
        started.elapsed()
    );
});
