// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask check-text`: the Rust side writes no English for people in
//! its code. What people read (errors and hints, the command line's output,
//! notifications) comes from the Fluent message files, so that it can be
//! translated; the UI has its own lint for the same rule.
//!
//! The check reads the crates whose text reaches people and refuses:
//!
//! - an `OpError` built from a literal or a `format!` with words in it;
//! - a hint (`with_hint`) given as such a literal;
//! - in the command line, which prints for people, a `print!`, `println!`,
//!   `eprint!`, `eprintln!`, `write!` or `writeln!` whose format string has
//!   words in it.
//!
//! Words are two letters or more, twice, with a space between. Tests (a
//! `#[cfg(test)]` module at the end of a file, `tests.rs`, `tests/`) may
//! write English. The few sentences that are allowed are listed in
//! [`ALLOWED`], each with its reason.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// The crates whose text reaches people, and whether their print macros
/// write for people (the command line) or for logs (everything else).
const CRATES: &[(&str, bool)] = &[
    ("crates/gk-engine/src", false),
    ("crates/gk-app/src", false),
    ("crates/gk-cli/src", true),
];

/// Text allowed in code, by file and literal, with the reason.
const ALLOWED: &[(&str, &str, &str)] = &[
    // Invariants of the engine: the catalog and the database always have
    // these, so the errors only report a bug; they are not translated.
    (
        "crates/gk-engine/src/ops/custom_field.rs",
        "no project",
        "an invariant: every field belongs to a project",
    ),
    (
        "crates/gk-engine/src/ops/history.rs",
        "no project",
        "an invariant: every change belongs to a project",
    ),
    (
        "crates/gk-engine/src/jobs.rs",
        "no recurrence.run",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/jobs.rs",
        "no automation.run",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/ops/automation.rs",
        "no handler for {op}",
        "an invariant: a rule's actions are commands of the catalog",
    ),
    (
        "crates/gk-engine/src/ops/automation.rs",
        "{op} is not a command",
        "an invariant: a rule's actions are commands of the catalog",
    ),
    (
        "crates/gk-engine/src/jobs.rs",
        "no trash.purge",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/jobs.rs",
        "no backup.restore",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/ops/file.rs",
        "file.attach is not in the catalog",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/transfer/csv/mod.rs",
        "transfer.import_csv is not in the catalog",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/transfer/wiki_import.rs",
        "wiki.import is not in the catalog",
        "an invariant: the operation is in the catalog",
    ),
    (
        "crates/gk-engine/src/transfer/import.rs",
        "no import",
        "an invariant: the operation is in the catalog",
    ),
];

/// The macros that print.
const PRINTS: &[&str] = &[
    "print!(",
    "println!(",
    "eprint!(",
    "eprintln!(",
    "write!(",
    "writeln!(",
];

/// One piece of English found in code.
#[derive(Debug, PartialEq, Eq)]
struct Found {
    line: usize,
    text: String,
}

pub(crate) fn run(root: &Path) -> Result<bool> {
    let mut found = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for (dir, prints_for_people) in CRATES {
        let mut files = Vec::new();
        collect(&root.join(dir), &mut files)?;
        files.sort();
        for file in files {
            let source = fs::read_to_string(&file)
                .with_context(|| format!("cannot read {}", file.display()))?;
            let name = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            for hit in check(&source, *prints_for_people) {
                let allowed = ALLOWED
                    .iter()
                    .position(|(path, text, _)| *path == name && *text == hit.text);
                if let Some(i) = allowed {
                    used[i] = true;
                } else {
                    found.push(format!("{name}:{}: \"{}\"", hit.line, hit.text));
                }
            }
        }
    }
    for ((path, text, _), used) in ALLOWED.iter().zip(&used) {
        if !used {
            found.push(format!("{path}: \"{text}\" is allowed but no longer there"));
        }
    }
    if found.is_empty() {
        eprintln!(
            "check-text: no English for people in the Rust code ({} allowed invariant errors)",
            ALLOWED.len()
        );
        Ok(true)
    } else {
        for line in &found {
            eprintln!("{line}");
        }
        eprintln!(
            "check-text: {} piece(s) of English in code; put the text in the crate's \
             i18n/en/*.ftl and use its message ID",
            found.len()
        );
        Ok(false)
    }
}

/// The Rust files under `dir`, without tests.
fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if name != "tests" {
                collect(&path, out)?;
            }
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(path);
        }
    }
    Ok(())
}

/// The English in `source`, without its test module.
fn check(source: &str, prints_for_people: bool) -> Vec<Found> {
    let code = source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap_or_default();
    let mut found = Vec::new();
    let mut look = |needle: &str| {
        for (at, _) in code.match_indices(needle) {
            // `println!(` inside `eprintln!(` is the same call.
            let joined = code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            if joined {
                continue;
            }
            let rest = &code[at + needle.len()..];
            for text in argument_texts(rest) {
                if has_words(&text) {
                    found.push(Found {
                        line: code[..at].lines().count().max(1),
                        text,
                    });
                }
            }
        }
    };
    for ctor in [
        "OpError::new(",
        "OpError::invalid(",
        "OpError::invalid_at(",
        "OpError::not_found(",
        "OpError::conflict(",
        "OpError::forbidden(",
        "OpError::limit(",
        "OpError::storage(",
    ] {
        look(ctor);
    }
    look(".with_hint(");
    if prints_for_people {
        for print in PRINTS {
            look(print);
        }
    }
    found.sort_by_key(|f| f.line);
    found
}

/// The literals among a call's arguments in `rest` (just after the
/// opening parenthesis): each argument that is a literal, or a `format!`
/// whose format string is one.
fn argument_texts(rest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = true;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        if start && !c.is_whitespace() {
            start = false;
            let arg = &rest[i..];
            let text = arg
                .strip_prefix('"')
                .or_else(|| arg.strip_prefix("format!(")?.trim_start().strip_prefix('"'));
            if let Some(text) = text.and_then(literal) {
                out.push(text);
            }
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            ',' if depth == 0 => start = true,
            _ => {}
        }
    }
    out
}

/// The literal that starts at `text` (just after its opening quote), up to
/// its closing quote.
fn literal(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

/// Two words of two letters or more with a space between, outside `{…}`
/// placeholders.
fn has_words(text: &str) -> bool {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => plain.push(' '),
        }
    }
    let word = |w: &str| w.chars().filter(|c| c.is_alphabetic()).count() >= 2;
    plain
        .split(' ')
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| word(pair[0]) && word(pair[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str, prints: bool) -> Vec<String> {
        check(source, prints).into_iter().map(|f| f.text).collect()
    }

    #[test]
    fn finds_english_in_errors_hints_and_prints() {
        let source = r#"
fn a() -> OpError {
    OpError::invalid(format!("The name {name} is taken."))
}
fn b() -> OpError {
    OpError::invalid_at(
        "key",
        "That isn't a key.",
    )
    .with_hint("Try another one.")
}
fn c(out: &mut String) {
    let _ = writeln!(out, "Nothing to show");
    println!("{}", text("cli-none", &[]));
}
"#;
        assert_eq!(
            texts(source, true),
            [
                "The name {name} is taken.",
                "That isn't a key.",
                "Try another one.",
                "Nothing to show"
            ]
        );
        assert_eq!(texts(source, false).len(), 3);
    }

    #[test]
    fn accepts_messages_codes_and_tests() {
        let source = r#"
fn a() -> OpError {
    messages::error("link-too-long", &[], |m| OpError::limit(m, 8))
}
fn b() -> OpError {
    OpError::storage(e.to_string())
}
fn c(out: &mut String) {
    let _ = writeln!(out, "{pad}{k}: {}", cell(v));
    eprintln!("glyphkeel-app: cannot hide the window: {err}");
}
#[cfg(test)]
mod tests {
    fn d() -> OpError { OpError::invalid("Some English here.") }
}
"#;
        assert_eq!(
            texts(source, true),
            ["glyphkeel-app: cannot hide the window: {err}"]
        );
        assert!(texts(source, false).is_empty());
    }

    #[test]
    fn words_are_two_letters_twice() {
        assert!(has_words("Not found."));
        assert!(!has_words("no"));
        assert!(!has_words("{pad}{k}: {v}"));
        assert!(!has_words("{field}: {} → {}"));
        assert!(has_words("{key} is due"));
    }

    #[test]
    fn the_rust_code_has_no_english_for_people() {
        let root = crate::workspace_root();
        assert!(run(&root).unwrap());
    }
}
