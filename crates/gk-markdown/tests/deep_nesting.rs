// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Documents nested thousands of levels deep are parsed, analysed and
//! rendered on an ordinary thread stack, also in an unoptimised build.
//!
//! The import works out plain text on worker threads with the standard
//! library's default stack, so every pass over the tree must keep its own
//! stack rather than recurse once per level.

#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;

use gk_markdown::{RenderOptions, analyze, checklist, headings, plain_text, references, render};

/// The default stack of a spawned thread in the standard library.
const THREAD_STACK: usize = 2 * 1024 * 1024;

fn deep_document() -> String {
    let mut s = String::new();
    s.push_str(&">".repeat(5_000));
    s.push_str(" deep quote\n\n");
    for i in 0..500 {
        let _ = writeln!(s, "{}- [ ] level {i}", "  ".repeat(i));
    }
    s.push('\n');
    for depth in 1..=300 {
        let _ = writeln!(s, "{} [!NOTE]", ">".repeat(depth));
    }
    s.push_str(&">".repeat(300));
    s.push_str(" callout\n\n");
    s.push_str(&"*".repeat(10_000));
    s.push_str("emphasis");
    s.push_str(&"*".repeat(10_000));
    s.push_str("\n\n");
    s.push_str(&"[".repeat(20_000));
    s.push('x');
    s.push_str(&"](https://example.invalid)".repeat(2_000));
    s.push_str("\n\n");
    s.push_str(&"<div>".repeat(10_000));
    s.push_str("inside\n");
    s
}

/// Runs `f` on a thread with the default stack size and fails if it
/// panics; a stack overflow aborts the whole test binary instead.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(THREAD_STACK)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn plain_text_of_a_deeply_nested_document() {
    on_small_stack(|| {
        let text = plain_text(&deep_document());
        assert!(text.contains("deep quote"), "{}", &text[..200]);
        assert!(text.contains("level 499"));
        assert!(text.contains("inside"));
    });
}

#[test]
fn analysis_of_a_deeply_nested_document() {
    on_small_stack(|| {
        let source = deep_document();
        let analysis = analyze(&source);
        assert!(analysis.plain_text.contains("level 0"));
        assert_eq!(checklist(&source).len(), analysis.checklist.len());
        let _ = references(&source);
        let _ = headings(&source);
    });
}

#[test]
fn rendering_a_deeply_nested_document() {
    on_small_stack(|| {
        let html = render(&deep_document(), &RenderOptions::new("doc"));
        assert!(html.contains("deep quote"));
        assert!(html.contains("inside"));
    });
}
