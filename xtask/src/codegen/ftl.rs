// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `crates/gk-cli/i18n/en/ops.ftl`: the help of the command line's
//! operation commands and their options, as Fluent messages, so that it can
//! be translated. The English text is the catalog's: each command's summary
//! and each input field's description. MCP tool descriptions keep reading
//! the catalog itself and stay English.

use std::collections::BTreeSet;
use std::fmt::Write;

use anyhow::{Result, bail};
use gk_api::{CATALOG, OpDescriptor};
use serde_json::Value;

const HEADER: &str = "# SPDX-License-Identifier: GPL-3.0-or-later\n\
# Copyright (C) 2026 Glyphkeel contributors\n\
#\n\
# Generated from the operation catalog by `cargo xtask gen`. Do not edit:\n\
# change the operation's summary or its field's description instead.\n\
#\n\
# The help of the command line's commands (op-<operation>) and of their\n\
# options (op-<operation>--<option>). Words in backquotes are commands,\n\
# options, values or query syntax: keep them as they are.\n";

/// The message ID of operation `op_id`'s command: `op-task-show`.
pub(crate) fn op_id(op_id: &str) -> String {
    format!("op-{}", op_id.replace(['.', '_'], "-"))
}

/// The message ID of option `field` of operation `op_id`:
/// `op-task-show--include`.
pub(crate) fn field_id(op_id: &str, field: &str) -> String {
    format!("{}--{}", self::op_id(op_id), field.replace('_', "-"))
}

/// The input fields of `d` that have a description, in schema order.
fn fields(d: &OpDescriptor) -> Vec<(String, String)> {
    let schema = gk_api::schema::portable(&(d.input_schema)().to_value());
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| {
            props
                .iter()
                .filter_map(|(name, prop)| {
                    let text = prop.get("description").and_then(Value::as_str)?;
                    Some((name.clone(), text.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `text` as a Fluent pattern: braces as string literals, each further
/// line indented, a line that Fluent would read as syntax (`[`, `*`, `.`
/// at its start) and an empty line written as literals.
pub(crate) fn pattern(text: &str) -> String {
    let escape = |line: &str| {
        let mut out = String::with_capacity(line.len());
        for c in line.chars() {
            match c {
                '{' => out.push_str("{\"{\"}"),
                '}' => out.push_str("{\"}\"}"),
                c => out.push(c),
            }
        }
        out
    };
    let mut out = String::new();
    for (i, line) in text.trim_end().lines().enumerate() {
        let line = line.trim_end();
        if i == 0 {
            out.push_str(&escape(line.trim_start()));
            continue;
        }
        out.push_str("\n    ");
        let line = line.trim_start();
        match line.chars().next() {
            None => out.push_str("{\"\"}"),
            Some(c @ ('[' | '*' | '.')) => {
                let _ = write!(out, "{{\"{c}\"}}{}", escape(&line[1..]));
            }
            Some(_) => out.push_str(&escape(line)),
        }
    }
    out
}

/// The file.
pub(super) fn cli_ops() -> Result<String> {
    let mut s = String::from(HEADER);
    let mut seen = BTreeSet::new();
    let mut entry = |s: &mut String, id: String, text: &str| -> Result<()> {
        if text.trim().is_empty() {
            bail!("{id} has no text");
        }
        if !seen.insert(id.clone()) {
            bail!("two commands or options get the message ID {id}");
        }
        let _ = writeln!(s, "{id} = {}", pattern(text));
        Ok(())
    };
    for d in CATALOG {
        let Some(command) = d.cli else { continue };
        let _ = write!(s, "\n## glyphkeel {command}\n\n");
        entry(&mut s, op_id(d.id), d.summary)?;
        for (field, text) in fields(d) {
            entry(&mut s, field_id(d.id, &field), &text)?;
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_becomes_a_safe_pattern() {
        assert_eq!(pattern("Plain."), "Plain.");
        assert_eq!(
            pattern("A {x} and }{.\n[not a variant]\n*not default\n.not an attribute\n\nend"),
            "A {\"{\"}x{\"}\"} and {\"}\"}{\"{\"}.\n    {\"[\"}not a variant]\n    {\"*\"}not default\n    {\".\"}not an attribute\n    {\"\"}\n    end"
        );
    }

    #[test]
    fn every_command_and_option_has_a_message() {
        let file = cli_ops().expect("generates");
        for d in CATALOG.iter().filter(|d| d.cli.is_some()) {
            assert!(file.contains(&format!("\n{} = ", op_id(d.id))), "{}", d.id);
            for (field, _) in fields(d) {
                assert!(
                    file.contains(&format!("\n{} = ", field_id(d.id, &field))),
                    "{}.{field}",
                    d.id
                );
            }
        }
    }
}
