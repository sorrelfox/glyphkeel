// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Custom fields: extra values on tasks, defined per project or for every
//! project.
//!
//! A field is named by its name (ignoring case) or ID. Queries name it as
//! `cf.<query name>` (`cf.story-points>3`), or by the query name alone when
//! no built-in field has it. Tasks get and change their values through
//! `task.create` and `task.update` (`fields`), and show them in `fields`.
//!
//! Values in inputs are JSON: text, Markdown, dates (`YYYY-MM-DD`),
//! options and links as strings; whole numbers and decimals as numbers;
//! yes/no as `true`/`false`; a multi-select field as a list of options.
//! Strings are also read for numbers and yes/no (`"3"`, `"yes"`), so the
//! command line can pass them as text. `null` clears a value.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Edge;

/// What a custom field holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldKind {
    /// One line of text, up to 1000 characters.
    #[default]
    Text,
    /// Formatted text (Markdown), up to 64 KB.
    Markdown,
    /// A whole number.
    Integer,
    /// A number with a fraction.
    Decimal,
    /// A day, `YYYY-MM-DD`.
    Date,
    /// Yes or no.
    Boolean,
    /// One option of the field's list.
    Select,
    /// Any number of options of the field's list.
    MultiSelect,
    /// A link with a scheme (`https://…`, `mailto:…`).
    Url,
}

/// A custom field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomField {
    /// The field's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// How queries name it: `cf.story-points`.
    pub query_name: String,
    /// What it holds.
    pub kind: CustomFieldKind,
    /// The project it belongs to (its key); left out for a field of every
    /// project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The options of a select or multi-select field, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// The value new tasks get; left out when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// The task types it is shown for, by name; empty: every type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// Whether tasks it is shown for must have a value.
    pub required: bool,
    /// Tasks with a value for it (not in the trash).
    pub tasks: u64,
}

/// The custom fields of a project, or of the whole workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldList {
    /// The fields in order: those of every project first, then the
    /// project's own.
    pub fields: Vec<CustomField>,
}

/// Which custom fields to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldListInput {
    /// The project: its key (current or old) or ID. Lists the fields its
    /// tasks have: its own and those of every project. Left out: every
    /// field of the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A custom field changed, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldChange {
    /// The field after the change; left out when it was deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<CustomField>,
    /// The saved views whose queries were rewritten because the field or
    /// one of its options was renamed or deleted, by name; left out when
    /// none was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views_rewritten: Vec<String>,
    /// How many tasks lost a value: of the deleted field, or of options
    /// taken off its list.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub values_removed: u64,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// A new custom field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldCreateInput {
    /// The project: its key (current or old) or ID. Left out: a field of
    /// every project. A project's sub-projects created later get a copy of
    /// its own fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The name, 1 to 50 characters with at least one letter or digit;
    /// unique among the fields the project's tasks have, ignoring case. A
    /// field of the same name in another project must hold the same kind.
    pub name: String,
    /// What it holds.
    pub kind: CustomFieldKind,
    /// The options of a select or multi-select field, in order (1 to 200,
    /// each 1 to 100 characters, unique ignoring case).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// The value new tasks get unless told otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Show it only for tasks of these types, by name; default: every type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// Tasks it is shown for must have a value: a new task needs one (or
    /// the default), and a value can't be cleared.
    #[serde(default)]
    pub required: bool,
    /// Place it before this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Place it after this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Place it first or last; default: last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
}

/// An option of a select field to rename.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldOptionRename {
    /// The option's current name (any case).
    pub from: String,
    /// Its new name.
    pub to: String,
}

/// A custom field setting that can be cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldSetting {
    /// The default value.
    Default,
    /// The task types: it shows for every type again.
    Types,
}

/// Changes to a custom field; settings left out keep their value. Its kind
/// can't change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldUpdateInput {
    /// The project whose fields to look in: its key (current or old) or
    /// ID; left out: the fields of every project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The field: its name or ID.
    pub field: String,
    /// A new name. Saved queries that use the field are rewritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Options to rename; tasks and saved queries keep up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rename_options: Vec<CustomFieldOptionRename>,
    /// The whole list of options after the renames, in order. Options left
    /// out are taken off every task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    /// A new default value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Show it only for tasks of these types, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    /// Whether tasks must have a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Move it before this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Move it after this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Move it first or last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Edge>,
    /// Settings to clear.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<CustomFieldSetting>,
}

/// A custom field to delete, with its values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldDeleteInput {
    /// The project whose fields to look in: its key (current or old) or
    /// ID; left out: the fields of every project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The field: its name or ID. Its values leave every task, and saved
    /// queries that use it lose those conditions.
    pub field: String,
    /// The number of tasks you expect to have a value for it; the deletion
    /// stops if the number differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_task_count: Option<u64>,
}

/// One custom field's value on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomFieldValue {
    /// The field's ID.
    pub id: String,
    /// Its name.
    pub name: String,
    /// How queries name it: `cf.story-points`.
    pub query_name: String,
    /// What it holds.
    pub kind: CustomFieldKind,
    /// The value: a string, a number, `true`/`false`, or a list of options
    /// for a multi-select field.
    pub value: serde_json::Value,
}

op!(
    /// The custom fields of a project or of the workspace.
    FieldListOp,
    "field.list",
    CustomFieldListInput,
    CustomFieldList
);
op!(
    /// A new custom field.
    FieldCreate,
    "field.create",
    CustomFieldCreateInput,
    CustomFieldChange
);
op!(
    /// Changes to a custom field.
    FieldUpdate,
    "field.update",
    CustomFieldUpdateInput,
    CustomFieldChange
);
op!(
    /// Deletes a custom field and its values.
    FieldDelete,
    "field.delete",
    CustomFieldDeleteInput,
    CustomFieldChange
);

const fn command(
    summary: &'static str,
    safety: Safety,
    cli: &'static str,
    input: &'static str,
) -> Meta {
    Meta {
        summary,
        kind: OpKind::Command,
        safety,
        exposure: Exposure::Data,
        cli: Some(cli),
        input_name: input,
        output_name: "CustomFieldChange",
    }
}

pub(crate) const FIELD_LIST: OpDescriptor = OpDescriptor::of::<FieldListOp>(Meta {
    summary: "The custom fields of a project (its own and those of every project) or of the whole workspace: name, query name, kind, options, default, task types, required, and how many tasks have a value.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("field list"),
    input_name: "CustomFieldListInput",
    output_name: "CustomFieldList",
});
pub(crate) const FIELD_CREATE: OpDescriptor = OpDescriptor::of::<FieldCreate>(command(
    "Add a custom field to a project, or to every project: text, Markdown, whole number, decimal, date, yes/no, select, multi-select or link; with options, a default, the task types it shows for, and whether it is required.",
    Safety::Write,
    "field add",
    "CustomFieldCreateInput",
));
pub(crate) const FIELD_UPDATE: OpDescriptor = OpDescriptor::of::<FieldUpdate>(command(
    "Rename a custom field, rename, add, reorder or remove its options, change its default, task types or whether it is required, or move it; saved queries keep up with renames.",
    Safety::Write,
    "field edit",
    "CustomFieldUpdateInput",
));
pub(crate) const FIELD_DELETE: OpDescriptor = OpDescriptor::of::<FieldDelete>(command(
    "Delete a custom field with its values on every task; saved queries lose their conditions on it. Can't be undone.",
    Safety::Permanent,
    "field delete",
    "CustomFieldDeleteInput",
));
