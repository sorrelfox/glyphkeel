// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Properties over generated documents: toggling changes exactly one byte,
//! and the reader's HTML stays inside the allowlist.

#![allow(clippy::unwrap_used)]

mod common;

use common::{html, violations};
use gk_markdown::{checklist, toggle};
use proptest::prelude::*;

/// A line of a document: a checklist item in some container, or filler.
fn line() -> impl Strategy<Value = String> {
    let container =
        prop::sample::select(vec!["", "> ", "> > ", "  ", "    ", "\t", "> [!NOTE]\n> "]);
    let bullet = prop::sample::select(vec!["-", "*", "+", "1.", "2)"]);
    let mark = prop::sample::select(vec![" ", "x", "X"]);
    let text = prop::sample::select(vec![
        "plain",
        "**bold** and _em_",
        "日本語 ✓",
        "TM-3 ref",
        "$a$ `code`",
        "tab\there",
        "a\u{0}nul",
        "<kbd>k</kbd>",
        "[[Page]]",
        "",
    ]);
    let eol = prop::sample::select(vec!["\n", "\r\n", "\r"]);
    let filler = prop::sample::select(vec![
        "\n",
        "paragraph\n",
        "```\n- [ ] fenced\n```\n",
        "| a |\n|---|\n| - [ ] cell |\n",
        "<!-- - [ ] comment -->\n",
        "Note[^1]\n\n[^1]: - [ ] in a footnote\n",
        "\u{feff}",
        "- not a task\n",
    ]);
    prop_oneof![
        3 => (container, bullet, mark, text, eol)
            .prop_map(|(c, b, m, t, e)| format!("{c}{b} [{m}] {t}{e}")),
        1 => filler.prop_map(str::to_owned),
    ]
}

fn document() -> impl Strategy<Value = String> {
    prop::collection::vec(line(), 1..20).prop_map(|lines| lines.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn toggle_changes_exactly_one_byte(source in document()) {
        let items = checklist(&source);
        for item in &items {
            let out = toggle(&source, item.number, &item.text).unwrap();
            prop_assert_eq!(out.len(), source.len());
            let changed: Vec<usize> = source
                .bytes()
                .zip(out.bytes())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, _)| i)
                .collect();
            prop_assert_eq!(changed, vec![item.symbol_offset]);
            let after = checklist(&out);
            prop_assert_eq!(after.len(), items.len());
            for (a, b) in items.iter().zip(&after) {
                prop_assert_eq!(&a.text, &b.text);
                prop_assert_eq!(a.checked != b.checked, a.number == item.number);
            }
        }
    }

    #[test]
    fn any_text_renders_inside_the_allowlist(source in "\\PC{0,200}") {
        let out = html(&source);
        prop_assert!(violations(&out).is_empty(), "{:?} -> {:?}", source, out);
    }

    #[test]
    fn toggle_never_panics_on_any_text(source in "\\PC{0,120}", number in 0usize..5) {
        let text = checklist(&source)
            .get(number.wrapping_sub(1))
            .map(|i| i.text.clone())
            .unwrap_or_default();
        let _ = toggle(&source, number, &text);
    }
}
