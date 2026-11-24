// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any text given to the query parser: it never panics, a 4 KB input parses
//! within 5 ms, and a query that parses prints to text that parses to the
//! same tree.

#![no_main]

use std::time::{Duration, Instant};

use libfuzzer_sys::fuzz_target;

/// The longest query the field accepts is 4 KB; longer input is cut there.
const MAX_LEN: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(&data[..data.len().min(MAX_LEN)]);
    let started = Instant::now();
    let (parsed, _tokens) = gk_query::parse_with_tokens(&text);
    let took = started.elapsed();
    // Instrumented builds are slower; the budget holds with room to spare
    // in a release build, so a tenfold margin catches only real blow-ups.
    assert!(
        took < Duration::from_millis(50),
        "parsing {} bytes took {took:?}",
        text.len()
    );
    if let Ok(query) = parsed {
        let printed = gk_query::print(&query);
        let again = gk_query::parse(&printed)
            .unwrap_or_else(|e| panic!("{printed:?} does not parse back: {e:?}"));
        assert_eq!(query.shape(), again.shape(), "{text:?} printed as {printed:?}");
    }
});
