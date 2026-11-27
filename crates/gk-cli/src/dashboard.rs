// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A dashboard for people: each widget as a heading with its figures under
//! it, a few lines each, instead of the whole reports. `--json` still
//! prints everything.

use std::fmt::Write as _;

use serde_json::Value;

use crate::messages::text;

/// `dashboard.show`: the dashboard's name, then every widget.
#[must_use]
pub(crate) fn show(value: &Value) -> String {
    let mut out = String::new();
    let dashboard = &value["dashboard"];
    let name = dashboard["name"].as_str().unwrap_or_default();
    let _ = match dashboard["project"].as_str() {
        Some(project) => writeln!(
            out,
            "{}",
            text(
                "cli-dashboard-heading-project",
                &[("name", name.to_owned()), ("project", project.to_owned())],
            )
        ),
        None => writeln!(
            out,
            "{}",
            text("cli-dashboard-heading", &[("name", name.to_owned())])
        ),
    };
    let widgets = value["widgets"].as_array().map_or(&[][..], Vec::as_slice);
    if widgets.is_empty() {
        let _ = writeln!(out, "{}", text("cli-dashboard-empty", &[]));
    }
    for (at, widget) in widgets.iter().enumerate() {
        out.push('\n');
        widget_text(&mut out, at + 1, widget);
    }
    out
}

/// `dashboard.widget`: one widget.
#[must_use]
pub(crate) fn widget(value: &Value) -> String {
    let mut out = String::new();
    widget_text(&mut out, 0, value);
    out
}

/// The kind's own title.
fn kind_title(kind: &str) -> String {
    let id = match kind {
        "query_count" => "cli-widget-query-count",
        "query_list" => "cli-widget-query-list",
        "created_closed" => "cli-widget-created-closed",
        "cumulative_flow" => "cli-widget-cumulative-flow",
        "cycle_time" => "cli-widget-cycle-time",
        "time_report" => "cli-widget-time-report",
        "burndown" => "cli-widget-burndown",
        "version_progress" => "cli-widget-version-progress",
        "tasks_by_field" => "cli-widget-tasks-by-field",
        "due_soon" => "cli-widget-due-soon",
        "overdue" => "cli-widget-overdue",
        "activity" => "cli-widget-activity",
        "automation_failures" => "cli-widget-automation-failures",
        _ => "cli-widget-note",
    };
    text(id, &[])
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

fn num(v: &Value) -> String {
    v.as_u64()
        .map(|n| n.to_string())
        .or_else(|| v.as_i64().map(|n| n.to_string()))
        .unwrap_or_else(|| "0".to_owned())
}

fn widget_text(out: &mut String, place: usize, value: &Value) {
    let w = &value["widget"];
    let kind = str_of(w, "kind");
    let title = match str_of(w, "title") {
        "" => kind_title(kind),
        t => t.to_owned(),
    };
    let heading = if place == 0 {
        title
    } else {
        text(
            "cli-dashboard-widget",
            &[("place", place.to_string()), ("title", title)],
        )
    };
    let _ = writeln!(out, "{heading}");
    let line = |out: &mut String, s: String| {
        let _ = writeln!(out, "  {s}");
    };
    if let Some(error) = value.get("error") {
        line(
            out,
            text(
                "cli-dashboard-error",
                &[("message", str_of(error, "message").to_owned())],
            ),
        );
        return;
    }
    if let Some(count) = value.get("count")
        && matches!(
            kind,
            "query_count" | "query_list" | "due_soon" | "overdue" | "tasks_by_field"
        )
    {
        line(out, text("cli-dashboard-tasks", &[("count", num(count))]));
    }
    if let Some(tasks) = value["tasks"].as_array() {
        for t in tasks {
            let mut s = format!("{}  {}", str_of(t, "key"), str_of(t, "title"));
            let status = str_of(&t["status"], "name");
            let _ = match t["due_date"].as_str() {
                Some(due) => write!(
                    s,
                    "  {}",
                    text(
                        "cli-dashboard-task-due",
                        &[("status", status.to_owned()), ("due", due.to_owned())],
                    )
                ),
                None => write!(s, "  ({status})"),
            };
            line(out, s);
        }
    }
    if let Some(bars) = value["bars"].as_array() {
        for b in bars {
            let name = b["value"]
                .as_str()
                .map_or_else(|| text("cli-dashboard-no-value", &[]), str::to_owned);
            line(
                out,
                text(
                    "cli-dashboard-bar",
                    &[("name", name), ("count", num(&b["count"]))],
                ),
            );
        }
    }
    if let Some(r) = value.get("created_closed") {
        let open = r["items"]
            .as_array()
            .and_then(|i| i.last())
            .map_or_else(|| num(&r["open_before"]), |last| num(&last["open"]));
        line(
            out,
            text(
                "cli-dashboard-created-closed",
                &[
                    ("created", num(&r["created"])),
                    ("closed", num(&r["closed"])),
                    ("open", open),
                    ("from", str_of(r, "from").to_owned()),
                    ("to", str_of(r, "to").to_owned()),
                ],
            ),
        );
    }
    if let Some(r) = value.get("cumulative_flow") {
        let last = r["items"].as_array().and_then(|i| i.last());
        let bands = r["bands"].as_array().map_or(&[][..], Vec::as_slice);
        if let Some(last) = last {
            for (i, band) in bands.iter().enumerate() {
                line(
                    out,
                    text(
                        "cli-dashboard-bar",
                        &[
                            ("name", str_of(band, "name").to_owned()),
                            ("count", num(&last["counts"][i])),
                        ],
                    ),
                );
            }
        }
    }
    if let Some(r) = value.get("cycle_time") {
        let days = |v: &Value| {
            v.as_f64().map_or_else(
                || text("cli-no-value", &[]),
                |d| format!("{:.1}", (d * 10.0).round() / 10.0),
            )
        };
        line(
            out,
            text(
                "cli-dashboard-cycle-time",
                &[
                    ("count", num(&r["lead"]["count"])),
                    ("cycle", days(&r["cycle"]["p50"])),
                    ("lead", days(&r["lead"]["p50"])),
                ],
            ),
        );
    }
    if let Some(r) = value.get("time") {
        line(
            out,
            text(
                "cli-dashboard-time-total",
                &[
                    ("total", str_of(r, "total_duration").to_owned()),
                    ("from", str_of(r, "from").to_owned()),
                    ("to", str_of(r, "to").to_owned()),
                ],
            ),
        );
        for row in r["items"].as_array().map_or(&[][..], Vec::as_slice) {
            let name = match str_of(row, "title") {
                "" => match str_of(row, "key") {
                    "" => text("cli-dashboard-no-value", &[]),
                    k => k.to_owned(),
                },
                t => t.to_owned(),
            };
            line(
                out,
                text(
                    "cli-dashboard-bar",
                    &[
                        ("name", name),
                        ("count", str_of(row, "duration").to_owned()),
                    ],
                ),
            );
        }
    }
    if let Some(r) = value.get("burndown") {
        let last = r["days"].as_array().and_then(|d| d.last());
        line(
            out,
            text(
                "cli-dashboard-burndown",
                &[
                    ("sprint", str_of(&r["sprint"], "name").to_owned()),
                    (
                        "open",
                        last.map_or_else(|| "0".to_owned(), |d| num(&d["open_tasks"])),
                    ),
                    (
                        "total",
                        last.map_or_else(|| "0".to_owned(), |d| num(&d["total_tasks"])),
                    ),
                ],
            ),
        );
    }
    if let Some(versions) = value["versions"].as_array() {
        if versions.is_empty() {
            line(out, text("cli-none", &[]));
        }
        for v in versions {
            let p = &v["progress"];
            line(
                out,
                text(
                    "cli-dashboard-version",
                    &[
                        ("name", str_of(v, "name").to_owned()),
                        ("closed", num(&p["closed"])),
                        ("total", num(&p["total"])),
                        ("percent", num(&p["percent"])),
                    ],
                ),
            );
        }
    }
    if let Some(entries) = value["activity"].as_array() {
        if entries.is_empty() {
            line(out, text("cli-none", &[]));
        }
        for e in entries {
            let item = e["task"]["key"]
                .as_str()
                .map_or_else(|| str_of(e, "entity").to_owned(), str::to_owned);
            line(
                out,
                text(
                    "cli-dashboard-change",
                    &[
                        ("at", str_of(e, "at").to_owned()),
                        ("actor", str_of(&e["actor"], "name").to_owned()),
                        ("op", str_of(e, "op").to_owned()),
                        ("item", item),
                    ],
                ),
            );
        }
    }
    if let Some(runs) = value["runs"].as_array() {
        if runs.is_empty() {
            line(out, text("cli-none", &[]));
        }
        for r in runs {
            line(
                out,
                text(
                    "cli-dashboard-run",
                    &[
                        ("at", str_of(r, "at").to_owned()),
                        ("rule", str_of(r, "rule_name").to_owned()),
                        ("message", str_of(&r["error"], "message").to_owned()),
                    ],
                ),
            );
        }
    }
    if let Some(note) = value["text"].as_str() {
        for l in note.lines() {
            line(out, l.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn widgets_show_as_headings_with_their_figures() {
        let v = json!({
            "dashboard": { "name": "Team", "project": "TM", "widgets": [] },
            "widgets": [
                { "widget": { "id": "a", "kind": "query_count", "title": "Open", "width": 1, "settings": {} }, "count": 3 },
                { "widget": { "id": "b", "kind": "tasks_by_field", "width": 1, "settings": {} }, "count": 3,
                  "bars": [ { "value": "To do", "count": 2 }, { "count": 1 } ] },
                { "widget": { "id": "c", "kind": "burndown", "width": 2, "settings": {} },
                  "error": { "code": "not_found", "message": "No active sprint." } },
                { "widget": { "id": "d", "kind": "note", "width": 1, "settings": {} }, "text": "Line one\nLine two" }
            ]
        });
        insta::assert_snapshot!(show(&v), @r"
        Team (TM)

        1. Open
          3 tasks

        2. Tasks by field
          3 tasks
          To do: 2
          (none): 1

        3. Sprint burndown
          ! No active sprint.

        4. Note
          Line one
          Line two
        ");
    }
}
