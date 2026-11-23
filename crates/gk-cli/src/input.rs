// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An operation's input on the command line: one option per field of its
//! JSON Schema, positional arguments where the command line names them, and
//! `--input` for the whole input as JSON.

use std::io::Read;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches};
use gk_api::{OpDescriptor, OpError};
use serde_json::{Map, Value};

/// The option that takes the whole input as JSON, or `-` for standard input.
pub(crate) const INPUT_ARG: &str = "input";
/// The option that confirms a permanent operation in advance.
pub(crate) const YES_ARG: &str = "yes";

/// Fields given as positional arguments rather than options, per operation,
/// where the catalog's command doesn't name them itself (`task show <key>`
/// does).
const POSITIONALS: &[(&str, &str)] = &[("history.revert", "groups")];

/// How one field's text on the command line becomes JSON.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// A boolean that is `false` unless given: `--dry-run`.
    Flag,
    /// `true` or `false`.
    Bool,
    /// A whole number.
    Integer,
    /// Any number.
    Number,
    /// Text.
    Text,
    /// One of a few words.
    Choice(Vec<String>),
    /// Anything else (objects, lists of objects), written as JSON.
    Json,
}

/// One field of an operation's input.
#[derive(Debug, Clone)]
pub(crate) struct Field {
    /// The JSON name.
    pub(crate) name: String,
    kind: Kind,
    /// A list: the option may be repeated.
    many: bool,
    required: bool,
    positional: bool,
    help: Option<String>,
}

impl Field {
    /// The long option, `--dry-run` for `dry_run`.
    pub(crate) fn long(&self) -> String {
        self.name.replace('_', "-")
    }
}

/// The fields of `op`'s input, in schema order.
pub(crate) fn fields(op: &OpDescriptor, extra_positionals: &[String]) -> Vec<Field> {
    let schema = gk_api::schema::portable(&(op.input_schema)().to_value());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let positional = |name: &str| {
        extra_positionals.iter().any(|p| p == name)
            || POSITIONALS.iter().any(|(id, f)| *id == op.id && *f == name)
    };
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .map(|(name, prop)| {
            let (kind, many) = match prop.get("type").and_then(Value::as_str) {
                Some("array") => {
                    let item = prop.get("items").map(kind_of).unwrap_or(Kind::Json);
                    match item {
                        Kind::Json | Kind::Flag => (Kind::Json, false),
                        k => (k, true),
                    }
                }
                _ => (kind_of(prop), false),
            };
            Field {
                name: name.clone(),
                kind,
                many,
                required: required.contains(&name.as_str()),
                positional: positional(name),
                help: prop
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }
        })
        .collect()
}

fn kind_of(prop: &Value) -> Kind {
    let choices = choices(prop);
    if !choices.is_empty() {
        return Kind::Choice(choices);
    }
    match prop.get("type").and_then(Value::as_str) {
        Some("boolean") if prop.get("default") == Some(&Value::Bool(false)) => Kind::Flag,
        Some("boolean") => Kind::Bool,
        Some("integer") => Kind::Integer,
        Some("number") => Kind::Number,
        Some("string") => Kind::Text,
        _ => Kind::Json,
    }
}

/// The words of an enum: `enum: [...]`, or `oneOf`/`anyOf` branches with a
/// `const` each.
fn choices(prop: &Value) -> Vec<String> {
    if let Some(values) = prop.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = prop.get(key).and_then(Value::as_array) {
            let words: Vec<String> = branches
                .iter()
                .filter_map(|b| b.get("const").and_then(Value::as_str))
                .map(str::to_owned)
                .collect();
            if words.len() == branches.len() {
                return words;
            }
        }
    }
    Vec::new()
}

/// The clap arguments for `fields`.
pub(crate) fn args(op: &OpDescriptor, fields: &[Field]) -> Vec<Arg> {
    let mut out = Vec::with_capacity(fields.len() + 2);
    for f in fields {
        let mut arg = Arg::new(f.name.clone());
        if let Some(help) = &f.help {
            arg = arg.help(help.clone());
        }
        if !f.positional {
            arg = arg.long(f.long());
        }
        arg = match &f.kind {
            Kind::Flag => arg.action(ArgAction::SetTrue),
            Kind::Choice(words) => {
                arg.value_parser(PossibleValuesParser::new(words.clone()).map(Value::String))
            }
            Kind::Bool => arg.value_parser(parse_bool),
            Kind::Integer => arg.value_parser(parse_integer),
            Kind::Number => arg.value_parser(parse_number),
            Kind::Text => arg.value_parser(parse_text),
            Kind::Json => arg.value_parser(parse_json).value_name("JSON"),
        };
        if f.kind != Kind::Flag {
            arg = if f.many {
                if f.positional {
                    arg.num_args(1..).action(ArgAction::Append)
                } else {
                    arg.action(ArgAction::Append)
                }
            } else {
                arg.action(ArgAction::Set)
            };
            if f.positional {
                let one = if f.many {
                    f.name.trim_end_matches('s')
                } else {
                    &f.name
                };
                arg = arg.value_name(one.to_uppercase());
            } else if f.kind != Kind::Json {
                arg = arg.value_name(f.long().to_uppercase());
            }
        }
        if f.required {
            arg = arg.required_unless_present(INPUT_ARG);
        }
        out.push(arg);
    }
    out.push(
        Arg::new(INPUT_ARG)
            .long(INPUT_ARG)
            .value_name("JSON")
            .help("The whole input as JSON, or `-` to read it from standard input; options given as well take precedence")
            .action(ArgAction::Set),
    );
    if op.safety == gk_api::Safety::Permanent {
        out.push(
            Arg::new(YES_ARG)
                .long(YES_ARG)
                .short('y')
                .help("Confirm in advance: this can't be undone")
                .action(ArgAction::SetTrue),
        );
    }
    out
}

/// The operation's JSON input from the parsed command line.
pub(crate) fn collect(fields: &[Field], m: &ArgMatches) -> Result<Value, OpError> {
    let mut input = match m.get_one::<String>(INPUT_ARG) {
        Some(text) => read_input(text)?,
        None => Map::new(),
    };
    for f in fields {
        if m.value_source(&f.name) != Some(ValueSource::CommandLine) {
            continue;
        }
        let value = match (&f.kind, f.many) {
            (Kind::Flag, _) => Value::Bool(m.get_flag(&f.name)),
            (_, true) => Value::Array(
                m.get_many::<Value>(&f.name)
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default(),
            ),
            (_, false) => match m.get_one::<Value>(&f.name) {
                Some(v) => v.clone(),
                None => continue,
            },
        };
        input.insert(f.name.clone(), value);
    }
    Ok(Value::Object(input))
}

fn read_input(text: &str) -> Result<Map<String, Value>, OpError> {
    let text = if text == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| OpError::invalid(format!("Can't read the input: {e}.")))?;
        buf
    } else {
        text.to_owned()
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err(OpError::invalid_at("", "The input must be a JSON object.")),
        Err(e) => Err(OpError::invalid(format!(
            "The input is not valid JSON: {e}."
        ))),
    }
}

fn parse_bool(s: &str) -> Result<Value, String> {
    match s {
        "true" | "yes" => Ok(Value::Bool(true)),
        "false" | "no" => Ok(Value::Bool(false)),
        _ => Err("expected `true` or `false`".to_owned()),
    }
}

fn parse_integer(s: &str) -> Result<Value, String> {
    s.trim()
        .parse::<i64>()
        .map(Value::from)
        .map_err(|_| "expected a whole number".to_owned())
}

fn parse_number(s: &str) -> Result<Value, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .ok_or_else(|| "expected a number".to_owned())
}

#[allow(clippy::unnecessary_wraps)] // clap's value parsers return a Result
fn parse_text(s: &str) -> Result<Value, String> {
    Ok(Value::String(s.to_owned()))
}

fn parse_json(s: &str) -> Result<Value, String> {
    serde_json::from_str(s).map_err(|e| format!("expected JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_the_schema() {
        let op = gk_api::descriptor("history.feed").expect("in the catalog");
        let fields = fields(op, &[]);
        let kind = |name: &str| {
            fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.kind.clone())
                .expect("field")
        };
        assert_eq!(kind("sessions"), Kind::Flag);
        assert_eq!(kind("limit"), Kind::Integer);
        assert_eq!(kind("actor"), Kind::Text);
    }

    #[test]
    fn enums_become_choices() {
        let op = gk_api::descriptor("settings.update").expect("in the catalog");
        let week = fields(op, &[])
            .into_iter()
            .find(|f| f.name == "week_start")
            .expect("field");
        assert_eq!(
            week.kind,
            Kind::Choice(vec![
                "monday".to_owned(),
                "saturday".to_owned(),
                "sunday".to_owned()
            ])
        );
    }
}
