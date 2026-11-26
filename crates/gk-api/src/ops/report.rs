// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Flow reports, computed from the tasks' history: tasks created against
//! tasks closed, the cumulative flow by status, and cycle and lead time.
//!
//! A task is closed while its status is in a closed category (`done` or
//! `cancelled`), by the status's category today. Closing is the move from an
//! open status into a closed one; reopening the move back. Cycle time runs
//! from the first move into an `in_progress` status to the last close; lead
//! time from the task's creation to the last close.
//!
//! The reports count the tasks as they are now: their project, type, labels
//! and query match today decide whether they are in. Tasks in the trash are
//! left out; while a task was in the trash it is not counted.
//!
//! Days, weeks and months are those of the personal time zone; weeks start
//! on the workspace's first day of the week. The first and last periods are
//! cut to `from` and `to`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Category, Hue};

/// How long each period of a flow report is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlowBucket {
    /// One day.
    Day,
    /// One week, from the workspace's first day of the week.
    #[default]
    Week,
    /// One calendar month.
    Month,
}

/// What the bands of the cumulative flow are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlowGroupBy {
    /// One band per status name; statuses of the same name in several
    /// projects share a band.
    #[default]
    Status,
    /// One band per status category.
    Category,
}

/// Which tasks and which days a flow report covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowReportInput {
    /// The first day: `YYYY-MM-DD`, `today` or `yesterday`; default twelve
    /// periods before `to` (30 days, 12 weeks or 12 months).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The last day, included; default today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The length of a period: `day`, `week` (default) or `month`. At most
    /// 1,000 periods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<FlowBucket>,
    /// Only this project: a key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: its sub-projects' tasks too; default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_subprojects: Option<bool>,
    /// Only tasks of this type, by name.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// Only tasks with this label, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Only tasks that match this query (the task query language), as they
    /// are now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Created against closed and cycle time only: count only what `me`
    /// (the local user), `agent` (any agent) or the person or agent of that
    /// name did: creating a task, closing or reopening it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Cumulative flow only: bands by `status` (default) or `category`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<FlowGroupBy>,
    /// Cycle time only: also list every task closed in the period.
    #[serde(default)]
    pub tasks: bool,
    /// Also return the report as CSV text (comma-separated, a header line,
    /// CRLF line ends).
    #[serde(default)]
    pub csv: bool,
}

/// One period of tasks created against tasks closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedClosedRow {
    /// The period's first day.
    pub start: String,
    /// The period's last day, included.
    pub end: String,
    /// Tasks created in the period.
    pub created: u32,
    /// Times a task was closed in the period.
    pub closed: u32,
    /// Times a closed task was opened again in the period.
    pub reopened: u32,
    /// Open tasks at the end of the period.
    pub open: u32,
}

/// Tasks created against tasks closed, per period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedClosedReport {
    /// The first day.
    pub from: String,
    /// The last day, included.
    pub to: String,
    /// The length of a period.
    pub bucket: FlowBucket,
    /// The periods, oldest first.
    pub items: Vec<CreatedClosedRow>,
    /// Tasks created over the whole range.
    pub created: u32,
    /// Closings over the whole range.
    pub closed: u32,
    /// Reopenings over the whole range.
    pub reopened: u32,
    /// Open tasks just before `from`.
    pub open_before: u32,
    /// The report as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

/// One band of the cumulative flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowBand {
    /// The band's key: the status name in lower case, or the category.
    pub key: String,
    /// The status's name as written; the category's name for categories.
    pub name: String,
    /// The status's category.
    pub category: Category,
    /// The status's colour; left out for categories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
}

/// The tasks in each band at the end of one period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CumulativeFlowRow {
    /// The period's first day.
    pub start: String,
    /// The period's last day, included.
    pub end: String,
    /// The number of tasks in each band, in the order of `bands`.
    pub counts: Vec<u32>,
}

/// How many tasks were in each status at the end of each period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CumulativeFlowReport {
    /// The first day.
    pub from: String,
    /// The last day, included.
    pub to: String,
    /// The length of a period.
    pub bucket: FlowBucket,
    /// What the bands are.
    pub group_by: FlowGroupBy,
    /// The bands in workflow order: by category, then by board order.
    pub bands: Vec<FlowBand>,
    /// The periods, oldest first.
    pub items: Vec<CumulativeFlowRow>,
    /// The report as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

/// Summary figures of a set of durations, in days (two decimals). The
/// percentiles interpolate between the nearest values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowStats {
    /// How many tasks the figures cover.
    pub count: u32,
    /// The mean; left out without tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    /// The median.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p50: Option<f64>,
    /// The 85th percentile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p85: Option<f64>,
    /// The 95th percentile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95: Option<f64>,
    /// The shortest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The longest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Cycle and lead time of the tasks closed in one period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CycleTimeRow {
    /// The period's first day.
    pub start: String,
    /// The period's last day, included.
    pub end: String,
    /// Cycle time of the tasks closed in the period that were in progress.
    pub cycle: FlowStats,
    /// Lead time of the tasks closed in the period.
    pub lead: FlowStats,
}

/// One task closed in the period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CycleTimeTask {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// When it was last closed (RFC 3339, UTC).
    pub closed_at: String,
    /// The day it was last closed, in the personal time zone.
    pub closed_day: String,
    /// Days from the first move into progress to the last close; left out
    /// when the task was never in progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_days: Option<f64>,
    /// Days from its creation to the last close.
    pub lead_days: f64,
    /// How often it was opened again after a close.
    pub reopened: u32,
}

/// Cycle and lead time of the tasks closed over a range of days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CycleTimeReport {
    /// The first day.
    pub from: String,
    /// The last day, included.
    pub to: String,
    /// The length of a period.
    pub bucket: FlowBucket,
    /// Cycle time over the whole range.
    pub cycle: FlowStats,
    /// Lead time over the whole range.
    pub lead: FlowStats,
    /// The periods, oldest first.
    pub items: Vec<CycleTimeRow>,
    /// The tasks, by closing time, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<CycleTimeTask>>,
    /// One line per task as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

op!(
    /// Tasks created against tasks closed.
    ReportCreatedClosed,
    "report.created_closed",
    FlowReportInput,
    CreatedClosedReport
);
op!(
    /// The cumulative flow by status.
    ReportCumulativeFlow,
    "report.cumulative_flow",
    FlowReportInput,
    CumulativeFlowReport
);
op!(
    /// Cycle and lead time.
    ReportCycleTime,
    "report.cycle_time",
    FlowReportInput,
    CycleTimeReport
);

pub(crate) const REPORT_CREATED_CLOSED: OpDescriptor = OpDescriptor::of::<ReportCreatedClosed>(
    Meta {
        summary: "Tasks created against tasks closed and reopened per day, week or month, with the open tasks at the end of each period; from the history, with optional CSV.",
        kind: OpKind::Query,
        safety: Safety::Read,
        exposure: Exposure::Data,
        cli: Some("report created-closed"),
        input_name: "FlowReportInput",
        output_name: "CreatedClosedReport",
    },
);

pub(crate) const REPORT_CUMULATIVE_FLOW: OpDescriptor = OpDescriptor::of::<ReportCumulativeFlow>(
    Meta {
        summary: "How many tasks were in each status (or category) at the end of each day, week or month; from the history, with optional CSV.",
        kind: OpKind::Query,
        safety: Safety::Read,
        exposure: Exposure::Data,
        cli: Some("report flow"),
        input_name: "FlowReportInput",
        output_name: "CumulativeFlowReport",
    },
);

pub(crate) const REPORT_CYCLE_TIME: OpDescriptor = OpDescriptor::of::<ReportCycleTime>(Meta {
    summary: "Cycle time (first move into progress to the last close) and lead time (creation to the last close) of the tasks closed over a range of days, with percentiles per period; optional task list and CSV.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("report cycle-time"),
    input_name: "FlowReportInput",
    output_name: "CycleTimeReport",
});
