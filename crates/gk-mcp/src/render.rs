// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The short Markdown text that goes with every tool result, for clients
//! that read only the text. `structuredContent` carries the full result.
//!
//! Text that people or agents wrote is shown as data, never as part of the
//! surrounding sentence.

use std::fmt::Write as _;

use serde_json::Value;

/// The Markdown text for the result of tool `tool`.
#[must_use]
pub(crate) fn markdown(tool: &str, output: &Value) -> String {
    match tool {
        "get_workspace" => workspace(output),
        "get_activity" => activity(output),
        "revert_changes" => revert(output),
        "update_settings" => settings_change(output),
        _ => fallback(output),
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn workspace(v: &Value) -> String {
    let caller = &v["caller"];
    let mut out = format!(
        "Workspace {} · Glyphkeel {} · schema {}\n",
        s(v, "name"),
        s(v, "program_version"),
        v["schema_version"]
    );
    let _ = write!(
        out,
        "You are {} ({}",
        s(&caller["actor"], "name"),
        s(caller, "via")
    );
    if let Some(p) = caller.get("profile").and_then(Value::as_str) {
        let _ = write!(out, ", profile {p}");
    }
    if let Some(id) = caller.get("session").and_then(Value::as_str) {
        let _ = write!(out, ", session {}", short(id));
    }
    out.push_str(")\n");
    if let Some(settings) = v.get("settings").and_then(Value::as_object) {
        let list: Vec<String> = settings
            .iter()
            .map(|(k, v)| format!("{k} {}", plain(v)))
            .collect();
        let _ = writeln!(out, "Settings: {}", list.join(" · "));
    }
    let guide = s(v, "guide");
    if !guide.is_empty() {
        out.push('\n');
        out.push_str(guide);
        out.push('\n');
    }
    out
}

fn activity(v: &Value) -> String {
    let items = v["items"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = format!("{} of {} changes, newest first\n", items.len(), v["total"]);
    for item in items {
        let fields: Vec<String> = item["fields"]
            .as_object()
            .map(|f| {
                f.iter()
                    .map(|(name, change)| {
                        let side =
                            |k: &str| change.get(k).map_or_else(|| "(none)".to_owned(), plain);
                        format!("{name} {} → {}", side("before"), side("after"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = write!(
            out,
            "- #{} · {} · {} via {} · {} · {} {}: {}",
            item["seq"],
            s(item, "at"),
            s(&item["actor"], "name"),
            s(item, "via"),
            s(item, "op"),
            s(item, "entity"),
            short(s(item, "id")),
            fields.join(", ")
        );
        let _ = write!(out, " · group {}", short(s(item, "group")));
        if let Some(by) = item.get("reverted_by").and_then(Value::as_str) {
            let _ = write!(out, " · reverted by {}", short(by));
        }
        out.push('\n');
    }
    if let Some(cursor) = v.get("next_cursor").and_then(Value::as_str) {
        let _ = writeln!(out, "More: pass cursor \"{cursor}\"");
    }
    if let Some(sessions) = v.get("sessions").and_then(Value::as_array) {
        let _ = writeln!(out, "\nAgent sessions ({}):", sessions.len());
        for ses in sessions {
            let _ = writeln!(
                out,
                "- {} · {} · {} · started {}{} · {} reads, {} writes",
                short(s(ses, "id")),
                s(ses, "name"),
                s(ses, "profile"),
                s(ses, "started_at"),
                ses.get("ended_at")
                    .and_then(Value::as_str)
                    .map_or_else(|| " · running".to_owned(), |e| format!(" · ended {e}")),
                ses["reads"],
                ses["writes"]
            );
        }
    }
    out
}

fn revert(v: &Value) -> String {
    let reverted = v["reverted"].as_array().map_or(0, Vec::len);
    let skipped = v["skipped"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = match v.get("group").and_then(Value::as_str) {
        Some(group) => format!("Reverted {reverted} groups as group {}.", short(group)),
        None if reverted == 0 => "Nothing was reverted.".to_owned(),
        None => format!("Dry run: would revert {reverted} groups."),
    };
    out.push('\n');
    if !skipped.is_empty() {
        let _ = writeln!(out, "Skipped {}:", skipped.len());
        for g in skipped {
            let _ = writeln!(out, "- {}: {}", short(s(g, "group")), s(g, "reason"));
        }
    }
    out
}

fn settings_change(v: &Value) -> String {
    let changed: Vec<String> = v["changed"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter_map(Value::as_str)
                .map(|k| format!("{k} = {}", plain(&v["settings"][k])))
                .collect()
        })
        .unwrap_or_default();
    match v.get("group").and_then(Value::as_str) {
        Some(group) if !changed.is_empty() => {
            format!("Changed {} · group {}\n", changed.join(", "), short(group))
        }
        _ => "Nothing changed.\n".to_owned(),
    }
}

fn fallback(v: &Value) -> String {
    format!(
        "```json\n{}\n```\n",
        serde_json::to_string_pretty(v).unwrap_or_default()
    )
}

/// A value without JSON quotes around strings.
fn plain(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// The first eight characters of a UUID, as the app shows them; any other
/// ID as it is.
fn short(id: &str) -> &str {
    let is_uuid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if is_uuid { &id[..8] } else { id }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn workspace_overview() {
        let text = markdown(
            "get_workspace",
            &json!({
                "id": "x", "name": "Home", "schema_version": 1, "program_version": "0.1.0",
                "caller": { "actor": { "kind": "agent", "name": "Claude Code" }, "via": "mcp",
                            "profile": "standard", "session": "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b" },
                "settings": { "hours_per_day": 8 },
                "guide": "Projects have keys."
            }),
        );
        assert_eq!(
            text,
            "Workspace Home · Glyphkeel 0.1.0 · schema 1\nYou are Claude Code (mcp, profile \
             standard, session 0193a1b2)\nSettings: hours_per_day 8\n\nProjects have keys.\n"
        );
    }

    #[test]
    fn revert_lists_skipped_groups() {
        let text = markdown(
            "revert_changes",
            &json!({ "reverted": ["a"], "skipped": [{ "group": "bbbbbbbb-c3d4-7e5f-8a9b-0c1d2e3f4a5b", "reason": "changed since" }] }),
        );
        assert_eq!(
            text,
            "Dry run: would revert 1 groups.\nSkipped 1:\n- bbbbbbbb: changed since\n"
        );
    }

    #[test]
    fn activity_shows_both_sides_of_a_change() {
        let text = markdown(
            "get_activity",
            &json!({ "total": 1, "items": [{
                "seq": 3, "group": "0193a1b2-c3d4", "at": "2026-11-23T08:00:00Z", "op": "settings.update",
                "actor": { "kind": "agent", "name": "Claude Code" }, "via": "mcp",
                "entity": "settings", "id": "workspace",
                "fields": { "hours_per_day": { "after": 7 } }
            }]}),
        );
        assert_eq!(
            text,
            "1 of 1 changes, newest first\n- #3 · 2026-11-23T08:00:00Z · Claude Code via mcp · \
             settings.update · settings workspace: hours_per_day (none) → 7 · group 0193a1b2-c3d4\n"
        );
    }

    #[test]
    fn unknown_tools_fall_back_to_json() {
        assert!(markdown("nope", &json!({"a": 1})).starts_with("```json"));
    }
}
