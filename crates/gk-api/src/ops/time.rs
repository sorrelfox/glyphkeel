// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Time entries, the timer, the time report and reminders.
//!
//! Durations are written `1w 2d 4h 30m`, `90m`, `1.5h`, `1,5h` or `1:30`,
//! with the workspace's working day and week; a leading `+` is allowed
//! (`+30m`). Days are `YYYY-MM-DD`, or `today` and `yesterday`, in the
//! computer's time zone. Moments are RFC 3339 (`2026-11-24T09:00:00Z`), or
//! a local `YYYY-MM-DD HH:MM`.
//!
//! One timer runs per workspace. Starting it on another task stops the
//! running one first and logs its time. A paused timer keeps its time until
//! it is started again or stopped.
//!
//! Time may be marked with a kind of work (Development, Design, Meeting…)
//! from the workspace's list, so the report can group by it. A kind that
//! entries use is archived rather than deleted: it leaves the pickers and
//! keeps its name in reports.
//!
//! A task has at most one personal reminder: a moment, or a number of days
//! before its due date. Open tasks with a due date also alert the day before
//! the due date, at the workspace's reminder time.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;
use crate::ops::task::TaskRef;

// ---------------------------------------------------------------------
// Time entries

/// One time entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeEntry {
    /// The entry's ID.
    pub id: String,
    /// The task's key.
    pub task: String,
    /// The day the work was done (`YYYY-MM-DD`).
    pub date: String,
    /// The time in minutes.
    pub minutes: i64,
    /// The time as a duration in the workspace's units (`1h 30m`).
    pub duration: String,
    /// What was done; left out when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The kind of work; left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<WorkKindRef>,
    /// Who logged it.
    pub author: Actor,
    /// When it was logged (RFC 3339, UTC).
    pub created_at: String,
    /// When it was last edited; left out when never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    /// Goes up by one with every edit; pass it as `expected_version`.
    pub version: i64,
}

/// Time to log on a task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeLogInput {
    /// The task: its key or ID.
    pub key: String,
    /// How long: `1h 30m`, `90m`, `1.5h`, `1:30`, `+30m`; at most 24 hours.
    pub duration: String,
    /// The day of the work: `YYYY-MM-DD`, `today` or `yesterday`; default
    /// today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// What was done, at most 2000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The kind of work: its name (any case) or ID, from `work_kind.list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<String>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of logging the time twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// Changes to a time entry; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeUpdateInput {
    /// The entry's ID.
    pub entry: String,
    /// The new duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// The new day: `YYYY-MM-DD`, `today` or `yesterday`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The new note; an empty text removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The new kind of work (name or ID); an empty text removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<String>,
    /// Refuse the change unless the entry still has this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

/// Names one time entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeEntryRef {
    /// The entry's ID.
    pub entry: String,
}

/// A time entry changed, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeEntryChange {
    /// The entry after the change.
    pub entry: TimeEntry,
    /// Whether it is in the trash now.
    #[serde(default)]
    pub deleted: bool,
    /// The task's logged time after the change, in minutes.
    pub spent_minutes: i64,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

// ---------------------------------------------------------------------
// The time report

/// How the time report groups the entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeGroupBy {
    /// By the day of the work.
    Day,
    /// By the week of the work; weeks start on the workspace's first day
    /// of the week.
    Week,
    /// By the calendar month of the work.
    Month,
    /// By task.
    #[default]
    Task,
    /// By project.
    Project,
    /// By the person or agent who logged the time.
    Person,
    /// By kind of work; time without one is a group of its own.
    WorkKind,
}

/// What the time report also returns as CSV text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeReportCsv {
    /// One line per group, as `items`.
    Groups,
    /// One line per entry, oldest first.
    Entries,
}

/// Which time to report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeReportInput {
    /// The first day: `YYYY-MM-DD`, `today` or `yesterday`; default the
    /// first day of the current week.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The last day, included; default six days after `from`. At most 366
    /// days after `from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Only this project: a key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: its sub-projects' time too; default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_subprojects: Option<bool>,
    /// Only this task and its subtasks: a key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Only the time logged by `me` (the local user), `agent` (any agent)
    /// or the person or agent of that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    /// Only the time of this kind of work (name or ID), or `none` for time
    /// without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<String>,
    /// How to group: `day`, `week`, `month`, `task` (default), `project`,
    /// `person` or `work_kind`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<TimeGroupBy>,
    /// Also list the entries, oldest first.
    #[serde(default)]
    pub entries: bool,
    /// Also return the report as CSV text: `groups` or `entries`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<TimeReportCsv>,
}

/// One group of the time report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeReportRow {
    /// The day; the week's first day; the month (`YYYY-MM`); the task's
    /// or the project's key; the person (`user:Name`, `agent:Name`); or
    /// the kind of work's ID (`none` for time without one).
    pub key: String,
    /// The weekday; the week's days (`2026-11-23..2026-11-29`); the month;
    /// the task's title; the project's name; the person's name; or the kind
    /// of work's name (empty for time without one).
    pub title: String,
    /// The person or agent, when grouped by person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<Actor>,
    /// The time in minutes.
    pub minutes: i64,
    /// The time in the workspace's units.
    pub duration: String,
    /// How many entries.
    pub entry_count: u32,
}

/// Logged time over a range of days, grouped, with the total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeReport {
    /// The first day.
    pub from: String,
    /// The last day, included.
    pub to: String,
    /// How the rows are grouped.
    pub group_by: TimeGroupBy,
    /// The groups: days, weeks and months in order; tasks, projects and
    /// people by most time first.
    pub items: Vec<TimeReportRow>,
    /// The total in minutes.
    pub total_minutes: i64,
    /// The total in the workspace's units.
    pub total_duration: String,
    /// The working day the units use, in hours.
    pub hours_per_day: u32,
    /// The working week the units use, in days.
    pub days_per_week: u32,
    /// The entries, oldest first, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<TimeEntry>>,
    /// The report as CSV text (comma-separated, a header line, CRLF line
    /// ends), when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

// ---------------------------------------------------------------------
// Kinds of work

/// A kind of work as entries name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindRef {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
}

/// A kind of work of the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKind {
    /// Its ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Archived: kept for the entries that name it, not offered for new
    /// time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    /// How many live entries name it.
    pub entries: u64,
}

/// Which kinds of work to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindListInput {
    /// Also the archived ones.
    #[serde(default)]
    pub include_archived: bool,
}

/// The workspace's kinds of work, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindList {
    /// The kinds.
    pub items: Vec<WorkKind>,
}

/// A new kind of work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindCreateInput {
    /// Its name, 1 to 60 characters, unique in the workspace (any case).
    pub name: String,
}

/// Changes to a kind of work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindUpdateInput {
    /// The kind: its name (any case) or ID.
    pub kind: String,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Archive it (`true`) or offer it again (`false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
}

/// Names one kind of work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindDeleteInput {
    /// The kind: its name (any case) or ID.
    pub kind: String,
}

/// A kind of work after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkKindChange {
    /// The kind after the change; as it was, when deleted.
    pub kind: WorkKind,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

// ---------------------------------------------------------------------
// The timer

/// Whether the timer runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimerState {
    /// It counts.
    Running,
    /// It keeps its time but doesn't count.
    Paused,
}

/// The workspace's timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Timer {
    /// The task it times.
    pub task: TaskRef,
    /// Running or paused.
    pub state: TimerState,
    /// When it was first started (RFC 3339, UTC).
    pub started_at: String,
    /// When the current running stretch began; left out while paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_since: Option<String>,
    /// The time so far, in seconds.
    pub elapsed_seconds: i64,
    /// The time so far in the workspace's units, rounded to minutes.
    pub elapsed: String,
    /// The note the entry will get; left out when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The kind of work the entry will get; left out when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<WorkKindRef>,
    /// Who started it.
    pub started_by: Actor,
    /// Whether the task is in the trash; stop the timer with `discard`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub task_trashed: bool,
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerGetInput {}

/// The timer, if there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerStatus {
    /// The timer; left out when none runs or is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Timer>,
}

/// Start the timer, or start a paused one again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerStartInput {
    /// The task: its key or ID. Left out, the paused timer runs again. A
    /// timer on another task is stopped first and its time logged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The note the entry will get, at most 2000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The kind of work the entry will get (name or ID); an empty text
    /// removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<String>,
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerPauseInput {}

/// Stop the timer and log its time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerStopInput {
    /// The entry's note; default the timer's own note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The entry's kind of work (name or ID); default the timer's own; an
    /// empty text logs none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_kind: Option<String>,
    /// Log this duration instead of the time measured, for example when the
    /// timer ran on by mistake.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Throw the time away instead of logging it.
    #[serde(default)]
    pub discard: bool,
}

/// The timer after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerChange {
    /// The timer now; left out when it was stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Timer>,
    /// The entry the stopped timer logged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<TimeEntry>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// What didn't stop the change, such as time under half a minute that
    /// was not logged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------
// Reminders

/// A task's personal reminder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reminder {
    /// The reminder's ID.
    pub id: String,
    /// The task's key.
    pub task: String,
    /// When it fires (RFC 3339, UTC); left out while a reminder relative to
    /// the due date has no due date to follow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// For a reminder that follows the due date: how many days before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_before_due: Option<u32>,
    /// For a reminder that follows the due date: the time of day (`HH:MM`);
    /// left out, the workspace's reminder time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Its text; left out when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Whether it has fired and was acknowledged.
    pub acknowledged: bool,
    /// Who set it.
    pub set_by: Actor,
    /// When it was set (RFC 3339, UTC).
    pub created_at: String,
}

/// A personal reminder on a task; it replaces the one the task had.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderSetInput {
    /// The task: its key or ID.
    pub key: String,
    /// When: RFC 3339, or a local `YYYY-MM-DD HH:MM`. Give this or
    /// `days_before_due`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Fire this many days (0 to 365) before the due date, and again when
    /// the due date changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_before_due: Option<u32>,
    /// With `days_before_due`: the time of day, `HH:MM`; default the
    /// workspace's reminder time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Its text, at most 500 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Names the task whose reminder to remove.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderClearInput {
    /// The task: its key or ID.
    pub key: String,
}

/// A task's reminder after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderChange {
    /// The task's key.
    pub task: String,
    /// The reminder now; left out when it was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder: Option<Reminder>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// What reminds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReminderKind {
    /// A personal reminder on a task.
    Reminder,
    /// A task's due date is near: the day before it, at the reminder time.
    Deadline,
    /// The timer has run for longer than the workspace's setting.
    Timer,
    /// A notification an automation rule left; `note` is its text.
    Automation,
}

/// Which reminders are due.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderDueInput {
    /// Reminders due up to this moment (RFC 3339 or local `YYYY-MM-DD
    /// HH:MM`); default now. A later moment lists what is coming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// Reminders due before this moment are marked `missed`: pass the time
    /// your program started, so what came due while it was closed shows as
    /// missed. Default: due more than 5 minutes ago.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// At most this many, 1 to 500; default 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// One reminder that is due.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DueReminder {
    /// The ID to acknowledge it with.
    pub id: String,
    /// What reminds.
    pub kind: ReminderKind,
    /// When it came due (RFC 3339, UTC).
    pub at: String,
    /// Whether it came due before `since`: missed while no program showed
    /// it.
    pub missed: bool,
    /// The task.
    pub task: TaskRef,
    /// The task's due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The reminder's text; left out when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// The reminders that are due and not acknowledged, oldest first, and when
/// the next one comes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DueReminders {
    /// The reminders due, oldest first.
    pub items: Vec<DueReminder>,
    /// How many are due, also beyond `limit`.
    pub total: u64,
    /// When the next reminder after `until` comes due (RFC 3339, UTC); left
    /// out when none is set. Schedule the next check for then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_at: Option<String>,
    /// The moment the list was made for (`until`).
    pub until: String,
}

/// Reminders that were shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderAckInput {
    /// The `id`s of due reminders, as the list of due reminders gives them.
    pub ids: Vec<String>,
}

/// What became of the acknowledged reminders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderAcknowledged {
    /// The reminders acknowledged; they won't come due again.
    pub acknowledged: Vec<String>,
    /// IDs that name no due reminder (already acknowledged, changed or
    /// removed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<String>,
}

// ---------------------------------------------------------------------
// Operations

op!(
    /// Time logged on a task.
    TimeLog,
    "time.log",
    TimeLogInput,
    TimeEntryChange
);
op!(
    /// A time entry changed.
    TimeUpdate,
    "time.update",
    TimeUpdateInput,
    TimeEntryChange
);
op!(
    /// A time entry to the trash.
    TimeDelete,
    "time.delete",
    TimeEntryRef,
    TimeEntryChange
);
op!(
    /// Logged time over a range of days.
    TimeReportOp,
    "time.report",
    TimeReportInput,
    TimeReport
);
op!(
    /// The workspace's kinds of work.
    WorkKindListOp,
    "work_kind.list",
    WorkKindListInput,
    WorkKindList
);
op!(
    /// Creates a kind of work.
    WorkKindCreate,
    "work_kind.create",
    WorkKindCreateInput,
    WorkKindChange
);
op!(
    /// Changes a kind of work.
    WorkKindUpdate,
    "work_kind.update",
    WorkKindUpdateInput,
    WorkKindChange
);
op!(
    /// Deletes a kind of work no entry names.
    WorkKindDelete,
    "work_kind.delete",
    WorkKindDeleteInput,
    WorkKindChange
);
op!(
    /// The timer.
    TimerGet,
    "timer.get",
    TimerGetInput,
    TimerStatus
);
op!(
    /// Start the timer.
    TimerStart,
    "timer.start",
    TimerStartInput,
    TimerChange
);
op!(
    /// Pause the timer.
    TimerPause,
    "timer.pause",
    TimerPauseInput,
    TimerChange
);
op!(
    /// Stop the timer and log its time.
    TimerStop,
    "timer.stop",
    TimerStopInput,
    TimerChange
);
op!(
    /// Set a task's reminder.
    ReminderSet,
    "reminder.set",
    ReminderSetInput,
    ReminderChange
);
op!(
    /// Remove a task's reminder.
    ReminderClear,
    "reminder.clear",
    ReminderClearInput,
    ReminderChange
);
op!(
    /// The reminders that are due.
    ReminderDue,
    "reminder.due",
    ReminderDueInput,
    DueReminders
);
op!(
    /// Acknowledge reminders that were shown.
    ReminderAcknowledge,
    "reminder.acknowledge",
    ReminderAckInput,
    ReminderAcknowledged
);

pub(crate) const WORK_KIND_LIST: OpDescriptor = OpDescriptor::of::<WorkKindListOp>(Meta {
    summary: "The workspace's kinds of work for logged time (Development, Design, Meeting…), by name, with how many entries use each.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("work-kind list"),
    input_name: "WorkKindListInput",
    output_name: "WorkKindList",
});

pub(crate) const WORK_KIND_CREATE: OpDescriptor = OpDescriptor::of::<WorkKindCreate>(Meta {
    summary: "Add a kind of work that time entries can be marked with.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("work-kind create"),
    input_name: "WorkKindCreateInput",
    output_name: "WorkKindChange",
});

pub(crate) const WORK_KIND_UPDATE: OpDescriptor = OpDescriptor::of::<WorkKindUpdate>(Meta {
    summary: "Rename a kind of work, or archive it so it is no longer offered while its entries keep it.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("work-kind edit <kind>"),
    input_name: "WorkKindUpdateInput",
    output_name: "WorkKindChange",
});

pub(crate) const WORK_KIND_DELETE: OpDescriptor = OpDescriptor::of::<WorkKindDelete>(Meta {
    summary: "Delete a kind of work that no time entry names; archive one that is in use.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("work-kind delete <kind>"),
    input_name: "WorkKindDeleteInput",
    output_name: "WorkKindChange",
});

pub(crate) const TIME_LOG: OpDescriptor = OpDescriptor::of::<TimeLog>(Meta {
    summary: "Log time on a task: a duration, the day of the work (default today), a note and a kind of work.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("time log <key>"),
    input_name: "TimeLogInput",
    output_name: "TimeEntryChange",
});

pub(crate) const TIME_UPDATE: OpDescriptor = OpDescriptor::of::<TimeUpdate>(Meta {
    summary: "Change a time entry's duration, day, note or kind of work.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("time edit <entry>"),
    input_name: "TimeUpdateInput",
    output_name: "TimeEntryChange",
});

pub(crate) const TIME_DELETE: OpDescriptor = OpDescriptor::of::<TimeDelete>(Meta {
    summary: "Move a time entry to the trash.",
    kind: OpKind::Command,
    safety: Safety::Trash,
    exposure: Exposure::Data,
    cli: Some("time delete <entry>"),
    input_name: "TimeEntryRef",
    output_name: "TimeEntryChange",
});

pub(crate) const TIME_REPORT: OpDescriptor = OpDescriptor::of::<TimeReportOp>(Meta {
    summary: "Logged time over a range of days, grouped by day, week, month, task, project, person or kind of work, with totals in the workspace's units and optional CSV.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("time report"),
    input_name: "TimeReportInput",
    output_name: "TimeReport",
});

pub(crate) const TIMER_GET: OpDescriptor = OpDescriptor::of::<TimerGet>(Meta {
    summary: "The workspace's timer: its task, whether it runs and the time so far.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("timer status"),
    input_name: "TimerGetInput",
    output_name: "TimerStatus",
});

pub(crate) const TIMER_START: OpDescriptor = OpDescriptor::of::<TimerStart>(Meta {
    summary: "Start the timer on a task, or start the paused timer again; a timer on another task is stopped and its time logged.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("timer start <key>"),
    input_name: "TimerStartInput",
    output_name: "TimerChange",
});

pub(crate) const TIMER_PAUSE: OpDescriptor = OpDescriptor::of::<TimerPause>(Meta {
    summary: "Pause the running timer; it keeps its time.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("timer pause"),
    input_name: "TimerPauseInput",
    output_name: "TimerChange",
});

pub(crate) const TIMER_STOP: OpDescriptor = OpDescriptor::of::<TimerStop>(Meta {
    summary: "Stop the timer and log its time as an entry on its task.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("timer stop"),
    input_name: "TimerStopInput",
    output_name: "TimerChange",
});

pub(crate) const REMINDER_SET: OpDescriptor = OpDescriptor::of::<ReminderSet>(Meta {
    summary: "Set a task's personal reminder: a moment, or a number of days before its due date.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("reminder set <key>"),
    input_name: "ReminderSetInput",
    output_name: "ReminderChange",
});

pub(crate) const REMINDER_CLEAR: OpDescriptor = OpDescriptor::of::<ReminderClear>(Meta {
    summary: "Remove a task's personal reminder.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("reminder clear <key>"),
    input_name: "ReminderClearInput",
    output_name: "ReminderChange",
});

pub(crate) const REMINDER_DUE: OpDescriptor = OpDescriptor::of::<ReminderDue>(Meta {
    summary: "The reminders, deadline alerts and timer reminders that are due and not acknowledged, with those missed marked, and when the next one comes.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("reminder due"),
    input_name: "ReminderDueInput",
    output_name: "DueReminders",
});

pub(crate) const REMINDER_ACKNOWLEDGE: OpDescriptor =
    OpDescriptor::of::<ReminderAcknowledge>(Meta {
        summary: "Acknowledge reminders that were shown, so they don't come due again.",
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some("reminder ack <ids>"),
        input_name: "ReminderAckInput",
        output_name: "ReminderAcknowledged",
    });
