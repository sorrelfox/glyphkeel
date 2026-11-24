// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The command line's own text by message ID. The English text lives in
//! `i18n/en/cli.ftl`; errors carry the message's ID and arguments
//! (`message_id`, `args`) as the engine's do.

use gk_api::OpError;
use gk_engine::Messages;
use serde_json::{Map, Value};

/// The English messages, in Fluent's syntax.
pub(crate) const CLI_FTL: &str = include_str!("../i18n/en/cli.ftl");

const CLI: Messages = Messages(CLI_FTL);

/// Message `id` with its placeables filled in.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    CLI.text(id, args)
}

fn args_value(args: &[(&str, String)]) -> Value {
    let map: Map<String, Value> = args
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Value::from(v.clone())))
        .collect();
    Value::Object(map)
}

/// The error `make` builds from message `id`, with the ID and arguments
/// added.
pub(crate) fn error(
    id: &str,
    args: &[(&str, String)],
    make: impl FnOnce(String) -> OpError,
) -> OpError {
    let mut error = make(text(id, args)).with_detail("message_id", id);
    if !args.is_empty() {
        error = error.with_detail("args", args_value(args));
    }
    error
}

/// Sets `error`'s hint to message `id`, with its ID (`hint_id`).
pub(crate) fn hint(error: OpError, id: &str, args: &[(&str, String)]) -> OpError {
    let mut error = error.with_hint(text(id, args)).with_detail("hint_id", id);
    if !args.is_empty() {
        error = error.with_detail("hint_args", args_value(args));
    }
    error
}
