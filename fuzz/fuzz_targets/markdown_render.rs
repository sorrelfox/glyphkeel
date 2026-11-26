// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Any text given to the Markdown renderer: it never panics, a 64 KB text
//! renders within a bounded time, and the HTML it writes holds only what
//! the reader's allowlist allows. Every tag is an allowed one, every
//! attribute is allowed on its tag, links go only to the web, to mail
//! addresses, to attachments and to fragments, and images load only
//! attachments. So no text can bring script, styles, frames, SVG, `data:`
//! or `javascript:` addresses into the page.

#![no_main]

use std::time::{Duration, Instant};

use gk_markdown::{RenderOptions, allowlist, render};
use libfuzzer_sys::fuzz_target;

const MAX_LEN: usize = 64 * 1024;

/// An attribute value as the page reads it.
fn unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&nbsp;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn starts_with_scheme(value: &str, scheme: &str) -> bool {
    value
        .get(..scheme.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
}

fn check_attribute(source: &str, tag: &str, name: &str, value: &str) {
    let allowed = allowlist::GENERIC_ATTRIBUTES.contains(&name)
        || allowlist::reader_attributes(tag).contains(&name);
    assert!(
        allowed,
        "{tag} has attribute {name}={value:?} for {source:?}"
    );
    let value = unescape(value);
    match (tag, name) {
        ("a", "href") => {
            let ok = value.starts_with('#')
                || starts_with_scheme(&value, "https:")
                || starts_with_scheme(&value, "http:")
                || starts_with_scheme(&value, "mailto:")
                || allowlist::is_attachment_link(&value);
            assert!(ok, "link to {value:?} for {source:?}");
        }
        ("img", "src") => {
            // Schemes are case-insensitive; the protocol itself answers
            // only for an attachment's exact address.
            assert!(
                starts_with_scheme(&value, "gk-file:"),
                "image from {value:?} for {source:?}"
            );
        }
        ("img", "data-external-src") => {
            assert!(
                starts_with_scheme(&value, "https:") || starts_with_scheme(&value, "http:"),
                "external image {value:?} for {source:?}"
            );
        }
        _ => {}
    }
}

/// Reads the tags of `html`, as html5ever writes it: `<` in text is always
/// `&lt;`, attribute values are always in double quotes.
fn check(source: &str, html: &str) {
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let closing = rest.starts_with('/');
        if closing {
            rest = &rest[1..];
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            .unwrap_or(rest.len());
        let tag = &rest[..end];
        assert!(
            allowlist::READER_TAGS.contains(&tag),
            "tag {tag:?} for {source:?} in {html:?}"
        );
        rest = &rest[end..];
        if closing {
            continue;
        }
        loop {
            rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix('>') {
                rest = after;
                break;
            }
            if let Some(after) = rest.strip_prefix("/>") {
                rest = after;
                break;
            }
            assert!(!rest.is_empty(), "unclosed tag {tag} for {source:?}");
            let name_end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '=' || c == '>' || c == '/')
                .unwrap_or(rest.len());
            let name = &rest[..name_end];
            rest = &rest[name_end..];
            let mut value = "";
            if let Some(after) = rest.strip_prefix("=\"") {
                let close = after.find('"').expect("a closing quote");
                value = &after[..close];
                rest = &after[close + 1..];
            }
            check_attribute(source, tag, name, value);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(&data[..data.len().min(MAX_LEN)]);
    let keys = |key: &str| key.len() % 2 == 0;
    let options = RenderOptions::new("fuzz").with_task_keys(&keys);
    let started = Instant::now();
    let html = render(&source, &options);
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(2),
        "rendering {} bytes took {took:?}",
        source.len()
    );
    check(&source, &html);
    // Plain text and the reference scan read the same text.
    let _ = gk_markdown::plain_text(&source);
    let _ = gk_markdown::analyze(&source);
});
