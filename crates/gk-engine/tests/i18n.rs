// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Translation checks: every message the engine's code names is in its
//! message file, every message there is named by the code, and every
//! message renders.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use gk_engine::{ENGINE_MESSAGES, MESSAGES_FTL, Messages};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "tests.rs")
        {
            out.push(path);
        }
    }
}

fn sources(dirs: &[&str]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in dirs {
        rust_files(&root.join(dir), &mut files);
    }
    files.sort();
    files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect()
}

#[test]
fn every_message_the_engine_names_exists_and_every_message_is_named() {
    // The engine's messages also hold those of the values gk-core checks.
    let texts = sources(&["gk-engine/src", "gk-core/src"]);
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    // Messages about queries are gk-query's own.
    let query = Messages::new(gk_query::error::MESSAGES_FTL).ids();
    let (missing, unused) = ENGINE_MESSAGES.check_usage(&refs, &query);
    assert!(
        missing.is_empty(),
        "named in the code but not in engine.ftl: {missing:?}"
    );
    assert!(
        unused.is_empty(),
        "in engine.ftl but named nowhere: {unused:?}"
    );
}

#[test]
fn every_message_renders_and_ids_are_unique() {
    let ids = ENGINE_MESSAGES.ids();
    let mut seen = std::collections::BTreeSet::new();
    for id in &ids {
        assert!(seen.insert(*id), "{id} is defined twice");
        let shown = ENGINE_MESSAGES.text(id, &[("count", "2".into())]);
        assert!(!shown.is_empty(), "{id}");
        assert!(!shown.contains(['{', '}']), "{id}: {shown}");
        assert_ne!(shown, *id, "{id} has no text");
    }
    // Every line is a comment, a message, or the indented rest of one.
    for line in MESSAGES_FTL.lines() {
        let ok = line.is_empty()
            || line.starts_with('#')
            || line.starts_with(' ')
            || line.contains(" = ")
            || line.ends_with(" =");
        assert!(ok, "not Fluent: {line}");
    }
}

/// The message of a value's error says what its English `Display` says.
fn same_as_display(e: &(impl gk_core::Message + std::fmt::Display)) {
    let display = e.to_string();
    let mut chars = display.chars();
    let expected = chars.next().map_or_else(String::new, |c| {
        format!("{}{}.", c.to_uppercase(), chars.as_str())
    });
    assert_eq!(
        ENGINE_MESSAGES.text(e.message_id(), &e.message_args()),
        expected
    );
}

#[test]
fn messages_of_values_match_their_english_errors() {
    use gk_core::{Day, DurationUnits, ProjectKey, TaskKey};
    same_as_display(&"2026-13-01".parse::<Day>().unwrap_err());
    same_as_display(&"soon".parse::<Day>().unwrap_err());
    same_as_display(&gk_core::parse_minutes("", DurationUnits::default()).unwrap_err());
    same_as_display(&gk_core::parse_minutes("90", DurationUnits::default()).unwrap_err());
    same_as_display(&gk_core::parse_minutes("1x", DurationUnits::default()).unwrap_err());
    same_as_display(&"a".parse::<ProjectKey>().unwrap_err());
    same_as_display(&"1AB".parse::<ProjectKey>().unwrap_err());
    same_as_display(&"A-B".parse::<ProjectKey>().unwrap_err());
    same_as_display(&"TM".parse::<TaskKey>().unwrap_err());
    same_as_display(&"x".parse::<gk_core::Id>().unwrap_err());
    same_as_display(&"someday".parse::<gk_core::Priority>().unwrap_err());
    same_as_display(&"chartreuse".parse::<gk_core::Hue>().unwrap_err());
    same_as_display(&gk_core::check_title(" ").unwrap_err());
    same_as_display(&gk_core::check_title(&"a".repeat(1000)).unwrap_err());
    same_as_display(&gk_core::check_comment(&"a".repeat(300 * 1024)).unwrap_err());
    same_as_display(&gk_core::check_page_title("a|b").unwrap_err());
    let start: Day = "2026-11-26".parse().unwrap();
    same_as_display(&gk_core::recurrence::parse("sometimes", start).unwrap_err());
    let mut rule = gk_core::recurrence::Rule::every(gk_core::recurrence::Frequency::Day, 0, start);
    same_as_display(&rule.check().unwrap_err());
    rule.interval = 1;
    rule.until = Some("2026-01-01".parse().unwrap());
    same_as_display(&rule.check().unwrap_err());
    rule.until = None;
    rule.count = Some(0);
    same_as_display(&rule.check().unwrap_err());
}
