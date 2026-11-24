// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Saved views: a named query with its columns, order, grouping and
//! totals, for one project or for the whole workspace. A view stores the
//! query text, which people can read and edit; relative dates in it are
//! evaluated each time it runs.
//!
//! Views belong to the workspace and everyone using it sees them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};

/// Which screen a view is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// The task list (the default).
    #[default]
    List,
    /// The board.
    Board,
    /// The schedule.
    Gantt,
    /// The calendar.
    Calendar,
}

impl ViewKind {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Board => "board",
            Self::Gantt => "gantt",
            Self::Calendar => "calendar",
        }
    }
}

/// A saved view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedView {
    /// Its name.
    pub name: String,
    /// The screen it is for.
    pub kind: ViewKind,
    /// The project's key; left out for a view of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The query text.
    pub query: String,
    /// The columns shown, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// The order when the query has no `sort:` (`due,-priority`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// The field rows are grouped by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    /// The columns totalled (`estimate`, `spent`, `remaining`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<String>,
    /// Shown first when its screen opens.
    pub is_default: bool,
    /// Its place among the views.
    pub rank: String,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// Its ID.
    pub id: String,
}

/// Which views to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewListInput {
    /// The views of this project (key or ID); left out: the views of the
    /// whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only views for this screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ViewKind>,
    /// Every view, of every project and the workspace.
    #[serde(default)]
    pub all: bool,
}

/// Saved views, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewList {
    /// The views.
    pub items: Vec<SavedView>,
    /// How many.
    pub total: u64,
}

/// A new saved view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewCreateInput {
    /// Its name, unique among the views of its project and screen.
    pub name: String,
    /// The query text; it must be valid.
    pub query: String,
    /// The screen; default: the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ViewKind>,
    /// The project (key or ID); left out: the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The columns shown, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// The order when the query has no `sort:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// The field rows are grouped by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    /// The columns totalled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<String>,
    /// Make it the screen's default view.
    #[serde(default)]
    pub is_default: bool,
}

/// A view field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewField {
    /// The description.
    Description,
    /// The columns.
    Columns,
    /// The order.
    Sort,
    /// The grouping.
    GroupBy,
    /// The totals.
    Totals,
}

/// Changes to a saved view; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewUpdateInput {
    /// The view: its ID, or its name.
    pub view: String,
    /// With a name: the view's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// A new description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// New columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// A new order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// A new grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    /// New totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totals: Option<Vec<String>>,
    /// Make it, or stop it being, the screen's default view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<ViewField>,
}

/// Which view to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewDeleteInput {
    /// The view: its ID, or its name.
    pub view: String,
    /// With a name: the view's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A view after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewChange {
    /// The view after the change (before it, for a deletion).
    pub view: SavedView,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// The person's own layout of a list: the order of its columns, their
/// widths and the groups folded away, kept per project (and once for the
/// whole workspace's list).
/// Unlike saved views it belongs to the person, not to the workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListLayout {
    /// The columns shown, in order; empty: the list's own choice.
    #[serde(default)]
    pub columns: Vec<String>,
    /// Column widths in pixels, by column; the columns left out keep their
    /// default width.
    #[serde(default)]
    pub widths: BTreeMap<String, u32>,
    /// The groups the person folded away, by grouping (`status`,
    /// `priority`, `label`, `parent`, `due_week`, `project`): each group's
    /// value as `task.search` gives it, and an empty text for the group of
    /// tasks without a value. Left out when none are.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collapsed: BTreeMap<String, Vec<String>>,
}

/// Which list layout to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListLayoutGetInput {
    /// The project (key or ID); left out: the list of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Changes to a list layout; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListLayoutUpdateInput {
    /// The project (key or ID); left out: the list of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Go back to the default layout first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    /// The columns shown, in order (machine names such as `due`); empty:
    /// the list's own choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    /// Widths to set, in pixels (40 to 1200), by column; the others keep
    /// theirs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, u32>,
    /// The folded groups to keep, by grouping; each grouping given replaces
    /// its list, and an empty list unfolds them all. The others keep theirs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collapsed: BTreeMap<String, Vec<String>>,
}

/// A list layout, after a change or as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListLayoutChange {
    /// The project's key; left out for the list of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The layout.
    pub layout: ListLayout,
    /// The history group; left out when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// Saved views.
    ViewListOp,
    "view.list",
    ViewListInput,
    ViewList
);
op!(
    /// Saves a new view.
    ViewCreate,
    "view.create",
    ViewCreateInput,
    ViewChange
);
op!(
    /// Changes a saved view.
    ViewUpdate,
    "view.update",
    ViewUpdateInput,
    ViewChange
);
op!(
    /// Deletes a saved view.
    ViewDelete,
    "view.delete",
    ViewDeleteInput,
    ViewChange
);

op!(
    /// The person's layout of a list.
    ListLayoutGet,
    "list.layout.get",
    ListLayoutGetInput,
    ListLayoutChange
);
op!(
    /// Changes the person's layout of a list.
    ListLayoutUpdate,
    "list.layout.update",
    ListLayoutUpdateInput,
    ListLayoutChange
);

pub(crate) const LIST_LAYOUT_GET: OpDescriptor = OpDescriptor::of::<ListLayoutGet>(Meta {
    summary: "The person's layout of a project's list, or of the whole workspace's: column order and widths, and the folded groups.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It holds how the person's own list screen looks (the order and widths of its columns), not the workspace's data; agents and scripts choose columns in saved views.",
    },
    cli: None,
    input_name: "ListLayoutGetInput",
    output_name: "ListLayoutChange",
});

pub(crate) const LIST_LAYOUT_UPDATE: OpDescriptor = OpDescriptor::of::<ListLayoutUpdate>(Meta {
    summary: "Reorder the columns of a list, change their widths, fold or unfold its groups, or go back to the default layout.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "It changes how the person's own list screen looks (the order and widths of its columns), not the workspace's data; agents and scripts choose columns in saved views.",
    },
    cli: None,
    input_name: "ListLayoutUpdateInput",
    output_name: "ListLayoutChange",
});

pub(crate) const VIEW_LIST: OpDescriptor = OpDescriptor::of::<ViewListOp>(Meta {
    summary: "The saved views of a project or of the whole workspace: name, query, columns, order, grouping, totals and which is the default.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("view list"),
    input_name: "ViewListInput",
    output_name: "ViewList",
});

pub(crate) const VIEW_CREATE: OpDescriptor = OpDescriptor::of::<ViewCreate>(Meta {
    summary: "Save a query as a named view of a project or of the whole workspace, with its columns, order, grouping and totals.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("view create"),
    input_name: "ViewCreateInput",
    output_name: "ViewChange",
});

pub(crate) const VIEW_UPDATE: OpDescriptor = OpDescriptor::of::<ViewUpdate>(Meta {
    summary: "Rename a saved view, change its query, columns, order, grouping or totals, or make it the default.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("view edit <view>"),
    input_name: "ViewUpdateInput",
    output_name: "ViewChange",
});

pub(crate) const VIEW_DELETE: OpDescriptor = OpDescriptor::of::<ViewDelete>(Meta {
    summary: "Delete a saved view; it can be brought back with revert.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("view delete <view>"),
    input_name: "ViewDeleteInput",
    output_name: "ViewChange",
});
