// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! A project's board, and the person's own layout of it.
//!
//! The layout (hidden, collapsed and ordered columns, sort modes, card
//! fields, card colours and covers, swimlanes, older closed cards,
//! sub-projects) is personal and kept per project; it changes how the board
//! looks, never the project's statuses.

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

/// What colours the edge of a card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardColourBy {
    /// No coloured edge (the default).
    #[default]
    None,
    /// The colour of the task's type.
    Type,
    /// A colour for each priority: urgent red, high orange, normal blue,
    /// low grey; none has no edge.
    Priority,
    /// The colour of the task's first label.
    Label,
    /// The first of the layout's colour rules whose query the task matches.
    Query,
}

/// One colour rule: the cards a query matches get its colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColourRule {
    /// A query in the query language, such as `label:urgent due<today`.
    pub query: String,
    /// The colour of the cards it matches.
    pub hue: Hue,
    /// A name for the rule, shown with the edge; empty shows the query.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// Which image a card shows as its cover.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverMode {
    /// No covers (the default).
    #[default]
    Off,
    /// The image chosen as the task's cover (`file.cover`), if it has one.
    Chosen,
    /// The chosen image, or else the task's first attached image.
    FirstImage,
}

/// What the board's swimlanes group the cards by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LaneBy {
    /// No swimlanes (the default).
    #[default]
    None,
    /// The parent task, such as an epic.
    Parent,
    /// The priority.
    Priority,
    /// The first label.
    Label,
    /// The sprint.
    Sprint,
    /// The type.
    Type,
    /// A select custom field, named by `lane_field`.
    Field,
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
    /// What colours the edge of the cards.
    #[serde(default)]
    pub colour_by: CardColourBy,
    /// The colour rules for `colour_by: query`, in order; the first whose
    /// query a card matches colours it.
    #[serde(default)]
    pub colour_rules: Vec<ColourRule>,
    /// Which image the cards show as their cover.
    #[serde(default)]
    pub covers: CoverMode,
    /// What the swimlanes group the cards by.
    #[serde(default)]
    pub lanes: LaneBy,
    /// With `lanes: field`, the select custom field, as `cf.<query name>`;
    /// empty otherwise.
    #[serde(default)]
    pub lane_field: String,
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
            colour_by: CardColourBy::None,
            colour_rules: Vec::new(),
            covers: CoverMode::Off,
            lanes: LaneBy::None,
            lane_field: String::new(),
        }
    }
}

/// The coloured edge of a card and the rule that gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CardEdge {
    /// The colour.
    pub hue: Hue,
    /// What chose it.
    pub by: CardColourBy,
    /// What it stands for: the type's or label's name, the priority, or the
    /// rule's name (its query when it has none).
    pub name: String,
}

/// The image a card shows as its cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CardCover {
    /// The attachment's ID.
    pub attachment: String,
    /// Its file name, for the image's text.
    pub name: String,
    /// Its media type.
    pub mime: String,
    /// Its width in pixels, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Its height in pixels, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
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
    /// The sprint it is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<crate::ops::sprint::SprintRef>,
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
    /// Whether it is the open occurrence of a recurring task.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recurring: bool,
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
    /// Its coloured edge, as the layout's `colour_by` chooses; left out
    /// when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<CardEdge>,
    /// Its cover image, as the layout's `covers` chooses; left out when it
    /// has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<CardCover>,
    /// The ID of the swimlane it is in; left out without swimlanes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
}

/// The ID of the swimlane of the cards without a value.
pub const EMPTY_LANE: &str = "none";

/// One swimlane: a band across the columns for one value of the field the
/// layout's `lanes` chooses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Swimlane {
    /// The lane's ID: the priority, the option, or the ID of the type,
    /// label, sprint or parent task; `none` for the lane of the cards
    /// without a value.
    pub id: String,
    /// Its name: the priority, the type's, label's, sprint's or option's
    /// name, or the parent's title; empty for the lane without a value.
    pub name: String,
    /// The parent task's key, for a lane of a parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// What a card dropped into the lane gets, as `task.move`'s `changes`
    /// take it: the priority, the type's or label's name, the sprint's or
    /// parent's ID, or the option; left out for the lane without a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The lane of the cards without a value; it comes last.
    pub empty: bool,
    /// Its colour, for a type or label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Whether cards can be dropped into it to take its value; a completed
    /// sprint can't take tasks.
    pub settable: bool,
    /// The cards it holds on the board, closed cards behind "Show older"
    /// and hidden columns aside.
    pub count: u64,
    /// Those cards by column ID.
    pub counts: BTreeMap<String, u64>,
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
    /// Group the cards into swimlanes by this; default: as the layout says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<LaneBy>,
    /// With `lanes: field`, the select custom field (name or `cf.<query
    /// name>`); default: the layout's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_field: Option<String>,
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
    /// The swimlanes in order, the lane without a value last; empty
    /// without swimlanes. Each card names its lane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<Swimlane>,
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
    /// What colours the edge of the cards: `none`, `type`, `priority`,
    /// `label` or `query` (the colour rules).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_by: Option<CardColourBy>,
    /// The colour rules, in order, replacing the old ones: each a query and
    /// a colour, and an optional name. At most 20.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_rules: Option<Vec<ColourRule>>,
    /// Which image the cards show as their cover: `off`, `chosen` or
    /// `first_image`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<CoverMode>,
    /// What the swimlanes group the cards by: `none`, `parent`, `priority`,
    /// `label`, `sprint`, `type` or `field` (with `lane_field`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<LaneBy>,
    /// The select custom field the lanes group by, by name or as
    /// `cf.<query name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_field: Option<String>,
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
    summary: "A project's board as the person sees it: columns in the personal order with hidden ones flagged, WIP state, compact cards and the swimlanes with their counts.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("board <project>"),
    input_name: "BoardGetInput",
    output_name: "Board",
});

pub(crate) const LAYOUT_GET: OpDescriptor = OpDescriptor::of::<LayoutGet>(Meta {
    summary: "The person's layout of a project's board: column order, hidden and collapsed columns, sort modes, card fields, card colours, covers and swimlanes.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("board layout show <project>"),
    input_name: "LayoutGetInput",
    output_name: "LayoutChange",
});

pub(crate) const LAYOUT_UPDATE: OpDescriptor = OpDescriptor::of::<LayoutUpdate>(Meta {
    summary: "Hide, show, collapse or reorder board columns, set sort modes, card fields, card colours (by type, priority, label or ordered query rules), covers, swimlanes and older closed cards, or reset to the project's layout.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("board layout set <project>"),
    input_name: "LayoutUpdateInput",
    output_name: "LayoutChange",
});
