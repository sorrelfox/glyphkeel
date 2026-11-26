// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Tasks: create, read, change and move.
//!
//! A task is named by its key (`TM-41`, any case; old keys after a move or a
//! project's new key still work) or its ID. Statuses, types and labels are
//! named by their name (ignoring case) or ID. Dates are `YYYY-MM-DD` or
//! natural forms (`today`, `fri`, `next mon`, `+3d`, `in 2 weeks`);
//! durations are written `1w 2d 4h 30m`, `90m`, `1.5h`, `1,5h` or `1:30`,
//! with the workspace's working day and week.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Actor, Category, Edge, Glyph, Hue, Priority};
use crate::ops::description::{
    ChecklistItem, ConflictResolution, DescriptionSave, RevisionInfo, TextEdit,
};
use crate::ops::schedule::MovedTask;
use crate::ops::trash::TrashChange;

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
    /// Its custom field values, in the fields' order; fields without a
    /// value are left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<crate::ops::field::CustomFieldValue>,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The version (release) it is planned for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<crate::ops::version::VersionRef>,
    /// Its description (Markdown).
    pub description: String,
    /// The description's revision; pass it as `base_revision` when you save
    /// a new description. Left out while it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_revision: Option<String>,
    /// Edits of the description that clashed with another and wait to be
    /// resolved (conflict copies).
    pub conflict_copies: u32,
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
    /// Its progress in percent: the progress set by hand, else 100 when
    /// closed, else the share of ticked checklist items, else its subtasks'
    /// progress weighted by their estimates, else 0.
    pub progress_percent: u8,
    /// Totals over the task and all its subtasks; only for a task with
    /// subtasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totals: Option<Totals>,
    /// Its subtasks: closed and all.
    pub subtask_count: Checklist,
    /// Whether a task that blocks it is still open.
    pub blocked: bool,
    /// How many comments it has.
    pub comment_count: u32,
    /// How many files are attached to it and to its comments.
    pub file_count: u32,
    /// The agent that claimed it, while the claim lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// Its series, for a recurring task: the rule, the next date and the
    /// details new occurrences get.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<crate::ops::recurrence::Recurrence>,
    /// Its subtasks in order, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtasks: Option<Vec<TaskRef>>,
    /// Its links to other tasks, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
    /// Its comments, oldest first, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<crate::ops::comment::Comment>>,
    /// The files attached to it, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<crate::ops::file::Attachment>>,
    /// The description's checklist items, numbered, when asked for with
    /// `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checklist_items: Option<Vec<ChecklistItem>>,
    /// The description's revisions, newest first, conflict copies
    /// included, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revisions: Option<Vec<RevisionInfo>>,
    /// Its time entries, newest day first, when asked for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_entries: Option<Vec<crate::ops::time::TimeEntry>>,
    /// Its personal reminder, when asked for with `include` and it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder: Option<crate::ops::time::Reminder>,
    /// The wiki pages, other tasks and comments that mention it, when asked
    /// for with `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backlinks: Option<Vec<crate::ops::link::Backlink>>,
    /// What its description and comments link to, when asked for with
    /// `include`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<crate::ops::link::Reference>>,
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

/// Estimate and time over a task and all its subtasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    /// The estimates added up, in minutes.
    pub estimate_minutes: i64,
    /// The logged time added up, in minutes.
    pub spent_minutes: i64,
    /// Estimate minus spent time, in minutes; below 0 when over.
    pub remaining_minutes: i64,
}

/// An agent's claim on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// The agent's name.
    pub agent: String,
    /// The agent session holding it.
    pub session: String,
    /// When the claim ends unless renewed (RFC 3339, UTC).
    pub until: String,
}

/// Another task, as links and subtask lists show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskRef {
    /// Its key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its status.
    pub status: StatusRef,
    /// Whether it is closed (done or cancelled).
    pub closed: bool,
    /// Its ID.
    pub id: String,
}

/// How a link reads from one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// This task must be finished before the other.
    Blocks,
    /// The other task must be finished before this one.
    BlockedBy,
    /// The two are related.
    Relates,
    /// This task duplicates the other.
    Duplicates,
    /// The other task duplicates this one.
    DuplicatedBy,
    /// This task is a copy of the other.
    Clones,
    /// The other task is a copy of this one.
    ClonedBy,
}

/// A link from one task to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// How the link reads from this task.
    pub kind: LinkKind,
    /// The task at the other end.
    pub task: TaskRef,
    /// The link's ID.
    pub id: String,
    /// For `blocks` and `blocked_by`: the days between the end of the
    /// blocking task and the start of the blocked one; left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag_days: Option<i64>,
}

/// What `task.get` adds to each task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskPart {
    /// The subtasks in order.
    Subtasks,
    /// The links to other tasks.
    Links,
    /// The comments.
    Comments,
    /// The attached files.
    Files,
    /// The description's checklist items, numbered.
    Checklist,
    /// The description's revisions and conflict copies.
    Revisions,
    /// The time entries, newest day first.
    Time,
    /// The personal reminder.
    Reminder,
    /// The wiki pages, other tasks and comments that mention it.
    Backlinks,
    /// The pages and tasks its description and comments link to.
    References,
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
    /// What became of a new description: saved, merged with another edit,
    /// or kept as a conflict copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_save: Option<DescriptionSave>,
    /// The tasks "also move dependents" moved, blockers first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<MovedTask>,
    /// The duplicates closed with the task (`close_duplicates`), by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicates_closed: Vec<String>,
}

/// Which tasks to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskGetInput {
    /// One to 20 task keys or IDs.
    pub keys: Vec<String>,
    /// What to add to each task: `subtasks`, `links`, `comments`, `files`,
    /// `checklist`, `revisions`, `time`, `reminder`, `backlinks`,
    /// `references`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<TaskPart>,
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
    /// The version it is planned for (name or ID): an open version of the
    /// project or one shared with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<String>,
    /// The parent task, for a subtask (same project, at most 5 levels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The start date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`,
    /// `in 2 weeks`, `dec 1`, in the workspace's time zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date: `YYYY-MM-DD`, or `today`, `fri`, `next mon`, `+3d`,
    /// `in 2 weeks`, `dec 1`, in the workspace's time zone.
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
    /// Custom field values, by field name, query name or ID (see
    /// `field.list`): a string, a number, `true`/`false`, or a list of
    /// options for a multi-select field. Fields with a default get it
    /// unless given here; a required field needs a value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
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
    /// The version it is planned for.
    FixVersion,
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
    /// The description revision your new description was edited from. If
    /// somebody saved in between, the two edits are merged; when they
    /// changed the same lines, yours is kept as a conflict copy and the
    /// description stays as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    /// Targeted changes to the description instead of a new one: each
    /// `old_text` must occur exactly once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub description_edits: Vec<TextEdit>,
    /// Resolve a conflict copy of the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_conflict: Option<ConflictResolution>,
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
    /// Plan it for this version (name or ID): an open version of the
    /// project or one shared with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<String>,
    /// A new parent task (same project, at most 5 levels, no loops).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// A new start date: `YYYY-MM-DD`, or a natural date such as `fri` or
    /// `in 3d`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// A new due date: `YYYY-MM-DD`, or a natural date such as `next fri`
    /// or `+3d`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// A new estimate, as a duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Progress set by hand, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Custom field values to set, by field name, query name or ID: a
    /// string, a number, `true`/`false`, a list of options for a
    /// multi-select field, or `null` to clear the value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<TaskField>,
    /// Change the task although another agent claimed it (full profile
    /// only).
    #[serde(default)]
    pub override_claim: bool,
    /// When the dates change, also move the tasks that depend on this one
    /// (through `blocks` links, and theirs in turn) later, as far as each
    /// must to begin after its blockers end. Their lengths stay, nothing
    /// moves earlier, and closed tasks stay where they are. One change in
    /// history with this one.
    #[serde(default)]
    pub move_dependents: bool,
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
    /// With `project`: which status, type or label of that project stands
    /// in for one it has no match for.
    #[serde(default, skip_serializing_if = "MoveMapping::is_empty")]
    pub mapping: MoveMapping,
    /// Move the task although another agent claimed it (full profile only).
    #[serde(default)]
    pub override_claim: bool,
    /// When the move closes the task: `true` closes the open tasks marked
    /// as its duplicates too, in the same change (each into the status of
    /// the same name in its project, else the first one of the same
    /// category); `false` leaves them open without a warning. Left out, they
    /// stay open and a warning names them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_duplicates: Option<bool>,
}

/// What tasks moving to another project take there in place of the
/// statuses, types and labels of their own project. Each map goes from a
/// name in the task's project to a name in the target project (any case).
/// A name left out keeps the target's item of the same name; without one, a
/// status becomes the target's first status, a type its default type, and
/// a label is left off. Every change says which.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveMapping {
    /// Status names: from the task's project to the target project.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub statuses: BTreeMap<String, String>,
    /// Type names, the same way.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, String>,
    /// Label names, the same way.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl MoveMapping {
    /// Whether it maps nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty() && self.types.is_empty() && self.labels.is_empty()
    }
}

/// Tasks that might move to another project, to see first what the move
/// would do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskMoveCheckInput {
    /// The tasks: keys or IDs, 1 to 500. Their subtasks would move with them.
    pub keys: Vec<String>,
    /// The project they would move to.
    pub project: String,
}

/// What moving tasks to another project would do: their new keys, and how
/// their statuses, types and labels would be matched there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveCheck {
    /// The target project's key.
    pub project: String,
    /// Every task that would move, subtasks included, with its key now and
    /// the key it would get.
    pub keys: Vec<MovedKey>,
    /// The statuses the tasks have now.
    pub statuses: Vec<MoveMatch>,
    /// Their types.
    pub types: Vec<MoveMatch>,
    /// Their labels.
    pub labels: Vec<MoveMatch>,
    /// The custom fields they have values of. The target is the target's
    /// field that would hold the values; absent when they would be left off.
    #[serde(default)]
    pub fields: Vec<MoveMatch>,
    /// The versions they are planned for. The target is the same version
    /// when the target project can plan tasks for it; absent when the move
    /// would take it off.
    #[serde(default)]
    pub versions: Vec<MoveMatch>,
    /// The target's status for those it has no match for: its first one.
    pub fallback_status: String,
    /// The target's type for those it has no match for: its default one.
    pub fallback_type: String,
}

/// A key now and the one a move would give.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovedKey {
    /// The key now.
    pub from: String,
    /// The key in the target project.
    pub to: String,
}

/// A status, type, label or custom field of the moving tasks, and its match
/// in the target project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveMatch {
    /// Its name.
    pub name: String,
    /// How many of the moving tasks have it.
    pub tasks: u64,
    /// The target's item of the same name; absent when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Where to put a subtask among its parent's subtasks. Subtasks keep an
/// order of their own, apart from their places on the board.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubtaskReorderInput {
    /// The subtask: its key or ID.
    pub key: String,
    /// Refuse the change unless the subtask still has this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
    /// Place it before this subtask of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this subtask of the same parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last among the parent's subtasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Reorder it although another agent claimed it (full profile only).
    #[serde(default)]
    pub override_claim: bool,
}

/// A copy of a task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskDuplicateInput {
    /// The task to copy: its key or ID.
    pub key: String,
    /// The copy's title; default: the same title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The copy's status; default: the project's first to-do status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Leave the subtasks out; by default they are copied too, in order.
    #[serde(default)]
    pub without_subtasks: bool,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of making a second copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// A link to add or remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelationInput {
    /// The task: its key or ID.
    pub key: String,
    /// How the link reads from this task. `subtask_of` and `parent_of`
    /// change the parent instead of adding a link.
    pub kind: RelationKind,
    /// The other task: its key or ID.
    pub other: String,
    /// For `blocks` and `blocked_by` with `relation.add`: the days that must
    /// pass after the blocking task ends before the blocked one may begin
    /// (finish-to-start plus a lag), 0 to 365. On an existing link it
    /// changes the lag; 0 removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag_days: Option<i64>,
}

/// The kinds of link, as read from the first task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// This task must be finished before the other.
    Blocks,
    /// The other task must be finished before this one.
    BlockedBy,
    /// The two are related.
    Relates,
    /// This task duplicates the other.
    Duplicates,
    /// The other task duplicates this one.
    DuplicatedBy,
    /// This task is a copy of the other.
    Clones,
    /// The other task is a copy of this one.
    ClonedBy,
    /// This task is a subtask of the other.
    SubtaskOf,
    /// The other task is a subtask of this one.
    ParentOf,
}

/// A claim on a task, for an agent working on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskClaimInput {
    /// The task: its key or ID.
    pub key: String,
    /// How long the claim lasts unless renewed, 1 to 480 minutes; default
    /// 60. Every change you make to the task renews it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_minutes: Option<u32>,
    /// The status to move it to; default: the first in-progress status,
    /// unless it is in progress already.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Leave the status as it is.
    #[serde(default)]
    pub keep_status: bool,
    /// Take the claim over from another agent (full profile only).
    #[serde(default)]
    pub override_claim: bool,
}

/// Giving up a claim.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskReleaseInput {
    /// The task: its key or ID.
    pub key: String,
    /// A status to move it to, for example Review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// A comment to add (Markdown), for example what was done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Release another agent's claim (full profile only; the person can
    /// always).
    #[serde(default)]
    pub override_claim: bool,
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
op!(
    /// What a move to another project would do.
    TaskMoveCheck,
    "task.move_check",
    TaskMoveCheckInput,
    MoveCheck
);
op!(
    /// Moves a subtask to another place among its parent's subtasks.
    TaskReorderSubtask,
    "task.reorder_subtask",
    SubtaskReorderInput,
    TaskChange
);

op!(
    /// A copy of a task, with its subtasks, labels, checklist and files.
    TaskDuplicate,
    "task.duplicate",
    TaskDuplicateInput,
    TaskChange
);
op!(
    /// Claims a task for an agent session.
    TaskClaim,
    "task.claim",
    TaskClaimInput,
    TaskChange
);
op!(
    /// Gives up a claim.
    TaskRelease,
    "task.release",
    TaskReleaseInput,
    TaskChange
);
op!(
    /// Links two tasks.
    RelationAdd,
    "relation.add",
    RelationInput,
    TaskChange
);
op!(
    /// Removes a link between two tasks.
    RelationRemove,
    "relation.remove",
    RelationInput,
    TaskChange
);

const fn command(summary: &'static str, cli: &'static str, input: &'static str) -> Meta {
    Meta {
        summary,
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some(cli),
        input_name: input,
        output_name: "TaskChange",
    }
}

pub(crate) const TASK_REORDER_SUBTASK: OpDescriptor = OpDescriptor::of::<TaskReorderSubtask>(
    command(
        "Move a subtask to another place among its parent's subtasks (before or after another one, first or last); returns the parent with its subtasks in the new order.",
        "task reorder-subtask <key>",
        "SubtaskReorderInput",
    ),
);
pub(crate) const TASK_DUPLICATE: OpDescriptor = OpDescriptor::of::<TaskDuplicate>(command(
    "Copy a task with its subtasks (in order), labels, checklist and files; the copy is linked as a clone.",
    "task duplicate <key>",
    "TaskDuplicateInput",
));
pub(crate) const TASK_CLAIM: OpDescriptor = OpDescriptor::of::<TaskClaim>(command(
    "Claim a task for your agent session for a while, so other agents leave it alone; moves it into progress.",
    "task claim <key>",
    "TaskClaimInput",
));
pub(crate) const TASK_RELEASE: OpDescriptor = OpDescriptor::of::<TaskRelease>(command(
    "Give up a claim on a task, optionally moving it and adding a comment.",
    "task release <key>",
    "TaskReleaseInput",
));
pub(crate) const RELATION_ADD: OpDescriptor = OpDescriptor::of::<RelationAdd>(command(
    "Link two tasks (blocks, with an optional lag in days; relates, duplicates, clones; or make one a subtask of the other).",
    "task link <key>",
    "RelationInput",
));
pub(crate) const RELATION_REMOVE: OpDescriptor = OpDescriptor::of::<RelationRemove>(command(
    "Remove a link between two tasks, or a subtask from its parent.",
    "task unlink <key>",
    "RelationInput",
));

pub(crate) const TASK_GET: OpDescriptor = OpDescriptor::of::<TaskGet>(Meta {
    summary: "One to 20 tasks by key, with their fields, description, labels and version; subtasks, links, comments and files on request.",
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
    summary: "Change a task's title, description (whole, with a base revision, or by targeted edits), type, priority, labels, parent, dates, estimate or progress.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task edit <key>"),
    input_name: "TaskUpdateInput",
    output_name: "TaskChange",
});

/// Tasks to move to the trash.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskDeleteInput {
    /// The tasks: keys or IDs, 1 to 500. Their subtasks go with them.
    pub keys: Vec<String>,
}

op!(
    /// Tasks to the trash.
    TaskDelete,
    "task.delete",
    TaskDeleteInput,
    TrashChange
);

pub(crate) const TASK_DELETE: OpDescriptor = OpDescriptor::of::<TaskDelete>(Meta {
    summary: "Move tasks to the trash, each with its subtasks; they can be restored from there.",
    kind: OpKind::Command,
    safety: Safety::Trash,
    exposure: Exposure::Data,
    cli: Some("task delete <keys>"),
    input_name: "TaskDeleteInput",
    output_name: "TrashChange",
});

pub(crate) const TASK_MOVE_CHECK: OpDescriptor = OpDescriptor::of::<TaskMoveCheck>(Meta {
    summary: "What moving tasks to another project would do: their new keys, and which of their statuses, types and labels the target project has no match for.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("task move-check <keys>"),
    input_name: "TaskMoveCheckInput",
    output_name: "MoveCheck",
});

pub(crate) const TASK_MOVE: OpDescriptor = OpDescriptor::of::<TaskMove>(Meta {
    summary: "Move a task to another status or place in its column, or to another project (new key; the old one forwards); closing it can close its duplicates too.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("task move <key>"),
    input_name: "TaskMoveInput",
    output_name: "TaskChange",
});
