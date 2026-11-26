// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A project's board, and the person's own layout of it.
//!
//! The layout (hidden, collapsed and ordered columns, sort modes, card
//! fields, older closed cards, sub-projects) is personal and kept per
//! project; it changes how the board looks, never the project's statuses.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Category, Hue, Priority};
use crate::ops::task::{Checklist, Claim, LabelRef, TypeRef};

/// How a column's cards are ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    /// By hand, by dragging (the default).
    #[default]
    Manual,
    /// Highest priority first.
    Priority,
    /// Soonest due date first; no due date last.
    Due,
    /// Most recently changed first.
    Updated,
}

/// The person's layout of one project's board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardLayout {
    /// The column order, as status IDs. Statuses not listed keep their
    /// project position between their neighbours.
    pub order: Vec<String>,
    /// Hidden columns, as status IDs.
    pub hidden: Vec<String>,
    /// Collapsed columns, as status IDs.
    pub collapsed: Vec<String>,
    /// The sort mode of every column without its own.
    pub sort_mode: SortMode,
    /// Sort modes of single columns, by status ID.
    pub column_sort: BTreeMap<String, SortMode>,
    /// Cards closed more than this many days ago are hidden behind "Show
    /// older"; 0 shows every card.
    pub hide_done_after_days: u32,
    /// The fields shown on cards; custom fields as `cf.<query name>`.
    pub card_fields: Vec<String>,
    /// Whether the sub-projects' tasks are on the board too.
    pub include_subprojects: bool,
}

impl Default for BoardLayout {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            hidden: Vec::new(),
            collapsed: Vec::new(),
            sort_mode: SortMode::Manual,
            column_sort: BTreeMap::new(),
            hide_done_after_days: 14,
            card_fields: ["type", "priority", "due", "labels", "checklist"]
                .map(str::to_owned)
                .to_vec(),
            include_subprojects: false,
        }
    }
}

/// One card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Card {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key (differs from the board's for sub-project tasks).
    pub project: String,
    /// Its type.
    #[serde(rename = "type")]
    pub task_type: TypeRef,
    /// Its priority.
    pub priority: Priority,
    /// Its labels.
    pub labels: Vec<LabelRef>,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The estimate in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// The description's checklist.
    pub checklist: Checklist,
    /// Its progress in percent, as tasks show it.
    pub progress_percent: u8,
    /// Its subtasks: closed and all.
    pub subtasks: Checklist,
    /// Whether a task that blocks it is still open.
    pub blocked: bool,
    /// How many comments it has.
    pub comment_count: u32,
    /// How many files are attached to it and to its comments.
    pub file_count: u32,
    /// Logged time in minutes.
    pub spent_minutes: i64,
    /// The agent that claimed it, while the claim lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// Its place in the column.
    pub rank: String,
    /// When it entered its status.
    pub status_entered_at: String,
    /// When anything about it last changed.
    pub updated_at: String,
    /// When it was closed; left out while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    /// Its version, for `expected_version`.
    pub version: i64,
    /// The task's ID.
    pub id: String,
    /// Its values of the custom fields the layout shows on cards, by query
    /// name (without `cf.`); left out when it has none of them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
}

/// One column: a status, or with sub-projects, every status of that name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Column {
    /// The status's ID in the board's own project; a status that exists
    /// only in sub-projects has the first sub-project's.
    pub id: String,
    /// The status name.
    pub name: String,
    /// Its category.
    pub category: Category,
    /// Its colour.
    pub hue: Hue,
    /// Every status shown in this column, by project key: one, or more when
    /// sub-projects are included.
    pub statuses: BTreeMap<String, String>,
    /// The WIP limit, if the status has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<u32>,
    /// Whether the column holds more tasks than its WIP limit.
    pub over_limit: bool,
    /// The WIP minimum, if the status has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_min: Option<u32>,
    /// Whether the column holds fewer tasks than its WIP minimum.
    #[serde(default)]
    pub under_minimum: bool,
    /// Tasks in the column, including hidden older ones.
    pub count: u64,
    /// Closed cards hidden behind "Show older".
    pub older: u64,
    /// Hidden by the layout; its cards are left out.
    pub hidden: bool,
    /// Collapsed by the layout.
    pub collapsed: bool,
    /// How its cards are ordered.
    pub sort_mode: SortMode,
    /// The cards, in order.
    pub cards: Vec<Card>,
}

/// Which board to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoardGetInput {
    /// The project: its key (current or old) or ID.
    pub project: String,
    /// Show the sub-projects' tasks too; default: as the layout says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_subprojects: Option<bool>,
    /// Show every closed card, however old.
    #[serde(default)]
    pub show_older: bool,
}

/// A project's board as the person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Board {
    /// The project's key.
    pub project: String,
    /// The columns, in the layout's order, hidden ones included and
    /// flagged.
    pub columns: Vec<Column>,
    /// The layout the board was built with.
    pub layout: BoardLayout,
    /// Hidden columns and the tasks in them.
    pub hidden_columns: u64,
    /// Tasks in hidden columns.
    pub hidden_tasks: u64,
}

/// Changes to a board layout; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutUpdateInput {
    /// The project: its key (current or old) or ID.
    pub project: String,
    /// Go back to the project's layout first: every column shown and
    /// expanded, in project order, sorted by hand.
    #[serde(default)]
    pub reset: bool,
    /// Columns to hide, by status name or ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
    /// Columns to show again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub show: Vec<String>,
    /// Columns to collapse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collapse: Vec<String>,
    /// Columns to expand again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expand: Vec<String>,
    /// The column order, by status name or ID; columns left out follow in
    /// project order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// The sort mode of every column without its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_mode: Option<SortMode>,
    /// Sort modes of single columns, by status name or ID; `manual` columns
    /// can be reordered by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_sort: Option<BTreeMap<String, SortMode>>,
    /// Hide cards closed more than this many days ago; 0 shows all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_done_after_days: Option<u32>,
    /// The fields shown on cards: `type`, `priority`, `due`, `start`,
    /// `labels`, `checklist`, `subtasks`, `estimate`, `days_in_column`,
    /// `updated`, `key`, and custom fields of the project as
    /// `cf.<query name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_fields: Option<Vec<String>>,
    /// Show the sub-projects' tasks too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_subprojects: Option<bool>,
}

/// A layout after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutChange {
    /// The project's key.
    pub project: String,
    /// The layout after the change.
    pub layout: BoardLayout,
    /// The history group; left out when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Which layout to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutGetInput {
    /// The project: its key (current or old) or ID.
    pub project: String,
}

op!(
    /// A project's board.
    BoardGet,
    "board.get",
    BoardGetInput,
    Board
);
op!(
    /// The person's layout of a board.
    LayoutGet,
    "layout.get",
    LayoutGetInput,
    LayoutChange
);
op!(
    /// Changes the person's layout of a board.
    LayoutUpdate,
    "layout.update",
    LayoutUpdateInput,
    LayoutChange
);

pub(crate) const BOARD_GET: OpDescriptor = OpDescriptor::of::<BoardGet>(Meta {
    summary: "A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state and compact cards.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("board <project>"),
    input_name: "BoardGetInput",
    output_name: "Board",
});

pub(crate) const LAYOUT_GET: OpDescriptor = OpDescriptor::of::<LayoutGet>(Meta {
    summary: "The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("board layout show <project>"),
    input_name: "LayoutGetInput",
    output_name: "LayoutChange",
});

pub(crate) const LAYOUT_UPDATE: OpDescriptor = OpDescriptor::of::<LayoutUpdate>(Meta {
    summary: "Hide, show, collapse or reorder board columns, set sort modes, card fields and older closed cards, or reset to the project's layout.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("board layout set <project>"),
    input_name: "LayoutUpdateInput",
    output_name: "LayoutChange",
});
