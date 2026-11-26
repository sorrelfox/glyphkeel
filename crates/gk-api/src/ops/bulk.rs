// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Bulk changes: the same change to many tasks, chosen by key or by query,
//! as one change in history (one Undo, one revert). A preview comes first:
//! `dry_run` is on unless turned off, and the real run must say how many
//! tasks it expects, so it stops when the query now matches others.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Priority;
use crate::ops::task::{MoveMapping, TaskField};

/// The change to make to every task. Fields left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulkChanges {
    /// Move to the status of this name (in each task's project).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The type of this name.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// The priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// Replace the labels with these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// Labels to add.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_labels: Vec<String>,
    /// Labels to take off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_labels: Vec<String>,
    /// Plan them for the version of this name (in each task's project,
    /// or shared with it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<String>,
    /// Make them subtasks of this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The start date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date, written as for `start_date`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Move start dates by calendar days, weeks, months or years (`+2d`,
    /// `-1w`); tasks without one keep none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_start: Option<String>,
    /// Move due dates the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_due: Option<String>,
    /// The estimate, as a duration (`1d 4h`, `90m`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Move them to this project (new keys; old keys forward).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: which status, type or label of that project stands
    /// in for one it has no match for.
    #[serde(default, skip_serializing_if = "MoveMapping::is_empty")]
    pub mapping: MoveMapping,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<TaskField>,
    /// Custom field values to set, by field name, query name or ID, as for
    /// `task.update`; `null` clears one. Each task's project must have the
    /// field.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
}

/// A bulk change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulkUpdateInput {
    /// The tasks, by key or ID. Give these or `query`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// A query choosing the tasks (`project:TM label:old is:open`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The change.
    pub changes: BulkChanges,
    /// Only show what would change; default true. Turn it off to make the
    /// change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
    /// How many tasks the change is for, from the preview; needed to make
    /// the change, which stops when the tasks now number otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_count: Option<u64>,
}

/// A field of one task before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulkFieldChange {
    /// The field.
    pub field: String,
    /// Its value before; left out when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Its value after; left out when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// One task of a bulk change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulkItem {
    /// Its key (after the change).
    pub key: String,
    /// Its title.
    pub title: String,
    /// What changes, or changed; empty when it already was as asked.
    pub changes: Vec<BulkFieldChange>,
}

/// What a bulk change does or did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulkResult {
    /// Only a preview: nothing changed.
    pub dry_run: bool,
    /// How many tasks the change is for; pass it as `expected_count`.
    pub count: u64,
    /// How many of them change.
    pub changed: u64,
    /// Every task, with what changes.
    pub items: Vec<BulkItem>,
    /// The history group, for revert and Undo; left out for a preview or
    /// when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Problems that did not stop the change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

op!(
    /// The same change to many tasks.
    TaskBulkUpdate,
    "task.bulk_update",
    BulkUpdateInput,
    BulkResult
);

pub(crate) const TASK_BULK_UPDATE: OpDescriptor = OpDescriptor::of::<TaskBulkUpdate>(Meta {
    summary: "Make the same change to many tasks chosen by key or query (status, type, priority, labels, parent, dates set or shifted, estimate, custom fields, project), previewed first, as one change for Undo and revert.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task bulk"),
    input_name: "BulkUpdateInput",
    output_name: "BulkResult",
});
