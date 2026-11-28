// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Schedule baselines: a named snapshot of a project's plan, to compare the
//! live plan with. A baseline keeps each dated task's start date, due date
//! and whether it is a milestone, as they were when it was saved, with the
//! task's key and title then. A task deleted later keeps its entry.
//!
//! A project may have several baselines. Saving, renaming and deleting one
//! are recorded in history and can be undone.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;

/// A saved baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    /// The baseline's ID.
    pub id: String,
    /// Its project's key.
    pub project: String,
    /// Its name, unique in the project (ignoring case).
    pub name: String,
    /// How many tasks it holds.
    pub tasks: u32,
    /// Who saved it.
    pub created_by: Actor,
    /// When it was saved (RFC 3339).
    pub created_at: String,
    /// When it was last renamed, or saved (RFC 3339).
    pub updated_at: String,
}

/// Which baselines to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineListInput {
    /// The project (key or ID).
    pub project: String,
}

/// A project's baselines, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineList {
    /// The baselines.
    pub items: Vec<Baseline>,
    /// How many there are.
    pub total: u64,
}

/// A new baseline of a project's current plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineCreateInput {
    /// The project (key or ID). Its tasks with dates are saved, at every
    /// level of subtasks; its sub-projects' are not.
    pub project: String,
    /// The baseline's name, unique in the project (1 to 100 characters).
    pub name: String,
}

/// One baseline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineRef {
    /// The project (key or ID).
    pub project: String,
    /// The baseline: its ID or its name.
    pub baseline: String,
}

/// A new name for a baseline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineRenameInput {
    /// The project (key or ID).
    pub project: String,
    /// The baseline: its ID or its name.
    pub baseline: String,
    /// Its new name, unique in the project.
    pub name: String,
}

/// A baseline after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineChange {
    /// The baseline after the change (before it, for a deletion).
    pub baseline: Baseline,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// A project's baselines.
    BaselineListOp,
    "baseline.list",
    BaselineListInput,
    BaselineList
);
op!(
    /// Saves a project's current plan as a baseline.
    BaselineCreate,
    "baseline.create",
    BaselineCreateInput,
    BaselineChange
);
op!(
    /// Renames a baseline.
    BaselineRename,
    "baseline.rename",
    BaselineRenameInput,
    BaselineChange
);
op!(
    /// Deletes a baseline.
    BaselineDelete,
    "baseline.delete",
    BaselineRef,
    BaselineChange
);

pub(crate) const BASELINE_LIST: OpDescriptor = OpDescriptor::of::<BaselineListOp>(Meta {
    summary: "A project's schedule baselines, newest first: each one's name, how many tasks it holds, and who saved it when.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("baseline list <project>"),
    input_name: "BaselineListInput",
    output_name: "BaselineList",
});

pub(crate) const BASELINE_CREATE: OpDescriptor = OpDescriptor::of::<BaselineCreate>(Meta {
    summary: "Save a project's current plan as a named baseline: every dated task's start and due date. Compare with it through the schedule's baseline and the query field slip.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("baseline create <project>"),
    input_name: "BaselineCreateInput",
    output_name: "BaselineChange",
});

pub(crate) const BASELINE_RENAME: OpDescriptor = OpDescriptor::of::<BaselineRename>(Meta {
    summary: "Rename a baseline of a project.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("baseline rename <project>"),
    input_name: "BaselineRenameInput",
    output_name: "BaselineChange",
});

pub(crate) const BASELINE_DELETE: OpDescriptor = OpDescriptor::of::<BaselineDelete>(Meta {
    summary: "Delete a baseline of a project; the tasks stay as they are. Revert brings it back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("baseline delete <project>"),
    input_name: "BaselineRef",
    output_name: "BaselineChange",
});
