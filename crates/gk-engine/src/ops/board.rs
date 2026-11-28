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
//!
//! Card colours and covers are worked out here with the board, for the
//! cards shown only: each colour rule's query runs once over the board's
//! projects, and a card takes the colour of the first rule that matched it.
//! A rule whose query no longer reads (a label renamed away, say) colours
//! nothing rather than failing the board.

use std::collections::{BTreeMap, HashMap, HashSet};

use gk_api::OpError;
use gk_api::ops::board::{
    Board, BoardGet, BoardGetInput, BoardLayout, Card, CardColourBy, CardEdge, ColourRule, Column,
    CoverMode, LaneBy, LayoutChange, LayoutGet, LayoutGetInput, LayoutUpdate, LayoutUpdateInput,
    SortMode,
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
use crate::messages;
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
/// The most colour rules a board keeps.
const MAX_COLOUR_RULES: usize = 20;

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
    wip_min: Option<i64>,
}

fn statuses(tx: &Transaction<'_>, projects: &[Id]) -> Result<Vec<StatusInfo>, Failure> {
    let mut out = Vec::new();
    for p in projects {
        let mut stmt = tx.prepare_cached(
            "SELECT s.id, p.key, s.name, s.category, s.hue, s.wip_limit, s.wip_min
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
                wip_min: r.get(6)?,
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
                wip_min: s.wip_min.and_then(|n| u32::try_from(n).ok()),
                under_minimum: false,
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
    // The custom fields the cards show, by query name.
    let custom: Vec<String> = layout
        .card_fields
        .iter()
        .filter_map(|f| gk_query::fields::strip_custom_prefix(f).map(str::to_owned))
        .collect();
    let colours = Colours::load(q, &layout, &p.key, include)?;
    let lane_by = input.lanes.unwrap_or(layout.lanes);
    let lane_field = match (lane_by, &input.lane_field) {
        (LaneBy::Field, Some(text)) => Some(
            super::swimlane::select_field(q.tx, p.id, text)?
                .ok_or_else(|| lane_field_unknown(&p, text, "lane_field"))?,
        ),
        (LaneBy::Field, None) if !layout.lane_field.is_empty() => {
            super::swimlane::select_field(q.tx, p.id, &layout.lane_field)?
        }
        (LaneBy::Field, None) => {
            return Err(messages::invalid(
                "lane_field",
                "board-lane-field-needed",
                &[],
            ));
        }
        _ => None,
    };
    // A field deleted since the layout named it: no lanes.
    let lane_by = if lane_by == LaneBy::Field && lane_field.is_none() {
        LaneBy::None
    } else {
        lane_by
    };
    let lanes = super::swimlane::Lanes::load(q.tx, lane_by, &projects, lane_field.as_ref())?;
    let mut parents: HashMap<String, String> = HashMap::new();
    for column in &mut columns {
        column.hidden = layout.hidden.contains(&column.id);
        column.collapsed = layout.collapsed.contains(&column.id);
        column.sort_mode = layout
            .column_sort
            .get(&column.id)
            .copied()
            .unwrap_or(layout.sort_mode);
        let mut cards = Vec::new();
        let mut older = 0;
        for status in column.statuses.values() {
            let status: Id = status
                .parse()
                .map_err(|_| Failure::Op(messages::storage("storage-status-id", &[])))?;
            cards.extend(cards_of(q.tx, status, q.now, cutoff)?);
            older += older_of(q.tx, status, cutoff)?;
        }
        column.count = cards.len() as u64 + older;
        column.over_limit = column
            .wip_limit
            .is_some_and(|l| column.count > u64::from(l));
        column.under_minimum = column.wip_min.is_some_and(|m| column.count < u64::from(m));
        column.older = older;
        sort(&mut cards, column.sort_mode);
        if column.hidden {
            hidden_columns += 1;
            hidden_tasks += column.count;
        } else {
            // Only the cards shown get their labels, progress, parent and
            // field values: older closed cards and hidden columns can hold
            // most of a large project.
            let mut shown = Vec::with_capacity(cards.len());
            for (mut card, _) in cards {
                if lane_by == LaneBy::Parent
                    && let Some(parent) = &card.parent
                {
                    parents.insert(card.id.clone(), parent.clone());
                }
                finish_card(q.tx, &mut card)?;
                if !custom.is_empty() {
                    card_values(q.tx, &mut card, &custom)?;
                }
                card.edge = colours.edge(&card);
                if layout.covers != CoverMode::Off
                    && card.file_count > 0
                    && let Ok(id) = card.id.parse::<Id>()
                {
                    card.cover =
                        super::file::card_cover(q.tx, id, layout.covers == CoverMode::FirstImage)?;
                }
                shown.push(card);
            }
            column.cards = shown;
        }
    }
    let lanes = lanes.assign(q.tx, &mut columns, &parents)?;
    let lane_field = lane_field.map_or_else(String::new, |f| {
        format!("{}{}", gk_query::fields::CUSTOM_PREFIX, f.query_name)
    });
    Ok(Board {
        project: p.key,
        columns,
        layout: BoardLayout {
            include_subprojects: include,
            lanes: lane_by,
            lane_field,
            ..layout
        },
        hidden_columns,
        hidden_tasks,
        lanes,
    })
}

fn lane_field_unknown(p: &ProjectRow, text: &str, path: &str) -> Failure {
    messages::invalid(
        path,
        "board-lane-field-unknown",
        &[
            ("field", text.trim().to_owned()),
            ("project", p.key.clone()),
        ],
    )
}

/// What the board's cards are coloured by, read once per board.
enum Colours {
    None,
    /// Type colours by type ID.
    Types(HashMap<String, Hue>),
    Priority,
    Label,
    /// Each rule with the tasks its query matched.
    Rules(Vec<(ColourRule, HashSet<String>)>),
}

impl Colours {
    fn load(
        q: &QueryCtx<'_>,
        layout: &BoardLayout,
        project: &str,
        include: bool,
    ) -> Result<Self, Failure> {
        Ok(match layout.colour_by {
            CardColourBy::None => Self::None,
            CardColourBy::Priority => Self::Priority,
            CardColourBy::Label => Self::Label,
            CardColourBy::Type => {
                let mut stmt = q.tx.prepare_cached("SELECT id, hue FROM task_types")?;
                let hues = stmt
                    .query_map([], |r| {
                        Ok((
                            id_at(r, 0)?.to_string(),
                            parse_enum(&r.get::<_, String>(1)?).unwrap_or(Hue::Grey),
                        ))
                    })?
                    .collect::<Result<HashMap<_, _>, _>>()?;
                Self::Types(hues)
            }
            CardColourBy::Query => {
                let mut rules = Vec::with_capacity(layout.colour_rules.len());
                for rule in &layout.colour_rules {
                    let Ok((_, checked)) =
                        super::query::try_compile(q.tx, &q.env.zone, q.now, &rule.query)?
                    else {
                        continue;
                    };
                    let compiled = gk_query::Compiled::new(
                        &checked,
                        &gk_query::SqlEnv {
                            now: q.now,
                            scope: Some(gk_query::Scope {
                                project: project.to_owned(),
                                subprojects: include,
                            }),
                            default_sort: Vec::new(),
                        },
                    );
                    let compiled = super::query::with_schedule(q.tx, &checked, compiled)?;
                    let ids = super::query::matching_ids(q.tx, &compiled)?
                        .into_iter()
                        .map(|id| id.to_string())
                        .collect();
                    rules.push((rule.clone(), ids));
                }
                Self::Rules(rules)
            }
        })
    }

    /// The edge of `card`, which has its labels.
    fn edge(&self, card: &Card) -> Option<CardEdge> {
        let edge = |hue, by, name: &str| {
            Some(CardEdge {
                hue,
                by,
                name: name.to_owned(),
            })
        };
        match self {
            Self::None => None,
            Self::Types(hues) => edge(
                hues.get(&card.task_type.id).copied().unwrap_or(Hue::Grey),
                CardColourBy::Type,
                &card.task_type.name,
            ),
            Self::Priority => {
                let (hue, name) = match card.priority {
                    Priority::None => return None,
                    Priority::Low => (Hue::Grey, "low"),
                    Priority::Normal => (Hue::Blue, "normal"),
                    Priority::High => (Hue::Orange, "high"),
                    Priority::Urgent => (Hue::Red, "urgent"),
                };
                edge(hue, CardColourBy::Priority, name)
            }
            Self::Label => card
                .labels
                .first()
                .and_then(|l| edge(l.hue, CardColourBy::Label, &l.name)),
            Self::Rules(rules) => rules
                .iter()
                .find(|(_, ids)| ids.contains(&card.id))
                .and_then(|(rule, _)| {
                    let name = if rule.name.trim().is_empty() {
                        rule.query.trim()
                    } else {
                        rule.name.trim()
                    };
                    edge(rule.hue, CardColourBy::Query, name)
                }),
        }
    }
}

/// How many live cards of `status` closed before `cutoff`, which the
/// board leaves out.
fn older_of(tx: &Transaction<'_>, status: Id, cutoff: Option<i64>) -> Result<u64, Failure> {
    let Some(cutoff) = cutoff else {
        return Ok(0);
    };
    let n: i64 = tx
        .prepare_cached(
            "SELECT count(*) FROM tasks t
              WHERE t.project_id = (SELECT project_id FROM statuses WHERE id = ?1)
                AND t.status_id = ?1 AND t.deleted_at IS NULL AND t.closed_at < ?2",
        )?
        .query_row(params![b(status), cutoff], |r| r.get(0))?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// The columns a card is read from, over `tasks t`, `projects p` and
/// `task_types y`; [`card_row`] reads them.
pub(crate) fn card_columns() -> String {
    use super::recurrence::RECURRING_SQL;
    format!(
        "t.id, p.key, t.number, t.title, y.id, y.name, y.glyph, t.priority, t.parent_id,
         t.start_date, t.due_date, t.estimate_min, t.checklist_done, t.checklist_total,
         t.rank, t.status_entered_at, t.updated_at, t.closed_at, t.version,
         {SUBTASKS_SQL}, {CLOSED_SUBTASKS_SQL}, {BLOCKED_SQL}, {COMMENTS_SQL}, {FILES_SQL},
         t.spent_min, t.claim_session, t.claim_name, t.claim_until, {RECURRING_SQL}"
    )
}

/// The number of columns [`card_columns`] reads.
pub(crate) const CARD_COLUMN_COUNT: usize = 29;

/// A card as [`card_columns`] read it, and when it was closed.
pub(crate) fn card_row(
    r: &gk_store::rusqlite::Row<'_>,
    now: i64,
) -> gk_store::rusqlite::Result<(Card, Option<i64>)> {
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
            sprint: None,
            start_date: r.get(9)?,
            due_date: r.get(10)?,
            estimate_minutes: r.get(11)?,
            checklist: Checklist {
                done: u32::try_from(r.get::<_, i64>(12)?).unwrap_or(0),
                total: u32::try_from(r.get::<_, i64>(13)?).unwrap_or(0),
            },
            progress_percent: 0,
            subtasks: Checklist {
                done: u32::try_from(r.get::<_, i64>(20)?).unwrap_or(0),
                total: u32::try_from(r.get::<_, i64>(19)?).unwrap_or(0),
            },
            blocked: r.get(21)?,
            comment_count: u32::try_from(r.get::<_, i64>(22)?).unwrap_or(0),
            file_count: u32::try_from(r.get::<_, i64>(23)?).unwrap_or(0),
            spent_minutes: r.get(24)?,
            claim: live_claim(opt_id_at(r, 25)?, r.get(26)?, r.get(27)?, now),
            recurring: r.get(28)?,
            rank: r.get(14)?,
            status_entered_at: rfc3339(r.get(15)?),
            updated_at: rfc3339(r.get(16)?),
            closed_at: closed.map(rfc3339),
            version: r.get(18)?,
            id: id_at(r, 0)?.to_string(),
            fields: BTreeMap::new(),
            edge: None,
            cover: None,
            lane: None,
            status: None,
        },
        closed,
    ))
}

/// The live cards of `status`, without those closed before `cutoff`, in
/// rank order. A status belongs to one project; naming it lets the board
/// index find the column's tasks without reading the others.
fn cards_of(
    tx: &Transaction<'_>,
    status: Id,
    now: i64,
    cutoff: Option<i64>,
) -> Result<Vec<(Card, Option<i64>)>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {}
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN task_types y ON y.id = t.type_id
          WHERE t.project_id = (SELECT project_id FROM statuses WHERE id = ?1)
            AND t.status_id = ?1 AND t.deleted_at IS NULL
            AND (?2 IS NULL OR t.closed_at IS NULL OR t.closed_at >= ?2)
          ORDER BY t.rank, t.id",
        card_columns()
    ))?;
    let rows = stmt
        .query_map(params![b(status), cutoff], |r| card_row(r, now))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Gives a card read by [`cards_of`] its labels, its progress and its
/// parent's key (in place of the parent's ID).
pub(crate) fn finish_card(tx: &Transaction<'_>, card: &mut Card) -> Result<(), Failure> {
    let id: Id = card
        .id
        .parse()
        .map_err(|_| Failure::Op(messages::storage("storage-task-id", &[])))?;
    card.labels = task_labels(tx, id)?;
    card.sprint = super::sprint::of_task(tx, id)?;
    card.progress_percent = super::rollup::progress(tx, id)?;
    if let Some(parent) = card.parent.take()
        && let Ok(parent) = parent.parse::<Id>()
        && let Some(row) = lookup::task_by_id(tx, parent)?
    {
        card.parent = Some(lookup::task_key(tx, row.project, row.number)?);
    }
    Ok(())
}

/// Gives `card` its values of the custom fields `shown` (query names).
fn card_values(tx: &Transaction<'_>, card: &mut Card, shown: &[String]) -> Result<(), Failure> {
    let id: Id = card
        .id
        .parse()
        .map_err(|_| Failure::Op(messages::storage("storage-task-id", &[])))?;
    card.fields = super::custom_field::listed_values(tx, id)?
        .into_iter()
        .filter(|(name, _)| shown.contains(name))
        .collect();
    Ok(())
}

/// After a custom field was renamed or deleted: the board layouts of
/// `project` (of every project when `None`) that show it on cards show it
/// under its new name, or no longer. `column` maps a `cf.<name>` entry as
/// for a saved view's columns: `Some(Some(new))`, `Some(None)` to drop it,
/// `None` to keep it.
pub(crate) fn rewrite_card_fields(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    column: &dyn Fn(&str) -> Option<Option<String>>,
) -> Result<(), Failure> {
    let scopes: Vec<String> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT scope FROM view_settings WHERE view_kind = ?1 ORDER BY scope",
        )?;
        stmt.query_map([VIEW], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for scope in scopes {
        let Ok(id) = scope.parse::<Id>() else {
            continue;
        };
        if project.is_some_and(|p| p != id) {
            continue;
        }
        let Some(before) = raw(w.tx(), id)? else {
            continue;
        };
        let mut after = before.clone();
        let mut touched = false;
        if let Some(fields) = before.get("card_fields").and_then(Value::as_array) {
            let mut kept = Vec::with_capacity(fields.len());
            for f in fields {
                let Some(text) = f.as_str() else { continue };
                match column(text) {
                    Some(Some(new)) => {
                        kept.push(Value::String(new));
                        touched = true;
                    }
                    Some(None) => touched = true,
                    None => kept.push(f.clone()),
                }
            }
            after["card_fields"] = Value::Array(kept);
        }
        // The swimlanes' field follows too; a deleted one ends the lanes.
        if let Some(text) = before.get("lane_field").and_then(Value::as_str)
            && !text.is_empty()
        {
            match column(text) {
                Some(Some(new)) => {
                    after["lane_field"] = Value::String(new);
                    touched = true;
                }
                Some(None) => {
                    after["lane_field"] = Value::String(String::new());
                    after["lanes"] = Value::String("none".to_owned());
                    touched = true;
                }
                None => {}
            }
        }
        if touched {
            write_raw(w, id, Some(&after))?;
            w.record_field(ENTITY, id, Some(id), "settings", Some(before), Some(after));
        }
    }
    Ok(())
}

/// After a project, status, type, label or custom field was renamed or
/// removed: the colour rules of the board layouts of `project` (of every
/// project when `None`) read as `rewrite` gives them (`None` keeps a
/// query).
pub(crate) fn rewrite_colour_rules(
    w: &mut WriteCtx<'_>,
    project: Option<Id>,
    rewrite: &dyn Fn(&str) -> Option<String>,
) -> Result<(), Failure> {
    let scopes: Vec<String> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT scope FROM view_settings WHERE view_kind = ?1 ORDER BY scope",
        )?;
        stmt.query_map([VIEW], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for scope in scopes {
        let Ok(id) = scope.parse::<Id>() else {
            continue;
        };
        if project.is_some_and(|p| p != id) {
            continue;
        }
        let Some(before) = raw(w.tx(), id)? else {
            continue;
        };
        let Some(rules) = before.get("colour_rules").and_then(Value::as_array) else {
            continue;
        };
        let mut touched = false;
        let mut kept = Vec::with_capacity(rules.len());
        for rule in rules {
            let mut rule = rule.clone();
            if let Some(new) = rule.get("query").and_then(Value::as_str).and_then(rewrite) {
                rule["query"] = Value::String(new);
                touched = true;
            }
            kept.push(rule);
        }
        if touched {
            let mut after = before.clone();
            after["colour_rules"] = Value::Array(kept);
            write_raw(w, id, Some(&after))?;
            w.record_field(ENTITY, id, Some(id), "settings", Some(before), Some(after));
        }
    }
    Ok(())
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
    let args = [
        ("project", p.key.clone()),
        ("column", text.trim().to_owned()),
    ];
    Err(messages::with("board-column-not-found", &args, |m| {
        OpError::not_found(m, &names.iter().map(String::as_str).collect::<Vec<_>>())
    })
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
            return Err(messages::invalid(
                "hide_done_after_days",
                "board-hide-done-range",
                &[("max", "3650".to_owned())],
            ));
        }
        layout.hide_done_after_days = days;
    }
    if let Some(fields) = &input.card_fields {
        let custom = super::custom_field::rows(w.tx(), Some(p.id))?;
        let mut chosen = Vec::new();
        for (i, f) in fields.iter().enumerate() {
            if let Some(name) = gk_query::fields::strip_custom_prefix(f.trim()) {
                let wanted = fold(name);
                let Some(field) = custom
                    .iter()
                    .find(|c| c.query_name == wanted || fold(&c.name) == wanted)
                else {
                    let names: Vec<String> = custom
                        .iter()
                        .map(|c| format!("{}{}", gk_query::fields::CUSTOM_PREFIX, c.query_name))
                        .collect();
                    return Err(messages::invalid_at(
                        &format!("card_fields[{i}]"),
                        "board-card-custom-field",
                        &[("field", f.trim().to_owned()), ("project", p.key.clone())],
                    )
                    .with_detail("choices", names)
                    .into());
                };
                let written = format!("{}{}", gk_query::fields::CUSTOM_PREFIX, field.query_name);
                if !chosen.contains(&written) {
                    chosen.push(written);
                }
                continue;
            }
            let f = f.trim().to_lowercase();
            if !CARD_FIELDS.contains(&f.as_str()) {
                return Err(messages::invalid_at(
                    &format!("card_fields[{i}]"),
                    "board-card-field",
                    &[("field", f.clone())],
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
    if let Some(by) = input.colour_by {
        layout.colour_by = by;
    }
    if let Some(rules) = &input.colour_rules {
        if rules.len() > MAX_COLOUR_RULES {
            return Err(messages::invalid(
                "colour_rules",
                "board-colour-rules-max",
                &[("max", MAX_COLOUR_RULES.to_string())],
            ));
        }
        let mut kept = Vec::with_capacity(rules.len());
        for (i, rule) in rules.iter().enumerate() {
            let field = format!("colour_rules[{i}].query");
            let query = rule.query.trim();
            if query.is_empty() {
                return Err(messages::invalid(&field, "board-colour-rule-empty", &[]));
            }
            super::query::compile(w.tx(), w.zone(), w.now(), query, &field)?;
            kept.push(ColourRule {
                query: query.to_owned(),
                hue: rule.hue,
                name: rule.name.trim().to_owned(),
            });
        }
        layout.colour_rules = kept;
    }
    if let Some(covers) = input.covers {
        layout.covers = covers;
    }
    lanes_update(w.tx(), &p, &mut layout, &input)?;
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

/// The swimlane part of a layout change: what the lanes group by and the
/// select field.
fn lanes_update(
    tx: &Transaction<'_>,
    p: &ProjectRow,
    layout: &mut BoardLayout,
    input: &LayoutUpdateInput,
) -> Result<(), Failure> {
    if let Some(text) = &input.lane_field {
        if text.trim().is_empty() {
            layout.lane_field.clear();
        } else {
            let field = super::swimlane::select_field(tx, p.id, text)?
                .ok_or_else(|| lane_field_unknown(p, text, "lane_field"))?;
            layout.lane_field = format!("{}{}", gk_query::fields::CUSTOM_PREFIX, field.query_name);
        }
    }
    if let Some(by) = input.lanes {
        layout.lanes = by;
    }
    if layout.lanes != LaneBy::Field {
        layout.lane_field.clear();
    } else if layout.lane_field.is_empty() {
        return Err(messages::invalid(
            "lane_field",
            "board-lane-field-needed",
            &[],
        ));
    }
    Ok(())
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
            wip_min: None,
            under_minimum: false,
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
