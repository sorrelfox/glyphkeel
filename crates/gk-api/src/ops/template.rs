// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task templates and project templates.
//!
//! A task template belongs to a project and holds a task to start from:
//! title, description (with its checklist), type, priority, labels,
//! estimate, subtasks, and dates as days after the day it is used. A
//! project template belongs to the workspace and holds a project's set-up:
//! statuses, types (with their default due dates), labels, modules,
//! description, wiki pages, tasks and task templates.
//!
//! A template's tasks also keep their custom field values (by field name)
//! and the version they are planned for (by name, only while it is open).
//! A project template keeps the project's own custom fields and the rules
//! of its statuses and for agents; it keeps no versions.
//!
//! Templates name statuses, types, labels, custom fields and versions by
//! name. Using a task template in another project takes what that project
//! has by those names and leaves out the rest. Templates are named by their name (ignoring
//! case) or ID; a task template's name is looked up in `project`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use std::collections::BTreeMap;

use crate::ops::common::{Actor, Category, Glyph, Hue, Priority};
use crate::ops::field::CustomFieldKind;
use crate::ops::project::{Modules, Project};
use crate::ops::rules::{AgentRules, StatusRules};
use crate::ops::task::Task;

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_no_priority(p: &Priority) -> bool {
    *p == Priority::None
}

/// What a template makes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    /// A task, with its subtasks.
    #[default]
    Task,
    /// A project.
    Project,
}

/// A task to start from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskBlueprint {
    /// The title, 1 to 500 characters.
    pub title: String,
    /// The description (Markdown); its checklist items start unticked.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The type, by name; default the project's default type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// The priority; left out for none.
    #[serde(default, skip_serializing_if = "is_no_priority")]
    pub priority: Priority,
    /// Labels, by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// The estimate, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// The start date: this many days after the day the template is used
    /// (negative: before).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_in_days: Option<i32>,
    /// The due date: this many days after the day the template is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_in_days: Option<i32>,
    /// Custom field values by field name, as `task.create` takes them.
    /// Fields the project doesn't have are left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// The version it is planned for, by name; left out when the project
    /// has no open version by that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<String>,
    /// `active`: the task goes into the project's active sprint, when one
    /// is running as it is made. Without it the task is in no sprint (the
    /// backlog); a template never names a planned or closed sprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<String>,
    /// Its subtasks at every level, each after the one it sits under, in
    /// order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<SubtaskBlueprint>,
}

/// A subtask of a [`TaskBlueprint`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubtaskBlueprint {
    /// The title, 1 to 500 characters.
    pub title: String,
    /// The description (Markdown); its checklist items start unticked.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The type, by name; default the project's default type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// The priority; left out for none.
    #[serde(default, skip_serializing_if = "is_no_priority")]
    pub priority: Priority,
    /// Labels, by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// The estimate, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// The start date: days after the day the template is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_in_days: Option<i32>,
    /// The due date: days after the day the template is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_in_days: Option<i32>,
    /// Custom field values by field name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// The version it is planned for, by name, while it is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<String>,
    /// The subtask it sits under: its place in `subtasks` (from 0), an
    /// earlier one; left out for a subtask of the task itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<u32>,
}

/// A status of a project template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusBlueprint {
    /// Its name.
    pub name: String,
    /// What it means for the work in it.
    pub category: Category,
    /// Its colour.
    pub hue: Hue,
    /// Most tasks the column should hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<u32>,
    /// Fewest tasks the column should hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wip_min: Option<u32>,
    /// Its rules; custom fields are named as `cf:` and the ID their
    /// [`FieldBlueprint`] has in the template.
    #[serde(default, skip_serializing_if = "StatusRules::is_empty")]
    pub rules: StatusRules,
}

/// A task type of a project template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeBlueprint {
    /// Its name.
    pub name: String,
    /// Its shape.
    pub glyph: Glyph,
    /// Its colour.
    pub hue: Hue,
    /// Whether new tasks get it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_default: bool,
    /// New tasks of this type are due this many days after they are made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_offset_days: Option<u32>,
}

/// A custom field of a project template: one of the project's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldBlueprint {
    /// Its ID in the template, which the rules name; the new project's
    /// field gets a new one.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it holds.
    pub kind: CustomFieldKind,
    /// The options of a select field, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// The value new tasks get.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// The task types it shows for, by name; empty: every type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// Whether new tasks need a value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
}

/// A label of a project template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabelBlueprint {
    /// Its name.
    pub name: String,
    /// Its colour.
    pub hue: Hue,
}

/// A wiki page of a project template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageBlueprint {
    /// Its title.
    pub title: String,
    /// Its text (Markdown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    /// The title of the page it sits in; that page comes earlier in the
    /// list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// A task template inside a project template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedBlueprint {
    /// The task template's name.
    pub name: String,
    /// What it is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The task.
    pub task: TaskBlueprint,
}

/// A project's set-up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectBlueprint {
    /// The project's description (Markdown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Which parts are switched on; default all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<Modules>,
    /// The statuses, in board order; empty: the default statuses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<StatusBlueprint>,
    /// The task types, in order; empty: the default types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<TypeBlueprint>,
    /// The labels, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelBlueprint>,
    /// Wiki pages, parents before their children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wiki: Vec<PageBlueprint>,
    /// Tasks to start with; their dates count from the day the project is
    /// made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskBlueprint>,
    /// Task templates the new project gets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_templates: Vec<NamedBlueprint>,
    /// The project's own custom fields, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldBlueprint>,
    /// The rules for agents.
    #[serde(default, skip_serializing_if = "AgentRules::is_empty")]
    pub agent_rules: AgentRules,
}

/// A template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// The template's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it makes.
    pub kind: TemplateKind,
    /// The project's key, for a task template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// A task template's task, as `task` of `template.create` takes it
    /// (a task blueprint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<serde_json::Value>,
    /// A project template's set-up, as `setup` of `template.create` takes
    /// it (a project blueprint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<serde_json::Value>,
    /// Its place in the list.
    pub rank: String,
    /// Who made it.
    pub created_by: Actor,
    /// When it was made (RFC 3339, UTC).
    pub created_at: String,
    /// When it last changed (RFC 3339, UTC).
    pub updated_at: String,
    /// For a template that comes with Glyphkeel: its key (`software`). Such
    /// a template is in every workspace and can't be changed or deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_in: Option<String>,
}

/// One template in a list, without its content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateSummary {
    /// The template's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it makes.
    pub kind: TemplateKind,
    /// The project's key, for a task template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// A task template's title; a project template's number of tasks and
    /// pages is in `items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Tasks it makes (subtasks included), plus a project template's wiki
    /// pages.
    pub items: u32,
    /// For a template that comes with Glyphkeel: its key (`software`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_in: Option<String>,
}

/// Which templates to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateListInput {
    /// Task templates of this project (key or ID); left out: of every
    /// project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Only templates of this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TemplateKind>,
}

/// Templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateList {
    /// The templates: project templates first, then task templates by
    /// project, each in order.
    pub items: Vec<TemplateSummary>,
    /// How many.
    pub total: u64,
}

/// Names a template.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateRef {
    /// The template: its name or ID.
    pub template: String,
    /// For a task template named by name: its project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A new template.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateCreateInput {
    /// The name, 1 to 100 characters, unique among task templates of the
    /// project or among project templates, ignoring case.
    pub name: String,
    /// What it makes; default: a task, or what `from_task`/`from_project`
    /// names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TemplateKind>,
    /// For a task template: its project (key or ID); default the project
    /// of `from_task`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// What it is for, at most 1000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Make it from this task (key or ID), with its subtasks; dates become
    /// days after the day it was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_task: Option<String>,
    /// Make a project template from this project (key or ID): its
    /// statuses, types, labels, modules, description, wiki pages and task
    /// templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_project: Option<String>,
    /// With `from_project`: also its open top-level tasks with their
    /// subtasks (at most 500 tasks).
    #[serde(default)]
    pub include_tasks: bool,
    /// The task, for a task template not made from a task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskBlueprint>,
    /// The set-up, for a project template not made from a project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<ProjectBlueprint>,
}

/// Changes to a template; fields left out keep their value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateUpdateInput {
    /// The template: its name or ID.
    pub template: String,
    /// For a task template named by name: its project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A new description; empty removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A task template's new task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskBlueprint>,
    /// A project template's new set-up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<ProjectBlueprint>,
}

/// Uses a template.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateApplyInput {
    /// The template: its name or ID.
    pub template: String,
    /// A task template: the project the task goes to (default the
    /// template's), and where a template named by name is looked up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// A task template: the title instead of the template's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A task template: make it a subtask of this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// A task template: the status; default the project's first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The day the template's dates count from (a date: `2026-12-01`,
    /// `next mon`); default today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// With a task template: the new task's due date instead of the
    /// template's (a date: `2026-12-01`, or `fri`, `next mon`, `+3d`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// With a task template: the new task's priority instead of the
    /// template's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// A project template: the new project's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A project template: the new project's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// A project template: the parent project, for a sub-project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_project: Option<String>,
    /// A project template: the folder to put the new top-level project in
    /// (ID or name); not with `parent_project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// A project template: its colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of using the template twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// A template after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateChange {
    /// The template (before it was deleted, for a deletion).
    pub template: Template,
    /// Whether it was deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// What using a template made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateApplied {
    /// The new task, for a task template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
    /// The new project, for a project template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Project>,
    /// How many tasks were made, subtasks included.
    pub tasks: u32,
    /// Names of statuses, types and labels the template named that the
    /// project lacks; they were left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left_out: Vec<String>,
    /// The history group, for revert and Undo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

// ---------------------------------------------------------------------
// Operations

op!(
    /// Templates.
    TemplateListOp,
    "template.list",
    TemplateListInput,
    TemplateList
);
op!(
    /// One template.
    TemplateGet,
    "template.get",
    TemplateRef,
    Template
);
op!(
    /// Make a template.
    TemplateCreate,
    "template.create",
    TemplateCreateInput,
    TemplateChange
);
op!(
    /// Change a template.
    TemplateUpdate,
    "template.update",
    TemplateUpdateInput,
    TemplateChange
);
op!(
    /// Delete a template.
    TemplateDelete,
    "template.delete",
    TemplateRef,
    TemplateChange
);
op!(
    /// Use a template.
    TemplateApply,
    "template.apply",
    TemplateApplyInput,
    TemplateApplied
);

pub(crate) const TEMPLATE_LIST: OpDescriptor = OpDescriptor::of::<TemplateListOp>(Meta {
    summary: "The project templates and the task templates of a project or of every project.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("template list"),
    input_name: "TemplateListInput",
    output_name: "TemplateList",
});

pub(crate) const TEMPLATE_GET: OpDescriptor = OpDescriptor::of::<TemplateGet>(Meta {
    summary: "One template with its content.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("template show <template>"),
    input_name: "TemplateRef",
    output_name: "Template",
});

pub(crate) const TEMPLATE_CREATE: OpDescriptor = OpDescriptor::of::<TemplateCreate>(Meta {
    summary: "Make a task template (from a task, or given) or a project template (from a project, or given).",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("template create <name>"),
    input_name: "TemplateCreateInput",
    output_name: "TemplateChange",
});

pub(crate) const TEMPLATE_UPDATE: OpDescriptor = OpDescriptor::of::<TemplateUpdate>(Meta {
    summary: "Change a template's name, description or content.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("template edit <template>"),
    input_name: "TemplateUpdateInput",
    output_name: "TemplateChange",
});

pub(crate) const TEMPLATE_DELETE: OpDescriptor = OpDescriptor::of::<TemplateDelete>(Meta {
    summary: "Delete a template; it can be brought back with revert. What was made from it stays.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("template delete <template>"),
    input_name: "TemplateRef",
    output_name: "TemplateChange",
});

pub(crate) const TEMPLATE_APPLY: OpDescriptor = OpDescriptor::of::<TemplateApply>(Meta {
    summary: "Use a template: a task template makes a task with its subtasks; a project template makes a project with its set-up, pages and tasks.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("template use <template>"),
    input_name: "TemplateApplyInput",
    output_name: "TemplateApplied",
});
