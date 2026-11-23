// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The CommonMark 0.31.2 and GitHub Flavored Markdown spec examples.
//!
//! - The parser, with the spec's own settings, passes the examples (the
//!   baseline). The few GFM failures come from the spec's older version and
//!   from the tag filter, which the reader replaces with its allowlist.
//! - The reader's HTML for every example stays inside the allowlist, and its
//!   checkboxes are numbered as the checklist numbers them.
//!
//! Run with `--nocapture` to see the failing baseline examples.

#![allow(clippy::unwrap_used, clippy::print_stdout)]

mod common;

use common::{html, violations};
use regex::Regex;

struct Example {
    number: usize,
    section: String,
    markdown: String,
    html: String,
    disabled: bool,
}

fn commonmark() -> Vec<Example> {
    let text = include_str!("spec/commonmark-spec.json");
    let json: serde_json::Value = serde_json::from_str(text).unwrap();
    json.as_array()
        .unwrap()
        .iter()
        .map(|e| Example {
            number: e["example"].as_u64().unwrap() as usize,
            section: e["section"].as_str().unwrap().into(),
            markdown: e["markdown"].as_str().unwrap().into(),
            html: e["html"].as_str().unwrap().into(),
            disabled: false,
        })
        .collect()
}

fn gfm() -> Vec<Example> {
    let text = include_str!("spec/gfm-spec.txt");
    let fence = "`".repeat(32);
    let opening = format!("{fence} example");
    let mut examples = Vec::new();
    let mut section = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with('#') {
            section = line.trim_start_matches('#').trim().to_owned();
            continue;
        }
        let Some(kind) = line.strip_prefix(&opening) else {
            continue;
        };
        let mut markdown = String::new();
        for l in lines.by_ref() {
            if l == "." {
                break;
            }
            markdown.push_str(l);
            markdown.push('\n');
        }
        let mut html = String::new();
        for l in lines.by_ref() {
            if l == fence {
                break;
            }
            html.push_str(l);
            html.push('\n');
        }
        examples.push(Example {
            number: examples.len() + 1,
            section: section.clone(),
            markdown: markdown.replace('→', "\t"),
            html: html.replace('→', "\t"),
            disabled: kind.trim() == "disabled",
        });
    }
    examples
}

fn normalize(html: &str) -> String {
    let between_tags = Regex::new(r">\s+<").unwrap();
    let html = html.replace(" />", ">").replace("\r\n", "\n");
    between_tags.replace_all(html.trim(), "><").into_owned()
}

fn spec_options(gfm: bool) -> comrak::Options<'static> {
    let mut options = comrak::Options::default();
    options.render.r#unsafe = true;
    if gfm {
        options.extension.table = true;
        options.extension.strikethrough = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
    }
    options
}

/// Runs every example; returns the numbers whose baseline output differs.
fn run(name: &str, examples: &[Example], gfm: bool) -> Vec<usize> {
    let options = spec_options(gfm);
    let item = Regex::new(r#"data-item="(\d+)""#).unwrap();
    let mut failed = Vec::new();
    let mut unsafe_outputs = Vec::new();
    for example in examples.iter().filter(|e| !e.disabled) {
        let baseline = comrak::markdown_to_html(&example.markdown, &options);
        if normalize(&baseline) != normalize(&example.html) {
            println!("{name} #{} [{}]", example.number, example.section);
            failed.push(example.number);
        }
        let reader = html(&example.markdown);
        let found = violations(&reader);
        if !found.is_empty() {
            unsafe_outputs.push(format!(
                "#{}: {:?} -> {reader:?} {found:?}",
                example.number, example.markdown
            ));
        }
        let numbers: Vec<usize> = item
            .captures_iter(&reader)
            .map(|c| c[1].parse().unwrap())
            .collect();
        let expected: Vec<usize> = gk_markdown::checklist(&example.markdown)
            .iter()
            .map(|i| i.number)
            .collect();
        assert_eq!(numbers, expected, "{name} #{}", example.number);
    }
    assert!(unsafe_outputs.is_empty(), "{}", unsafe_outputs.join("\n"));
    failed
}

#[test]
fn commonmark_examples() {
    let examples = commonmark();
    assert_eq!(examples.len(), 652);
    let failed = run("CommonMark", &examples, false);
    assert!(failed.is_empty(), "baseline failures: {failed:?}");
}

#[test]
fn gfm_examples() {
    let examples = gfm();
    let enabled = examples.iter().filter(|e| !e.disabled).count();
    assert_eq!(enabled, 670);
    let failed = run("GFM", &examples, true);
    // 14 known differences: the spec predates CommonMark 0.31 in a few
    // places, and the tag filter is not used (the reader has its allowlist).
    assert!(
        failed.len() <= 14,
        "{} baseline failures: {failed:?}",
        failed.len()
    );
}
