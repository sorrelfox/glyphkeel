// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The headings of a document, for links to a heading (`[[Page#Heading]]`)
//! and their completion.

use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};

use crate::text::{collapse_whitespace, push_inline_text};

/// One heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 1 to 6.
    pub level: u8,
    /// Its visible text, whitespace collapsed; what a link names after `#`.
    pub text: String,
    /// The line of the source it starts on, from 1.
    pub line: usize,
}

/// The headings of `source` in reading order, empty ones left out.
///
/// ```
/// let h = gk_markdown::headings("# Plan\n\nText.\n\nDates\n-----\n\n### `cargo` *steps*\n");
/// let texts: Vec<&str> = h.iter().map(|h| h.text.as_str()).collect();
/// assert_eq!(texts, ["Plan", "Dates", "cargo steps"]);
/// assert_eq!((h[1].level, h[1].line), (2, 5));
/// ```
#[must_use]
pub fn headings(source: &str) -> Vec<Heading> {
    let arena = Arena::new();
    let options = crate::options();
    let root = parse_document(&arena, source, &options);
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data();
        if let NodeValue::Heading(h) = &data.value {
            let mut text = String::new();
            push_inline_text(node, &mut text);
            let text = collapse_whitespace(&text);
            if !text.is_empty() {
                out.push(Heading {
                    level: h.level,
                    text,
                    line: data.sourcepos.start.line,
                });
            }
        }
    }
    out
}
