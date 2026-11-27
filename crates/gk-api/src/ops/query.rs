// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task queries: reading a query for the query field (tokens, chips,
//! errors) and listing the tasks that match one, with totals and paging.
//!
//! The query language is one text form for every filter: `is:open
//! due<+7d`, `label:bug,crash -status:Done`, `(type:Bug OR label:bug)
//! sort:due,-priority`, `"connection timeout"`, `TM-41`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::{Actor, Hue, Priority};
use crate::ops::task::{Checklist, Claim, LabelRef, StatusRef, TypeRef};
use crate::ops::version::VersionRef;

/// A query to read for the query field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryParseInput {
    /// The query text.
    pub query: String,
    /// Also return the field registry, for completion.
    #[serde(default)]
    pub fields: bool,
}

/// What a piece of the query text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryTokenKind {
    /// A field name.
    Field,
    /// An operator.
    Operator,
    /// A value.
    Value,
    /// A comma between values.
    Comma,
    /// `AND`, `OR` or `NOT`.
    Keyword,
    /// A leading `-`.
    Negation,
    /// A parenthesis.
    Paren,
    /// Free text.
    Text,
    /// A task key.
    Key,
    /// `sort:` and its keys.
    Sort,
    /// Text that could not be read.
    Error,
}

/// A range of the query text, in UTF-16 code units (as the page's editor
/// counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextRange {
    /// The first unit.
    pub from: u32,
    /// One past the last unit.
    pub to: u32,
}

/// A highlighted piece of the query text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryToken {
    /// What it is.
    pub kind: QueryTokenKind,
    /// Where it is.
    pub range: TextRange,
}

/// A mistake in a query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryProblem {
    /// The message in English.
    pub message: String,
    /// The message's ID in the message files, for translation.
    pub message_id: String,
    /// The message's arguments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// Where the mistake is.
    pub range: TextRange,
}

/// One chip of the condition builder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryChip {
    /// The field; left out for the free-text chip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The operator (`:`, `=`, `<`, `<=`, `>`, `>=`); left out for text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    /// The values, or the words and phrases of the text chip.
    pub values: Vec<String>,
    /// "is not" or "doesn't contain".
    pub negated: bool,
    /// The text the chip stands for; editing the chip replaces only these.
    pub ranges: Vec<TextRange>,
}

/// One field of the query language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryField {
    /// Its name.
    pub name: String,
    /// Other names for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// The operators it takes.
    pub operators: Vec<String>,
    /// Whether `a..b` ranges work.
    pub ranges: bool,
    /// Whether it can be a sort key.
    pub sortable: bool,
    /// The values it takes, in words.
    pub values: String,
    /// Its fixed values, when it has some.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

/// A query as the query field shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParsedQuery {
    /// Whether the query can run: it reads, fits the fields and names
    /// only what exists.
    pub valid: bool,
    /// The query in its normal form; left out when it doesn't read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    /// The highlighted pieces, in order.
    pub tokens: Vec<QueryToken>,
    /// The chips, when the query is a plain AND of conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chips: Option<Vec<QueryChip>>,
    /// The query has `OR` or parentheses: it is edited as text.
    pub advanced: bool,
    /// The sort keys of its `sort:`, `-` for descending.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<String>,
    /// The first mistake, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<QueryProblem>,
    /// The field registry, when asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<QueryField>>,
}

/// Which tasks to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSearchInput {
    /// The query, for example `is:open due<+7d sort:due`; empty lists every
    /// open and closed task of active projects.
    #[serde(default)]
    pub query: String,
    /// Only this project (key or ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// With `project`: its sub-projects' tasks too.
    #[serde(default)]
    pub include_subprojects: bool,
    /// The order when the query has no `sort:`, written like its keys
    /// (`due,-priority`). Default: by relevance for text searches, else by
    /// status, then place on the board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// Tasks per page, 1 to 200; default 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The `next_cursor` of the page before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Groups the rows: they come group by group (each group sorted by the
    /// query's order), and the result names the groups with their counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<ListGrouping>,
    /// Groups the rows by a custom field instead (`cf.<name>`): a select
    /// field by its option, in the field's order (a multi-select by its
    /// first option), a yes/no field by `yes` and `no` (no value counts as
    /// no). Not with `group_by`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by_field: Option<String>,
}

/// What a list's rows are grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListGrouping {
    /// The status, in board order.
    Status,
    /// The priority, highest first.
    Priority,
    /// The first label in alphabetical order; tasks without one last.
    Label,
    /// The parent task; tasks without one last.
    Parent,
    /// The week of the due date, earliest first; tasks without one last.
    DueWeek,
    /// The project.
    Project,
    /// The type, in the order of the project's types.
    Type,
}

/// One group of a grouped list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListGroup {
    /// What its tasks share: the status, type, label or project name, the
    /// priority (`urgent` … `none`), the parent's key, the first day of
    /// the due week (`YYYY-MM-DD`), or a custom field's option or `yes` or
    /// `no`; left out for the tasks without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The parent's title or the project's name, for grouping by parent or
    /// by project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The status's or label's colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<Hue>,
    /// Its tasks.
    pub count: u64,
}

/// A task as lists show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListedTask {
    /// The task's key.
    pub key: String,
    /// Its title.
    pub title: String,
    /// Its project's key.
    pub project: String,
    /// Its status.
    pub status: StatusRef,
    /// Its type.
    #[serde(rename = "type")]
    pub task_type: TypeRef,
    /// Its priority.
    pub priority: Priority,
    /// Its labels.
    pub labels: Vec<LabelRef>,
    /// Its custom field values by query name (`story-points`): a string,
    /// a number, `true`/`false`, or a list of options; fields without a
    /// value are left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
    /// The version it is planned for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_version: Option<VersionRef>,
    /// The sprint it is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<crate::ops::sprint::SprintRef>,
    /// The parent task's key, for a subtask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The start date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// The due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// The estimate in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_minutes: Option<i64>,
    /// Logged time in minutes.
    pub spent_minutes: i64,
    /// Estimate minus logged time, in minutes; only with an estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_minutes: Option<i64>,
    /// Its progress in percent.
    pub progress_percent: u8,
    /// The description's checklist.
    pub checklist: Checklist,
    /// Its subtasks: closed and all.
    pub subtask_count: Checklist,
    /// Whether a task that blocks it is still open.
    pub blocked: bool,
    /// How many comments it has.
    pub comment_count: u32,
    /// How many files are attached to it and to its comments.
    pub file_count: u32,
    /// The agent that claimed it, while the claim lasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// Whether it is the open occurrence of a recurring task.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recurring: bool,
    /// Its place in its status column.
    pub rank: String,
    /// Who created it.
    pub created_by: Actor,
    /// When it was created (RFC 3339, UTC).
    pub created_at: String,
    /// When anything about it last changed.
    pub updated_at: String,
    /// When it was closed; left out while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
    /// Its version, for `expected_version`.
    pub version: i64,
    /// The task's ID.
    pub id: String,
}

/// Totals over every matching task, not only the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTotals {
    /// Matching tasks.
    pub count: u64,
    /// Of them, open.
    pub open: u64,
    /// Their estimates added up, in minutes.
    pub estimate_minutes: i64,
    /// Their logged time added up, in minutes.
    pub spent_minutes: i64,
    /// Estimate minus logged time over the tasks with an estimate, in
    /// minutes.
    pub remaining_minutes: i64,
}

/// One page of matching tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSearchResult {
    /// The tasks of this page, in order.
    pub items: Vec<ListedTask>,
    /// How many tasks match.
    pub total: u64,
    /// Pass as `cursor` for the next page; left out on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// The query in its normal form.
    pub query: String,
    /// The order used, as sort keys (`-` for descending).
    pub sort: Vec<String>,
    /// Totals over every matching task.
    pub totals: ListTotals,
    /// With `group_by`: the groups, in the order the rows come.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ListGroup>,
}

op!(
    /// Reads a query for the query field.
    QueryParse,
    "query.parse",
    QueryParseInput,
    ParsedQuery
);
op!(
    /// The tasks matching a query.
    TaskSearch,
    "task.search",
    TaskSearchInput,
    TaskSearchResult
);

pub(crate) const QUERY_PARSE: OpDescriptor = OpDescriptor::of::<QueryParse>(Meta {
    summary: "Read a task query for the query field: highlighted pieces, the chips of the condition builder, the normal form and the first mistake with its place.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It serves the app's query field as the person types; agents and scripts run queries with task.search, which reports the same mistakes.",
    },
    cli: None,
    input_name: "QueryParseInput",
    output_name: "ParsedQuery",
});

pub(crate) const TASK_SEARCH: OpDescriptor = OpDescriptor::of::<TaskSearch>(Meta {
    summary: "List the tasks matching a query (is:open due<+7d label:bug sort:due), a page at a time, with totals of estimates and logged time.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("task list"),
    input_name: "TaskSearchInput",
    output_name: "TaskSearchResult",
});
