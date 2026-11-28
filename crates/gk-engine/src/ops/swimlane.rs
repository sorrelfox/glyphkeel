// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The board's swimlanes: bands across the columns, one for each value of
//! the field the layout groups by, and one last for the cards without one.
//!
//! The lanes are worked out from the cards the board already read, so the
//! board makes no query per lane: the values a lane needs beyond the card
//! (the project's types and open sprints, a select field's values, the
//! parents' titles) are read once for the whole board. Priorities, types,
//! open sprints and a select field's options always have their lane, so a
//! card can be dropped into one that is empty yet; labels, parents,
//! projects and completed sprints have one while a card shows them. With sub-projects,
//! types and labels are matched by name, as columns are by status name.

use std::collections::{BTreeMap, HashMap};

use gk_api::ops::board::{Card, Column, EMPTY_LANE, LaneBy, Swimlane};
use gk_api::ops::common::{Hue, Priority};
use gk_core::{FieldKind, Id, fold};
use gk_store::rusqlite::{Transaction, params_from_iter};

use super::custom_field::{self, FieldRow};
use super::lookup::{b, id_at};
use super::project::parse_enum;
use crate::failure::Failure;

/// The most parents whose titles one statement reads.
const CHUNK: usize = 500;

/// The select field a board's lanes group by, among those of `project`: by
/// name, query name (with or without `cf.`) or ID.
pub(crate) fn select_field(
    tx: &Transaction<'_>,
    project: Id,
    text: &str,
) -> Result<Option<FieldRow>, Failure> {
    let text = text.trim();
    let id = text.parse::<Id>().ok();
    let bare = gk_query::fields::strip_custom_prefix(text).unwrap_or(text);
    let wanted = fold(bare);
    Ok(custom_field::rows(tx, Some(project))?
        .into_iter()
        .filter(|f| f.kind == FieldKind::Select)
        .find(|f| Some(f.id) == id || f.query_name == wanted || fold(&f.name) == wanted))
}

/// A lane before its counts.
struct Draft {
    lane: Swimlane,
    /// Where it goes among the lanes found on cards.
    order: String,
    /// For a type or label, its folded name: one lane for every type or
    /// label of that name.
    named: bool,
}

fn lane(id: String, name: String, value: Option<String>) -> Swimlane {
    Swimlane {
        id,
        name,
        key: None,
        value,
        empty: false,
        hue: None,
        settable: true,
        count: 0,
        counts: BTreeMap::new(),
    }
}

fn priority_word(p: Priority) -> Option<&'static str> {
    match p {
        Priority::None => None,
        Priority::Low => Some("low"),
        Priority::Normal => Some("normal"),
        Priority::High => Some("high"),
        Priority::Urgent => Some("urgent"),
    }
}

/// What the lanes of one board group by, with what was read for it.
pub(crate) struct Lanes {
    by: LaneBy,
    /// The lanes every board of this kind has, in order.
    fixed: Vec<Swimlane>,
    /// The lane ID of a type's or label's folded name (with sub-projects,
    /// one lane for every type or label of that name).
    by_name: HashMap<String, String>,
    /// A select field's value of each task, by task ID.
    values: HashMap<String, String>,
    /// For lanes by project: each project's place, name and colour, by key.
    projects: HashMap<String, (usize, String, Hue)>,
}

impl Lanes {
    /// Reads what lanes by `by` need for a board of `projects` (the
    /// board's own first); `field` is the select field for `LaneBy::Field`.
    pub(crate) fn load(
        tx: &Transaction<'_>,
        by: LaneBy,
        projects: &[Id],
        field: Option<&FieldRow>,
    ) -> Result<Self, Failure> {
        let mut fixed = Vec::new();
        let mut by_name = HashMap::new();
        let mut values = HashMap::new();
        let mut known = HashMap::new();
        match by {
            LaneBy::None | LaneBy::Label | LaneBy::Parent => {}
            LaneBy::Project => {
                let mut stmt =
                    tx.prepare_cached("SELECT key, name, hue FROM projects WHERE id = ?1")?;
                for (i, p) in projects.iter().enumerate() {
                    let row = stmt
                        .query_map([b(*p)], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                            ))
                        })?
                        .next()
                        .transpose()?;
                    if let Some((key, name, hue)) = row {
                        known.insert(key, (i, name, parse_enum(&hue).unwrap_or(Hue::Grey)));
                    }
                }
            }
            LaneBy::Priority => {
                for p in [
                    Priority::Urgent,
                    Priority::High,
                    Priority::Normal,
                    Priority::Low,
                ] {
                    let word = priority_word(p).unwrap_or_default().to_owned();
                    fixed.push(lane(word.clone(), word.clone(), Some(word)));
                }
            }
            LaneBy::Type => {
                for p in projects {
                    let mut stmt = tx.prepare_cached(
                        "SELECT id, name, hue FROM task_types WHERE project_id = ?1
                          ORDER BY rank, id",
                    )?;
                    let rows = stmt
                        .query_map([b(*p)], |r| {
                            Ok((id_at(r, 0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    for (id, name, hue) in rows {
                        let key = fold(&name);
                        if by_name.contains_key(&key) {
                            continue;
                        }
                        by_name.insert(key, id.to_string());
                        let mut l = lane(id.to_string(), name.clone(), Some(name));
                        l.hue = Some(parse_enum(&hue).unwrap_or(Hue::Grey));
                        fixed.push(l);
                    }
                }
            }
            LaneBy::Sprint => {
                for p in projects {
                    let mut stmt = tx.prepare_cached(
                        "SELECT id, name FROM sprints WHERE project_id = ?1 AND state <> 'closed'
                          ORDER BY CASE state WHEN 'active' THEN 0 ELSE 1 END, rank, id",
                    )?;
                    let rows = stmt
                        .query_map([b(*p)], |r| Ok((id_at(r, 0)?, r.get::<_, String>(1)?)))?
                        .collect::<Result<Vec<_>, _>>()?;
                    for (id, name) in rows {
                        fixed.push(lane(id.to_string(), name, Some(id.to_string())));
                    }
                }
            }
            LaneBy::Field => {
                if let Some(field) = field {
                    for option in &field.options {
                        fixed.push(lane(option.clone(), option.clone(), Some(option.clone())));
                    }
                    let mut stmt = tx.prepare_cached(
                        "SELECT task_id, value FROM custom_values
                          WHERE field_id = ?1 AND typeof(value) = 'text'",
                    )?;
                    let rows = stmt
                        .query_map([b(field.id)], |r| {
                            Ok((id_at(r, 0)?.to_string(), r.get::<_, String>(1)?))
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    values.extend(rows);
                }
            }
        }
        Ok(Self {
            by,
            fixed,
            by_name,
            values,
            projects: known,
        })
    }

    /// The lane of `card` (`None` for the lane without a value), and a
    /// lane to add for it when it is the first card of a lane found on
    /// cards. `parent` is the parent task's ID.
    fn of(&self, card: &Card, parent: Option<&str>) -> (Option<String>, Option<Draft>) {
        match self.by {
            LaneBy::None => (None, None),
            LaneBy::Priority => (priority_word(card.priority).map(str::to_owned), None),
            LaneBy::Type => {
                let key = fold(&card.task_type.name);
                if let Some(id) = self.by_name.get(&key) {
                    return (Some(id.clone()), None);
                }
                let id = card.task_type.id.clone();
                let name = card.task_type.name.clone();
                let draft = Draft {
                    lane: lane(id.clone(), name.clone(), Some(name)),
                    order: key,
                    named: true,
                };
                (Some(id), Some(draft))
            }
            LaneBy::Label => {
                let Some(label) = card.labels.first() else {
                    return (None, None);
                };
                let key = fold(&label.name);
                let mut l = lane(
                    label.id.clone(),
                    label.name.clone(),
                    Some(label.name.clone()),
                );
                l.hue = Some(label.hue);
                let draft = Draft {
                    lane: l,
                    order: key,
                    named: true,
                };
                (Some(label.id.clone()), Some(draft))
            }
            LaneBy::Sprint => {
                let Some(sprint) = &card.sprint else {
                    return (None, None);
                };
                let mut l = lane(
                    sprint.id.clone(),
                    sprint.name.clone(),
                    Some(sprint.id.clone()),
                );
                l.settable = sprint.state != gk_api::ops::sprint::SprintState::Closed;
                let order = sprint.end_date.clone().unwrap_or_default();
                let draft = Draft {
                    lane: l,
                    order,
                    named: false,
                };
                (Some(sprint.id.clone()), Some(draft))
            }
            LaneBy::Parent => {
                let Some(parent) = parent else {
                    return (None, None);
                };
                let mut l = lane(parent.to_owned(), String::new(), Some(parent.to_owned()));
                l.key.clone_from(&card.parent);
                let order = card.parent.as_deref().map(key_order).unwrap_or_default();
                let draft = Draft {
                    lane: l,
                    order,
                    named: false,
                };
                (Some(parent.to_owned()), Some(draft))
            }
            LaneBy::Project => {
                let (order, name, hue) = self.projects.get(&card.project).cloned().unwrap_or((
                    usize::MAX,
                    card.project.clone(),
                    Hue::Grey,
                ));
                let mut l = lane(card.project.clone(), name, None);
                l.hue = Some(hue);
                // A card changes project by a move of its own, never by a drop.
                l.settable = false;
                let draft = Draft {
                    lane: l,
                    order: format!("{order:020}"),
                    named: false,
                };
                (Some(card.project.clone()), Some(draft))
            }
            LaneBy::Field => {
                let Some(value) = self.values.get(&card.id) else {
                    return (None, None);
                };
                let draft = Draft {
                    lane: lane(value.clone(), value.clone(), Some(value.clone())),
                    order: fold(value),
                    named: false,
                };
                (Some(value.clone()), Some(draft))
            }
        }
    }

    /// Gives every shown card of `columns` its lane and returns the lanes
    /// in order with their counts; `parents` holds each card's parent ID
    /// by card ID.
    pub(crate) fn assign(
        self,
        tx: &Transaction<'_>,
        columns: &mut [Column],
        parents: &HashMap<String, String>,
    ) -> Result<Vec<Swimlane>, Failure> {
        if self.by == LaneBy::None {
            return Ok(Vec::new());
        }
        let mut fixed = self.fixed.clone();
        let mut found: Vec<Draft> = Vec::new();
        let mut empty = lane(EMPTY_LANE.to_owned(), String::new(), None);
        empty.empty = true;
        let mut has_empty = false;
        let zero: BTreeMap<String, u64> = columns
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| (c.id.clone(), 0))
            .collect();
        let mut tally: HashMap<String, BTreeMap<String, u64>> = HashMap::new();
        for column in columns.iter_mut().filter(|c| !c.hidden) {
            for card in &mut column.cards {
                let (id, draft) = self.of(card, parents.get(&card.id).map(String::as_str));
                let mut id = id.unwrap_or_else(|| {
                    has_empty = true;
                    EMPTY_LANE.to_owned()
                });
                if let Some(draft) = draft
                    && !fixed.iter().any(|l| l.id == draft.lane.id)
                {
                    let same = found.iter().find(|d| {
                        d.lane.id == draft.lane.id
                            || (draft.named && d.named && d.order == draft.order)
                    });
                    match same {
                        Some(d) => id.clone_from(&d.lane.id),
                        None => found.push(draft),
                    }
                }
                *tally
                    .entry(id.clone())
                    .or_insert_with(|| zero.clone())
                    .entry(column.id.clone())
                    .or_insert(0) += 1;
                card.lane = Some(id);
            }
        }
        if self.by == LaneBy::Parent {
            name_parents(tx, &mut found)?;
        }
        found.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then_with(|| a.lane.id.cmp(&b.lane.id))
        });
        // Completed sprints follow the open ones, as a type or label only
        // a sub-project has follows those of the board's own project.
        fixed.extend(found.into_iter().map(|d| d.lane));
        // Every task has a type and a project: no lane without one.
        if !matches!(self.by, LaneBy::Type | LaneBy::Project) || has_empty {
            fixed.push(empty);
        }
        for l in &mut fixed {
            l.counts = tally.remove(&l.id).unwrap_or_else(|| zero.clone());
            l.count = l.counts.values().sum();
        }
        Ok(fixed)
    }
}

/// Orders keys such as `TM-9` before `TM-10`.
fn key_order(key: &str) -> String {
    match key.rsplit_once('-') {
        Some((project, number)) => format!(
            "{project}-{:0>12}",
            number.parse::<u64>().unwrap_or(u64::MAX)
        ),
        None => key.to_owned(),
    }
}

/// Gives the lanes of parents their parents' titles, a few hundred parents
/// a statement.
fn name_parents(tx: &Transaction<'_>, lanes: &mut [Draft]) -> Result<(), Failure> {
    let ids: Vec<Id> = lanes
        .iter()
        .filter_map(|d| d.lane.id.parse::<Id>().ok())
        .collect();
    let mut titles: HashMap<String, String> = HashMap::new();
    for chunk in ids.chunks(CHUNK) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let mut stmt = tx.prepare(&format!(
            "SELECT id, title FROM tasks WHERE id IN ({marks})"
        ))?;
        let rows = stmt
            .query_map(params_from_iter(chunk.iter().map(|id| b(*id))), |r| {
                Ok((id_at(r, 0)?.to_string(), r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        titles.extend(rows);
    }
    for d in lanes {
        if let Some(title) = titles.remove(&d.lane.id) {
            d.lane.name = title;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::key_order;

    #[test]
    fn keys_order_by_their_number() {
        assert!(key_order("TM-9") < key_order("TM-10"));
        assert!(key_order("AB-100") < key_order("TM-1"));
    }
}
