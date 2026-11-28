// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A board for people: each column as a heading with its count and limits,
//! then its cards, one a line: key, title, the coloured edge's name and the
//! cover's file name. With swimlanes, the columns' headings come first, then
//! each lane with its columns and their cards. `--json` still prints the
//! whole board.

use std::fmt::Write as _;

use serde_json::Value;

use crate::messages::text;

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

fn count(v: &Value) -> String {
    v.as_u64().unwrap_or(0).to_string()
}

/// `board.get`: the project's board, column by column.
#[must_use]
pub(crate) fn show(value: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}",
        text(
            "cli-board-heading",
            &[("project", str_of(value, "project").to_owned())]
        )
    );
    let columns = value["columns"].as_array().map_or(&[][..], Vec::as_slice);
    let shown: Vec<&Value> = columns
        .iter()
        .filter(|c| !c["hidden"].as_bool().unwrap_or(false))
        .collect();
    // Keys line up across the board.
    let key_width = shown
        .iter()
        .flat_map(|c| c["cards"].as_array().map_or(&[][..], Vec::as_slice))
        .map(|card| str_of(card, "key").chars().count())
        .max()
        .unwrap_or(0);
    let lanes = value["lanes"].as_array().map_or(&[][..], Vec::as_slice);
    if lanes.is_empty() {
        for column in shown {
            out.push('\n');
            column_text(&mut out, column, key_width);
        }
    } else {
        out.push('\n');
        let _ = writeln!(out, "{}", text("cli-board-columns", &[]));
        for column in &shown {
            let _ = writeln!(out, "  {}", heading(column));
        }
        let by = str_of(&value["layout"], "lanes");
        for lane in lanes {
            out.push('\n');
            lane_text(&mut out, lane, by, &shown, key_width);
        }
    }
    let hidden = value["hidden_columns"].as_u64().unwrap_or(0);
    if hidden > 0 {
        out.push('\n');
        let _ = writeln!(
            out,
            "{}",
            text(
                "cli-board-hidden",
                &[
                    ("columns", hidden.to_string()),
                    ("tasks", count(&value["hidden_tasks"])),
                ],
            )
        );
    }
    out
}

/// A lane: its heading, then each column holding its cards, with them.
fn lane_text(out: &mut String, lane: &Value, by: &str, columns: &[&Value], key_width: usize) {
    let name = if lane["empty"].as_bool().unwrap_or(false) {
        text("cli-board-lane-empty", &[("by", by.to_owned())])
    } else if by == "priority" {
        text(
            "cli-board-lane-priority",
            &[("priority", str_of(lane, "id").to_owned())],
        )
    } else if let Some(key) = lane["key"].as_str() {
        text(
            "cli-board-lane-parent",
            &[
                ("key", key.to_owned()),
                ("title", str_of(lane, "name").to_owned()),
            ],
        )
    } else {
        str_of(lane, "name").to_owned()
    };
    let line = text(
        "cli-board-lane",
        &[("name", name), ("count", count(&lane["count"]))],
    );
    let _ = writeln!(out, "{line}");
    let id = str_of(lane, "id");
    let mut any = false;
    for column in columns {
        let cards: Vec<&Value> = column["cards"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(|c| str_of(c, "lane") == id)
            .collect();
        if cards.is_empty() {
            continue;
        }
        any = true;
        let _ = writeln!(
            out,
            "  {}",
            text(
                "cli-board-lane-column",
                &[
                    ("name", str_of(column, "name").to_owned()),
                    ("count", cards.len().to_string()),
                ],
            )
        );
        for card in cards {
            card_line(out, card, key_width, "    ");
        }
    }
    if !any {
        let _ = writeln!(out, "  {}", text("cli-board-no-cards", &[]));
    }
}

/// A column's heading: its name and count, and its limits.
fn heading(column: &Value) -> String {
    let mut heading = text(
        "cli-board-column",
        &[
            ("name", str_of(column, "name").to_owned()),
            ("count", count(&column["count"])),
        ],
    );
    let mut note = |id: &str, args: &[(&str, String)]| {
        heading.push_str("  ");
        heading.push_str(&text(id, args));
    };
    if let Some(limit) = column["wip_limit"].as_u64() {
        let id = if column["over_limit"].as_bool().unwrap_or(false) {
            "cli-board-over-limit"
        } else {
            "cli-board-limit"
        };
        note(id, &[("limit", limit.to_string())]);
    }
    if let Some(min) = column["wip_min"].as_u64()
        && column["under_minimum"].as_bool().unwrap_or(false)
    {
        note("cli-board-under-minimum", &[("minimum", min.to_string())]);
    }
    if column["collapsed"].as_bool().unwrap_or(false) {
        note("cli-board-collapsed", &[]);
    }
    heading
}

fn column_text(out: &mut String, column: &Value, key_width: usize) {
    let _ = writeln!(out, "{}", heading(column));
    let cards = column["cards"].as_array().map_or(&[][..], Vec::as_slice);
    if cards.is_empty() {
        let _ = writeln!(out, "  {}", text("cli-board-no-cards", &[]));
    }
    for card in cards {
        card_line(out, card, key_width, "  ");
    }
    let older = column["older"].as_u64().unwrap_or(0);
    if older > 0 {
        let _ = writeln!(
            out,
            "  {}",
            text("cli-board-older", &[("count", older.to_string())])
        );
    }
}

/// One card: key, title, the edge's name and the cover's file name.
fn card_line(out: &mut String, card: &Value, key_width: usize, indent: &str) {
    let key = str_of(card, "key");
    let fill = key_width.saturating_sub(key.chars().count()) + 2;
    let mut line = format!("{indent}{key}{:fill$}{}", "", str_of(card, "title"));
    if let Some(edge) = card.get("edge") {
        line.push_str("  ");
        line.push_str(&text(
            "cli-board-edge",
            &[("name", str_of(edge, "name").to_owned())],
        ));
    }
    if let Some(cover) = card.get("cover") {
        line.push_str("  ");
        line.push_str(&text(
            "cli-board-cover",
            &[("name", str_of(cover, "name").to_owned())],
        ));
    }
    let _ = writeln!(out, "{}", line.trim_end());
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::show;

    #[test]
    fn columns_list_their_cards_with_the_edge_and_the_cover() {
        let board = json!({
            "project": "TM",
            "columns": [
                { "name": "To do", "count": 2, "older": 0, "hidden": false, "collapsed": false,
                  "wip_limit": 1, "over_limit": true, "cards": [
                    { "key": "TM-1", "title": "Write the notes",
                      "edge": { "hue": "red", "by": "query", "name": "Urgent bugs" },
                      "cover": { "attachment": "x", "name": "shot.png", "mime": "image/png" } },
                    { "key": "TM-12", "title": "Second" },
                ] },
                { "name": "Done", "count": 3, "older": 3, "hidden": false, "collapsed": true,
                  "cards": [] },
                { "name": "Parked", "count": 1, "older": 0, "hidden": true, "collapsed": false,
                  "cards": [] },
            ],
            "hidden_columns": 1,
            "hidden_tasks": 1,
        });
        let text = show(&board);
        assert_eq!(
            text,
            "Board of TM\n\
             \n\
             To do (2 tasks)  over its limit of 1\n\
             \x20 TM-1   Write the notes  edge: Urgent bugs  cover: shot.png\n\
             \x20 TM-12  Second\n\
             \n\
             Done (3 tasks)  collapsed\n\
             \x20 no cards\n\
             \x20 3 older closed tasks not shown\n\
             \n\
             1 hidden column, with 1 task\n"
        );
    }
}
