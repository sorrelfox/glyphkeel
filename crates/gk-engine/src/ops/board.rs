// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A project's board and the person's layout of it.
//!
//! The layout is kept per project in `view_settings` and recorded in
//! history like any other change, so it can be reverted. A status the
//! layout doesn't know yet shows at its project position, next to the
//! column it follows there. With sub-projects included, columns are matched
//! by status name; statuses only sub-projects have become extra columns at
//! the end.

use std::collections::BTreeMap;

use gk_api::OpError;
use gk_api::ops::board::{
    Board, BoardGet, BoardGetInput, BoardLayout, Card, Column, LayoutChange, LayoutGet,
    LayoutGetInput, LayoutUpdate, LayoutUpdateInput, SortMode,
};
use gk_api::ops::common::{Category, Glyph, Hue, Priority};
use gk_api::ops::task::{Checklist, TypeRef};
use gk_core::{Id, fold};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::priority_name;
use super::lookup::{self, ProjectRow, b, id_at, opt_id_at};
use super::project::{parse_enum, subtree};
use super::task::{
    BLOCKED_SQL, CLOSED_SUBTASKS_SQL, COMMENTS_SQL, FILES_SQL, SUBTASKS_SQL, live_claim,
    task_labels,
};
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for board layouts; the item is the project.
pub(crate) const ENTITY: &str = "layout";
/// The view kind of board layouts in `view_settings`.
const VIEW: &str = "board";
/// The fields a card can show.
const CARD_FIELDS: &[&str] = &[
    "key",
    "type",
    "priority",
    "due",
    "start",
    "labels",
    "checklist",
    "subtasks",
    "estimate",
    "days_in_column",
    "updated",
];
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub(super) fn register(r: &mut Registry) {
    r.query::<BoardGet, _>(desc::<BoardGet>(), board);
    r.query::<LayoutGet, _>(desc::<LayoutGet>(), |q, input: LayoutGetInput| {
        let p = lookup::project(q.tx, "project", &input.project)?;
        Ok(LayoutChange {
            project: p.key,
            layout: load_layout(q.tx, p.id)?,
            group: None,
        })
    });
    r.command::<LayoutUpdate, _>(desc::<LayoutUpdate>(), update);
}

/// The stored layout value of project `id`: the JSON as written, or `None`.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    let text: Option<String> = tx
        .query_row(
            "SELECT settings FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
            params![id.to_string(), VIEW],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

/// Writes the stored layout of project `id` (`None` removes it) without
/// recording it.
pub(crate) fn write_raw(w: &WriteCtx<'_>, id: Id, value: Option<&Value>) -> Result<(), Failure> {
    match value {
        Some(v) => {
            w.tx().execute(
                "INSERT INTO view_settings (scope, view_kind, settings, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (scope, view_kind) DO UPDATE SET settings = excluded.settings,
                   updated_at = excluded.updated_at",
                params![id.to_string(), VIEW, v.to_string(), w.now()],
            )?;
        }
        None => {
            w.tx().execute(
                "DELETE FROM view_settings WHERE scope = ?1 AND view_kind = ?2",
                params![id.to_string(), VIEW],
            )?;
        }
    }
    Ok(())
}

/// The layout of project `id`, with defaults for what was never set.
fn load_layout(tx: &Transaction<'_>, id: Id) -> Result<BoardLayout, Failure> {
    let Value::Object(mut merged) = serde_json::to_value(BoardLayout::default())
        .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?
    else {
        return Ok(BoardLayout::default());
    };
    if let Some(Value::Object(stored)) = raw(tx, id)? {
        for (k, v) in stored {
            if merged.contains_key(&k) {
                merged.insert(k, v);
            }
        }
    }
    Ok(serde_json::from_value(Value::Object(merged)).unwrap_or_default())
}

struct StatusInfo {
    id: Id,
    project_key: String,
    name: String,
    category: String,
    hue: String,
    wip: Option<i64>,
}

fn statuses(tx: &Transaction<'_>, projects: &[Id]) -> Result<Vec<StatusInfo>, Failure> {
    let mut out = Vec::new();
    for p in projects {
        let mut stmt = tx.prepare_cached(
            "SELECT s.id, p.key, s.name, s.category, s.hue, s.wip_limit
               FROM statuses s JOIN projects p ON p.id = s.project_id
              WHERE s.project_id = ?1 ORDER BY s.rank, s.id",
        )?;
        let rows = stmt.query_map([b(*p)], |r| {
            Ok(StatusInfo {
                id: id_at(r, 0)?,
                project_key: r.get(1)?,
                name: r.get(2)?,
                category: r.get(3)?,
                hue: r.get(4)?,
                wip: r.get(5)?,
            })
        })?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

/// The board's columns before the layout: the project's statuses in order,
/// then statuses only sub-projects have, matched by name.
fn columns(all: &[StatusInfo]) -> Vec<Column> {
    let mut out: Vec<(String, Column)> = Vec::new();
    for s in all {
        let key = fold(&s.name);
        if let Some((_, c)) = out.iter_mut().find(|(k, _)| *k == key) {
            c.statuses
                .entry(s.project_key.clone())
                .or_insert_with(|| s.id.to_string());
            continue;
        }
        let mut statuses = BTreeMap::new();
        statuses.insert(s.project_key.clone(), s.id.to_string());
        out.push((
            key,
            Column {
                id: s.id.to_string(),
                name: s.name.clone(),
                category: parse_enum(&s.category).unwrap_or(Category::Todo),
                hue: parse_enum(&s.hue).unwrap_or(Hue::Grey),
                statuses,
                wip_limit: s.wip.and_then(|n| u32::try_from(n).ok()),
                over_limit: false,
                count: 0,
                older: 0,
                hidden: false,
                collapsed: false,
                sort_mode: SortMode::Manual,
                cards: Vec::new(),
            },
        ));
    }
    out.into_iter().map(|(_, c)| c).collect()
}

/// Orders `columns` (in project order) by the layout's order; columns it
/// doesn't list follow the column they follow in project order.
fn arrange(columns: Vec<Column>, order: &[String]) -> Vec<Column> {
    let mut result: Vec<Column> = Vec::new();
    for id in order {
        if let Some(c) = columns.iter().find(|c| &c.id == id)
            && !result.iter().any(|r| r.id == c.id)
        {
            result.push(c.clone());
        }
    }
    for (i, c) in columns.iter().enumerate() {
        if result.iter().any(|r| r.id == c.id) {
            continue;
        }
        let at = columns[..i]
            .iter()
            .rev()
            .find_map(|prev| result.iter().position(|r| r.id == prev.id))
            .map_or(0, |p| p + 1);
        result.insert(at, c.clone());
    }
    result
}

fn board(q: &QueryCtx<'_>, input: BoardGetInput) -> Result<Board, Failure> {
    let p = lookup::project(q.tx, "project", &input.project)?;
    let layout = load_layout(q.tx, p.id)?;
    let include = input
        .include_subprojects
        .unwrap_or(layout.include_subprojects);
    let projects = if include {
        subtree(q.tx, p.id)?
    } else {
        vec![p.id]
    };
    let infos = statuses(q.tx, &projects)?;
    // Statuses only sub-projects have go at the end, whatever the layout.
    let (own, extra): (Vec<Column>, Vec<Column>) = columns(&infos)
        .into_iter()
        .partition(|c| c.statuses.contains_key(&p.key));
    let mut columns = arrange(own, &layout.order);
    columns.extend(arrange(extra, &layout.order));
    let cutoff = (layout.hide_done_after_days > 0 && !input.show_older)
        .then(|| q.now - i64::from(layout.hide_done_after_days) * DAY_MS);
    let (mut hidden_columns, mut hidden_tasks) = (0, 0);
    for column in &mut columns {
        column.hidden = layout.hidden.contains(&column.id);
        column.collapsed = layout.collapsed.contains(&column.id);
        column.sort_mode = layout
            .column_sort
            .get(&column.id)
            .copied()
            .unwrap_or(layout.sort_mode);
        let mut cards = Vec::new();
        for status in column.statuses.values() {
            let status: Id = status
                .parse()
                .map_err(|_| Failure::Op(OpError::storage("A status ID is broken.")))?;
            cards.extend(cards_of(q.tx, status, q.now)?);
        }
        column.count = cards.len() as u64;
        column.over_limit = column
            .wip_limit
            .is_some_and(|l| column.count > u64::from(l));
        if let Some(cutoff) = cutoff {
            let before = cards.len();
            cards.retain(|(_, closed)| closed.is_none_or(|t| t >= cutoff));
            column.older = (before - cards.len()) as u64;
        }
        sort(&mut cards, column.sort_mode);
        if column.hidden {
            hidden_columns += 1;
            hidden_tasks += column.count;
        } else {
            column.cards = cards.into_iter().map(|(c, _)| c).collect();
        }
    }
    Ok(Board {
        project: p.key,
        columns,
        layout: BoardLayout {
            include_subprojects: include,
            ..layout
        },
        hidden_columns,
        hidden_tasks,
    })
}

fn cards_of(
    tx: &Transaction<'_>,
    status: Id,
    now: i64,
) -> Result<Vec<(Card, Option<i64>)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT t.id, p.key, t.number, t.title, y.id, y.name, y.glyph, t.priority, t.parent_id,
                t.start_date, t.due_date, t.estimate_min, t.checklist_done, t.checklist_total,
                t.rank, t.status_entered_at, t.updated_at, t.closed_at, t.version,
                {SUBTASKS_SQL}, {CLOSED_SUBTASKS_SQL}, {BLOCKED_SQL}, {COMMENTS_SQL}, {FILES_SQL},
                t.spent_min, t.claim_session, t.claim_name, t.claim_until
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN task_types y ON y.id = t.type_id
          WHERE t.status_id = ?1 AND t.deleted_at IS NULL
          ORDER BY t.rank, t.id"
    ))?;
    let rows = stmt
        .query_map([b(status)], |r| {
            let closed: Option<i64> = r.get(17)?;
            Ok((
                Card {
                    key: format!("{}-{}", r.get::<_, String>(1)?, r.get::<_, i64>(2)?),
                    title: r.get(3)?,
                    project: r.get(1)?,
                    task_type: TypeRef {
                        id: id_at(r, 4)?.to_string(),
                        name: r.get(5)?,
                        glyph: parse_enum(&r.get::<_, String>(6)?).unwrap_or(Glyph::Task),
                    },
                    priority: parse_enum(priority_name(r.get(7)?)).unwrap_or(Priority::None),
                    labels: Vec::new(),
                    parent: opt_id_at(r, 8)?.map(|id| id.to_string()),
                    start_date: r.get(9)?,
                    due_date: r.get(10)?,
                    estimate_minutes: r.get(11)?,
                    checklist: Checklist {
                        done: u32::try_from(r.get::<_, i64>(12)?).unwrap_or(0),
                        total: u32::try_from(r.get::<_, i64>(13)?).unwrap_or(0),
                    },
                    subtasks: Checklist {
                        done: u32::try_from(r.get::<_, i64>(20)?).unwrap_or(0),
                        total: u32::try_from(r.get::<_, i64>(19)?).unwrap_or(0),
                    },
                    blocked: r.get(21)?,
                    comment_count: u32::try_from(r.get::<_, i64>(22)?).unwrap_or(0),
                    file_count: u32::try_from(r.get::<_, i64>(23)?).unwrap_or(0),
                    spent_minutes: r.get(24)?,
                    claim: live_claim(opt_id_at(r, 25)?, r.get(26)?, r.get(27)?, now),
                    rank: r.get(14)?,
                    status_entered_at: rfc3339(r.get(15)?),
                    updated_at: rfc3339(r.get(16)?),
                    closed_at: closed.map(rfc3339),
                    version: r.get(18)?,
                    id: id_at(r, 0)?.to_string(),
                },
                closed,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (mut card, closed) in rows {
        let id: Id = card
            .id
            .parse()
            .map_err(|_| Failure::Op(OpError::storage("A task ID is broken.")))?;
        card.labels = task_labels(tx, id)?;
        if let Some(parent) = card.parent.take()
            && let Ok(parent) = parent.parse::<Id>()
            && let Some(row) = lookup::task_by_id(tx, parent)?
        {
            card.parent = Some(lookup::task_key(tx, row.project, row.number)?);
        }
        out.push((card, closed));
    }
    Ok(out)
}

fn sort(cards: &mut [(Card, Option<i64>)], mode: SortMode) {
    let priority = |p: Priority| match p {
        Priority::None => 0,
        Priority::Low => 1,
        Priority::Normal => 2,
        Priority::High => 3,
        Priority::Urgent => 4,
    };
    match mode {
        SortMode::Manual => {}
        SortMode::Priority => {
            cards.sort_by_key(|c| std::cmp::Reverse(priority(c.0.priority)));
        }
        SortMode::Due => cards.sort_by(|a, b| match (&a.0.due_date, &b.0.due_date) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }),
        SortMode::Updated => {
            cards.sort_by_key(|c| std::cmp::Reverse(c.0.updated_at.clone()));
        }
    }
}

/// The column ID that `text` names among the statuses of `p` and its
/// sub-projects: a status ID, or a name ignoring case.
fn column_id(
    tx: &Transaction<'_>,
    p: &ProjectRow,
    field: &str,
    text: &str,
) -> Result<String, Failure> {
    let infos = statuses(tx, &subtree(tx, p.id)?)?;
    let cols = columns(&infos);
    let wanted = fold(text.trim());
    if let Some(c) = cols.iter().find(|c| {
        c.id == text.trim()
            || c.statuses.values().any(|s| s == text.trim())
            || fold(&c.name) == wanted
    }) {
        return Ok(c.id.clone());
    }
    let names: Vec<String> = cols.into_iter().map(|c| c.name).collect();
    Err(OpError::not_found(
        format!("The board of {} has no column \"{}\".", p.key, text.trim()),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .with_detail("field", field)
    .into())
}

fn ids(
    tx: &Transaction<'_>,
    p: &ProjectRow,
    field: &str,
    list: &[String],
) -> Result<Vec<String>, Failure> {
    list.iter()
        .enumerate()
        .map(|(i, t)| column_id(tx, p, &format!("{field}[{i}]"), t))
        .collect()
}

fn update(w: &mut WriteCtx<'_>, input: LayoutUpdateInput) -> Result<LayoutChange, Failure> {
    let p = lookup::project(w.tx(), "project", &input.project)?;
    let mut layout = if input.reset {
        BoardLayout::default()
    } else {
        load_layout(w.tx(), p.id)?
    };
    let add = |list: &mut Vec<String>, ids: Vec<String>| {
        for id in ids {
            if !list.contains(&id) {
                list.push(id);
            }
        }
    };
    let hide = ids(w.tx(), &p, "hide", &input.hide)?;
    let show = ids(w.tx(), &p, "show", &input.show)?;
    let collapse = ids(w.tx(), &p, "collapse", &input.collapse)?;
    let expand = ids(w.tx(), &p, "expand", &input.expand)?;
    add(&mut layout.hidden, hide);
    layout.hidden.retain(|id| !show.contains(id));
    add(&mut layout.collapsed, collapse);
    layout.collapsed.retain(|id| !expand.contains(id));
    if let Some(order) = &input.order {
        let mut ordered = Vec::new();
        add(&mut ordered, ids(w.tx(), &p, "order", order)?);
        // The columns left out follow, in the order the board shows them.
        let shown = arrange(
            columns(&statuses(w.tx(), &subtree(w.tx(), p.id)?)?),
            &layout.order,
        );
        add(&mut ordered, shown.into_iter().map(|c| c.id).collect());
        layout.order = ordered;
    }
    if let Some(mode) = input.sort_mode {
        layout.sort_mode = mode;
    }
    if let Some(sorts) = &input.column_sort {
        for (column, mode) in sorts {
            let id = column_id(w.tx(), &p, "column_sort", column)?;
            if *mode == layout.sort_mode {
                layout.column_sort.remove(&id);
            } else {
                layout.column_sort.insert(id, *mode);
            }
        }
    }
    if let Some(days) = input.hide_done_after_days {
        if days > 3650 {
            return Err(OpError::invalid_at("hide_done_after_days", "Use 0 to 3650 days.").into());
        }
        layout.hide_done_after_days = days;
    }
    if let Some(fields) = &input.card_fields {
        let mut chosen = Vec::new();
        for (i, f) in fields.iter().enumerate() {
            let f = f.trim().to_lowercase();
            if !CARD_FIELDS.contains(&f.as_str()) {
                return Err(OpError::invalid_at(
                    format!("card_fields[{i}]"),
                    format!("Cards can't show \"{f}\"."),
                )
                .with_detail("choices", CARD_FIELDS)
                .into());
            }
            if !chosen.contains(&f) {
                chosen.push(f);
            }
        }
        layout.card_fields = chosen;
    }
    if let Some(include) = input.include_subprojects {
        layout.include_subprojects = include;
    }
    let before = raw(w.tx(), p.id)?;
    let after = if layout == BoardLayout::default() {
        None
    } else {
        Some(
            serde_json::to_value(&layout)
                .map_err(|e| Failure::Op(OpError::storage(e.to_string())))?,
        )
    };
    let effective_before = before
        .clone()
        .map_or_else(|| serde_json::to_value(BoardLayout::default()).ok(), Some);
    let effective_after = after
        .clone()
        .map_or_else(|| serde_json::to_value(BoardLayout::default()).ok(), Some);
    if effective_before != effective_after || before.is_some() != after.is_some() {
        write_raw(w, p.id, after.as_ref())?;
        w.record_field(ENTITY, p.id, Some(p.id), "settings", before, after);
    }
    Ok(LayoutChange {
        project: p.key,
        layout,
        group: w.group_if_changed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(id: &str) -> Column {
        Column {
            id: id.to_owned(),
            name: id.to_owned(),
            category: Category::Todo,
            hue: Hue::Grey,
            statuses: BTreeMap::new(),
            wip_limit: None,
            over_limit: false,
            count: 0,
            older: 0,
            hidden: false,
            collapsed: false,
            sort_mode: SortMode::Manual,
            cards: Vec::new(),
        }
    }

    #[test]
    fn unknown_columns_keep_their_project_position() {
        let project: Vec<Column> = ["a", "b", "new", "c"].map(column).to_vec();
        let order = ["c", "a", "b"].map(str::to_owned);
        let ids: Vec<String> = arrange(project, &order).into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["c", "a", "b", "new"]);
        let first: Vec<Column> = ["new", "a"].map(column).to_vec();
        let ids: Vec<String> = arrange(first, &["a".to_owned()])
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, ["new", "a"]);
    }
}
