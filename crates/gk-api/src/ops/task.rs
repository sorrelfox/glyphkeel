// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Tasks: create, read, change and move.
//!
//! A task is named by its key (`TM-41`, any case; old keys after a move or a
//! project's new key still work) or its ID. Statuses, types and labels are
//! named by their name (ignoring case) or ID. Dates are `YYYY-MM-DD`;
//! durations are written `1w 2d 4h 30m`, `90m`, `1.5h`, `1,5h` or `1:30`,
//! with the workspace's working day and week.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Actor, Category, Edge, Glyph, Hue, Priority};

/// A task's status, as tasks carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusRef {
    /// The status's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its category.
    pub category: Category,
}

/// A task's type, as tasks carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeRef {
    /// The type's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its shape.
    pub glyph: Glyph,
}

/// A label on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabelRef {
    /// The label's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colour.
    pub hue: Hue,
}

/// Ticked and all checklist items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checklist {
    /// Ticked items.
    pub done: u32,
    /// All items.
    pub total: u32,
}

/// One task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// The task's key (`TM-41`).
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key.
    pub project: String,
    /// Its status.
    pub status: StatusRef,
    /// Its type.
    #[serde(rename = "type")]
    pub task_type: TypeRef,
    /// Its priority.
    pub priority: Priority,
    /// Its labels, in the project's order.
    pub labels: Vec<LabelRef>,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Its description (Markdown).
    pub description: String,
    /// The start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The estimate, as a duration (`1d 4h`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// The estimate in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// Progress set by hand, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Logged time in minutes.
    pub spent_minutes: i64,
    /// The description's checklist.
    pub checklist: Checklist,
    /// Its place in its status column.
    pub rank: String,
    /// Keys it had before; they still find it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub old_keys: Vec<String>,
    /// Who created it.
    pub created_by: Actor,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When its own fields last changed.
    pub edited_at: String,
    /// When anything about it last changed.
    pub updated_at: String,
    /// When it entered its status.
    pub status_entered_at: String,
    /// When it was closed; left out while it is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    /// Goes up by one with every change; pass it as `expected_version` to
    /// make sure nobody changed the task since you read it.
    pub version: i64,
    /// The task's ID.
    pub id: String,
}

/// A task changed, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskChange {
    /// The task after the change.
    pub task: Task,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Problems that did not stop the change, such as closing a task whose
    /// subtasks are still open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Which tasks to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskGetInput {
    /// One to 20 task keys or IDs.
    pub keys: Vec<String>,
}

/// The tasks asked for, in the order asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskList {
    /// The tasks.
    pub items: Vec<Task>,
    /// How many.
    pub total: u64,
}

/// A new task. Only the project and the title are needed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskCreateInput {
    /// The project: its key (current or old) or ID.
    pub project: String,
    /// The title, 1 to 500 characters; tabs and line breaks become spaces.
    pub title: String,
    /// The description (Markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The status; default: the project's first to-do status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The type; default: the project's default type.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// The priority; default: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// Labels, by name or ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// The parent task, for a subtask (same project, at most 5 levels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The start date, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The estimate, as a duration (`1d 4h`, `90m`, `1.5h`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Place it before this task of the same status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this task of the same status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last in its status; default: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of creating a second task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// A task field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskField {
    /// The description.
    Description,
    /// The parent: the task stops being a subtask.
    Parent,
    /// The start date.
    StartDate,
    /// The due date.
    DueDate,
    /// The estimate.
    Estimate,
    /// The progress set by hand.
    Progress,
    /// Every label.
    Labels,
}

/// Changes to a task; fields left out keep their value. Status and
/// position change with `task.move`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskUpdateInput {
    /// The task: its key or ID.
    pub key: String,
    /// Refuse the change unless the task still has this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
    /// A new title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A new description (Markdown), replacing the old one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A new type.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// A new priority.
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
    /// A new parent task (same project, at most 5 levels, no loops).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// A new start date, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// A new due date, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// A new estimate, as a duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Progress set by hand, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<TaskField>,
}

/// Where to move a task: another status, another place in its column, or
/// another project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskMoveInput {
    /// The task: its key or ID.
    pub key: String,
    /// Refuse the move unless the task still has this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
    /// The new status, by name or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Place it before this task of the target status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this task of the target status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last in the target status; default when the status
    /// changes: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Move it to another project: it gets the next key there, and the old
    /// key keeps finding it. Its subtasks move with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

op!(
    /// Tasks by key.
    TaskGet,
    "task.get",
    TaskGetInput,
    TaskList
);
op!(
    /// A new task.
    TaskCreate,
    "task.create",
    TaskCreateInput,
    TaskChange
);
op!(
    /// Changes to a task's fields.
    TaskUpdate,
    "task.update",
    TaskUpdateInput,
    TaskChange
);
op!(
    /// Moves a task to another status, place or project.
    TaskMove,
    "task.move",
    TaskMoveInput,
    TaskChange
);

pub(crate) const TASK_GET: OpDescriptor = OpDescriptor::of::<TaskGet>(Meta {
    summary: "One to 20 tasks by key, with their fields, description, labels and version.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("task show <keys>"),
    input_name: "TaskGetInput",
    output_name: "TaskList",
});

pub(crate) const TASK_CREATE: OpDescriptor = OpDescriptor::of::<TaskCreate>(Meta {
    summary: "Create a task from a title alone or with its fields; it gets the project's next key.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task create"),
    input_name: "TaskCreateInput",
    output_name: "TaskChange",
});

pub(crate) const TASK_UPDATE: OpDescriptor = OpDescriptor::of::<TaskUpdate>(Meta {
    summary: "Change a task's title, description, type, priority, labels, parent, dates, estimate or progress.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task edit <key>"),
    input_name: "TaskUpdateInput",
    output_name: "TaskChange",
});

pub(crate) const TASK_MOVE: OpDescriptor = OpDescriptor::of::<TaskMove>(Meta {
    summary: "Move a task to another status or place in its column, or to another project (new key; the old one forwards).",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task move <key>"),
    input_name: "TaskMoveInput",
    output_name: "TaskChange",
});
