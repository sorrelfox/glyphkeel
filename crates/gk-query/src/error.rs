// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Errors in a query: where the mistake is, a message ID with its
//! arguments, and the English text. The English messages live in
//! `i18n/en/query.ftl`, which the app's translations follow.

use std::fmt;

use crate::syntax::Span;

/// The English messages, in Fluent's syntax.
pub const MESSAGES_FTL: &str = include_str!("../i18n/en/query.ftl");

/// A mistake in a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    /// Where the mistake is, in bytes of the query text.
    pub span: Span,
    /// The message's ID in the message files (`query-unknown-field`).
    pub id: &'static str,
    /// The message's arguments, by name.
    pub args: Vec<(&'static str, String)>,
}

impl QueryError {
    /// An error with no arguments yet.
    #[must_use]
    pub fn new(id: &'static str, span: Span) -> Self {
        Self {
            span,
            id,
            args: Vec::new(),
        }
    }

    /// Adds an argument.
    #[must_use]
    pub fn arg(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    /// The value of argument `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The message in English.
    #[must_use]
    pub fn message(&self) -> String {
        render(self.id, &self.args)
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for QueryError {}

/// The English pattern of message `id`.
#[must_use]
pub fn pattern(id: &str) -> Option<&'static str> {
    MESSAGES_FTL.lines().find_map(|line| {
        let (name, value) = line.split_once(" = ")?;
        (name.trim() == id).then_some(value)
    })
}

/// Message `id` in English with its placeables filled in.
#[must_use]
pub fn render(id: &str, args: &[(&'static str, String)]) -> String {
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

/// The closest of `choices` to `word`, if one is close enough to be what
/// was meant.
#[must_use]
pub fn closest<'a>(word: &str, choices: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let word = word.to_lowercase();
    let limit = match word.chars().count() {
        0..=2 => 1,
        3..=5 => 2,
        _ => 3,
    };
    choices
        .into_iter()
        .map(|c| (distance(&word, &c.to_lowercase()), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// The edit distance between two words, with swapped neighbours counting
/// as one edit.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 3 || a.len() > 64 || b.len() > 64 {
        return usize::MAX;
    }
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_render_with_their_arguments() {
        let e = QueryError::new("query-unknown-field", Span::new(0, 6))
            .arg("field", "stauts")
            .arg("suggestion", "status");
        assert_eq!(
            e.message(),
            "Unknown field \"stauts\". Did you mean \"status\"?"
        );
    }

    #[test]
    fn every_message_line_is_well_formed() {
        for line in MESSAGES_FTL.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (id, value) = line.split_once(" = ").expect("id = value");
            assert!(id.starts_with("query-"), "{id}");
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{id}"
            );
            assert_eq!(
                value.matches('{').count(),
                value.matches("{ $").count(),
                "{id}: only placeables use braces"
            );
        }
    }

    #[test]
    fn close_words_are_suggested() {
        let fields = ["status", "start", "subtasks", "spent"];
        assert_eq!(closest("stauts", fields), Some("status"));
        assert_eq!(closest("strat", fields), Some("start"));
        assert_eq!(closest("zzzzzz", fields), None);
    }
}
