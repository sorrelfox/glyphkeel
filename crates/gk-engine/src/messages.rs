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

/// The English pattern of message `id`.
fn pattern(id: &str) -> Option<&'static str> {
    MESSAGES_FTL.lines().find_map(|line| {
        let (name, value) = line.split_once(" = ")?;
        (name.trim() == id).then_some(value)
    })
}

/// Message `id` in English with its placeables filled in.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    let Some(pattern) = pattern(id) else {
        return id.to_owned();
    };
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    while let Some(open) = rest.find("{ $") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 3..];
        let Some(close) = after.find(" }") else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, v)) => out.push_str(v),
            None => out.push_str(name),
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
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

/// An `invalid` error on input field `path`, with message `id`.
pub(crate) fn invalid(path: &str, id: &str, args: &[(&str, String)]) -> Failure {
    Failure::Op(tag(OpError::invalid_at(path, text(id, args)), id, args))
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
            if line.starts_with('#') || line.trim().is_empty() {
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
