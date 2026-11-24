// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! User-facing messages by ID. The English text lives in
//! `i18n/en/engine.ftl`; errors carry the message's ID and arguments
//! (`message_id`, `args`) next to the English text, so the app can show its
//! own translation.

use gk_api::OpError;
use serde_json::{Map, Value};

use crate::failure::Failure;

/// The English messages, in Fluent's syntax.
pub const MESSAGES_FTL: &str = include_str!("../i18n/en/engine.ftl");

/// A file of English messages in Fluent's syntax, as other crates use
/// it for their own text (the command line's help and output, say).
#[derive(Debug, Clone, Copy)]
pub struct Messages(pub &'static str);

/// The engine's own messages.
pub const ENGINE: Messages = Messages(MESSAGES_FTL);

impl Messages {
    /// The English pattern of message `id`: its first line and the
    /// indented lines that continue it (the variants of a plural).
    fn pattern(&self, id: &str) -> Option<String> {
        let mut lines = self.0.lines();
        let first = lines.find_map(|line| {
            let (name, value) = line.split_once(" =")?;
            (!line.starts_with([' ', '#']) && name.trim() == id).then_some(value.trim_start())
        })?;
        let mut out = first.to_owned();
        for line in lines.take_while(|l| l.starts_with(' ')) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line.trim());
        }
        Some(out)
    }

    /// Message `id` in English with its placeables filled in; the ID itself
    /// when the file has no such message.
    #[must_use]
    pub fn text(&self, id: &str, args: &[(&str, String)]) -> String {
        match self.pattern(id) {
            Some(p) => render(&p, args),
            None => id.to_owned(),
        }
    }

    /// Whether the file has message `id`.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.pattern(id).is_some()
    }

    /// The IDs of every message in the file, in file order.
    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.0
            .lines()
            .filter(|l| !l.starts_with(['#', ' ']))
            .filter_map(|l| l.split_once(" =").map(|(id, _)| id.trim()))
            .filter(|id| !id.is_empty())
            .collect()
    }
}

impl Messages {
    /// Checks the file against the Rust `sources` that use it, for the
    /// translation checks: returns the message IDs the sources use that
    /// the file lacks, and the file's IDs no source uses. A string literal
    /// in the sources counts as a message ID when it is in kebab case and
    /// starts with the first word of one of the file's IDs (`task-…`);
    /// literals in `ignore` don't count. A `#[cfg(test)]` module at the end
    /// of a file is left out, so a message only a test names is unused.
    #[must_use]
    pub fn check_usage(
        &self,
        sources: &[&str],
        ignore: &[&str],
    ) -> (Vec<String>, Vec<&'static str>) {
        let ids = self.ids();
        let prefixes: std::collections::BTreeSet<&str> =
            ids.iter().filter_map(|id| id.split('-').next()).collect();
        let mut used = std::collections::BTreeSet::new();
        for source in sources {
            let code = source
                .split("#[cfg(test)]\nmod tests {")
                .next()
                .unwrap_or_default();
            used.extend(kebab_literals(code));
        }
        let missing = used
            .iter()
            .filter(|l| !ignore.contains(&l.as_str()))
            .filter(|l| l.split('-').next().is_some_and(|p| prefixes.contains(p)))
            .filter(|l| !self.has(l))
            .cloned()
            .collect();
        let unused = ids.into_iter().filter(|id| !used.contains(*id)).collect();
        (missing, unused)
    }
}

/// The string literals in `code` that are in kebab case (`task-not-found`).
fn kebab_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        let literal = &after[..end];
        let kebab = literal.contains('-')
            && literal.starts_with(|c: char| c.is_ascii_lowercase())
            && literal
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if kebab {
            out.push(literal.to_owned());
            rest = &after[end + 1..];
        } else {
            // Not an ID: go on from the closing quote, which may open the
            // next literal when this one was code between two literals.
            rest = after;
        }
    }
    out
}

/// Message `id` in English with its placeables filled in.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    ENGINE.text(id, args)
}

/// The index just after the `}` that closes the placeable opened at
/// `open`.
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

/// Sets `error`'s hint to message `id`, and adds the ID and its arguments
/// (`hint_id`, `hint_args`) so the app can show its own translation.
pub(crate) fn hint(error: OpError, id: &str, args: &[(&str, String)]) -> OpError {
    let mut error = error.with_hint(text(id, args)).with_detail("hint_id", id);
    if !args.is_empty() {
        let map: Map<String, Value> = args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::from(v.clone())))
            .collect();
        error = error.with_detail("hint_args", Value::Object(map));
    }
    error
}

/// Adds message `id` and its arguments to `error`.
pub(crate) fn tag(error: OpError, id: &str, args: &[(&str, String)]) -> OpError {
    let mut error = error.with_detail("message_id", id);
    if !args.is_empty() {
        let map: Map<String, Value> = args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::from(v.clone())))
            .collect();
        error = error.with_detail("args", Value::Object(map));
    }
    error
}

/// The error `make` builds from message `id`'s English text, with the ID
/// and arguments added: `with("view-not-found", &args, |m|
/// OpError::not_found(m, &names))`.
pub(crate) fn with(
    id: &str,
    args: &[(&str, String)],
    make: impl FnOnce(String) -> OpError,
) -> OpError {
    tag(make(text(id, args)), id, args)
}

/// An `invalid` error on input field `path`, with message `id`, as an
/// [`OpError`].
pub(crate) fn invalid_at(path: &str, id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::invalid_at(path, text(id, args)), id, args)
}

/// A `storage` error with message `id`.
pub(crate) fn storage(id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::storage(text(id, args)), id, args)
}

/// A `forbidden` error with message `id`.
pub(crate) fn forbidden(id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::forbidden(text(id, args)), id, args)
}

/// An `invalid` error on input field `path`, with message `id`.
pub(crate) fn invalid(path: &str, id: &str, args: &[(&str, String)]) -> Failure {
    Failure::Op(tag(OpError::invalid_at(path, text(id, args)), id, args))
}

/// An error of kind `code` with message `id`.
pub(crate) fn error(code: gk_api::ErrorCode, id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::new(code, text(id, args)), id, args)
}

/// A `not_found` error with message `id`.
pub(crate) fn not_found(id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::not_found(text(id, args), &[]), id, args)
}

/// A `conflict` error with message `id`.
pub(crate) fn conflict(id: &str, args: &[(&str, String)]) -> OpError {
    tag(OpError::conflict(text(id, args)), id, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_has_an_id_and_renders() {
        for line in MESSAGES_FTL.lines() {
            if line.starts_with(['#', ' ']) || line.trim().is_empty() {
                continue;
            }
            let (id, value) = line.split_once(" = ").expect("id = text");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
                "{id}"
            );
            assert!(!value.trim().is_empty(), "{id}");
        }
        assert_eq!(
            text("wiki-too-deep", &[("max", "8".to_owned())]),
            "Pages can be nested at most 8 deep."
        );
        assert_eq!(text("no-such-message", &[]), "no-such-message");
        for id in MESSAGES_FTL
            .lines()
            .filter(|l| !l.starts_with(['#', ' ']) && l.contains(" = "))
            .filter_map(|l| l.split_once(" = ").map(|(id, _)| id))
        {
            let shown = text(id, &[("count", "2".into()), ("max", "2".into())]);
            assert!(!shown.contains(['{', '}']), "{id}: {shown}");
            assert!(
                !shown.contains("*[") && !shown.contains("[one]"),
                "{id}: {shown}"
            );
        }
    }

    #[test]
    fn plurals_pick_their_variant() {
        let pattern = "{ $count ->\n[one] one task\n*[other] { $count } tasks\n}, not { $n }.";
        let one = render(pattern, &[("count", "1".into()), ("n", "3".into())]);
        let many = render(pattern, &[("count", "4".into()), ("n", "3".into())]);
        assert_eq!(one, "one task, not 3.");
        assert_eq!(many, "4 tasks, not 3.");
        assert_eq!(render("{ $x -> \n*[other] y\n}", &[]), "y");
    }

    #[test]
    fn errors_carry_the_id_and_arguments() {
        let e = conflict(
            "wiki-title-taken",
            &[("project", "TM".into()), ("title", "Setup".into())],
        );
        assert_eq!(e.message, "Project TM already has a page called \"Setup\".");
        assert_eq!(e.details["message_id"], "wiki-title-taken");
        assert_eq!(e.details["args"]["title"], "Setup");
    }
}
