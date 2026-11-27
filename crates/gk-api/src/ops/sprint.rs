// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Sprints: short, time-boxed cycles of work in a project. A sprint is
//! planned, then started (at most one active sprint per project), then
//! completed; completing it moves its unfinished tasks to another sprint or
//! back to the backlog. A task is in at most one sprint, of its own
//! project. The burndown shows how the sprint's open work went down day by
//! day.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Priority;
use crate::ops::task::StatusRef;

/// Where a sprint stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SprintState {
    /// Being planned; not started yet (the default).
    #[default]
    Planned,
    /// Running: started and not completed. One per project at most.
    Active,
    /// Completed.
    Closed,
}

impl SprintState {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

/// A sprint as a task shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintRef {
    /// Its name.
    pub name: String,
    /// Where it stands.
    pub state: SprintState,
    /// Its first day (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its last day (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Its ID.
    pub id: String,
}

/// How far a sprint has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintProgress {
    /// The tasks in it (not in the trash).
    pub total: u32,
    /// How many of them are closed.
    pub closed: u32,
    /// Their estimates added up, in minutes.
    pub estimate_minutes: i64,
    /// The estimates of the closed ones, in minutes.
    pub closed_estimate_minutes: i64,
}

/// A task of a sprint or of the backlog, as the sprints page lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintTask {
    /// Its key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its status.
    pub status: StatusRef,
    /// Whether it is closed (done or cancelled).
    pub closed: bool,
    /// Its type's name.
    #[serde(rename = "type")]
    pub task_type: String,
    /// Its priority.
    pub priority: Priority,
    /// Its estimate in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Its ID.
    pub id: String,
}

/// A sprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sprint {
    /// Its name, unique in its project (ignoring case).
    pub name: String,
    /// The key of the project it belongs to.
    pub project: String,
    /// What the sprint is meant to achieve: plain text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub goal: String,
    /// Where it stands.
    pub state: SprintState,
    /// Its first day (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its last day (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// When it was started (RFC 3339, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When it was completed (RFC 3339, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Its progress.
    pub progress: SprintProgress,
    /// Its tasks, when asked for: by board order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<SprintTask>>,
    /// Its place among the project's sprints.
    pub rank: String,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// Its ID.
    pub id: String,
}

/// Which sprints to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintListInput {
    /// The project (key or ID).
    pub project: String,
    /// Only sprints in these states; default: all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<SprintState>,
    /// Include each sprint's tasks and the backlog, for the sprints page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_tasks: bool,
}

/// Sprints: closed ones first, then the active one, then planned ones, each
/// by their order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintList {
    /// The sprints.
    pub items: Vec<Sprint>,
    /// How many.
    pub total: u64,
    /// With `include_tasks`: the project's open tasks in no sprint, by
    /// board order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backlog: Option<Vec<SprintTask>>,
}

/// One sprint to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintGetInput {
    /// The sprint: its ID, its name, or `active` for the project's active
    /// sprint.
    pub sprint: String,
    /// With a name or `active`: the project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A new sprint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintCreateInput {
    /// The project (key or ID).
    pub project: String,
    /// Its name, up to 100 characters, unique in the project; not `active`,
    /// `none` or `backlog`.
    pub name: String,
    /// What it is meant to achieve: plain text, up to 4,000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Its first day: `YYYY-MM-DD`, or `today`, `mon`, `+1w`, `dec 1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its last day: `YYYY-MM-DD`, or `today`, `fri`, `+2w`, `dec 12`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Tasks to put in it straight away (keys or IDs, of the project).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
}

/// A sprint field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SprintField {
    /// The goal.
    Goal,
    /// The start date.
    StartDate,
    /// The end date.
    EndDate,
}

/// Changes to a sprint; fields left out keep their value. Start and
/// complete it with `sprint.start` and `sprint.complete`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintUpdateInput {
    /// The sprint: its ID, its name, or `active`.
    pub sprint: String,
    /// With a name or `active`: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name; saved views that name the old one are rewritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new goal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// A new start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// A new end date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<SprintField>,
}

/// Which sprint to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintDeleteInput {
    /// The sprint: its ID or its name. An active sprint is completed
    /// first, not deleted.
    pub sprint: String,
    /// With a name: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Which sprint to start, and its dates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintStartInput {
    /// The sprint (planned): its ID or its name.
    pub sprint: String,
    /// With a name: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Its first day; default: its start date, or today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its last day; default: its end date, or two weeks after the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// A new goal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
}

/// Which sprint to complete, and where its unfinished tasks go.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintCompleteInput {
    /// The sprint (active): its ID, its name, or `active`.
    pub sprint: String,
    /// With a name or `active`: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Where its open tasks go: a planned sprint of the project (name or
    /// ID), or `backlog` (the default) for no sprint. Closed tasks stay in
    /// the completed sprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_open_tasks_to: Option<String>,
}

/// Tasks to put in a sprint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintAddTasksInput {
    /// The sprint (planned or active): its ID, its name, or `active`.
    pub sprint: String,
    /// With a name or `active`: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The tasks (keys or IDs), of the sprint's project; up to 500. A task
    /// in another sprint moves to this one.
    pub tasks: Vec<String>,
}

/// Tasks to take out of their sprint, back to the backlog.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintRemoveTasksInput {
    /// The tasks (keys or IDs); up to 500. Tasks in no sprint are left
    /// as they are.
    pub tasks: Vec<String>,
}

/// A sprint after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintChange {
    /// The sprint after the change (before it, for a deletion); left out
    /// when tasks were only taken out of their sprints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<Sprint>,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The keys of the tasks whose sprint changed: added, taken out, moved
    /// on when it was completed, or cleared by a deletion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks_changed: Vec<String>,
    /// The saved views whose query was rewritten because they named the
    /// sprint (renamed or deleted), by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views_rewritten: Vec<String>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Which sprint's burndown to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintBurndownInput {
    /// The sprint (active or closed): its ID, its name, or `active`.
    pub sprint: String,
    /// With a name or `active`: the sprint's project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// One day of a burndown, as it stood at the end of the day (or at the
/// moment the sprint was completed, on that day).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BurndownDay {
    /// The day (`YYYY-MM-DD`).
    pub date: String,
    /// The tasks in the sprint that were still open.
    pub open_tasks: u32,
    /// All the tasks in the sprint, open or closed (its scope).
    pub total_tasks: u32,
    /// The open tasks' estimates added up, in minutes.
    pub open_estimate_minutes: i64,
    /// All the tasks' estimates added up, in minutes.
    pub total_estimate_minutes: i64,
}

/// How a sprint's open work went down, day by day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SprintBurndown {
    /// The sprint.
    pub sprint: SprintRef,
    /// Its days from the first to the last (or to today, for an active
    /// sprint that runs over); days still to come are left out.
    pub days: Vec<BurndownDay>,
    /// The work at the start: the first day's totals.
    pub start_tasks: u32,
    /// The estimates at the start, in minutes.
    pub start_estimate_minutes: i64,
}

op!(
    /// A project's sprints.
    SprintListOp,
    "sprint.list",
    SprintListInput,
    SprintList
);
op!(
    /// One sprint with its tasks.
    SprintGet,
    "sprint.get",
    SprintGetInput,
    Sprint
);
op!(
    /// Creates a sprint.
    SprintCreate,
    "sprint.create",
    SprintCreateInput,
    SprintChange
);
op!(
    /// Changes a sprint.
    SprintUpdate,
    "sprint.update",
    SprintUpdateInput,
    SprintChange
);
op!(
    /// Deletes a sprint.
    SprintDelete,
    "sprint.delete",
    SprintDeleteInput,
    SprintChange
);
op!(
    /// Starts a sprint.
    SprintStart,
    "sprint.start",
    SprintStartInput,
    SprintChange
);
op!(
    /// Completes a sprint.
    SprintComplete,
    "sprint.complete",
    SprintCompleteInput,
    SprintChange
);
op!(
    /// Puts tasks in a sprint.
    SprintAddTasks,
    "sprint.add_tasks",
    SprintAddTasksInput,
    SprintChange
);
op!(
    /// Takes tasks out of their sprint.
    SprintRemoveTasks,
    "sprint.remove_tasks",
    SprintRemoveTasksInput,
    SprintChange
);
op!(
    /// A sprint's burndown.
    SprintBurndownOp,
    "sprint.burndown",
    SprintBurndownInput,
    SprintBurndown
);

pub(crate) const SPRINT_LIST: OpDescriptor = OpDescriptor::of::<SprintListOp>(Meta {
    summary: "The sprints of a project with their state, dates and progress; with their tasks and the backlog for the sprints page.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("sprint list <project>"),
    input_name: "SprintListInput",
    output_name: "SprintList",
});

pub(crate) const SPRINT_GET: OpDescriptor = OpDescriptor::of::<SprintGet>(Meta {
    summary: "One sprint with its progress and its tasks.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("sprint show <sprint>"),
    input_name: "SprintGetInput",
    output_name: "Sprint",
});

pub(crate) const SPRINT_CREATE: OpDescriptor = OpDescriptor::of::<SprintCreate>(Meta {
    summary: "Plan a sprint in a project: name, goal, start and end dates, and tasks to put in it.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint create"),
    input_name: "SprintCreateInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_UPDATE: OpDescriptor = OpDescriptor::of::<SprintUpdate>(Meta {
    summary: "Rename a sprint or change its goal or dates.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint edit <sprint>"),
    input_name: "SprintUpdateInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_DELETE: OpDescriptor = OpDescriptor::of::<SprintDelete>(Meta {
    summary: "Delete a planned or closed sprint; its tasks go back to the backlog. Revert brings both back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint delete <sprint>"),
    input_name: "SprintDeleteInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_START: OpDescriptor = OpDescriptor::of::<SprintStart>(Meta {
    summary: "Start a planned sprint, with its dates; a project has one active sprint at a time.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint start <sprint>"),
    input_name: "SprintStartInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_COMPLETE: OpDescriptor = OpDescriptor::of::<SprintComplete>(Meta {
    summary: "Complete the active sprint and move its open tasks to a planned sprint or back to the backlog.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint complete <sprint>"),
    input_name: "SprintCompleteInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_ADD_TASKS: OpDescriptor = OpDescriptor::of::<SprintAddTasks>(Meta {
    summary: "Put tasks in a planned or active sprint of their project, moving them from any other sprint.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint add <sprint>"),
    input_name: "SprintAddTasksInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_REMOVE_TASKS: OpDescriptor = OpDescriptor::of::<SprintRemoveTasks>(Meta {
    summary: "Take tasks out of their sprint, back to the backlog.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("sprint remove"),
    input_name: "SprintRemoveTasksInput",
    output_name: "SprintChange",
});

pub(crate) const SPRINT_BURNDOWN: OpDescriptor = OpDescriptor::of::<SprintBurndownOp>(Meta {
    summary: "The burndown of an active or closed sprint: its open and total tasks and estimates at the end of each day.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("sprint burndown <sprint>"),
    input_name: "SprintBurndownInput",
    output_name: "SprintBurndown",
});
