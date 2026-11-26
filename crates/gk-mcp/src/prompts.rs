// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The prompts: `prompts/list`, `prompts/get` and the completion of their
//! arguments, over the prompts of the catalog crate.
//!
//! A prompt's arguments are checked against the workspace before its text
//! is filled in: a project, a task or a version that doesn't exist (or lies
//! outside the session's project) is an error naming it, and the text gets
//! the canonical key or name. The text itself holds no workspace data, only
//! keys and names the agent then reads with the tools.

use std::collections::BTreeMap;

use gk_api::prompts::{ArgumentKind, McpPrompt};
use gk_api::{OpError, Profile, Toolset};
use gk_engine::{Context, Engine};
use rmcp::model::{CompletionInfo, GetPromptResult, JsonObject, Prompt, PromptMessage, Role};
use serde_json::{Value, json};

use crate::messages;

/// The most values one completion returns, as MCP allows.
const MAX_VALUES: usize = CompletionInfo::MAX_VALUES;

/// The prompts an agent with the effective `profile` sees with `toolsets`.
pub(crate) fn listed(profile: Profile, toolsets: &[Toolset]) -> Vec<Prompt> {
    gk_api::prompts::listed(profile, toolsets)
        .map(|p| {
            serde_json::from_value(p.to_json())
                .unwrap_or_else(|e| panic!("the prompt {} doesn't fit MCP: {e}", p.name))
        })
        .collect()
}

/// The prompt `name`, if it is listed for `profile` and `toolsets`.
pub(crate) fn find(
    name: &str,
    profile: Profile,
    toolsets: &[Toolset],
) -> Result<&'static McpPrompt, OpError> {
    let names = || -> Vec<String> {
        gk_api::prompts::listed(profile, toolsets)
            .map(|p| p.name.clone())
            .collect()
    };
    let Some(prompt) = gk_api::prompts::find(name) else {
        let listed = names();
        let listed: Vec<&str> = listed.iter().map(String::as_str).collect();
        return Err(messages::hint(
            messages::error("mcp-no-such-prompt", &[("prompt", name.to_owned())], |m| {
                OpError::not_found(m, &listed)
            }),
            "mcp-no-such-prompt-hint",
            &[],
        ));
    };
    if names().iter().any(|n| n == name) {
        Ok(prompt)
    } else {
        Err(messages::hint(
            messages::error(
                "mcp-prompt-not-allowed",
                &[
                    ("prompt", name.to_owned()),
                    ("profile", profile.to_string()),
                ],
                OpError::forbidden,
            ),
            "mcp-prompt-not-allowed-hint",
            &[],
        ))
    }
}

/// The prompt's text for `arguments`, after checking them: an argument the
/// prompt doesn't have, a required one missing, or a value that names
/// nothing in the workspace is an error. In a session limited to a project,
/// a `project` argument left out is that project.
pub(crate) fn get(
    engine: &Engine,
    ctx: &Context,
    prompt: &McpPrompt,
    arguments: Option<JsonObject>,
    scope: Option<&str>,
) -> Result<GetPromptResult, OpError> {
    let mut given = BTreeMap::new();
    for (name, value) in arguments.unwrap_or_default() {
        if prompt.argument(&name).is_none() {
            return Err(unknown_argument(prompt, &name));
        }
        let text = match value {
            Value::String(s) => s,
            Value::Null => continue,
            other => other.to_string(),
        };
        let text = text.trim();
        if !text.is_empty() {
            given.insert(name, text.to_owned());
        }
    }
    if let Some(scope) = scope
        && prompt.argument("project").is_some()
    {
        given
            .entry("project".to_owned())
            .or_insert_with(|| scope.to_owned());
    }
    for a in &prompt.arguments {
        if a.required && !given.contains_key(&a.name) {
            return Err(messages::error(
                "mcp-prompt-missing-argument",
                &[
                    ("prompt", prompt.name.clone()),
                    ("argument", a.name.clone()),
                ],
                OpError::invalid,
            )
            .with_detail("path", a.name.clone()));
        }
    }
    // Projects first: a version is looked up in its project.
    let mut resolved = BTreeMap::new();
    let order = [
        ArgumentKind::Project,
        ArgumentKind::Task,
        ArgumentKind::Version,
    ];
    for kind in order {
        for a in prompt.arguments.iter().filter(|a| a.complete == Some(kind)) {
            let Some(value) = given.get(&a.name) else {
                continue;
            };
            let canonical = resolve(engine, ctx, kind, value, resolved.get("project"))
                .map_err(|e| e.with_detail("path", a.name.clone()))?;
            resolved.insert(a.name.clone(), canonical);
        }
    }
    for (name, value) in given {
        resolved.entry(name).or_insert(value);
    }
    let text = prompt.render(&resolved);
    Ok(
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
            .with_description(prompt.description.clone()),
    )
}

fn unknown_argument(prompt: &McpPrompt, name: &str) -> OpError {
    let valid: Vec<&str> = prompt.arguments.iter().map(|a| a.name.as_str()).collect();
    messages::error(
        "mcp-prompt-unknown-argument",
        &[
            ("prompt", prompt.name.clone()),
            ("argument", name.to_owned()),
        ],
        |m| OpError::not_found(m, &valid),
    )
    .with_detail("path", name.to_owned())
}

/// The canonical key or name of `value`, an argument of `kind`.
fn resolve(
    engine: &Engine,
    ctx: &Context,
    kind: ArgumentKind,
    value: &str,
    project: Option<&String>,
) -> Result<String, OpError> {
    let out = match kind {
        ArgumentKind::Project => engine.call(ctx, "project.get", json!({ "project": value }))?,
        ArgumentKind::Task => {
            let out = engine.call(ctx, "task.get", json!({ "keys": [value] }))?;
            out["items"][0].clone()
        }
        ArgumentKind::Version => engine.call(
            ctx,
            "version.get",
            json!({ "project": project, "version": value }),
        )?,
    };
    let field = if kind == ArgumentKind::Version {
        "name"
    } else {
        "key"
    };
    Ok(out[field].as_str().unwrap_or(value).to_owned())
}

/// The values that complete argument `argument` of `prompt` from what was
/// typed (`value`), with the other arguments already chosen (`context`).
/// Only what the session may see is offered. A value that names nothing
/// completes to nothing rather than failing.
pub(crate) fn complete(
    engine: &Engine,
    ctx: &Context,
    prompt: &McpPrompt,
    argument: &str,
    value: &str,
    context: &BTreeMap<String, String>,
    scope: Option<&str>,
) -> Result<CompletionInfo, OpError> {
    let Some(a) = prompt.argument(argument) else {
        return Err(unknown_argument(prompt, argument));
    };
    let typed = value.trim();
    let values = match a.complete {
        None => Vec::new(),
        Some(ArgumentKind::Project) => projects(engine, ctx, typed)?,
        Some(ArgumentKind::Version) => {
            let project = context
                .get("project")
                .map(String::as_str)
                .filter(|p| !p.trim().is_empty())
                .or(scope);
            match project {
                Some(project) => versions(engine, ctx, project, typed)?,
                None => Vec::new(),
            }
        }
        Some(ArgumentKind::Task) => tasks(engine, ctx, typed)?,
    };
    let total = values.len();
    let has_more = total > MAX_VALUES;
    let values: Vec<String> = values.into_iter().take(MAX_VALUES).collect();
    Ok(
        CompletionInfo::with_pagination(values, u32::try_from(total).ok(), has_more)
            .unwrap_or_default(),
    )
}

/// Not-found errors complete to nothing; others are errors.
fn or_nothing(result: Result<Value, OpError>) -> Result<Option<Value>, OpError> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.code == gk_api::ErrorCode::NotFound || e.code == gk_api::ErrorCode::Invalid => {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// The values of `items` (value, other text such as a name) that start
/// with `typed` first, then those whose value or other text contains it,
/// both ignoring case.
fn matching<'a>(items: impl Iterator<Item = (&'a str, &'a str)>, typed: &str) -> Vec<String> {
    let typed = typed.to_lowercase();
    let mut first = Vec::new();
    let mut then = Vec::new();
    for (value, also) in items {
        let lower = value.to_lowercase();
        if lower.starts_with(&typed) {
            first.push(value.to_owned());
        } else if lower.contains(&typed) || also.to_lowercase().contains(&typed) {
            then.push(value.to_owned());
        }
    }
    first.extend(then);
    first
}

fn items(output: &Value) -> &[Value] {
    output["items"].as_array().map_or(&[], Vec::as_slice)
}

fn projects(engine: &Engine, ctx: &Context, typed: &str) -> Result<Vec<String>, OpError> {
    let Some(out) = or_nothing(engine.call(ctx, "project.list", json!({})))? else {
        return Ok(Vec::new());
    };
    Ok(matching(
        items(&out).iter().map(|p| {
            (
                p["key"].as_str().unwrap_or_default(),
                p["name"].as_str().unwrap_or_default(),
            )
        }),
        typed,
    ))
}

fn versions(
    engine: &Engine,
    ctx: &Context,
    project: &str,
    typed: &str,
) -> Result<Vec<String>, OpError> {
    let Some(out) = or_nothing(engine.call(ctx, "version.list", json!({ "project": project })))?
    else {
        return Ok(Vec::new());
    };
    Ok(matching(
        items(&out)
            .iter()
            .map(|v| (v["name"].as_str().unwrap_or_default(), "")),
        typed,
    ))
}

/// Tasks by key (`TM-4` offers TM-4, TM-41, …) or, for other text, by the
/// words of the title, most recently changed first.
fn tasks(engine: &Engine, ctx: &Context, typed: &str) -> Result<Vec<String>, OpError> {
    let (project, prefix, words) = match typed.split_once('-') {
        Some((key, number))
            if is_project_key(key) && number.chars().all(|c| c.is_ascii_digit()) =>
        {
            (Some(key), typed.to_uppercase(), None)
        }
        _ if typed.is_empty() => (None, String::new(), None),
        _ if is_project_key(typed) => (Some(typed), typed.to_uppercase(), None),
        _ => (None, String::new(), Some(typed)),
    };
    let mut input = json!({ "query": "sort:-updated", "limit": 200 });
    if let Some(project) = project {
        input["project"] = json!(project);
    }
    if let Some(words) = words {
        let words: String = words
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect();
        input["query"] = json!(format!("{} sort:-updated", words.trim()));
    }
    let mut found = or_nothing(engine.call(ctx, "task.search", input))?;
    // A bare word that is no project's key may still be a title's word.
    if found.as_ref().is_none_or(|o| items(o).is_empty())
        && words.is_none()
        && !typed.is_empty()
        && !typed.contains('-')
    {
        found = or_nothing(engine.call(
            ctx,
            "task.search",
            json!({ "query": format!("{typed} sort:-updated"), "limit": 200 }),
        ))?;
        let Some(out) = found else {
            return Ok(Vec::new());
        };
        return Ok(keys(&out, ""));
    }
    let Some(out) = found else {
        return Ok(Vec::new());
    };
    Ok(keys(&out, &prefix))
}

fn keys(output: &Value, prefix: &str) -> Vec<String> {
    items(output)
        .iter()
        .filter_map(|t| t["key"].as_str())
        .filter(|k| k.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

fn is_project_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_matches_come_before_other_matches() {
        let items = [
            ("TM", "Task manager"),
            ("XT", "Extra TM work"),
            ("ATM", "Cash"),
        ];
        assert_eq!(
            matching(items.iter().map(|(a, b)| (*a, *b)), "tm"),
            vec!["TM", "XT", "ATM"]
        );
        assert_eq!(
            matching(items.iter().map(|(a, b)| (*a, *b)), ""),
            vec!["TM", "XT", "ATM"]
        );
    }

    #[test]
    fn project_keys_start_with_a_letter() {
        assert!(is_project_key("TM"));
        assert!(is_project_key("tm2"));
        assert!(!is_project_key("2TM"));
        assert!(!is_project_key(""));
        assert!(!is_project_key("T M"));
    }

    #[test]
    fn every_prompt_fits_mcp() {
        for profile in Profile::ALL {
            let prompts = listed(profile, &Toolset::ALL);
            assert!(!prompts.is_empty(), "{profile}");
        }
    }
}
