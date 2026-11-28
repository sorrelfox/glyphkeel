// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Dashboards: named pages of widgets, for the whole workspace or pinned to
//! one project. A widget shows one figure, list or chart the engine already
//! computes elsewhere (a query's count or tasks, a flow report, the time
//! report, a sprint's burndown, versions' progress, the activity log, the
//! automation run log) or a Markdown note. Widgets are laid out in a grid,
//! in order, each one to three columns wide.
//!
//! Dashboards belong to the workspace: everyone using it sees the same
//! ones. Each is kept whole in history, so every change can be undone.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::error::OpError;
use crate::ops::automation::AutomationRun;
use crate::ops::common::{Actor, ActorKind, Hue, Priority};
use crate::ops::history::ActivityEntry;
use crate::ops::planning::{BurnupReport, EstimatesReport, PlanMeasure, VelocityReport};
use crate::ops::report::{CreatedClosedReport, CumulativeFlowReport, CycleTimeReport, FlowBucket};
use crate::ops::sprint::SprintBurndown;
use crate::ops::task::StatusRef;
use crate::ops::time::{TimeGroupBy, TimeReport};
use crate::ops::version::Version;

/// What a widget shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    /// How many tasks match a query.
    QueryCount,
    /// The first tasks matching a query.
    QueryList,
    /// Tasks created against tasks closed, per period.
    CreatedClosed,
    /// The cumulative flow by status.
    CumulativeFlow,
    /// Cycle and lead time.
    CycleTime,
    /// Logged time over the last days, grouped.
    TimeReport,
    /// The burndown of a project's active sprint.
    Burndown,
    /// The progress of a project's open versions.
    VersionProgress,
    /// The burn-up of a version, or of a project's active sprint.
    Burnup,
    /// The velocity of a project's completed sprints.
    Velocity,
    /// How closed tasks' logged time compared with their estimates.
    EstimateAccuracy,
    /// Open tasks counted by a field: status, priority, type, label or a
    /// custom select or yes/no field.
    TasksByField,
    /// Open tasks due in the next days.
    DueSoon,
    /// Open tasks past their due date.
    Overdue,
    /// The latest changes.
    Activity,
    /// Automation rules' runs that failed or were limited.
    AutomationFailures,
    /// A Markdown note.
    Note,
}

impl WidgetKind {
    /// Every kind, in the order the app offers them.
    pub const ALL: [Self; 17] = [
        Self::QueryCount,
        Self::QueryList,
        Self::TasksByField,
        Self::DueSoon,
        Self::Overdue,
        Self::CreatedClosed,
        Self::CumulativeFlow,
        Self::CycleTime,
        Self::TimeReport,
        Self::Burndown,
        Self::VersionProgress,
        Self::Burnup,
        Self::Velocity,
        Self::EstimateAccuracy,
        Self::Activity,
        Self::AutomationFailures,
        Self::Note,
    ];

    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::QueryCount => "query_count",
            Self::QueryList => "query_list",
            Self::CreatedClosed => "created_closed",
            Self::CumulativeFlow => "cumulative_flow",
            Self::CycleTime => "cycle_time",
            Self::TimeReport => "time_report",
            Self::Burndown => "burndown",
            Self::VersionProgress => "version_progress",
            Self::Burnup => "burnup",
            Self::Velocity => "velocity",
            Self::EstimateAccuracy => "estimate_accuracy",
            Self::TasksByField => "tasks_by_field",
            Self::DueSoon => "due_soon",
            Self::Overdue => "overdue",
            Self::Activity => "activity",
            Self::AutomationFailures => "automation_failures",
            Self::Note => "note",
        }
    }

    /// The settings a widget of this kind takes.
    #[must_use]
    pub const fn settings(self) -> &'static [WidgetSetting] {
        use WidgetSetting::{
            ActorKind, Bucket, Days, Field, GroupBy, Limit, Measure, Project, Query, Text, Version,
        };
        match self {
            Self::QueryCount => &[Query, Project],
            Self::QueryList => &[Query, Project, Limit],
            Self::CreatedClosed | Self::CumulativeFlow | Self::CycleTime => {
                &[Query, Project, Bucket]
            }
            Self::TimeReport => &[Project, Days, GroupBy],
            Self::Burndown | Self::VersionProgress => &[Project],
            Self::Burnup => &[Project, Version, Measure, Field],
            Self::Velocity => &[Project, Measure, Field],
            Self::EstimateAccuracy => &[Query, Project, Days],
            Self::TasksByField => &[Field, Query, Project],
            Self::DueSoon => &[Query, Project, Days, Limit],
            Self::Overdue => &[Query, Project, Limit],
            Self::Activity => &[Project, ActorKind, Days, Limit],
            Self::AutomationFailures => &[Project, Limit],
            Self::Note => &[Text],
        }
    }
}

/// One setting of a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetSetting {
    /// The query.
    Query,
    /// The project.
    Project,
    /// The field counted by.
    Field,
    /// How many rows.
    Limit,
    /// How many days.
    Days,
    /// The length of a report's periods.
    Bucket,
    /// The time report's grouping.
    GroupBy,
    /// Whose changes.
    ActorKind,
    /// The note's text.
    Text,
    /// How work is measured.
    Measure,
    /// The version.
    Version,
}

impl WidgetSetting {
    /// The name as written in input.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Project => "project",
            Self::Field => "field",
            Self::Limit => "limit",
            Self::Days => "days",
            Self::Bucket => "bucket",
            Self::GroupBy => "group_by",
            Self::ActorKind => "actor_kind",
            Self::Text => "text",
            Self::Measure => "measure",
            Self::Version => "version",
        }
    }
}

/// What a widget shows, beyond its kind. Each kind takes only some of
/// them (see `kind`); the others are refused. The same type is read and
/// written, so its optional fields are left out rather than `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetSettings {
    /// A task query (`is:open label:bug`): the tasks counted or listed, or
    /// only these tasks in a report, in `tasks_by_field`, `due_soon` and
    /// `overdue`. Relative dates in it are evaluated each time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub query: Option<String>,
    /// Only this project and its sub-projects (key or ID); on a project's
    /// dashboard, the dashboard's project when left out. `burndown` and
    /// `version_progress` need one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub project: Option<String>,
    /// `tasks_by_field`: `status`, `priority`, `type`, `label`, or a custom
    /// select or yes/no field (`cf.<name>`); `velocity` and `burnup` with
    /// `measure: field`: the number field (`cf.<name>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub field: Option<String>,
    /// How many tasks, changes or runs to list, 1 to 50; default 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "u32")]
    pub limit: Option<u32>,
    /// `due_soon`: due within this many days (default 7); `time_report`:
    /// the last this many days, today included (default 7); `activity`:
    /// only the changes of the last this many days; `estimate_accuracy`:
    /// tasks closed in the last this many days (default 90). 1 to 366.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "u32")]
    pub days: Option<u32>,
    /// Flow reports: the length of a period: twelve of them are shown
    /// (30 days, 12 weeks or 12 months); default `week`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "FlowBucket")]
    pub bucket: Option<FlowBucket>,
    /// `time_report`: how to group the time; default `project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TimeGroupBy")]
    pub group_by: Option<TimeGroupBy>,
    /// `activity`: only changes by the person (`user`), by agents
    /// (`agent`) or by automation rules (`automation`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ActorKind")]
    pub actor_kind: Option<ActorKind>,
    /// `note`: the note, in Markdown, up to 10,000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub text: Option<String>,
    /// `velocity` and `burnup`: `tasks` (default), `estimate` (hours) or
    /// `field`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "PlanMeasure")]
    pub measure: Option<PlanMeasure>,
    /// `burnup`: the version (name or ID); left out: the project's active
    /// sprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub version: Option<String>,
}

/// One widget of a dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Widget {
    /// Its ID, unique in the workspace.
    pub id: String,
    /// What it shows.
    pub kind: WidgetKind,
    /// Its title; left out for the kind's own title.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// How many of the grid's three columns it spans: 1 to 3.
    pub width: u8,
    /// What it shows, beyond its kind. A project is named by its key.
    pub settings: WidgetSettings,
}

/// A dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dashboard {
    /// Its name, unique among the dashboards of its project (or of the
    /// workspace).
    pub name: String,
    /// The key of the project it is pinned to; left out for a dashboard of
    /// the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Its widgets, in the order of the grid.
    pub widgets: Vec<Widget>,
    /// Its place among the dashboards.
    pub rank: String,
    /// Who made it.
    pub created_by: Actor,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// Its ID.
    pub id: String,
}

/// Which dashboards to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardListInput {
    /// The dashboards pinned to this project (key or ID); left out: the
    /// dashboards of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Every dashboard, of the workspace and of every project.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
}

/// Dashboards, in order: the workspace's first, then by project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardList {
    /// The dashboards.
    pub items: Vec<Dashboard>,
    /// How many.
    pub total: u64,
}

/// Which dashboard.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardRef {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID); left out for
    /// the workspace's dashboards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Which widget to compute.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardWidgetInput {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The widget: its ID, or its place in the grid (`1` is the first).
    pub widget: String,
}

/// A task as a widget lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetTask {
    /// Its key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its status.
    pub status: StatusRef,
    /// Its priority.
    pub priority: Priority,
    /// Its due date (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
}

/// One bar of `tasks_by_field`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetBar {
    /// What its tasks share: the status, type or label name, the priority
    /// (`urgent` … `none`), a custom field's option or `yes`/`no`; left out
    /// for the tasks without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Its colour, for statuses, types and labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// How many tasks.
    pub count: u64,
    /// The query that lists them; left out for the tasks without a value
    /// when no query can name them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// A widget with what it shows now. Only the fields of its kind are
/// given; a widget that can't be computed (its project is gone, its query
/// no longer valid) has `error` instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetValue {
    /// The widget.
    pub widget: Widget,
    /// Why it can't be shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OpError>,
    /// The query the figure or list comes from, in its normal form, with
    /// the project it is limited to (`project:TM/*`): open it with
    /// `task list --query`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// How many tasks match (`query_count`, `query_list`, `due_soon`,
    /// `overdue`, `tasks_by_field`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// The first matching tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<WidgetTask>>,
    /// `tasks_by_field`: the bars, in the field's order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars: Option<Vec<WidgetBar>>,
    /// `created_closed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_closed: Option<CreatedClosedReport>,
    /// `cumulative_flow`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_flow: Option<CumulativeFlowReport>,
    /// `cycle_time`, with the tasks closed in the period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_time: Option<CycleTimeReport>,
    /// `time_report`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeReport>,
    /// `burndown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burndown: Option<SprintBurndown>,
    /// `burnup`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burnup: Option<BurnupReport>,
    /// `velocity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity: Option<VelocityReport>,
    /// `estimate_accuracy`, grouped by type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimates: Option<EstimatesReport>,
    /// `version_progress`: the project's open and locked versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<Version>>,
    /// `activity`: the latest changes, newest first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Vec<ActivityEntry>>,
    /// `automation_failures`: the latest runs that failed or were limited,
    /// newest first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<AutomationRun>>,
    /// `note`: its Markdown text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A dashboard with what each widget shows now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardShow {
    /// The dashboard.
    pub dashboard: Dashboard,
    /// Its widgets with their values, in order.
    pub widgets: Vec<WidgetValue>,
}

/// A widget to add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetSpec {
    /// What it shows.
    pub kind: WidgetKind,
    /// Its title; default: the kind's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its width in grid columns, 1 to 3; default 1 (2 for charts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u8>,
    /// Its settings.
    #[serde(default)]
    pub settings: WidgetSettings,
}

/// A new dashboard.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardCreateInput {
    /// Its name, unique among the dashboards of its project (or of the
    /// workspace).
    pub name: String,
    /// The project to pin it to (key or ID); left out: the whole
    /// workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Start with the starter widgets: open tasks, overdue and due soon,
    /// open tasks by status, created against closed and the latest
    /// changes. Widgets given as well come after them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub starter: bool,
    /// Its widgets, in order; at most 40.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<WidgetSpec>,
}

/// A dashboard field that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DashboardField {
    /// The description.
    Description,
    /// The project it is pinned to: it then belongs to the whole workspace.
    PinTo,
}

/// Changes to a dashboard; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardUpdateInput {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Pins it to this project (key or ID); its widgets that name no
    /// project then look at that project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_to: Option<String>,
    /// Fields to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<DashboardField>,
}

/// A widget to add to a dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardAddWidgetInput {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The widget.
    pub widget: WidgetSpec,
    /// Its place in the grid (`1` is the first); default: the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
}

/// Changes to one widget; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardUpdateWidgetInput {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The widget: its ID, or its place in the grid (`1` is the first).
    pub widget: String,
    /// A new title; empty: the kind's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A new width in grid columns, 1 to 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u8>,
    /// Move it to this place in the grid (`1` is the first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// Settings to change; those left out keep their value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<WidgetSettings>,
    /// Settings to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<WidgetSetting>,
}

/// Which widget to remove.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardRemoveWidgetInput {
    /// The dashboard: its ID, or its name.
    pub dashboard: String,
    /// With a name: the project it is pinned to (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The widget: its ID, or its place in the grid (`1` is the first).
    pub widget: String,
}

/// A dashboard after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DashboardChange {
    /// The dashboard after the change (before it, for a deletion).
    pub dashboard: Dashboard,
    /// The widget added or changed, or the one removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<Widget>,
    /// Whether the dashboard was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// Dashboards.
    DashboardListOp,
    "dashboard.list",
    DashboardListInput,
    DashboardList
);
op!(
    /// One dashboard with every widget's value.
    DashboardShowOp,
    "dashboard.show",
    DashboardRef,
    DashboardShow
);
op!(
    /// One widget's value.
    DashboardWidgetOp,
    "dashboard.widget",
    DashboardWidgetInput,
    WidgetValue
);
op!(
    /// Makes a dashboard.
    DashboardCreate,
    "dashboard.create",
    DashboardCreateInput,
    DashboardChange
);
op!(
    /// Renames a dashboard or changes its description.
    DashboardUpdate,
    "dashboard.update",
    DashboardUpdateInput,
    DashboardChange
);
op!(
    /// Deletes a dashboard.
    DashboardDelete,
    "dashboard.delete",
    DashboardRef,
    DashboardChange
);
op!(
    /// Adds a widget.
    DashboardAddWidget,
    "dashboard.add_widget",
    DashboardAddWidgetInput,
    DashboardChange
);
op!(
    /// Changes, resizes or moves a widget.
    DashboardUpdateWidget,
    "dashboard.update_widget",
    DashboardUpdateWidgetInput,
    DashboardChange
);
op!(
    /// Removes a widget.
    DashboardRemoveWidget,
    "dashboard.remove_widget",
    DashboardRemoveWidgetInput,
    DashboardChange
);

pub(crate) const DASHBOARD_LIST: OpDescriptor = OpDescriptor::of::<DashboardListOp>(Meta {
    summary: "The dashboards of the whole workspace or of a project, with their widgets and settings.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("dashboard list"),
    input_name: "DashboardListInput",
    output_name: "DashboardList",
});

pub(crate) const DASHBOARD_SHOW: OpDescriptor = OpDescriptor::of::<DashboardShowOp>(Meta {
    summary: "One dashboard with what each of its widgets shows now: counts, tasks, report figures, burndown, versions, changes, failed automation runs and notes.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("dashboard show <dashboard>"),
    input_name: "DashboardRef",
    output_name: "DashboardShow",
});

pub(crate) const DASHBOARD_WIDGET: OpDescriptor = OpDescriptor::of::<DashboardWidgetOp>(Meta {
    summary: "What one widget of a dashboard shows now.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("dashboard widget <dashboard> <widget>"),
    input_name: "DashboardWidgetInput",
    output_name: "WidgetValue",
});

pub(crate) const DASHBOARD_CREATE: OpDescriptor = OpDescriptor::of::<DashboardCreate>(Meta {
    summary: "Make a dashboard for the whole workspace or pinned to a project, empty, with the starter widgets or with the widgets given.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("dashboard create"),
    input_name: "DashboardCreateInput",
    output_name: "DashboardChange",
});

pub(crate) const DASHBOARD_UPDATE: OpDescriptor = OpDescriptor::of::<DashboardUpdate>(Meta {
    summary: "Rename a dashboard or change its description.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("dashboard edit <dashboard>"),
    input_name: "DashboardUpdateInput",
    output_name: "DashboardChange",
});

pub(crate) const DASHBOARD_DELETE: OpDescriptor = OpDescriptor::of::<DashboardDelete>(Meta {
    summary: "Delete a dashboard; it can be brought back with revert.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("dashboard delete <dashboard>"),
    input_name: "DashboardRef",
    output_name: "DashboardChange",
});

pub(crate) const DASHBOARD_ADD_WIDGET: OpDescriptor =
    OpDescriptor::of::<DashboardAddWidget>(Meta {
        summary: "Add a widget to a dashboard, at the end or at a given place.",
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some("dashboard add-widget <dashboard>"),
        input_name: "DashboardAddWidgetInput",
        output_name: "DashboardChange",
    });

pub(crate) const DASHBOARD_UPDATE_WIDGET: OpDescriptor = OpDescriptor::of::<DashboardUpdateWidget>(
    Meta {
        summary: "Change a widget's title or settings, make it wider or narrower, or move it to another place in the grid.",
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some("dashboard edit-widget <dashboard> <widget>"),
        input_name: "DashboardUpdateWidgetInput",
        output_name: "DashboardChange",
    },
);

pub(crate) const DASHBOARD_REMOVE_WIDGET: OpDescriptor =
    OpDescriptor::of::<DashboardRemoveWidget>(Meta {
        summary: "Remove a widget from a dashboard.",
        kind: OpKind::Command,
        safety: Safety::Write,
        exposure: Exposure::Data,
        cli: Some("dashboard remove-widget <dashboard> <widget>"),
        input_name: "DashboardRemoveWidgetInput",
        output_name: "DashboardChange",
    });
