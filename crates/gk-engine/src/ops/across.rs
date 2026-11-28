// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The board across projects ("My work"): the work of every active project
//! on one board, with a column for each status category, because projects
//! name their statuses differently.
//!
//! The cards are found by one query over every active project (in SQL, so
//! a large workspace is read once), narrowed to the caller's project when
//! it is limited to one and to a folder's projects when asked. Archived
//! projects are left out even when the query names them: the board shows
//! work that can move. Done cards closed in the last two weeks show; older
//! ones are counted. Cancelled work has no column.
//!
//! A column names each project's first status of its category, where a
//! card dropped into it goes; the projects' statuses come with the board,
//! so a person can choose another when a project has several.

use std::collections::{BTreeMap, HashMap};

use gk_api::ops::board::{
    AcrossProject, AcrossStatus, BoardAcross, BoardAcrossInput, BoardGetAcross, CardStatus, Column,
    LaneBy, SortMode,
};
use gk_api::ops::common::{Category, Hue};
use gk_core::Id;
use gk_store::rusqlite::{Transaction, params_from_iter};

use super::desc;
use super::lookup::{b, id_at, opt_id_at};
use super::project::parse_enum;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};

/// The swimlane kinds the board across projects offers, as written.
pub(crate) const LANE_KINDS: &[&str] = &[
    "project", "priority", "label", "sprint", "type", "parent", "none",
];

/// Closed cards older than this many days are counted, not listed.
const HIDE_DONE_AFTER_DAYS: u32 = 14;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The order of the cards in a column when the query has no `sort:`.
const DEFAULT_SORT: &str = "-priority,due";

/// The columns, in order.
const CATEGORIES: [Category; 3] = [Category::Todo, Category::InProgress, Category::Done];

pub(super) fn register(r: &mut Registry) {
    r.query::<BoardGetAcross, _>(desc::<BoardGetAcross>(), across);
}

/// The swimlane kind `text` names, among [`LANE_KINDS`].
pub(crate) fn lane_kind(text: &str) -> Option<LaneBy> {
    Some(match text.trim().to_lowercase().as_str() {
        "project" => LaneBy::Project,
        "priority" => LaneBy::Priority,
        "label" => LaneBy::Label,
        "sprint" => LaneBy::Sprint,
        "type" => LaneBy::Type,
        "parent" => LaneBy::Parent,
        "none" => LaneBy::None,
        _ => return None,
    })
}

const fn category_name(c: Category) -> &'static str {
    match c {
        Category::Todo => "todo",
        Category::InProgress => "in_progress",
        Category::Done => "done",
        Category::Cancelled => "cancelled",
    }
}

const fn category_hue(c: Category) -> Hue {
    match c {
        Category::Todo => Hue::Grey,
        Category::InProgress => Hue::Blue,
        Category::Done => Hue::Green,
        Category::Cancelled => Hue::Red,
    }
}

/// An SQL list of ID literals, for a condition without parameters.
fn id_list(ids: &[Id]) -> String {
    let hex: Vec<String> = ids
        .iter()
        .map(|id| {
            let bytes: String = b(*id).iter().map(|x| format!("{x:02x}")).collect();
            format!("x'{bytes}'")
        })
        .collect();
    if hex.is_empty() {
        "NULL".to_owned()
    } else {
        hex.join(", ")
    }
}

/// The task IDs of `sql` in order.
fn ids_of(tx: &Transaction<'_>, sql: &gk_query::Sql) -> Result<Vec<Id>, Failure> {
    let mut stmt = tx.prepare(&sql.text)?;
    Ok(stmt
        .query_map(
            params_from_iter(super::query::sql_params(&sql.params)),
            |r| id_at(r, 0),
        )?
        .collect::<Result<Vec<_>, _>>()?)
}

fn across(q: &QueryCtx<'_>, input: BoardAcrossInput) -> Result<BoardAcross, Failure> {
    let saved = match &input.view {
        Some(text) => Some(super::view::across_view(q.tx, text)?),
        None => None,
    };
    let query_text = input
        .query
        .clone()
        .or_else(|| saved.as_ref().map(|v| v.query.clone()))
        .unwrap_or_default();
    let folder: Option<(Id, String)> = match &input.folder {
        Some(text) if !text.trim().is_empty() => {
            let (id, r) = super::folder::find(q.tx, "folder", text)?;
            Some((id, r.name))
        }
        Some(_) => None,
        None => saved.as_ref().and_then(|v| v.folder.clone()),
    };
    let lanes_by = match input.lanes {
        Some(by) => by,
        None => saved
            .as_ref()
            .and_then(|v| v.lanes.as_deref())
            .and_then(lane_kind)
            .unwrap_or(LaneBy::Project),
    };
    if lanes_by == LaneBy::Field {
        return Err(messages::invalid("lanes", "board-across-lanes-field", &[]));
    }

    let (query, checked) = super::query::compile(q.tx, &q.env.zone, q.now, &query_text, "query")?;
    let default_sort = super::query::sort_keys(q.tx, &q.env.zone, q.now, DEFAULT_SORT, "sort")?;
    let mut compiled = gk_query::Compiled::new(
        &checked,
        &gk_query::SqlEnv {
            now: q.now,
            scope: None,
            default_sort,
        },
    );
    // Only work that can move: archived projects stay out, named or not.
    compiled.and("p.state = 'active'");
    if let Some(allowed) = crate::scope::projects() {
        compiled.and(&format!("t.project_id IN ({})", id_list(&allowed)));
    }
    if let Some((id, _)) = &folder {
        // A folder holds top-level projects; their sub-projects follow them.
        compiled.and(&format!(
            "t.project_id IN (WITH RECURSIVE f (id) AS (
               SELECT id FROM projects WHERE folder_id = {}
               UNION SELECT c.id FROM projects c JOIN f ON c.parent_id = f.id
             ) SELECT id FROM f)",
            id_list(&[*id])
        ));
    }
    let cutoff = (!input.show_older).then(|| q.now - i64::from(HIDE_DONE_AFTER_DAYS) * DAY_MS);
    let mut shown = compiled.clone();
    shown.and(&match cutoff {
        Some(c) => format!(
            "s.category IN ('todo', 'in_progress') OR (s.category = 'done' AND t.closed_at >= {c})"
        ),
        None => "s.category IN ('todo', 'in_progress', 'done')".to_owned(),
    });
    let older = match cutoff {
        Some(c) => {
            let mut old = compiled.clone();
            old.and(&format!("s.category = 'done' AND t.closed_at < {c}"));
            let sql = old.totals();
            let n: i64 = q.tx.query_row(
                &sql.text,
                params_from_iter(super::query::sql_params(&sql.params)),
                |r| r.get(0),
            )?;
            u64::try_from(n).unwrap_or(0)
        }
        None => 0,
    };
    let ids = ids_of(q.tx, &shown.ids())?;

    let mut columns: Vec<Column> = CATEGORIES
        .iter()
        .map(|c| Column {
            id: category_name(*c).to_owned(),
            name: category_name(*c).to_owned(),
            category: *c,
            hue: category_hue(*c),
            statuses: BTreeMap::new(),
            wip_limit: None,
            over_limit: false,
            wip_min: None,
            under_minimum: false,
            count: 0,
            older: 0,
            hidden: false,
            collapsed: false,
            sort_mode: SortMode::Manual,
            cards: Vec::new(),
        })
        .collect();
    let mut stmt = q.tx.prepare_cached(&format!(
        "SELECT {}, s.id, s.name, s.hue, s.category
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN task_types y ON y.id = t.type_id JOIN statuses s ON s.id = t.status_id
          WHERE t.id = ?1",
        super::board::card_columns()
    ))?;
    let at = super::board::CARD_COLUMN_COUNT;
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut keys: Vec<String> = Vec::new();
    for id in ids {
        let mut rows = stmt.query_map([b(id)], |r| {
            let (card, _) = super::board::card_row(r, q.now)?;
            let status = CardStatus {
                id: id_at(r, at)?.to_string(),
                name: r.get(at + 1)?,
                hue: parse_enum(&r.get::<_, String>(at + 2)?).unwrap_or(Hue::Grey),
            };
            let category: Category =
                parse_enum(&r.get::<_, String>(at + 3)?).unwrap_or(Category::Todo);
            Ok((card, status, category))
        })?;
        let Some(row) = rows.next().transpose()? else {
            continue;
        };
        drop(rows);
        let (mut card, status, category) = row;
        if lanes_by == LaneBy::Parent
            && let Some(parent) = &card.parent
        {
            parents.insert(card.id.clone(), parent.clone());
        }
        super::board::finish_card(q.tx, &mut card)?;
        card.status = Some(status);
        if !keys.contains(&card.project) {
            keys.push(card.project.clone());
        }
        if let Some(column) = columns.iter_mut().find(|c| c.category == category) {
            column.cards.push(card);
        }
    }
    drop(stmt);
    keys.sort();

    let projects = projects_of(q.tx, &keys)?;
    for column in &mut columns {
        for p in &projects {
            if let Some(first) = p.statuses.iter().find(|s| s.category == column.category) {
                column.statuses.insert(p.key.clone(), first.id.clone());
            }
        }
        column.count = column.cards.len() as u64;
        if column.category == Category::Done {
            column.older = older;
            column.count += older;
        }
    }
    let ids: Vec<Id> = project_ids(q.tx, &keys)?;
    let lanes = super::swimlane::Lanes::load(q.tx, lanes_by, &ids, None)?;
    let lanes = lanes.assign(q.tx, &mut columns, &parents)?;
    Ok(BoardAcross {
        query: gk_query::print(&query),
        folder: folder.map(|(_, name)| name),
        view: saved.map(|v| v.id.to_string()),
        columns,
        lanes_by,
        lanes,
        projects,
        hide_done_after_days: HIDE_DONE_AFTER_DAYS,
    })
}

/// The IDs of the projects with keys `keys`, in that order.
fn project_ids(tx: &Transaction<'_>, keys: &[String]) -> Result<Vec<Id>, Failure> {
    let mut stmt =
        tx.prepare_cached("SELECT id FROM projects WHERE key = ?1 AND deleted_at IS NULL")?;
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        if let Some(id) = stmt
            .query_map([key], |r| opt_id_at(r, 0))?
            .next()
            .transpose()?
            .flatten()
        {
            out.push(id);
        }
    }
    Ok(out)
}

/// The projects with keys `keys`, with their statuses, cancelled ones
/// aside (the board has no column for them).
fn projects_of(tx: &Transaction<'_>, keys: &[String]) -> Result<Vec<AcrossProject>, Failure> {
    let mut project = tx.prepare_cached(
        "SELECT id, name, hue FROM projects WHERE key = ?1 AND deleted_at IS NULL",
    )?;
    let mut statuses = tx.prepare_cached(
        "SELECT id, name, category, hue FROM statuses WHERE project_id = ?1
          AND category <> 'cancelled' ORDER BY rank, id",
    )?;
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let Some((id, name, hue)) = project
            .query_map([key], |r| {
                Ok((id_at(r, 0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .next()
            .transpose()?
        else {
            continue;
        };
        let list = statuses
            .query_map([b(id)], |r| {
                Ok(AcrossStatus {
                    id: id_at(r, 0)?.to_string(),
                    name: r.get(1)?,
                    category: parse_enum(&r.get::<_, String>(2)?).unwrap_or(Category::Todo),
                    hue: parse_enum(&r.get::<_, String>(3)?).unwrap_or(Hue::Grey),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        out.push(AcrossProject {
            key: key.clone(),
            name,
            hue: parse_enum(&hue).unwrap_or(Hue::Grey),
            statuses: list,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lane_kind_reads_back() {
        for kind in LANE_KINDS {
            assert!(lane_kind(kind).is_some(), "{kind}");
        }
        assert_eq!(lane_kind(" Project "), Some(LaneBy::Project));
        assert_eq!(lane_kind("field"), None);
    }
}
