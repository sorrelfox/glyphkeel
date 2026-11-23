// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Checkbox numbering and toggling.

#![allow(clippy::unwrap_used)]

mod common;

use common::{Xorshift, html};
use gk_markdown::{ToggleError, checklist, set_checked, toggle};
use regex::Regex;

/// Documents where finding the checkbox is easy to get wrong.
const TRICKY: &[&str] = &[
    "- [ ] a\n- [x] b\n- [X] c\n",
    "* [ ] star\n+ [ ] plus\n1. [ ] ordered\n2) [x] paren\n",
    "- [ ] parent\n  - [x] child\n    - [ ] grandchild\n- [ ] after\n",
    "> - [ ] quoted\n> > - [x] double quoted\n",
    "> [!NOTE]\n> - [ ] inside a callout\n",
    "-\t[ ] tab after marker\n\t- [x] tab-indented child?\n",
    "- [ ] crlf one\r\n- [x] crlf two\r\n",
    "- [ ] cr only\r- [x] two\r",
    "Grüße 日本語 — - [ ] not an item\n\n- [ ] ünïcödé ✓ before\n- [x] 日本語 item\n",
    "   - [ ] indented three\n",
    "```\n- [ ] in code, not an item\n```\n- [ ] real\n",
    "- [ ] item with `code` and $x^2$ and TM-41\n- [ ]\n- [ ] \n",
    "- [ ] first\n\n  continued paragraph\n\n- [x] loose second\n",
    "| a |\n|---|\n| - [ ] not in tables |\n\n- [ ] after table\n",
    "<!-- - [ ] in comment -->\n- [ ] after comment\n",
    "- [x] a\n  ```\n  - [ ] inside fenced code in item\n  ```\n- [ ] b\n",
    "\u{feff}- [ ] after a byte order mark\n- [x] second\n",
    "- a\u{0}b [ ] not\n- [ ] x\u{0}y\n\u{0}- [ ] z\n- [x] after \u{0}\u{0} nuls\n",
    "Text[^n]\n\n[^n]: - [ ] in a footnote\n\n- [x] in the body\n",
];

#[test]
fn symbol_offsets_point_at_the_symbol() {
    for source in TRICKY {
        for item in checklist(source) {
            let bytes = source.as_bytes();
            let symbol = bytes[item.symbol_offset];
            assert!(
                matches!(symbol, b' ' | b'x' | b'X'),
                "{source:?} item {} -> {:?}",
                item.number,
                symbol as char
            );
            assert_eq!(
                bytes[item.symbol_offset - 1],
                b'[',
                "{source:?} item {}",
                item.number
            );
            assert_eq!(
                bytes[item.symbol_offset + 1],
                b']',
                "{source:?} item {}",
                item.number
            );
            assert_eq!(item.checked, symbol != b' ');
        }
    }
}

#[test]
fn expected_items_are_found() {
    let items = checklist(TRICKY[2]);
    let found: Vec<_> = items
        .iter()
        .map(|i| (i.number, i.checked, i.depth, i.text.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (1, false, 0, "parent"),
            (2, true, 1, "child"),
            (3, false, 2, "grandchild"),
            (4, false, 0, "after"),
        ]
    );
    assert_eq!(checklist(TRICKY[10]).len(), 1);
    assert_eq!(checklist(TRICKY[13]).len(), 1);
    assert_eq!(checklist(TRICKY[14]).len(), 1);
    assert_eq!(checklist(TRICKY[15]).len(), 2);
    let texts: Vec<_> = checklist(TRICKY[11]).into_iter().map(|i| i.text).collect();
    assert_eq!(texts, ["item with code and x^2 and TM-41", "", ""]);
}

#[test]
fn toggle_changes_exactly_one_byte_and_only_that_item() {
    for source in TRICKY {
        let before = checklist(source);
        for item in &before {
            let out = toggle(source, item.number, &item.text).unwrap();
            assert_eq!(out.len(), source.len());
            let changed: Vec<usize> = (0..source.len())
                .filter(|&i| source.as_bytes()[i] != out.as_bytes()[i])
                .collect();
            assert_eq!(changed, [item.symbol_offset], "{source:?}");
            let after = checklist(&out);
            assert_eq!(after.len(), before.len());
            for (a, b) in before.iter().zip(&after) {
                assert_eq!(a.text, b.text);
                assert_eq!(a.symbol_offset, b.symbol_offset);
                assert_eq!(
                    a.checked != b.checked,
                    a.number == item.number,
                    "{source:?}"
                );
            }
            // Toggling back restores the text (an `X` comes back as `x`).
            if source.as_bytes()[item.symbol_offset] != b'X' {
                assert_eq!(toggle(&out, item.number, &item.text).unwrap(), *source);
            }
        }
    }
}

#[test]
fn html_numbers_match_the_checklist() {
    let pattern = Regex::new(r#"data-item="(\d+)"( checked="")?"#).unwrap();
    for source in TRICKY {
        let expected: Vec<(usize, bool)> = checklist(source)
            .iter()
            .map(|i| (i.number, i.checked))
            .collect();
        let out = html(source);
        let in_html: Vec<(usize, bool)> = pattern
            .captures_iter(&out)
            .map(|c| (c[1].parse().unwrap(), c.get(2).is_some()))
            .collect();
        assert_eq!(in_html, expected, "{source:?}\n{out}");
    }
}

#[test]
fn numbering_is_stable_under_edits_that_add_no_items() {
    let source = "Intro.\n\n- [ ] alpha\n- [x] beta\n  - [ ] gamma\n";
    let edited = "Intro, now **longer** with TM-4.\n\n> a quote\n\n- [ ] alpha\n- [x] beta\n  \
                  - [ ] gamma\n\nOutro with `- [ ] code`.\n";
    let a = checklist(source);
    let b = checklist(edited);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(
            (x.number, x.checked, &x.text),
            (y.number, y.checked, &y.text)
        );
    }
}

#[test]
fn a_stale_number_is_refused_by_the_text_check() {
    // Someone inserted an item above while a reader still showed the old
    // text; the reader clicks item 2, which was "beta".
    let current = "- [ ] new first\n- [ ] alpha\n- [ ] beta\n";
    assert_eq!(
        toggle(current, 2, "beta"),
        Err(ToggleError::TextMismatch {
            number: 2,
            expected: "beta".into(),
            actual: "alpha".into(),
        })
    );
    assert_eq!(
        toggle(current, 9, "x"),
        Err(ToggleError::NoSuchItem {
            number: 9,
            count: 3
        })
    );
    assert!(matches!(
        toggle(current, 0, "x"),
        Err(ToggleError::NoSuchItem { .. })
    ));
    // Whitespace differences don't count.
    assert!(toggle(current, 3, "  beta ").is_ok());
    // Errors read as sentences.
    assert_eq!(
        toggle(current, 2, "beta").unwrap_err().to_string(),
        "checklist item 2 reads \"alpha\", not \"beta\""
    );
}

#[test]
fn set_checked_is_idempotent() {
    let source = "- [ ] a\n- [X] b\n";
    assert_eq!(set_checked(source, 1, "a", false).unwrap(), None);
    assert_eq!(set_checked(source, 2, "b", true).unwrap(), None);
    assert_eq!(
        set_checked(source, 1, "a", true).unwrap().as_deref(),
        Some("- [x] a\n- [X] b\n")
    );
    assert_eq!(
        set_checked(source, 2, "b", false).unwrap().as_deref(),
        Some("- [ ] a\n- [ ] b\n")
    );
    assert!(set_checked(source, 2, "a", false).is_err());
}

#[test]
fn generated_documents_round_trip() {
    let mut random = Xorshift(42);
    for _ in 0..500 {
        let mut source = String::new();
        for _ in 0..(random.next() % 15 + 1) {
            let indent = "  ".repeat((random.next() % 3) as usize);
            let prefix = random.pick(&["", "> ", "> > "]);
            let mark = random.pick(&[" ", "x", "X"]);
            let bullet = random.pick(&["-", "*", "+", "1."]);
            let text = random.pick(&[
                "plain",
                "with **bold**",
                "日本語",
                "TM-3 ref",
                "$a$",
                "tab\there",
            ]);
            let eol = random.pick(&["\n", "\r\n"]);
            source.push_str(&format!("{prefix}{indent}{bullet} [{mark}] {text}{eol}"));
            if random.next().is_multiple_of(4) {
                source.push_str("\nparagraph\n\n");
            }
        }
        let items = checklist(&source);
        for item in &items {
            let out = toggle(&source, item.number, &item.text).unwrap();
            let changed = (0..source.len())
                .filter(|&i| source.as_bytes()[i] != out.as_bytes()[i])
                .count();
            assert_eq!(changed, 1, "{source:?}");
            let after = checklist(&out);
            assert_eq!(after.len(), items.len());
            assert_eq!(after[item.number - 1].checked, !item.checked);
        }
    }
}

#[test]
fn items_know_their_lines() {
    let text = "Intro\n\n- [ ] one\n  continued\n  - [x] nested\n- [ ] two\r\n\n> - [ ] quoted\n\n- [ ] last";
    let items = checklist(text);
    let spans: Vec<&str> = items.iter().map(|i| &text[i.lines.clone()]).collect();
    assert_eq!(
        spans,
        [
            "- [ ] one\n  continued\n  - [x] nested\n",
            "  - [x] nested\n",
            "- [ ] two\r\n",
            "> - [ ] quoted\n",
            "- [ ] last",
        ]
    );
}
