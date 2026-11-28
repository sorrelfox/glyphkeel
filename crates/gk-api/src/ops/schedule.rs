// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The schedule behind the Gantt chart: dated tasks with their summary
//! ranges, the `blocks` links as finish-to-start dependencies, and what is
//! wrong with the plan.
//!
//! Dates are whole days in the workspace's time zone. A task blocked by
//! another may begin on the day after its blocker's due date at the
//! earliest.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::task::{Claim, LabelRef, StatusRef};

/// Which tasks to schedule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleInput {
    /// The project (key or ID). Give this, `query`, or both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: its sub-projects' tasks too. Off unless asked.
    #[serde(default)]
    pub include_subprojects: bool,
    /// Only the tasks matching this query (`is:open label:release`); with
    /// `project`, inside that project. Its `sort:` orders the top-level
    /// rows; by default they go by start date, then due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Leave out tasks that end before this day: `YYYY-MM-DD` or a natural
    /// date such as `today` or `-2w`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Leave out tasks that begin after this day, written as for `from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Only the tasks on the critical path (and the parents they roll up
    /// to). Every task carries its slack either way.
    #[serde(default)]
    pub critical: bool,
    /// The end the critical path counts back from: a day (`YYYY-MM-DD` or
    /// a natural date), a milestone's task key (its due date) or the name
    /// of one of the project's versions (its due date). Left out: the
    /// latest finish of the tasks shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Compare with this baseline of the project (its ID or name): each
    /// task gets its baseline dates and slip, and the tasks the baseline
    /// had that are gone are listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
}

/// A task's place on the critical path, in days (`YYYY-MM-DD`) and working
/// days. Working days are the workspace's days per week, from Monday.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskTiming {
    /// The earliest day it can begin, after the tasks that block it.
    pub earliest_start: String,
    /// The earliest day it can finish.
    pub earliest_finish: String,
    /// The latest day it can begin without moving the end target.
    pub latest_start: String,
    /// The latest day it can finish without moving the end target.
    pub latest_finish: String,
    /// Total slack in working days: how far it can slip without moving the
    /// end target; negative when the target can't be met.
    pub slack_days: i64,
    /// On the critical path: no slack, or less.
    pub critical: bool,
}

/// Why a task has no place on the critical path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimingLeftOut {
    /// It has neither dates nor an estimate.
    NoDatesOrEstimate,
    /// It is in a loop of tasks that block each other.
    DependencyLoop,
}

/// A task the critical path leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LeftOutTask {
    /// The task's key.
    pub key: String,
    /// Why.
    pub reason: TimingLeftOut,
}

/// The critical path of the tasks shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalPath {
    /// The day the latest times count back from: the target asked for, or
    /// the plan's own finish.
    pub end: String,
    /// The earliest day the plan can finish.
    pub finish: String,
    /// Working days per week, from the workspace settings.
    pub working_days_per_week: u32,
    /// The critical tasks' keys, in the order of the chart's rows.
    pub tasks: Vec<String>,
    /// The tasks left out, and why; open tasks without dates and estimate
    /// are listed here, the first 200.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left_out: Vec<LeftOutTask>,
}

/// A task's dates in a baseline, and how far it moved since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineDates {
    /// Its start date then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its due date then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// How many calendar days later it ends now than then (its due date,
    /// or its start without one); negative when earlier. Left out when it
    /// has no dates now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slip_days: Option<i64>,
}

/// A task a baseline has that the schedule no longer shows: deleted, moved
/// to another project, or without dates now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemovedTask {
    /// Its key when the baseline was saved.
    pub key: String,
    /// Its title then.
    pub title: String,
    /// Its start date then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its due date then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Whether the task is deleted (in the trash or gone), rather than
    /// only without dates or out of view.
    pub deleted: bool,
}

/// The baseline a schedule is compared with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleBaseline {
    /// The baseline's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// When it was saved (RFC 3339).
    pub created_at: String,
    /// The tasks it has that the schedule doesn't show, the first 200.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<RemovedTask>,
    /// How many such tasks there are.
    #[serde(default)]
    pub removed_total: u32,
}

/// How the chart draws a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleShape {
    /// A bar from the start to the due date.
    Bar,
    /// Only a due date: a deadline marker.
    Deadline,
    /// Only a start date: a one-day bar with a "no deadline" warning.
    NoDeadline,
    /// A task of a milestone type: a diamond on its due date (or start).
    Milestone,
    /// A task with subtasks: a summary bar over `summary`, calculated from
    /// the subtasks. It is not dragged as a whole.
    Summary,
}

/// A range of days, both included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DayRange {
    /// The first day (`YYYY-MM-DD`).
    pub start: String,
    /// The last day (`YYYY-MM-DD`).
    pub end: String,
}

/// Why a task's dates can't be changed from the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadOnlyReason {
    /// Its project is archived, so nothing in it changes.
    ArchivedProject,
}

/// One row of the chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduledTask {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key.
    pub project: String,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// How deep it is in the chart's tree: 0 for a top-level row. A task
    /// whose parent isn't shown is a top-level row.
    pub depth: u8,
    /// Its status.
    pub status: StatusRef,
    /// Whether it is closed (done or cancelled).
    pub closed: bool,
    /// Its labels, in the project's label order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelRef>,
    /// Why its dates can't be changed; left out when they can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<ReadOnlyReason>,
    /// The agent session that claimed it, while the claim lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// Whether it is the open occurrence of a recurring task.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recurring: bool,
    /// How the chart draws it.
    pub shape: ScheduleShape,
    /// Its own start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its own due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// For a task with subtasks: from the earliest start to the latest due
    /// date among it and all its subtasks, at every level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<DayRange>,
    /// Its progress in percent.
    pub progress_percent: u8,
    /// Open and due before today.
    pub overdue: bool,
    /// A task that blocks it is still open.
    pub blocked: bool,
    /// How many tasks depend on it, directly or through others: the tasks
    /// "also move dependents" may move.
    pub dependents: u32,
    /// Its version, for `expected_version`.
    pub version: i64,
    /// The task's ID.
    pub id: String,
    /// Its custom field values by query name (without `cf.`); left out
    /// when it has none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// Its place on the critical path; for a parent, rolled up from its
    /// subtasks. Left out for tasks the path leaves out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<TaskTiming>,
    /// Its dates in the baseline compared with, when one is and has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineDates>,
    /// Added after the baseline compared with was saved: it has no dates
    /// there.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub new_since_baseline: bool,
}

/// A task without dates, for the Unscheduled lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnscheduledTask {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key.
    pub project: String,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Its status.
    pub status: StatusRef,
    /// Whether it is closed.
    pub closed: bool,
    /// Its labels, in the project's label order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelRef>,
    /// Why its dates can't be changed; left out when they can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<ReadOnlyReason>,
    /// The agent session that claimed it, while the claim lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// A task of a milestone type.
    pub milestone: bool,
    /// Its version, for `expected_version`.
    pub version: i64,
    /// The task's ID.
    pub id: String,
    /// Its custom field values by query name (without `cf.`); left out
    /// when it has none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// Its place on the critical path, from its estimate; left out when
    /// it has no estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<TaskTiming>,
}

/// A finish-to-start dependency: a `blocks` link between two shown tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleDependency {
    /// The blocking task's key.
    pub from: String,
    /// The blocked task's key.
    pub to: String,
    /// Extra days between the two; left out when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag_days: Option<i64>,
    /// The blocked task begins before its blocker ends.
    pub violated: bool,
    /// The link's ID, for `unlink_tasks`.
    pub id: String,
}

/// What kind of problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleProblemKind {
    /// The task begins before a task that blocks it ends (`other`).
    StartsBeforeBlockerEnds,
    /// The task is in a loop of tasks that block each other; `others`
    /// names the rest of the loop.
    DependencyLoop,
    /// The subtask begins before its parent (`other`) starts, or ends
    /// after the parent is due.
    OutsideParent,
    /// The task is open and its due date has passed.
    Overdue,
    /// The task has a start date but no due date.
    NoDueDate,
}

/// Something wrong with the schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleProblem {
    /// What kind of problem.
    pub kind: ScheduleProblemKind,
    /// The task it is about.
    pub task: String,
    /// The other task: the blocker, or the parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
    /// For a loop, the other tasks in it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub others: Vec<String>,
    /// The problem in English.
    pub message: String,
    /// The message's ID in the message files, for translation.
    pub message_id: String,
    /// The message's arguments, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

/// The schedule of a project or a query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// Today in the workspace's time zone (`YYYY-MM-DD`), for the today
    /// line.
    pub today: String,
    /// From the earliest to the latest day of the tasks shown; left out
    /// when none has a date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<DayRange>,
    /// The tasks with dates, and the tasks with subtasks that have dates,
    /// as the chart's rows: a tree in order, each task followed by its
    /// subtasks.
    pub tasks: Vec<ScheduledTask>,
    /// The tasks without dates, for the Unscheduled lane: the first 200 in
    /// the query's order. They don't count against the most tasks a
    /// schedule shows.
    pub unscheduled: Vec<UnscheduledTask>,
    /// How many tasks without dates there are, also those beyond the
    /// first 200 listed in `unscheduled`.
    #[serde(default)]
    pub unscheduled_total: u32,
    /// The `blocks` links between the tasks shown.
    pub dependencies: Vec<ScheduleDependency>,
    /// What is wrong with the schedule of the tasks shown, task by task.
    pub problems: Vec<ScheduleProblem>,
    /// The critical path of the tasks shown; left out when none has dates
    /// or an estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_path: Option<CriticalPath>,
    /// The baseline compared with, when one was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ScheduleBaseline>,
}

/// A task that "also move dependents" moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovedTask {
    /// The task's key.
    pub key: String,
    /// How many days later it now is.
    pub days: i64,
    /// Its new start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its new due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The task's ID.
    pub id: String,
}

/// Which change of dates to try out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SchedulePreviewInput {
    /// The task whose dates would change.
    pub key: String,
    /// Its new start date (`YYYY-MM-DD`); left out: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Its new due date (`YYYY-MM-DD`); left out: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
}

/// What "also move dependents" would do, without doing it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SchedulePreview {
    /// The tasks that would move and their new dates, blockers first.
    pub moved: Vec<MovedTask>,
}

/// A zoom level of the Gantt chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GanttZoom {
    /// One column per day.
    #[default]
    Day,
    /// Weeks.
    Week,
    /// Months.
    Month,
    /// Quarters.
    Quarter,
}

/// How the Gantt chart groups its rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GanttGrouping {
    /// Subtasks under their parent tasks: the schedule's tree.
    #[default]
    Parent,
    /// By the first label in alphabetical order, as the list groups;
    /// tasks without one last.
    Label,
    /// By status, in board order.
    Status,
    /// By the agent session that claimed the task; unclaimed tasks last.
    Claim,
    /// One flat list, without groups or nesting.
    None,
}

/// The person's own layout of a project's Gantt chart: the zoom level, the
/// table's width and its columns' widths, the grouping, and whether moving
/// a task also moves the tasks that depend on it.
/// Unlike saved views it belongs to the person, not to the workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GanttLayout {
    /// The zoom level.
    #[serde(default)]
    pub zoom: GanttZoom,
    /// The table's width in pixels; left out: the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<u32>,
    /// Column widths in pixels, by column; the columns left out keep their
    /// default width.
    #[serde(default)]
    pub widths: BTreeMap<String, u32>,
    /// How the rows are grouped.
    #[serde(default)]
    pub group_by: GanttGrouping,
    /// Whether a change of dates also moves the tasks that depend on the
    /// task ("also move dependents").
    #[serde(default)]
    pub move_dependents: bool,
}

/// Which Gantt layout to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GanttLayoutGetInput {
    /// The project (key or ID).
    pub project: String,
}

/// Changes to a Gantt layout; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GanttLayoutUpdateInput {
    /// The project (key or ID).
    pub project: String,
    /// Go back to the default layout first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    /// The zoom level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom: Option<GanttZoom>,
    /// The table's width in pixels (200 to 1200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<u32>,
    /// Drop the kept table width, so that the table goes back to its
    /// default width; `table` given as well wins.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_table: bool,
    /// Widths to set, in pixels (24 to 600), by column; the others keep
    /// theirs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, u32>,
    /// Columns whose kept width is dropped, so that they go back to their
    /// default width; applied before `widths`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear_widths: Vec<String>,
    /// How the rows are grouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<GanttGrouping>,
    /// Whether a change of dates also moves the dependent tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_dependents: Option<bool>,
}

/// A Gantt layout, after a change or as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GanttLayoutChange {
    /// The project's key.
    pub project: String,
    /// The layout.
    pub layout: GanttLayout,
    /// The history group; left out when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// The Gantt chart's data and the problems of the plan.
    ScheduleGet,
    "schedule.get",
    ScheduleInput,
    Schedule
);

pub(crate) const SCHEDULE_GET: OpDescriptor = OpDescriptor::of::<ScheduleGet>(Meta {
    summary: "The schedule of a project or a query for the Gantt chart: dated tasks as a tree with summary ranges for parents, milestones, the tasks without dates, blocks links as finish-to-start dependencies, problems such as a task starting before its blocker ends, each task's slack on the critical path, and its slip against a baseline.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("schedule"),
    input_name: "ScheduleInput",
    output_name: "Schedule",
});

op!(
    /// What "also move dependents" would move, without moving anything.
    SchedulePreviewMove,
    "schedule.preview_move",
    SchedulePreviewInput,
    SchedulePreview
);

pub(crate) const SCHEDULE_PREVIEW_MOVE: OpDescriptor = OpDescriptor::of::<SchedulePreviewMove>(
    Meta {
        summary: "The tasks that \"also move dependents\" would move, and their new dates, if a task got the given dates; nothing is changed.",
        kind: OpKind::Query,
        safety: Safety::Read,
        exposure: Exposure::App {
            reason: "It previews a drag in the Gantt chart while the pointer moves; agents and scripts change dates with update_task and move_dependents, whose answer lists the tasks that moved.",
        },
        cli: None,
        input_name: "SchedulePreviewInput",
        output_name: "SchedulePreview",
    },
);

op!(
    /// The person's layout of a project's Gantt chart.
    GanttLayoutGet,
    "gantt.layout.get",
    GanttLayoutGetInput,
    GanttLayoutChange
);
op!(
    /// Changes the person's layout of a project's Gantt chart.
    GanttLayoutUpdate,
    "gantt.layout.update",
    GanttLayoutUpdateInput,
    GanttLayoutChange
);

pub(crate) const GANTT_LAYOUT_GET: OpDescriptor = OpDescriptor::of::<GanttLayoutGet>(Meta {
    summary: "The person's layout of a project's Gantt chart: zoom level, table and column widths, grouping, and whether moving a task also moves its dependents.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It holds how the person's own Gantt chart looks, not the workspace's data; agents and scripts read the schedule itself.",
    },
    cli: None,
    input_name: "GanttLayoutGetInput",
    output_name: "GanttLayoutChange",
});

pub(crate) const GANTT_LAYOUT_UPDATE: OpDescriptor = OpDescriptor::of::<GanttLayoutUpdate>(Meta {
    summary: "Change the zoom level, table or column widths, grouping or the \"also move dependents\" switch of a project's Gantt chart, or go back to the default layout.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "It changes how the person's own Gantt chart looks, not the workspace's data; agents and scripts read the schedule itself.",
    },
    cli: None,
    input_name: "GanttLayoutUpdateInput",
    output_name: "GanttLayoutChange",
});
