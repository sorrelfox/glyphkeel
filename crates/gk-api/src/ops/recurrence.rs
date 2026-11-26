// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Recurring tasks.
//!
//! A recurring task belongs to a series: a rule, and the details each new
//! occurrence starts from (title, description with its checklist unticked,
//! type, priority, labels, estimate, custom field values, the version
//! while it is open, subtasks and dates relative to the occurrence's date).
//! The open occurrence is an ordinary task; changing it changes that
//! occurrence only. `recurrence.set` with `update_details` makes its
//! current details those of the occurrences still to come.
//!
//! The engine makes occurrences on its own, so it makes them even where a
//! task made by hand would be refused: a required custom field without a
//! value or a status's required fields stay empty for the person to fill
//! in, and rules for agents don't stop them. What a status does to tasks
//! entering it (setting or clearing fields) applies. Moving the current
//! occurrence to another project takes the series along; the next ones are
//! made there, with what that project has by the same names.
//!
//! The next occurrence is made when the current one is closed (done or
//! cancelled), or, with `create: ahead`, on its date (`days_ahead` days
//! before) whether or not the previous one is closed. Dates follow the
//! calendar (`from: schedule`; dates already past are passed over) or count
//! from the day the last one was done (`from: completion`: "3 days after
//! done"). "Today" is the person's day in their time zone setting.
//!
//! A rule is written in words (`text`: `every monday`, `every 2 weeks on
//! mon and thu`, `the 1st of each month`, `monthly on the last fri`,
//! `yearly in march`, `3 days after done`, `until 2027-06-30`, `10 times`)
//! or field by field; fields given next to `text` override it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::task::TaskRef;

/// The unit a rule repeats in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    /// Every N days.
    Day,
    /// Every N weeks.
    Week,
    /// Every N months.
    Month,
    /// Every N years.
    Year,
}

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    /// Monday.
    Mon,
    /// Tuesday.
    Tue,
    /// Wednesday.
    Wed,
    /// Thursday.
    Thu,
    /// Friday.
    Fri,
    /// Saturday.
    Sat,
    /// Sunday.
    Sun,
}

/// What the next date is counted from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepeatFrom {
    /// The calendar: the rule's dates, whenever the last one was done.
    #[default]
    Schedule,
    /// The day the last occurrence was closed.
    Completion,
}

/// When the next occurrence is made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreateWhen {
    /// When the current occurrence is closed.
    #[default]
    OnClose,
    /// On the occurrence's date (`days_ahead` days before it), even while
    /// the previous one is open. Only with `from: schedule`.
    Ahead,
}

/// A recurrence rule, as outputs carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceRule {
    /// The unit.
    pub every: Frequency,
    /// Every this many units, 1 to 999.
    pub interval: u32,
    /// Weekly: the weekdays, Monday first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weekdays: Vec<Weekday>,
    /// Monthly and yearly: the day of the month, 1 to 31, or -1 for the
    /// last day (the 31st is the last day in shorter months).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<i32>,
    /// Monthly and yearly: the first to fifth (1–5) or last (-1) `weekday`
    /// of the month, instead of a day of the month.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub week_of_month: Option<i32>,
    /// With `week_of_month`: the weekday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<Weekday>,
    /// Yearly: the month, 1 to 12.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<u32>,
    /// The first possible day, `YYYY-MM-DD`; the pattern counts from it.
    pub starts: String,
    /// The last possible day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// At most this many occurrences, the first included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// From the calendar or from completion.
    pub from: RepeatFrom,
    /// When the next occurrence is made.
    pub create: CreateWhen,
    /// With `create: ahead`: this many days before its date (0 to 365).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub days_ahead: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A rule given field by field or in words; shared by `recurrence.set` and
/// `recurrence.preview`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RulePreviewInput {
    /// The rule in words (`every 2 weeks on mon and thu`); fields given as
    /// well override it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The unit; needed without `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<Frequency>,
    /// Every this many units, 1 to 999; default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,
    /// Weekly: the weekdays; default the start's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weekdays: Vec<Weekday>,
    /// Monthly and yearly: the day of the month, 1 to 31, or -1 for the
    /// last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<i32>,
    /// Monthly and yearly: 1 to 5 or -1 (last), with `weekday`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub week_of_month: Option<i32>,
    /// With `week_of_month`: the weekday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<Weekday>,
    /// Yearly: the month, 1 to 12.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<u32>,
    /// The first possible day (a date: `2026-12-01`, `today`, `next mon`);
    /// default the task's due date, else its start date, else today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<String>,
    /// The last possible day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// At most this many occurrences, 1 to 10000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// From the calendar (default) or from completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<RepeatFrom>,
    /// When the next occurrence is made; default when the current one is
    /// closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<CreateWhen>,
    /// With `create: ahead`: this many days before its date, 0 to 365.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_ahead: Option<u32>,
    /// How many dates to show, 1 to 50; default 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Makes a task recurring, or changes its series.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceSetInput {
    /// The task: its key or ID.
    pub key: String,
    /// The rule in words (`every monday`); fields given as well override
    /// it. Leave the rule out to keep the series' rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<Frequency>,
    /// Every this many units, 1 to 999; default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,
    /// Weekly: the weekdays; default the start's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weekdays: Vec<Weekday>,
    /// Monthly and yearly: the day of the month, 1 to 31, or -1 for the
    /// last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<i32>,
    /// Monthly and yearly: 1 to 5 or -1 (last), with `weekday`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub week_of_month: Option<i32>,
    /// With `week_of_month`: the weekday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<Weekday>,
    /// Yearly: the month, 1 to 12.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<u32>,
    /// The first possible day; default the task's due date, else its start
    /// date, else today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts: Option<String>,
    /// The last possible day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// At most this many occurrences, 1 to 10000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// From the calendar (default) or from completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<RepeatFrom>,
    /// When the next occurrence is made; default when this one is closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<CreateWhen>,
    /// With `create: ahead`: this many days before its date, 0 to 365.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_ahead: Option<u32>,
    /// Make this task's current details (title, description, type,
    /// priority, labels, estimate, subtasks, dates) those of the
    /// occurrences still to come. A new series always starts from them.
    #[serde(default)]
    pub update_details: bool,
}

/// Names a recurring task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceTaskInput {
    /// The task: its key or ID.
    pub key: String,
}

/// Makes the occurrences whose time has come.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceRunInput {}

/// A task's series, as tasks carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recurrence {
    /// The series' ID.
    pub series: String,
    /// The rule.
    pub rule: RecurrenceRule,
    /// The rule in words, for people.
    pub summary: String,
    /// The key of the series' current occurrence; left out when it is in
    /// the trash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// The date of the next occurrence still to come; left out when the
    /// rule has no more, or when it counts from completion and the current
    /// one is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_date: Option<String>,
    /// Occurrences made or passed over so far, the first included.
    pub made: u32,
    /// Whether the series has ended: stopped, or no dates left.
    pub ended: bool,
    /// The title new occurrences get.
    pub title: String,
}

/// A task's series after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceChange {
    /// The task's key.
    pub task: String,
    /// The series now; left out when the task stopped repeating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<Recurrence>,
    /// The task's due date now (it moves when an occurrence is skipped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// The next dates of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrencePreview {
    /// The rule as read.
    pub rule: RecurrenceRule,
    /// The rule in words, for people.
    pub summary: String,
    /// The next dates from the start (from completion: counted from the
    /// start as if each was done on its date).
    pub dates: Vec<String>,
}

/// What a run made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecurrenceRun {
    /// The new occurrences.
    pub created: Vec<TaskRef>,
    /// When the next occurrence comes due (RFC 3339, UTC): local midnight
    /// of its day in the person's time zone; left out when none waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_at: Option<String>,
    /// The history group, for revert and Undo; left out when nothing was
    /// made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

// ---------------------------------------------------------------------
// Operations

op!(
    /// Make a task recurring or change its series.
    RecurrenceSet,
    "recurrence.set",
    RecurrenceSetInput,
    RecurrenceChange
);
op!(
    /// Stop a task repeating.
    RecurrenceClear,
    "recurrence.clear",
    RecurrenceTaskInput,
    RecurrenceChange
);
op!(
    /// Pass over the current occurrence.
    RecurrenceSkip,
    "recurrence.skip",
    RecurrenceTaskInput,
    RecurrenceChange
);
op!(
    /// The next dates of a rule.
    RecurrencePreviewOp,
    "recurrence.preview",
    RulePreviewInput,
    RecurrencePreview
);
op!(
    /// Make the occurrences whose time has come.
    RecurrenceRunOp,
    "recurrence.run",
    RecurrenceRunInput,
    RecurrenceRun
);

pub(crate) const RECURRENCE_SET: OpDescriptor = OpDescriptor::of::<RecurrenceSet>(Meta {
    summary: "Make a task recurring, change its rule, or (`update_details`) make its current details those of the occurrences to come.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("recurrence set <key>"),
    input_name: "RecurrenceSetInput",
    output_name: "RecurrenceChange",
});

pub(crate) const RECURRENCE_CLEAR: OpDescriptor = OpDescriptor::of::<RecurrenceClear>(Meta {
    summary: "Stop a task repeating: it stays, and no more occurrences are made.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("recurrence clear <key>"),
    input_name: "RecurrenceTaskInput",
    output_name: "RecurrenceChange",
});

pub(crate) const RECURRENCE_SKIP: OpDescriptor = OpDescriptor::of::<RecurrenceSkip>(Meta {
    summary: "Pass over a recurring task's current occurrence: its dates move to the next date of the rule.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("recurrence skip <key>"),
    input_name: "RecurrenceTaskInput",
    output_name: "RecurrenceChange",
});

pub(crate) const RECURRENCE_PREVIEW: OpDescriptor = OpDescriptor::of::<RecurrencePreviewOp>(Meta {
    summary: "Read a recurrence rule, in words or by fields, and list its next dates.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("recurrence preview"),
    input_name: "RulePreviewInput",
    output_name: "RecurrencePreview",
});

pub(crate) const RECURRENCE_RUN: OpDescriptor = OpDescriptor::of::<RecurrenceRunOp>(Meta {
    summary: "Make the occurrences of series that are made ahead whose day has come; the app does this by itself.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "Housekeeping that the app and the command line run on a schedule; agents' changes already make occurrences as tasks close.",
    },
    cli: Some("recurrence run"),
    input_name: "RecurrenceRunInput",
    output_name: "RecurrenceRun",
});
