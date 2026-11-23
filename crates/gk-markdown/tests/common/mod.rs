// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Shared test helpers: the hostile inputs and a checker for the reader's
//! HTML.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used)]

pub mod hostile;

use std::sync::OnceLock;

use gk_markdown::allowlist::{
    GENERIC_ATTRIBUTES, IMAGE_SCHEME, LINK_SCHEMES, READER_TAGS, reader_attributes,
};
use gk_markdown::{RenderOptions, render};
use regex::Regex;

/// Renders with every task key known.
pub fn html(source: &str) -> String {
    let all = |_: &str| true;
    render(source, &RenderOptions::new("doc").with_task_keys(&all))
}

fn tag_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"<(/?)([A-Za-z][A-Za-z0-9:-]*)((?:\s+[^\s=/>]+(?:="[^"]*")?)*)\s*/?>"#)
            .unwrap()
    })
}

fn attribute_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"\s+([^\s=/>]+)(?:="([^"]*)")?"#).unwrap())
}

/// Everything in `html` outside the reader's allowlist; empty when it is
/// safe. The sanitizer writes HTML canonically (attributes always in double
/// quotes, `"` and `&` escaped inside them, `<` escaped in text), so a
/// pattern over its output is exact: any `<` that doesn't start a recognised
/// tag is reported too.
pub fn violations(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    let outside_tags = tag_pattern().replace_all(html, "");
    if let Some(at) = outside_tags.find('<') {
        let context: String = outside_tags[at..].chars().take(60).collect();
        found.push(format!("stray '<': {context:?}"));
    }
    for tag in tag_pattern().captures_iter(html) {
        let name = tag[2].to_ascii_lowercase();
        if !READER_TAGS.contains(&name.as_str()) {
            found.push(format!("tag <{name}>"));
            continue;
        }
        for attribute in attribute_pattern().captures_iter(&tag[3]) {
            let attr = attribute[1].to_ascii_lowercase();
            let value = unescape(attribute.get(2).map_or("", |m| m.as_str()));
            let lower = value.trim().to_ascii_lowercase();
            if attr.starts_with("on") {
                found.push(format!("event handler {attr} on <{name}>"));
            }
            if !reader_attributes(&name).contains(&attr.as_str())
                && !GENERIC_ATTRIBUTES.contains(&attr.as_str())
            {
                found.push(format!("attribute {attr} on <{name}>"));
            }
            let web = lower.starts_with("http://") || lower.starts_with("https://");
            match attr.as_str() {
                "href" => {
                    let scheme_ok = LINK_SCHEMES
                        .iter()
                        .any(|s| lower.starts_with(&format!("{s}:")));
                    let fragment = lower.starts_with('#') && !lower.starts_with("#/");
                    if !(scheme_ok || fragment) {
                        found.push(format!("href {value:?}"));
                    }
                }
                "src" if !lower.starts_with(&format!("{IMAGE_SCHEME}:")) => {
                    found.push(format!("src {value:?}"));
                }
                "data-external-src" if !web => {
                    found.push(format!("data-external-src {value:?}"));
                }
                "type" if value != "checkbox" => found.push(format!("input type {value:?}")),
                "rel" if value != "noopener noreferrer" => found.push(format!("rel {value:?}")),
                _ => {}
            }
        }
    }
    found
}

/// Undoes the escaping the sanitizer applies inside attribute values.
pub fn unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// A small deterministic generator, so random tests repeat exactly.
pub struct Xorshift(pub u64);

impl Xorshift {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}
