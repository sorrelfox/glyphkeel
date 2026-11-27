// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The app's own text shown by Rust (native dialogs, file choosers), by
//! message ID. The English text lives in `i18n/en/app.ftl`, in Fluent's
//! syntax, so that it is translated with the page's messages.

/// The English messages.
const MESSAGES_FTL: &str = include_str!("../i18n/en/app.ftl");

/// Message `id` with its placeables filled in; the ID itself when there is
/// no such message.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    match pattern(id) {
        Some(p) => render(&p, args),
        None => id.to_owned(),
    }
}

fn args_value(args: &[(&str, String)]) -> serde_json::Value {
    serde_json::Value::Object(
        args.iter()
            .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(v.clone())))
            .collect(),
    )
}

/// The error `make` builds from message `id`'s English text, with the ID
/// and its arguments (`message_id`, `args`) added, so that the page shows
/// it in the person's language: `error("link-too-long", &[], |m|
/// OpError::limit(m, max))`.
pub(crate) fn error(
    id: &str,
    args: &[(&str, String)],
    make: impl FnOnce(String) -> gk_api::OpError,
) -> gk_api::OpError {
    let mut error = make(text(id, args)).with_detail("message_id", id);
    if !args.is_empty() {
        error = error.with_detail("args", args_value(args));
    }
    error
}

/// Sets `error`'s hint to message `id`, with its ID and arguments
/// (`hint_id`, `hint_args`).
pub(crate) fn hint(error: gk_api::OpError, id: &str, args: &[(&str, String)]) -> gk_api::OpError {
    let mut error = error.with_hint(text(id, args)).with_detail("hint_id", id);
    if !args.is_empty() {
        error = error.with_detail("hint_args", args_value(args));
    }
    error
}

/// The pattern of message `id`: its first line and the indented lines that
/// continue it (the variants of a plural).
fn pattern(id: &str) -> Option<String> {
    let mut lines = MESSAGES_FTL.lines();
    let first = lines.find_map(|line| {
        let (name, value) = line.split_once(" = ")?;
        (!line.starts_with([' ', '#']) && name.trim() == id).then_some(value)
    })?;
    let mut out = first.to_owned();
    for line in lines.take_while(|l| l.starts_with(' ')) {
        out.push('\n');
        out.push_str(line.trim());
    }
    Some(out)
}

/// The index just after the `}` that closes the placeable opened at `open`.
fn closing(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Fills in a pattern: `{ $name }` placeables, `{ "text" }` literals and
/// selections on a number (`{ $count -> [one] … *[other] … }`) with
/// English plural rules.
fn render(pattern: &str, args: &[(&str, String)]) -> String {
    let value = |name: &str| {
        args.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    };
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = closing(rest, open) else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = rest[open + 1..close - 1].trim();
        if let Some((selector, variants)) = inner.split_once("->") {
            let name = selector.trim().trim_start_matches('$');
            out.push_str(&render(select(value(name), variants), args));
        } else if let Some(name) = inner.strip_prefix('$') {
            out.push_str(value(name).unwrap_or(name));
        } else {
            out.push_str(inner.trim_matches('"'));
        }
        rest = &rest[close..];
    }
    out.push_str(rest);
    out
}

/// The pattern of the variant that `value` selects.
fn select<'a>(value: Option<&str>, variants: &'a str) -> &'a str {
    let category = match value.map(str::parse::<f64>) {
        Some(Ok(n)) if (n - 1.0).abs() < f64::EPSILON => "one",
        _ => "other",
    };
    let mut fallback = "";
    let mut chosen = None;
    for line in variants
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with(['[', '*']))
    {
        let default = line.starts_with('*');
        let line = line.trim_start_matches('*');
        let Some((key, text)) = line.trim_start_matches('[').split_once(']') else {
            continue;
        };
        let text = text.trim();
        if default {
            fallback = text;
        }
        if chosen.is_none() && (Some(key) == value || key == category) {
            chosen = Some(text);
        }
    }
    chosen.unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_renders_without_syntax_left_over() {
        let ids: Vec<&str> = MESSAGES_FTL
            .lines()
            .filter(|l| !l.starts_with(['#', ' ']) && l.contains(" = "))
            .filter_map(|l| l.split_once(" = ").map(|(id, _)| id))
            .collect();
        assert!(ids.len() > 10);
        for id in ids {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
                "{id}"
            );
            let args = [
                ("count", "2".to_owned()),
                ("name", "daily-2026-11-24".to_owned()),
                ("summary", "Something.".to_owned()),
            ];
            let shown = text(id, &args);
            assert!(!shown.trim().is_empty(), "{id}");
            assert!(!shown.contains(['{', '}', '[']), "{id}: {shown}");
        }
    }

    /// The Rust sources of the app, without their tests.
    fn sources(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("source folder") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "tests.rs")
            {
                out.push(std::fs::read_to_string(&path).expect("source file"));
            }
        }
    }

    #[test]
    fn every_message_the_app_names_exists_and_every_message_is_named() {
        let mut texts = Vec::new();
        sources(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut texts,
        );
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        // Window labels and their prefixes, not messages.
        let labels = ["quick-add", "task-", "wiki-"];
        let (missing, unused) = gk_engine::Messages(MESSAGES_FTL).check_usage(&refs, &labels);
        assert!(
            missing.is_empty(),
            "named in the code but not in app.ftl: {missing:?}"
        );
        assert!(
            unused.is_empty(),
            "in app.ftl but named nowhere: {unused:?}"
        );
    }

    #[test]
    fn plurals_and_placeables_fill_in() {
        assert_eq!(
            text("confirm-purge-items", &[("count", "1".into())]),
            "Delete one item in the trash for good? Its text is removed from the history too."
        );
        assert!(
            text("confirm-purge-items", &[("count", "3".into())]).starts_with("Delete 3 items")
        );
        assert!(
            text("confirm-backup-delete", &[("name", "manual-1".into())]).contains("“manual-1”")
        );
        assert_eq!(text("no-such-message", &[]), "no-such-message");
    }
}
