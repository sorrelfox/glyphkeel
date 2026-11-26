// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any file given to the CSV import, checked without importing (a dry
//! run): every encoding guess, separator and quote detection, header and
//! value mapping. It never panics, and a 256 KB file is checked within a
//! bounded time.

#![no_main]

mod common;

use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;
use serde_json::json;

/// Larger inputs are cut here: the parser's limits are tested elsewhere.
const MAX_LEN: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_LEN)];
    // The first byte picks a preset, so the Jira and Redmine mappings run
    // as well as the detected one.
    let (preset, bytes) = match data.split_first() {
        Some((b, rest)) => (
            match b % 3 {
                0 => None,
                1 => Some("jira"),
                _ => Some("redmine"),
            },
            rest,
        ),
        None => (None, data),
    };
    let fixture = common::fixture();
    let path = common::write_input("tasks.csv", bytes);
    let mut input = json!({
        "project": common::PROJECT,
        "path": path.to_string_lossy(),
        "dry_run": true,
    });
    if let Some(preset) = preset {
        input["preset"] = json!(preset);
    }
    let started = Instant::now();
    let result = fixture
        .engine
        .call(&common::person(), "transfer.import_csv", input);
    let took = started.elapsed();
    common::remove(&path);
    // Instrumented builds are slow; this catches only real blow-ups.
    assert!(
        took < Duration::from_secs(10),
        "{} bytes took {took:?}",
        bytes.len()
    );
    if let Err(error) = result {
        assert!(!error.message.is_empty());
    }
});
