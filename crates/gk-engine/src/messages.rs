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

/// A file of messages in Fluent's syntax, in English and in the languages
/// it has been translated to, as other crates use it for their own text
/// (the command line's help and output, say).
///
/// [`text`](Self::text) shows a message in the process's language (see
/// [`crate::language`]), falling back to English for a message the
/// translation lacks; [`english`](Self::english) always in English, for
/// machine text: the `message` of an error, the output of an operation.
#[derive(Debug, Clone, Copy)]
pub struct Messages {
    english: &'static str,
    /// Translations by language subtag (`de`), or by whole tag (`pt-BR`).
    translations: &'static [(&'static str, &'static str)],
}

/// The engine's own messages.
pub const ENGINE: Messages = Messages::new(MESSAGES_FTL);

/// A value for a message's placeable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// Text, shown as it is.
    Text(String),
    /// A number (`1234.5`), shown in the language's format; it also chooses
    /// a plural variant by the language's rules.
    Number(String),
    /// A date (`2026-11-27`) or timestamp, shown as a date in the
    /// language's format.
    Date(String),
}

impl Arg {
    /// A number.
    #[must_use]
    pub fn number(n: impl std::fmt::Display) -> Self {
        Self::Number(n.to_string())
    }

    fn raw(&self) -> &str {
        match self {
            Self::Text(s) | Self::Number(s) | Self::Date(s) => s,
        }
    }
}

impl From<String> for Arg {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

/// How a message is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// English, numbers as given, English plural rules: machine text.
    English,
    /// The process's language and formats.
    Shown,
    /// The pseudo-locale.
    Pseudo,
}

/// The pattern of message `id` in file `ftl`: its first line and the
/// indented lines that continue it (the variants of a plural).
fn pattern_in(ftl: &str, id: &str) -> Option<String> {
    let mut lines = ftl.lines();
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

impl Messages {
    /// Messages in English only.
    #[must_use]
    pub const fn new(english: &'static str) -> Self {
        Self {
            english,
            translations: &[],
        }
    }

    /// Messages with translations, by language (`("de", include_str!(…))`).
    #[must_use]
    pub const fn with_translations(
        english: &'static str,
        translations: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            english,
            translations,
        }
    }

    /// The English file.
    #[must_use]
    pub const fn source(&self) -> &'static str {
        self.english
    }

    /// The English pattern of message `id`.
    fn pattern(&self, id: &str) -> Option<String> {
        pattern_in(self.english, id)
    }

    /// The pattern of `id` in language `tag`, and how to show it: the
    /// translation's, else the English one.
    fn shown_pattern(&self, id: &str, tag: &str) -> Option<(String, Mode)> {
        if crate::language::is_pseudo(tag) {
            return self.pattern(id).map(|p| (p, Mode::Pseudo));
        }
        let subtag = crate::language::language_subtag(tag);
        let translated = self
            .translations
            .iter()
            .filter(|(lang, _)| lang.eq_ignore_ascii_case(tag))
            .chain(
                self.translations
                    .iter()
                    .filter(|(lang, _)| lang.eq_ignore_ascii_case(subtag)),
            )
            .find_map(|(_, ftl)| pattern_in(ftl, id));
        translated
            .or_else(|| self.pattern(id))
            .map(|p| (p, Mode::Shown))
    }

    /// Message `id` in the process's language with its placeables filled
    /// in; the ID itself when the file has no such message.
    #[must_use]
    pub fn text(&self, id: &str, args: &[(&str, String)]) -> String {
        let args: Vec<(&str, Arg)> = args
            .iter()
            .map(|(k, v)| (*k, Arg::Text(v.clone())))
            .collect();
        self.format(id, &args)
    }

    /// Message `id` in the process's language, with typed placeables:
    /// numbers and dates in the language's formats.
    #[must_use]
    pub fn format(&self, id: &str, args: &[(&str, Arg)]) -> String {
        match self.shown_pattern(id, &crate::language::language()) {
            Some((p, mode)) => render(&p, args, mode, true),
            None => id.to_owned(),
        }
    }

    /// Message `id` in English with its placeables filled in; the ID itself
    /// when the file has no such message.
    #[must_use]
    pub fn english(&self, id: &str, args: &[(&str, String)]) -> String {
        let args: Vec<(&str, Arg)> = args
            .iter()
            .map(|(k, v)| (*k, Arg::Text(v.clone())))
            .collect();
        match self.pattern(id) {
            Some(p) => render(&p, &args, Mode::English, true),
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
        self.english
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

/// Message `id` in English with its placeables filled in: the engine's
/// text is machine text (errors, operation outputs), which the programs
/// show in the person's language by its ID.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    ENGINE.english(id, args)
}

/// The index just after the `}` that closes the placeable opened at
/// `open`; braces inside string literals (`{ "{" }`) don't count.
fn closing(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in s[open..].char_indices() {
        if quoted {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
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
/// selections on a number (`{ $count -> [one] … *[other] … }`), with the
/// plural rules and formats of `mode`. `outer` is the message itself, not
/// a variant inside it: the pseudo-locale brackets it.
fn render(pattern: &str, args: &[(&str, Arg)], mode: Mode, outer: bool) -> String {
    let value = |name: &str| args.iter().find(|(n, _)| *n == name).map(|(_, v)| v);
    let literal = |out: &mut String, text: &str| {
        if mode == Mode::Pseudo {
            out.push_str(&crate::language::pseudo_text(text));
        } else {
            out.push_str(text);
        }
    };
    let mut out = String::with_capacity(pattern.len() + 16);
    if outer && mode == Mode::Pseudo {
        out.push('[');
    }
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        literal(&mut out, &rest[..open]);
        let Some(close) = closing(rest, open) else {
            literal(&mut out, &rest[open..]);
            rest = "";
            break;
        };
        let inner = rest[open + 1..close - 1].trim();
        if let Some((selector, variants)) = split_selector(inner) {
            let name = selector.trim().trim_start_matches('$');
            out.push_str(&render(
                select(value(name), variants, mode),
                args,
                mode,
                false,
            ));
        } else if let Some(name) = inner.strip_prefix('$') {
            out.push_str(&match (value(name), mode) {
                (None, _) => name.to_owned(),
                (Some(v), Mode::English) => v.raw().to_owned(),
                (Some(Arg::Number(n)), _) => crate::language::format_number(n),
                (Some(Arg::Date(d)), _) => crate::language::format_date(d),
                (Some(Arg::Text(t)), _) => t.clone(),
            });
        } else {
            out.push_str(&unquote(inner));
        }
        rest = &rest[close..];
    }
    literal(&mut out, rest);
    if outer && mode == Mode::Pseudo {
        out.push(']');
    }
    out
}

/// A selection's selector and variants, when `inner` is one: the first `->`
/// outside a string literal.
fn split_selector(inner: &str) -> Option<(&str, &str)> {
    if inner.starts_with('"') {
        return None;
    }
    inner.split_once("->")
}

/// The text of a string literal (`"{"`, `"\u00A0"`), or `inner` as it is.
fn unquote(inner: &str) -> String {
    let Some(body) = inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return inner.to_owned();
    };
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some('U') => {
                let hex: String = chars.by_ref().take(6).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// The pattern of the variant that `value` selects: an exact number key
/// (`[0]`), else the plural category, else the default.
fn select<'a>(value: Option<&Arg>, variants: &'a str, mode: Mode) -> &'a str {
    let raw = value.map(Arg::raw);
    let category = match mode {
        Mode::Shown => raw.and_then(crate::language::plural_category),
        Mode::English | Mode::Pseudo => None,
    }
    .unwrap_or_else(|| match raw.map(str::parse::<f64>) {
        Some(Ok(n)) if (n - 1.0).abs() < f64::EPSILON => "one",
        _ => "other",
    });
    let mut fallback = "";
    let mut exact = None;
    let mut by_category = None;
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
        let key = key.trim();
        let text = text.trim();
        if default {
            fallback = text;
        }
        if exact.is_none() && Some(key) == raw {
            exact = Some(text);
        }
        if by_category.is_none() && key == category {
            by_category = Some(text);
        }
    }
    exact.or(by_category).unwrap_or(fallback)
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
        let one = render(
            pattern,
            &[("count", "1".into()), ("n", "3".into())],
            Mode::English,
            true,
        );
        let many = render(
            pattern,
            &[("count", "4".into()), ("n", "3".into())],
            Mode::English,
            true,
        );
        assert_eq!(one, "one task, not 3.");
        assert_eq!(many, "4 tasks, not 3.");
        assert_eq!(
            render("{ $x -> \n*[other] y\n}", &[], Mode::English, true),
            "y"
        );
    }

    const SAMPLE: Messages = Messages::with_translations(
        "greeting = Hello, { $name }!\nboth = Only in English\nbrace = a {\"{\"} b\ncount = { $n ->\n    [one] one task\n   *[other] { $n } tasks\n    }\n",
        &[(
            "pl",
            "greeting = Cześć, { $name }!\ncount = { $n ->\n    [one] jedno\n    [few] { $n } zadania\n   *[other] { $n } zadań\n    }\n",
        )],
    );

    #[test]
    fn messages_follow_the_language_and_fall_back_to_english() {
        let name = [("name", "Ola".to_owned())];
        crate::language::with_language("pl-PL", || {
            assert_eq!(SAMPLE.text("greeting", &name), "Cześć, Ola!");
            assert_eq!(SAMPLE.text("both", &[]), "Only in English");
            assert_eq!(
                SAMPLE.format("count", &[("n", Arg::number(3))]),
                "3 zadania"
            );
            assert_eq!(SAMPLE.format("count", &[("n", Arg::number(5))]), "5 zadań");
            assert_eq!(SAMPLE.english("greeting", &name), "Hello, Ola!");
        });
        crate::language::with_language("de", || {
            assert_eq!(SAMPLE.text("greeting", &name), "Hello, Ola!");
            assert_eq!(
                SAMPLE.format("count", &[("n", Arg::number(1234))]),
                "1.234 tasks"
            );
        });
        assert_eq!(SAMPLE.text("brace", &[]), "a { b");
    }

    #[test]
    fn the_pseudo_locale_changes_the_text_but_not_the_placeables() {
        crate::language::with_language("en-XA", || {
            assert_eq!(
                SAMPLE.text("greeting", &[("name", "Ola".to_owned())]),
                "[Ĥééĺĺóó, Ola!]"
            );
            assert_eq!(
                SAMPLE.format("count", &[("n", Arg::number(2))]),
                "[2 ţáášķš]"
            );
            assert_eq!(SAMPLE.text("brace", &[]), "[áá { ƀ]");
        });
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
