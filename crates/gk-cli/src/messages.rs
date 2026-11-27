// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The command line's own text by message ID, in the language the command
//! line was started in (see [`choose_language`]). The English text lives
//! in `i18n/en/cli.ftl`, and the help of the operations' commands and
//! options in `i18n/en/ops.ftl`, which `cargo xtask gen` writes from the
//! operation catalog.
//!
//! Errors carry the message's ID and arguments (`message_id`, `args`) next
//! to their English `message`, as the engine's do: `--json` prints them as
//! they are, and the text for people is the message in the chosen
//! language.

use gk_api::{OpDescriptor, OpError};
use gk_engine::language;
use gk_engine::{ENGINE_MESSAGES, MessageArg, Messages};
use serde_json::{Map, Value};

/// The English messages, in Fluent's syntax.
pub(crate) const CLI_FTL: &str = include_str!("../i18n/en/cli.ftl");

/// The help of the operations' commands and options, generated from the
/// operation catalog.
pub(crate) const OPS_FTL: &str = include_str!("../i18n/en/ops.ftl");

const CLI: Messages = Messages::new(CLI_FTL);

/// The operations' help.
pub(crate) const OPS: Messages = Messages::new(OPS_FTL);

/// The query language's messages, which the engine's errors name too.
const QUERY: Messages = Messages::new(gk_query::error::MESSAGES_FTL);

/// The command line's option that chooses the language.
pub(crate) const LANG_FLAG: &str = "lang";

/// Placeables that hold a count or another number, shown in the language's
/// number format and choosing its plural form.
const NUMBER_ARGS: &[&str] = &[
    "count", "total", "shown", "limit", "minimum", "columns", "tasks", "created", "closed", "open",
    "percent", "cycle", "lead", "place",
];

/// Placeables that hold a day (`2026-11-27`), shown in the language's date
/// format.
const DATE_ARGS: &[&str] = &["due", "from", "to"];

/// Chooses the language of the command line's text, from the first of:
/// the `--lang` option (`flag`), `GLYPHKEEL_LANG`, the language chosen in
/// the app's Preferences, the system's language; else English.
pub(crate) fn choose_language(flag: Option<&str>) {
    let env = std::env::var(language::LANG_ENV).ok();
    let chosen = language::resolve(
        flag,
        env.as_deref(),
        language::ui_language_setting().as_deref(),
        language::system_language().as_deref(),
    );
    language::set_language(&chosen);
}

/// A placeable's value, typed by its name.
fn typed(name: &str, value: &str) -> MessageArg {
    if NUMBER_ARGS.contains(&name) && value.parse::<f64>().is_ok() {
        MessageArg::Number(value.to_owned())
    } else if DATE_ARGS.contains(&name) && is_day(value) {
        MessageArg::Date(value.to_owned())
    } else {
        MessageArg::Text(value.to_owned())
    }
}

/// A day written `YYYY-MM-DD`.
pub(crate) fn is_day(value: &str) -> bool {
    value.len() == 10
        && value.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

/// Message `id` in the chosen language with its placeables filled in:
/// counts and days in the language's formats.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    let args: Vec<(&str, MessageArg)> = args
        .iter()
        .map(|(name, value)| (*name, typed(name, value)))
        .collect();
    CLI.format(id, &args)
}

/// Message `id` in English: an error's `message`, which is machine text.
fn english(id: &str, args: &[(&str, String)]) -> String {
    CLI.english(id, args)
}

/// The ID of the message with the help of operation `op_id`'s command:
/// `op-task-show` for `task.show`.
pub(crate) fn op_message_id(op_id: &str) -> String {
    format!("op-{}", op_id.replace(['.', '_'], "-"))
}

/// The ID of the message with the help of option `field` of operation
/// `op_id`: `op-task-show--include`.
pub(crate) fn field_message_id(op_id: &str, field: &str) -> String {
    format!("{}--{}", op_message_id(op_id), field.replace('_', "-"))
}

/// The help line of operation `op`'s command in the chosen language.
pub(crate) fn op_summary(op: &OpDescriptor) -> String {
    let id = op_message_id(op.id);
    if OPS.has(&id) {
        OPS.text(&id, &[])
    } else {
        op.summary.to_owned()
    }
}

/// The help of option `field` of operation `op_id` in the chosen language;
/// `english` (the catalog's text) when the file lacks it.
pub(crate) fn field_help(op_id: &str, field: &str, english: &str) -> String {
    let id = field_message_id(op_id, field);
    if OPS.has(&id) {
        OPS.text(&id, &[])
    } else {
        english.to_owned()
    }
}

/// Message `id` named by an error, in the chosen language, from whichever
/// file has it: the command line's, the engine's or the query language's.
fn named(id: &str, args: Option<&Value>) -> Option<String> {
    let args: Vec<(&str, String)> = args
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(k, v)| {
                    let v = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.as_str(), v)
                })
                .collect()
        })
        .unwrap_or_default();
    if CLI.has(id) {
        Some(text(id, &args))
    } else if ENGINE_MESSAGES.has(id) {
        Some(ENGINE_MESSAGES.text(id, &args))
    } else if QUERY.has(id) {
        Some(QUERY.text(id, &args))
    } else {
        None
    }
}

/// An error's message for people, in the chosen language when its message
/// ID is known; its English `message` otherwise.
pub(crate) fn shown_message(e: &OpError) -> String {
    e.details
        .get("message_id")
        .and_then(Value::as_str)
        .and_then(|id| named(id, e.details.get("args")))
        .unwrap_or_else(|| e.message.clone())
}

/// An error's hint for people, in the chosen language when its message ID
/// is known.
pub(crate) fn shown_hint(e: &OpError) -> Option<String> {
    let hint = e.hint.as_ref()?;
    Some(
        e.details
            .get("hint_id")
            .and_then(Value::as_str)
            .and_then(|id| named(id, e.details.get("hint_args")))
            .unwrap_or_else(|| hint.clone()),
    )
}

fn args_value(args: &[(&str, String)]) -> Value {
    let map: Map<String, Value> = args
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Value::from(v.clone())))
        .collect();
    Value::Object(map)
}

/// The error `make` builds from message `id`'s English text, with the ID
/// and arguments added.
pub(crate) fn error(
    id: &str,
    args: &[(&str, String)],
    make: impl FnOnce(String) -> OpError,
) -> OpError {
    let mut error = make(english(id, args)).with_detail("message_id", id);
    if !args.is_empty() {
        error = error.with_detail("args", args_value(args));
    }
    error
}

/// Sets `error`'s hint to message `id`, with its ID (`hint_id`).
pub(crate) fn hint(error: OpError, id: &str, args: &[(&str, String)]) -> OpError {
    let mut error = error
        .with_hint(english(id, args))
        .with_detail("hint_id", id);
    if !args.is_empty() {
        error = error.with_detail("hint_args", args_value(args));
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_days_take_the_languages_formats() {
        language::with_language("de", || {
            assert_eq!(
                text(
                    "cli-shown-of-total",
                    &[("shown", "50".into()), ("total", "1234".into())]
                ),
                "50 of 1.234"
            );
            assert_eq!(
                text(
                    "cli-dashboard-created-closed",
                    &[
                        ("from", "2026-11-23".into()),
                        ("to", "2026-11-29".into()),
                        ("created", "1".into()),
                        ("closed", "2".into()),
                        ("open", "3".into()),
                    ],
                ),
                "23.11.2026 to 29.11.2026: 1 created, 2 closed, 3 open at the end"
            );
            // A name made of digits stays as it is.
            assert_eq!(
                text("cli-workspace-created", &[("name", "2026".into())]),
                "Made the workspace \"2026\"."
            );
        });
    }

    #[test]
    fn errors_keep_english_and_show_the_language() {
        language::with_language("en-XA", || {
            let e = hint(
                error("cli-input-not-object", &[], OpError::invalid),
                "cli-no-home-hint",
                &[],
            );
            assert_eq!(e.message, "The input must be a JSON object.");
            assert!(
                shown_message(&e).starts_with("[Ţĥéé"),
                "{}",
                shown_message(&e)
            );
            assert!(shown_hint(&e).is_some_and(|h| h.starts_with('[')));
            let plain = OpError::invalid("Plain.".to_owned());
            assert_eq!(shown_message(&plain), "Plain.");
        });
    }

    #[test]
    fn message_ids_of_operations() {
        assert_eq!(op_message_id("task.set_due"), "op-task-set-due");
        assert_eq!(
            field_message_id("task.show", "client_request_id"),
            "op-task-show--client-request-id"
        );
    }
}
