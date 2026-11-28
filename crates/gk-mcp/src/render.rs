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
        "delete_tasks" | "delete_wiki_page" => trash_change("Moved to the trash", output),
        "restore_from_trash" => trash_change("Restored", output),
        "purge_trash" => trash_change("Purged for good", output),
        "list_trash" => trash_list(output),
        "create_backup" => backup(output),
        "list_backups" => backups(output),
        "verify_backup" => backup_check(output),
        "export_data" => export(output),
        "import_data" => import(output),
        // Large by design and sized by its own `bytes`: compact, so the
        // text costs what the context says.
        "get_task_context" => output.to_string(),
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

/// One trash entry: its kind, key, title (as data) and ID.
fn trash_entry(e: &Value) -> String {
    let mut line = format!("- {}", s(e, "kind"));
    if let Some(key) = e.get("key").and_then(Value::as_str) {
        let _ = write!(line, " {key}");
    } else if let Some(task) = e.get("task").and_then(Value::as_str) {
        let _ = write!(line, " on {task}");
    }
    let _ = write!(line, " {}", e["title"]);
    if let Some(n) = e["contains"].as_u64().filter(|n| *n > 0) {
        let _ = write!(line, " (with {n} more)");
    }
    let _ = write!(line, " · id {}", s(e, "id"));
    line
}

fn trash_change(what: &str, v: &Value) -> String {
    let items = v["items"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = match v.get("group").and_then(Value::as_str) {
        Some(group) => format!("{what}: {} items · group {}\n", v["total"], short(group)),
        None if v.get("group").is_some() || !items.is_empty() => {
            format!("{what}: {} items\n", v["total"])
        }
        None => "Nothing changed.\n".to_owned(),
    };
    for e in items {
        out.push_str(&trash_entry(e));
        out.push('\n');
    }
    out
}

fn trash_list(v: &Value) -> String {
    let items = v["items"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = format!(
        "{} of {} trash entries, newest first\n",
        items.len(),
        v["total"]
    );
    for e in items {
        let _ = writeln!(
            out,
            "{} · deleted {} · purged {}",
            trash_entry(e),
            s(e, "deleted_at"),
            s(e, "purge_at")
        );
    }
    out
}

fn backup_line(b: &Value) -> String {
    format!(
        "{} ({}) · {} · {} bytes · {} stored files",
        s(b, "name"),
        s(b, "kind"),
        s(b, "created_at"),
        b["size"],
        b["files"]
    )
}

fn backup(v: &Value) -> String {
    format!("Backup made: {}\n", backup_line(v))
}

fn backups(v: &Value) -> String {
    let items = v["items"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = format!(
        "{} backups in {}, newest first\n",
        items.len(),
        s(v, "folder")
    );
    for b in items {
        let _ = writeln!(out, "- {}", backup_line(b));
    }
    out
}

fn backup_check(v: &Value) -> String {
    let name = s(&v["backup"], "name");
    let checked = &v["files_checked"];
    if v["ok"] == Value::Bool(true) {
        return format!("Backup {name} is intact ({checked} stored files checked).\n");
    }
    let mut out = format!("Backup {name} has problems ({checked} stored files checked):\n");
    for p in v["problems"].as_array().map_or(&[][..], Vec::as_slice) {
        let _ = writeln!(out, "- {}: {}", s(p, "kind"), s(p, "detail"));
    }
    out
}

fn counts(c: &Value) -> String {
    let parts: Vec<String> = c
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, n)| n.as_u64().is_some_and(|n| n > 0))
                .map(|(k, n)| format!("{k} {n}"))
                .collect()
        })
        .unwrap_or_default();
    if parts.is_empty() {
        "nothing".to_owned()
    } else {
        parts.join(", ")
    }
}

fn export(v: &Value) -> String {
    let projects: Vec<String> = v["projects"]
        .as_array()
        .map(|p| p.iter().map(plain).collect())
        .unwrap_or_default();
    format!(
        "Exported {} ({}) to {} · {} bytes\nProjects: {}\n",
        counts(&v["counts"]),
        s(v, "format"),
        s(v, "path"),
        v["size"],
        projects.join(", ")
    )
}

fn import(v: &Value) -> String {
    let dry = v["dry_run"] == Value::Bool(true);
    let mut out = if dry {
        format!("Dry run: the archive holds {}.\n", counts(&v["counts"]))
    } else {
        let group = v.get("group").and_then(Value::as_str).map_or("", short);
        format!("Imported {} · group {group}\n", counts(&v["counts"]))
    };
    for p in v["projects"].as_array().map_or(&[][..], Vec::as_slice) {
        let _ = writeln!(
            out,
            "- {} (from {}) {}",
            s(p, "key"),
            s(p, "from_key"),
            p["name"]
        );
    }
    let taken: Vec<String> = v["taken_keys"]
        .as_array()
        .map(|k| k.iter().map(plain).collect())
        .unwrap_or_default();
    if !taken.is_empty() {
        let _ = writeln!(
            out,
            "Keys taken here, which need new ones: {}",
            taken.join(", ")
        );
    }
    out
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
    fn trash_changes_and_lists_show_entries_as_data() {
        let entry = json!({
            "id": "0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b", "kind": "task", "key": "TM-1",
            "title": "Say \"hi\"", "contains": 2, "deleted_at": "2026-11-23T08:00:00Z",
            "purge_at": "2026-12-23T08:00:00Z", "group": "g"
        });
        let text = markdown(
            "delete_tasks",
            &json!({ "items": [entry], "total": 3, "group": "bbbbbbbb-c3d4-7e5f-8a9b-0c1d2e3f4a5b" }),
        );
        assert_eq!(
            text,
            "Moved to the trash: 3 items · group bbbbbbbb\n- task TM-1 \"Say \\\"hi\\\"\" (with 2 more) \
             · id 0193a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b\n"
        );
        let text = markdown("list_trash", &json!({ "items": [entry], "total": 1 }));
        assert!(text.starts_with("1 of 1 trash entries"), "{text}");
        assert!(text.contains("purged 2026-12-23T08:00:00Z"), "{text}");
        assert_eq!(
            markdown("restore_from_trash", &json!({ "items": [], "total": 0 })),
            "Nothing changed.\n"
        );
    }

    #[test]
    fn backups_exports_and_imports_in_short() {
        let b = json!({ "name": "2026-11-23T080000Z", "kind": "manual", "created_at": "2026-11-23T08:00:00Z",
                        "size": 4096, "files": 2, "files_size": 10, "schema_version": 1, "app_version": "0.0.7" });
        assert_eq!(
            markdown("create_backup", &b),
            "Backup made: 2026-11-23T080000Z (manual) · 2026-11-23T08:00:00Z · 4096 bytes · 2 stored files\n"
        );
        let check = markdown(
            "verify_backup",
            &json!({ "backup": b, "ok": false, "files_checked": 2,
                     "problems": [{ "kind": "missing_file", "detail": "ab12" }] }),
        );
        assert_eq!(
            check,
            "Backup 2026-11-23T080000Z has problems (2 stored files checked):\n- missing_file: ab12\n"
        );
        let import = markdown(
            "import_data",
            &json!({ "dry_run": true, "projects": [{ "key": "TM", "from_key": "TM", "name": "Tasks", "id": "x" }],
                     "counts": { "projects": 1, "tasks": 4, "comments": 0 }, "taken_keys": ["TM"],
                     "new_ids": 0, "format_version": 1, "app_version": "0.0.7" }),
        );
        assert_eq!(
            import,
            "Dry run: the archive holds projects 1, tasks 4.\n- TM (from TM) \"Tasks\"\n\
             Keys taken here, which need new ones: TM\n"
        );
        let export = markdown(
            "export_data",
            &json!({ "path": "/x/e.zip", "format": "zip", "projects": ["TM"], "counts": { "tasks": 4 }, "size": 99 }),
        );
        assert_eq!(
            export,
            "Exported tasks 4 (zip) to /x/e.zip · 99 bytes\nProjects: TM\n"
        );
    }

    #[test]
    fn unknown_tools_fall_back_to_json() {
        assert!(markdown("nope", &json!({"a": 1})).starts_with("```json"));
    }

    #[test]
    fn a_task_context_is_compact_json_charged_once() {
        let context = json!({ "task": { "key": "TM-1" }, "bytes": 27 });
        let text = markdown("get_task_context", &context);
        assert_eq!(text, context.to_string());
        assert_eq!(crate::server::charged(&text, &context), text.len());
        let other = markdown("get_task", &context);
        assert_eq!(
            crate::server::charged(&other, &context),
            other.len() + context.to_string().len()
        );
    }
}
