// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Planning reports, replayed from the tasks' history: the velocity of a
//! project's sprints, the burn-up of a version or a sprint, and how well
//! the time logged on closed tasks matched their estimates.
//!
//! Work is measured in tasks, in estimated hours, or in a number custom
//! field (story points, say), each task's value as it was at the moment
//! that counts. A task without the field's value counts as 0.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::sprint::SprintRef;
use crate::ops::version::VersionRef;

/// How work is measured.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanMeasure {
    /// Each task counts 1.
    #[default]
    Tasks,
    /// The task's estimate, in hours.
    Estimate,
    /// A number custom field, named in `field`.
    Field,
}

/// Which project's sprints the velocity covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VelocityInput {
    /// The project: a key or ID.
    pub project: String,
    /// `tasks` (default), `estimate` (hours) or `field`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<PlanMeasure>,
    /// With `measure: field`: a whole-number or decimal custom field (its
    /// name or `cf.<name>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// How many of the latest completed sprints to show, 1 to 50; default
    /// 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<u32>,
    /// Also return the report as CSV text (comma-separated, a header line,
    /// CRLF line ends).
    #[serde(default)]
    pub csv: bool,
}

/// One completed sprint: what it took on and what it finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VelocitySprint {
    /// The sprint.
    pub sprint: SprintRef,
    /// The day it was completed, in the personal time zone.
    pub completed_on: String,
    /// The work in it when it started that was still open then.
    pub committed: f64,
    /// The work in it at its end that was closed during it.
    pub completed: f64,
    /// How many tasks it took on at its start.
    pub committed_tasks: u32,
    /// How many tasks it completed.
    pub completed_tasks: u32,
    /// The mean of `completed` over this sprint and up to two before it.
    pub average: f64,
}

/// A sprint still to finish, with the work planned for it now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VelocityUpcoming {
    /// The sprint (active or planned).
    pub sprint: SprintRef,
    /// The work in it now, open or closed.
    pub planned: f64,
    /// How many tasks are in it now.
    pub tasks: u32,
}

/// The velocity of a project's completed sprints, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VelocityReport {
    /// The project's key.
    pub project: String,
    /// How work is measured.
    pub measure: PlanMeasure,
    /// With `measure: field`: the field's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The latest completed sprints, in the order they were completed.
    pub sprints: Vec<VelocitySprint>,
    /// The mean completed work of the last three completed sprints (fewer
    /// when there are fewer); left out without any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average: Option<f64>,
    /// How many sprints `average` covers: 0 to 3.
    pub average_of: u32,
    /// The active and planned sprints, with the work in them now.
    pub upcoming: Vec<VelocityUpcoming>,
    /// The report as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

/// Which version or sprint the burn-up covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BurnupInput {
    /// The version: its ID or name. Give this or `sprint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The sprint (active or closed): its ID, its name, or `active`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<String>,
    /// With a name or `active`: the project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// `tasks` (default), `estimate` (hours) or `field`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<PlanMeasure>,
    /// With `measure: field`: a whole-number or decimal custom field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Add the forecast; default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast: Option<bool>,
    /// Also return the days as CSV text.
    #[serde(default)]
    pub csv: bool,
}

/// One day of a burn-up, as it stood at the end of the day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BurnupDay {
    /// The day (`YYYY-MM-DD`).
    pub date: String,
    /// All the work planned for it.
    pub scope: f64,
    /// The closed part of it.
    pub completed: f64,
    /// How many tasks were planned for it.
    pub scope_tasks: u32,
    /// How many of them were closed.
    pub completed_tasks: u32,
}

/// Where the completed line is heading at its present pace: the straight
/// line that fits the days best, up to the day it meets the scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BurnupForecast {
    /// The day the line meets the scope of the last day.
    pub date: String,
    /// The line's value on the first day.
    pub start: f64,
    /// How much work it adds a day.
    pub per_day: f64,
}

/// Scope and completed work, day by day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BurnupReport {
    /// With `version`: the version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionRef>,
    /// With `sprint`: the sprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<SprintRef>,
    /// How work is measured.
    pub measure: PlanMeasure,
    /// With `measure: field`: the field's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The first day.
    pub from: String,
    /// The last day shown: the planned end, or today when that is earlier.
    pub to: String,
    /// The planned last day: the version's due date or the sprint's end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// The days, oldest first.
    pub days: Vec<BurnupDay>,
    /// The forecast; left out with fewer than three days, when no work is
    /// being completed, or when all of it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast: Option<BurnupForecast>,
    /// The days as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

/// How estimate accuracy is grouped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstimateGroupBy {
    /// One group of every task.
    #[default]
    None,
    /// By task type.
    Type,
    /// By label; a task with several labels is in each of their groups.
    Label,
    /// By project.
    Project,
    /// By the kind of work most of its time was logged as.
    WorkKind,
    /// By the month it was closed, for the trend.
    Month,
}

/// Which closed tasks to compare.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimatesInput {
    /// The first day tasks were closed on: `YYYY-MM-DD`, `today` or
    /// `yesterday`; default 89 days before `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The last day, included; default today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Only this project (key or ID) and its sub-projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only tasks that match this query, as they are now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// `none` (default), `type`, `label`, `project`, `work_kind` or
    /// `month`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<EstimateGroupBy>,
    /// Also list every task compared.
    #[serde(default)]
    pub tasks: bool,
    /// Also return the tasks as CSV text.
    #[serde(default)]
    pub csv: bool,
}

/// One closed task: its estimate against the time logged on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimateTask {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its type's name.
    #[serde(rename = "type")]
    pub task_type: String,
    /// The day it was closed, in the personal time zone.
    pub closed_on: String,
    /// Its estimate, in minutes.
    pub estimate_minutes: i64,
    /// The time logged on it, in minutes.
    pub spent_minutes: i64,
    /// Spent over estimate (two decimals): 1.5 took half as long again.
    pub ratio: f64,
}

/// The figures of one group of tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimateGroup {
    /// What its tasks share: a type, label, project key, kind of work or
    /// month (`YYYY-MM`); empty for every task, or for those without one.
    pub key: String,
    /// The name to show; empty like `key`.
    pub name: String,
    /// How many tasks.
    pub count: u32,
    /// Their estimates, in minutes.
    pub estimate_minutes: i64,
    /// The time logged on them, in minutes.
    pub spent_minutes: i64,
    /// The median ratio of spent to estimate (two decimals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_ratio: Option<f64>,
    /// The share of tasks within 20 % of their estimate (0 to 1, four
    /// decimals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_share: Option<f64>,
    /// How many took more than 20 % longer.
    pub over: u32,
    /// How many took more than 20 % less.
    pub under: u32,
    /// The ten furthest off, either way, the furthest first.
    pub outliers: Vec<EstimateTask>,
}

/// How closed tasks' logged time compared with their estimates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimatesReport {
    /// The first day.
    pub from: String,
    /// The last day, included.
    pub to: String,
    /// How the groups are made.
    pub group_by: EstimateGroupBy,
    /// Every task compared.
    pub total: EstimateGroup,
    /// The groups: by count, the largest first (months in order); empty
    /// with `group_by: none`.
    pub groups: Vec<EstimateGroup>,
    /// Tasks closed in the range without an estimate, left out.
    pub without_estimate: u32,
    /// Tasks closed in the range with an estimate but no time logged,
    /// left out.
    pub without_time: u32,
    /// The tasks compared, by closing day, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<EstimateTask>>,
    /// One line per task as CSV text, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

op!(
    /// Sprint velocity.
    ReportVelocity,
    "report.velocity",
    VelocityInput,
    VelocityReport
);
op!(
    /// A burn-up of a version or a sprint.
    ReportBurnup,
    "report.burnup",
    BurnupInput,
    BurnupReport
);
op!(
    /// Estimate accuracy.
    ReportEstimates,
    "report.estimates",
    EstimatesInput,
    EstimatesReport
);

pub(crate) const REPORT_VELOCITY: OpDescriptor = OpDescriptor::of::<ReportVelocity>(Meta {
    summary: "The velocity of a project's completed sprints: the work each took on at its start against the work it completed, in tasks, estimated hours or a number field, with the average of the last three and the work planned for the sprints to come.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("report velocity <project>"),
    input_name: "VelocityInput",
    output_name: "VelocityReport",
});

pub(crate) const REPORT_BURNUP: OpDescriptor = OpDescriptor::of::<ReportBurnup>(Meta {
    summary: "A burn-up of a version or a sprint: the work planned for it and the part completed at the end of each day, from the history, with a forecast of when the two meet.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("report burnup"),
    input_name: "BurnupInput",
    output_name: "BurnupReport",
});

pub(crate) const REPORT_ESTIMATES: OpDescriptor = OpDescriptor::of::<ReportEstimates>(Meta {
    summary: "Estimate accuracy: for tasks closed over a range of days with an estimate and logged time, the median ratio of spent to estimated time, the share within 20 %, and the outliers, overall and by type, label, project, kind of work or month.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("report estimates"),
    input_name: "EstimatesInput",
    output_name: "EstimatesReport",
});
