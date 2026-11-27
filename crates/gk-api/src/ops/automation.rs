// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Automation rules: when something happens to a task (the trigger) and the
//! task matches a query (the condition), the rule makes changes of its own
//! (the actions).
//!
//! A rule belongs to a project, or to the whole workspace. Its changes go
//! through the same rules as everyone's: status rules apply, and a rule
//! that an agent made or last changed, or that follows an agent's change,
//! is held to the rules for agents. History names the rule as the actor
//! (kind `automation`), in the same history group as the change that set it
//! off, so Undo takes both back together. Rules never call a model and
//! never reach the network.
//!
//! A rule's actions don't set off the same rule again, a chain of rules
//! setting each other off stops after [`AUTOMATION_DEPTH_MAX`] steps, and a
//! rule runs at most [`AUTOMATION_RUNS_PER_HOUR`] times an hour. Each rule
//! keeps a log of its last [`AUTOMATION_RUNS_KEPT`] runs.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OpError;
use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;
use crate::ops::recurrence::Weekday;
use crate::ops::task::RelationKind;

/// How many rules setting each other off one change can lead to.
pub const AUTOMATION_DEPTH_MAX: u32 = 5;
/// How often one rule may run in an hour; runs beyond are logged as
/// limited.
pub const AUTOMATION_RUNS_PER_HOUR: u32 = 100;
/// How many runs each rule's log keeps.
pub const AUTOMATION_RUNS_KEPT: usize = 50;
/// The most actions one rule has.
pub const AUTOMATION_ACTIONS_MAX: usize = 20;
/// The most tasks a scheduled or due date rule acts on in one run.
pub const AUTOMATION_TASKS_PER_RUN: usize = 200;

/// The task fields a `field_changed` trigger can watch, besides custom
/// fields (`cf:<id>`).
pub const WATCHABLE_FIELDS: &[&str] = &[
    "title",
    "description",
    "status",
    "type",
    "priority",
    "labels",
    "start_date",
    "due_date",
    "estimate",
    "progress",
    "fix_version",
    "parent",
];

/// The task fields a `set_fields` action can set, besides custom fields
/// (by name, query name or ID).
pub const AUTOMATION_SETTABLE_FIELDS: &[&str] = &[
    "title",
    "description",
    "type",
    "priority",
    "start_date",
    "due_date",
    "estimate",
    "progress",
    "fix_version",
];

/// What sets a rule off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// A task was created.
    TaskCreated,
    /// One of `fields` of a task changed (any watched field when none are
    /// named).
    FieldChanged,
    /// A task moved to `status` (any status when none is named).
    MovedToStatus,
    /// A comment was added to a task.
    CommentAdded,
    /// An open task's due date came: once per task and due date.
    DueDateReached,
    /// An open task's due date passed: once per task and due date.
    Overdue,
    /// At a time of day (`every: daily`) or of a week (`every: weekly` on
    /// `weekday`), for each task that matches the condition.
    Schedule,
}

impl TriggerKind {
    /// The name as stored.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaskCreated => "task_created",
            Self::FieldChanged => "field_changed",
            Self::MovedToStatus => "moved_to_status",
            Self::CommentAdded => "comment_added",
            Self::DueDateReached => "due_date_reached",
            Self::Overdue => "overdue",
            Self::Schedule => "schedule",
        }
    }
}

/// How often a scheduled rule runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleEvery {
    /// Every day at `time`.
    Daily,
    /// Every week on `weekday` at `time`.
    Weekly,
}

/// What sets a rule off, with what the kind needs. The same type is read
/// and written, so its optional fields are left out rather than `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationTrigger {
    /// The kind of event.
    pub on: TriggerKind,
    /// `field_changed`: the fields to watch (`status`, `priority`,
    /// `labels`, `due_date`, …, or a custom field as `cf:<id>`); none: any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    /// `moved_to_status`: the status, by name; none: any status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
    /// `schedule`: daily or weekly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ScheduleEvery")]
    pub every: Option<ScheduleEvery>,
    /// `schedule`: the time of day, `HH:MM`, in the workspace's time zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub time: Option<String>,
    /// `schedule` with `every: weekly`: the day of the week.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Weekday")]
    pub weekday: Option<Weekday>,
}

/// What an action does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Set task fields: `fields` maps a field to its value.
    SetFields,
    /// Add `label`.
    AddLabel,
    /// Take `label` off.
    RemoveLabel,
    /// Move the task to `status`.
    MoveToStatus,
    /// Add a comment with `text`.
    AddComment,
    /// Create a subtask titled `text`, from task template `template` when
    /// one is named.
    CreateSubtask,
    /// Create a task titled `text` in the same project, from task template
    /// `template` when one is named, linked to the task by `link`.
    CreateLinkedTask,
    /// Leave a notification with `text`; the app shows it with the
    /// reminders.
    Notify,
}

/// One thing a rule does. Texts may hold placeholders: `{key}`,
/// `{title}`, `{status}`, `{priority}`, `{due_date}`, `{project}`,
/// `{rule}`, `{actor}` (who made the change that set the rule off) and
/// `{today}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationAction {
    /// What the action does.
    pub kind: ActionKind,
    /// `set_fields`: field to value. `title`, `description`, `type`,
    /// `priority`, `start_date` and `due_date` (`YYYY-MM-DD`, `today`,
    /// `+3d`, `next mon`), `estimate` (`2h`), `progress` (0 to 100),
    /// `fix_version`, or a custom field by name, query name or ID; `null`
    /// clears a field.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    /// `add_label`, `remove_label`: the label, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub label: Option<String>,
    /// `move_to_status`: the status, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
    /// `add_comment`, `notify`: the text; `create_subtask`,
    /// `create_linked_task`: the new task's title (default the template's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub text: Option<String>,
    /// `create_subtask`, `create_linked_task`: a task template of the
    /// project, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub template: Option<String>,
    /// `create_linked_task`: how the link reads from the task the rule
    /// acts on; default `relates`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RelationKind")]
    pub link: Option<RelationKind>,
}

/// How a run of a rule ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    /// The actions were made.
    Done,
    /// An action failed; none of the run's changes were kept.
    Failed,
    /// The rule didn't run: too many runs this hour, or too long a chain
    /// of rules setting each other off.
    Limited,
}

/// One run of a rule, from its log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRun {
    /// The run's ID.
    pub id: String,
    /// The rule's ID.
    pub rule: String,
    /// The rule's name when it ran.
    pub rule_name: String,
    /// When it ran (RFC 3339, UTC).
    pub at: String,
    /// How it ended.
    pub result: RunResult,
    /// What set it off.
    pub trigger: TriggerKind,
    /// The task it acted on, by key; left out when it no longer exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The history group of its changes, when it made some.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// How many items it changed.
    pub changes: u32,
    /// How many rules came before it in the chain (0: set off by a change
    /// of a person, an agent or the schedule).
    pub depth: u32,
    /// Why it failed or was limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OpError>,
}

/// A rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRule {
    /// The rule's ID.
    pub id: String,
    /// Its name, unique in its project (or among the workspace's rules); it
    /// names the rule in history.
    pub name: String,
    /// The project, by key; left out for a rule of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Whether it runs.
    pub enabled: bool,
    /// What sets it off.
    pub trigger: AutomationTrigger,
    /// A query (TQL) the task must match; empty: every task.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
    /// What it does, in order.
    pub actions: Vec<AutomationAction>,
    /// Who made it.
    pub created_by: Actor,
    /// When it was made (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed (RFC 3339, UTC).
    pub updated_at: String,
    /// Whether an agent made or last changed it: its runs are then held to
    /// the rules for agents.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub held_as_agent: bool,
    /// Its latest run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<AutomationRun>,
    /// Its last runs, newest first; only `automation.get` fills it in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<AutomationRun>>,
}

/// Which rules to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationListInput {
    /// The project (key or ID): its rules and the workspace's; left out:
    /// every rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Rules, in the order they run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationList {
    /// The rules: the workspace's first, then by project.
    pub items: Vec<AutomationRule>,
    /// How many.
    pub total: u64,
}

/// Names one rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRef {
    /// The rule: its ID, or its name.
    pub rule: String,
    /// With a name: the project it belongs to (key or ID); left out, a
    /// rule of the workspace, or the only rule with that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A new rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationCreateInput {
    /// The project (key or ID); left out: a rule of the whole workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The name, 1 to 100 characters, unique in the project.
    pub name: String,
    /// Whether it runs; default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// What sets it off.
    pub trigger: AutomationTrigger,
    /// A query (TQL) the task must match; empty: every task. A scheduled
    /// rule needs one: it acts on every task that matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// What it does, 1 to 20 actions, in order.
    pub actions: Vec<AutomationAction>,
}

/// Changes to a rule; what is left out stays.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationUpdateInput {
    /// The rule: its ID, or its name.
    pub rule: String,
    /// With a name: the project it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Switch it on or off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// A new trigger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<AutomationTrigger>,
    /// A new condition; empty: every task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// New actions, replacing the old ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<AutomationAction>>,
}

/// A rule after a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationChange {
    /// The rule after the change (as it was, when deleted).
    pub rule: AutomationRule,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Which runs to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRunsInput {
    /// One rule's runs: its ID, or its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// The project: the runs of its rules and the workspace's (and, with
    /// `rule` as a name, where the rule is looked up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only runs that failed or were limited.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub problems_only: bool,
    /// At most this many, 1 to 200; default 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Runs, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRunList {
    /// The runs.
    pub items: Vec<AutomationRun>,
    /// How many there are, also beyond `limit`.
    pub total: u64,
}

/// A rule that isn't saved, for a dry run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationDraft {
    /// Its name, for placeholders and history; default "Test".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What sets it off; the dry run acts as if it had.
    pub trigger: AutomationTrigger,
    /// A query (TQL) the task must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// What it does.
    pub actions: Vec<AutomationAction>,
}

/// A dry run of a rule against one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationTestInput {
    /// The task: its key or ID.
    pub task: String,
    /// A saved rule: its ID, or its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// With `rule` as a name: the project it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A rule that isn't saved, instead of `rule`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<AutomationDraft>,
}

/// One change a dry run would make.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationTestChange {
    /// What changes: `task`, `comment`, `relation`, …
    pub entity: String,
    /// The task it belongs to, by key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The field, as history names it.
    pub field: String,
    /// Its value before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
    /// Its value after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Value>,
}

/// What a rule would do to a task; nothing was changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationTestResult {
    /// The task, by key.
    pub task: String,
    /// Whether the task matches the condition. When it doesn't, the rule
    /// wouldn't run and nothing else is filled in.
    pub matches: bool,
    /// Whether every action would work.
    pub ok: bool,
    /// The changes it would make, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<AutomationTestChange>,
    /// The notifications it would leave.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Why it would fail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OpError>,
    /// Which action would fail, counting from 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_action: Option<u32>,
}

/// Nothing to give: the scheduled run takes no input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationRunInput {}

/// What a scheduled run did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutomationTick {
    /// How many runs of rules it started.
    pub runs: u32,
}

op!(
    /// The automation rules of a project and the workspace.
    AutomationListOp,
    "automation.list",
    AutomationListInput,
    AutomationList
);
op!(
    /// One rule with its last runs.
    AutomationGet,
    "automation.get",
    AutomationRef,
    AutomationRule
);
op!(
    /// Make a rule.
    AutomationCreate,
    "automation.create",
    AutomationCreateInput,
    AutomationChange
);
op!(
    /// Change a rule.
    AutomationUpdate,
    "automation.update",
    AutomationUpdateInput,
    AutomationChange
);
op!(
    /// Delete a rule.
    AutomationDelete,
    "automation.delete",
    AutomationRef,
    AutomationChange
);
op!(
    /// The runs of rules.
    AutomationRunsOp,
    "automation.runs",
    AutomationRunsInput,
    AutomationRunList
);
op!(
    /// What a rule would do to a task.
    AutomationTest,
    "automation.test",
    AutomationTestInput,
    AutomationTestResult
);
op!(
    /// Run the rules whose time has come.
    AutomationRunOp,
    "automation.run",
    AutomationRunInput,
    AutomationTick
);

pub(crate) const AUTOMATION_LIST: OpDescriptor = OpDescriptor::of::<AutomationListOp>(Meta {
    summary: "The automation rules of a project and of the workspace, each with its trigger, condition, actions and latest run.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("automation list"),
    input_name: "AutomationListInput",
    output_name: "AutomationList",
});

pub(crate) const AUTOMATION_GET: OpDescriptor = OpDescriptor::of::<AutomationGet>(Meta {
    summary: "One automation rule with its last runs.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("automation show <rule>"),
    input_name: "AutomationRef",
    output_name: "AutomationRule",
});

pub(crate) const AUTOMATION_CREATE: OpDescriptor = OpDescriptor::of::<AutomationCreate>(Meta {
    summary: "Make an automation rule: when a trigger happens to a task that matches a query, set fields, change labels, move it, comment, create a subtask or linked task, or leave a notification.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("automation create"),
    input_name: "AutomationCreateInput",
    output_name: "AutomationChange",
});

pub(crate) const AUTOMATION_UPDATE: OpDescriptor = OpDescriptor::of::<AutomationUpdate>(Meta {
    summary: "Change an automation rule: its name, trigger, condition or actions, or switch it on or off.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("automation edit <rule>"),
    input_name: "AutomationUpdateInput",
    output_name: "AutomationChange",
});

pub(crate) const AUTOMATION_DELETE: OpDescriptor = OpDescriptor::of::<AutomationDelete>(Meta {
    summary: "Delete an automation rule; revert brings it back.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("automation delete <rule>"),
    input_name: "AutomationRef",
    output_name: "AutomationChange",
});

pub(crate) const AUTOMATION_RUNS: OpDescriptor = OpDescriptor::of::<AutomationRunsOp>(Meta {
    summary: "The run log of automation rules, newest first: what set each run off, the task, the changes and any error.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("automation runs"),
    input_name: "AutomationRunsInput",
    output_name: "AutomationRunList",
});

pub(crate) const AUTOMATION_TEST: OpDescriptor = OpDescriptor::of::<AutomationTest>(Meta {
    summary: "A dry run of an automation rule, saved or not, against one task: whether it matches and the changes the actions would make. Nothing is changed.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("automation test <task>"),
    input_name: "AutomationTestInput",
    output_name: "AutomationTestResult",
});

pub(crate) const AUTOMATION_RUN: OpDescriptor = OpDescriptor::of::<AutomationRunOp>(Meta {
    summary: "Run the scheduled and due date automation rules whose time has come; the app does this by itself.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "Housekeeping that the app runs on its own schedule; agents' changes already set rules off as they happen.",
    },
    cli: Some("automation run"),
    input_name: "AutomationRunInput",
    output_name: "AutomationTick",
});
