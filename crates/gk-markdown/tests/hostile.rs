// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Hostile input never yields a tag, attribute or URL scheme outside the
//! reader's allowlist, and can't forge the renderer's own attributes.

#![allow(clippy::unwrap_used)]

mod common;

use common::hostile::{FORGERIES, PAYLOADS};
use common::{Xorshift, html, violations};
use gk_markdown::allowlist::{READER_TAGS, RENDERER_ATTRIBUTES, USER_TAGS};

#[test]
fn every_payload_is_harmless_in_every_context() {
    let mut bad = Vec::new();
    for payload in PAYLOADS {
        for source in [
            payload.to_string(),
            format!("- {payload}"),
            format!("> {payload}"),
            format!("text {payload} text"),
            format!("| a |\n|---|\n| {payload} |"),
            format!("- [ ] {payload}"),
            format!("[^1]\n\n[^1]: {payload}"),
        ] {
            let out = html(&source);
            let found = violations(&out);
            if !found.is_empty() {
                bad.push(format!("{source:?}\n  -> {out:?}\n  {found:?}"));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn renderer_attributes_cannot_be_forged() {
    for payload in FORGERIES {
        let out = html(payload);
        for needle in [
            "data-ref",
            "data-comment",
            "data-wiki",
            "data-item",
            "data-math",
            "data-lang",
            "data-external-src",
            "class=",
            "<input",
            "<span",
            "<div",
        ] {
            assert!(
                !out.contains(needle),
                "{payload:?} -> {out:?} contains {needle}"
            );
        }
    }
}

#[test]
fn ids_are_limited_to_the_documents_own_footnotes() {
    let out = html("<p id=\"fn-1\">x</p><a id=\"__proto__\">x</a><a id=\"fn-other-1\">x</a>");
    assert!(!out.contains("id="), "{out}");
    // Raw HTML may only reuse this document's footnote ids.
    let out = html("<a id=\"fn-doc-1\">x</a>");
    assert!(out.contains("id=\"fn-doc-1\""), "{out}");
}

#[test]
fn unfinished_raw_html_does_not_swallow_the_renderers_markup() {
    let out = html("<details><a data-ref\n\n- [ ] TM-1 item\n");
    // The checkbox and the task link survive as the renderer wrote them, and
    // the user's `<a>` got no attribute from them.
    assert!(
        out.contains("<input type=\"checkbox\" data-item=\"1\">"),
        "{out}"
    );
    assert!(out.contains("<a data-ref=\"TM-1\""), "{out}");
    assert_eq!(out.matches("data-ref").count(), 1, "{out}");
    let out = html("<details title=\"\n\n- [x] TM-1 item\n\n\" open>");
    assert!(violations(&out).is_empty(), "{out}");
    assert!(!out.contains("title=\"\n<"), "{out}");
}

#[test]
fn allowed_user_html_survives() {
    let out = html(
        "<details open><summary>S</summary>\n\n*body*\n\n</details>\n\n<kbd>K</kbd> \
         H<sub>2</sub>O x<sup>2</sup> <mark>m</mark><br><img src=\"gk-file://a1\" width=\"120\" \
         height=\"80\" alt=\"a\"> <a href=\"https://ok.example\" title=\"t\">ok</a>",
    );
    for needle in [
        "<details open=\"\">",
        "<summary>S</summary>",
        "<em>body</em>",
        "<kbd>K</kbd>",
        "<sub>2</sub>",
        "<sup>2</sup>",
        "<mark>m</mark>",
        "<br>",
        "src=\"gk-file://a1\"",
        "width=\"120\"",
        "href=\"https://ok.example\"",
        "title=\"t\"",
    ] {
        assert!(out.contains(needle), "missing {needle} in {out}");
    }
    assert!(violations(&out).is_empty());
}

#[test]
fn the_allowlists_are_the_documented_ones() {
    // A change to either list is a change to the reader's security
    // contract: it must be made here too, on purpose.
    assert_eq!(
        USER_TAGS,
        [
            "details", "summary", "kbd", "sub", "sup", "mark", "br", "img", "a"
        ]
    );
    for tag in USER_TAGS {
        assert!(READER_TAGS.contains(tag), "{tag}");
    }
    for forbidden in [
        "script", "style", "iframe", "svg", "math", "form", "object", "embed",
    ] {
        assert!(!READER_TAGS.contains(&forbidden), "{forbidden}");
    }
    assert_eq!(
        RENDERER_ATTRIBUTES,
        [
            "class",
            "data-item",
            "data-ref",
            "data-comment",
            "data-wiki",
            "data-math",
            "data-lang",
            "data-external-src",
            "data-macro",
            "data-macro-note",
            "data-macro-arg"
        ]
    );
}

/// Random mixtures of payloads and Markdown fragments.
#[test]
fn random_mixtures_stay_safe() {
    const FRAGMENTS: &[&str] = &[
        "\n",
        "\n\n",
        "- ",
        "> ",
        "> [!NOTE]\n> ",
        "| a |\n|---|\n| ",
        "```\n",
        "$",
        "$$",
        "[[",
        "]]",
        "[",
        "](",
        ")",
        "<",
        ">",
        "\"",
        "'",
        "`",
        "*",
        "TM-12 ",
        "<details>",
        "</details>",
        "<a href=\"",
        "javascript:",
        "&#",
        "x;",
        " onerror=",
        "<svg",
        "</",
        "- [ ] ",
        "\t",
        "\r\n",
        "[^1]",
        "[^1]: ",
        "<!--",
        "-->",
        "=",
        "<a data-ref",
        "![i](",
    ];
    let mut random = Xorshift(0x5eed);
    let mut bad = Vec::new();
    for _ in 0..3000 {
        let mut source = String::new();
        for _ in 0..(random.next() % 12 + 2) {
            if random.next().is_multiple_of(3) {
                source.push_str(random.pick(PAYLOADS));
            } else {
                source.push_str(random.pick(FRAGMENTS));
            }
        }
        let out = html(&source);
        let found = violations(&out);
        if !found.is_empty() {
            bad.push(format!("{source:?}\n  -> {out:?}\n  {found:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
