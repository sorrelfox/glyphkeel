// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The field registry: every field a query can filter or sort by, with its
//! aliases, type, operators and values. The query field's autocomplete,
//! the condition builder and the reference documentation are made from
//! this one list.

use crate::syntax::Op;

/// What a field holds, which decides its operators and values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldType {
    /// A task key.
    Key,
    /// A project key, `TM` or `TM/*`.
    Project,
    /// The title: `:` all words appear, `=` the exact title.
    Title,
    /// Text through the search index.
    FullText(TextScope),
    /// Status names.
    Status,
    /// Status categories.
    Category,
    /// Task type names.
    Type,
    /// The ordered priorities.
    Priority,
    /// Label names; `:` any of them.
    Label,
    /// The parent task, or `none`.
    Parent,
    /// Any ancestor task.
    Under,
    /// A link to another task, or `any`.
    Link(LinkField),
    /// A day without a time.
    Date(DateColumn),
    /// The days from the start date to the due date, which must overlap
    /// the given days.
    Span,
    /// A moment, compared by local days.
    Moment(MomentColumn),
    /// A duration in minutes.
    Duration(DurationColumn),
    /// A percentage.
    Percent,
    /// Who created the task: `me`, `agent` or an agent's name.
    Author,
    /// The agent holding a claim, or `any`.
    Claimed,
    /// How many of something.
    Count(CountOf),
    /// A flag from a fixed list.
    Is,
    /// Whether something is present.
    Has,
}

/// Which text a full-text field searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextScope {
    /// Title, description and comments.
    All,
    /// The description.
    Description,
    /// The comments.
    Comments,
}

/// Which link a link field follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkField {
    /// This task blocks the other.
    Blocks,
    /// The other task blocks this one.
    BlockedBy,
    /// The two are related.
    Relates,
    /// This task duplicates the other.
    Duplicates,
}

/// The day columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateColumn {
    /// The start date.
    Start,
    /// The due date.
    Due,
}

/// The moment columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MomentColumn {
    /// When the task was created.
    Created,
    /// When its own fields last changed.
    Edited,
    /// When anything about it last changed.
    Updated,
    /// When it was closed.
    Closed,
}

/// The duration columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationColumn {
    /// The estimate.
    Estimate,
    /// Logged time.
    Spent,
    /// Estimate minus logged time.
    Remaining,
}

/// What a count field counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CountOf {
    /// Comments.
    Comments,
    /// Attached files.
    Files,
    /// Subtasks.
    Subtasks,
}

/// One field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The field's name.
    pub name: &'static str,
    /// Other names for it.
    pub aliases: &'static [&'static str],
    /// What it holds.
    pub ty: FieldType,
    /// The operators it takes.
    pub ops: &'static [Op],
    /// Whether `a..b` ranges work.
    pub ranges: bool,
    /// Whether it can be a sort key.
    pub sortable: bool,
    /// The values it takes, for people (English; the reference and the
    /// autocomplete's hint).
    pub values: &'static str,
    /// Fixed values, for completion and checking; empty when names come
    /// from the workspace or the value is free.
    pub choices: &'static [&'static str],
}

const NAME_OPS: &[Op] = &[Op::Is, Op::Eq, Op::Ne];
const ORDERED_OPS: &[Op] = &[Op::Is, Op::Eq, Op::Ne, Op::Lt, Op::Le, Op::Gt, Op::Ge];
const NUMBER_OPS: &[Op] = &[Op::Is, Op::Eq, Op::Lt, Op::Le, Op::Gt, Op::Ge];
const TEXT_OPS: &[Op] = &[Op::Is];
const TITLE_OPS: &[Op] = &[Op::Is, Op::Eq, Op::Ne];
const ONE_OP: &[Op] = &[Op::Is];

/// The status categories with the two groups.
pub const CATEGORIES: &[&str] = &["todo", "in_progress", "done", "cancelled", "open", "closed"];
/// The priorities, lowest first.
pub const PRIORITIES: &[&str] = &["none", "low", "normal", "high", "urgent"];
/// The flags of `is:`.
pub const IS_FLAGS: &[&str] = &[
    "open",
    "closed",
    "overdue",
    "due-soon",
    "blocked",
    "claimed",
    "subtask",
    "parent",
    "unscheduled",
    "milestone",
    "over-estimate",
];
/// What `has:` checks.
pub const HAS_PARTS: &[&str] = &[
    "due",
    "start",
    "estimate",
    "description",
    "labels",
    "files",
    "comments",
    "subtasks",
    "parent",
    "links",
    "time",
    "reminder",
];

const fn field(
    name: &'static str,
    aliases: &'static [&'static str],
    ty: FieldType,
    ops: &'static [Op],
    ranges: bool,
    sortable: bool,
    values: &'static str,
) -> Field {
    Field {
        name,
        aliases,
        ty,
        ops,
        ranges,
        sortable,
        values,
        choices: &[],
    }
}

const fn with_choices(mut f: Field, choices: &'static [&'static str]) -> Field {
    f.choices = choices;
    f
}

/// Every field, in the order the condition builder lists them.
pub static FIELDS: &[Field] = &[
    field(
        "key",
        &[],
        FieldType::Key,
        NAME_OPS,
        false,
        true,
        "a task key such as TM-41",
    ),
    field(
        "project",
        &[],
        FieldType::Project,
        NAME_OPS,
        false,
        true,
        "a project key; TM/* adds its sub-projects",
    ),
    field(
        "title",
        &[],
        FieldType::Title,
        TITLE_OPS,
        false,
        true,
        "words in the title; = the exact title",
    ),
    field(
        "text",
        &[],
        FieldType::FullText(TextScope::All),
        TEXT_OPS,
        false,
        false,
        "words in the title, description or comments",
    ),
    field(
        "description",
        &[],
        FieldType::FullText(TextScope::Description),
        TEXT_OPS,
        false,
        false,
        "words in the description",
    ),
    field(
        "comment",
        &[],
        FieldType::FullText(TextScope::Comments),
        TEXT_OPS,
        false,
        false,
        "words in a comment",
    ),
    field(
        "status",
        &[],
        FieldType::Status,
        NAME_OPS,
        false,
        true,
        "status names",
    ),
    with_choices(
        field(
            "category",
            &["cat"],
            FieldType::Category,
            NAME_OPS,
            false,
            true,
            "todo, in_progress, done, cancelled; open, closed",
        ),
        CATEGORIES,
    ),
    field(
        "type",
        &[],
        FieldType::Type,
        NAME_OPS,
        false,
        true,
        "task type names",
    ),
    with_choices(
        field(
            "priority",
            &["prio"],
            FieldType::Priority,
            ORDERED_OPS,
            false,
            true,
            "none < low < normal < high < urgent",
        ),
        PRIORITIES,
    ),
    field(
        "label",
        &["labels"],
        FieldType::Label,
        NAME_OPS,
        false,
        false,
        "label names",
    ),
    field(
        "parent",
        &[],
        FieldType::Parent,
        NAME_OPS,
        false,
        true,
        "a task key, or none",
    ),
    field(
        "under",
        &[],
        FieldType::Under,
        ONE_OP,
        false,
        false,
        "a task key: its subtasks at any depth",
    ),
    field(
        "blocks",
        &[],
        FieldType::Link(LinkField::Blocks),
        ONE_OP,
        false,
        false,
        "a task key, or any",
    ),
    field(
        "blocked-by",
        &[],
        FieldType::Link(LinkField::BlockedBy),
        ONE_OP,
        false,
        false,
        "a task key, or any",
    ),
    field(
        "relates",
        &[],
        FieldType::Link(LinkField::Relates),
        ONE_OP,
        false,
        false,
        "a task key, or any",
    ),
    field(
        "duplicates",
        &[],
        FieldType::Link(LinkField::Duplicates),
        ONE_OP,
        false,
        false,
        "a task key, or any",
    ),
    field(
        "start",
        &[],
        FieldType::Date(DateColumn::Start),
        ORDERED_OPS,
        true,
        true,
        "a date, today, +7d, this-week, or none",
    ),
    field(
        "due",
        &[],
        FieldType::Date(DateColumn::Due),
        ORDERED_OPS,
        true,
        true,
        "a date, today, +7d, this-week, or none",
    ),
    field(
        "during",
        &[],
        FieldType::Span,
        ONE_OP,
        true,
        false,
        "a date, this-week, or a range such as 2026-11-01..2026-11-30: start to due overlaps it",
    ),
    field(
        "created",
        &[],
        FieldType::Moment(MomentColumn::Created),
        ORDERED_OPS,
        true,
        true,
        "a date, today, -7d, this-month",
    ),
    field(
        "edited",
        &[],
        FieldType::Moment(MomentColumn::Edited),
        ORDERED_OPS,
        true,
        true,
        "a date: the task's own fields changed",
    ),
    field(
        "updated",
        &[],
        FieldType::Moment(MomentColumn::Updated),
        ORDERED_OPS,
        true,
        true,
        "a date: anything about the task changed",
    ),
    field(
        "closed",
        &[],
        FieldType::Moment(MomentColumn::Closed),
        ORDERED_OPS,
        true,
        true,
        "a date, or none",
    ),
    field(
        "estimate",
        &[],
        FieldType::Duration(DurationColumn::Estimate),
        NUMBER_OPS,
        true,
        true,
        "a duration such as 90m, 1.5h, 1h30m, 2d, or none",
    ),
    field(
        "spent",
        &[],
        FieldType::Duration(DurationColumn::Spent),
        NUMBER_OPS,
        true,
        true,
        "a duration",
    ),
    field(
        "remaining",
        &[],
        FieldType::Duration(DurationColumn::Remaining),
        NUMBER_OPS,
        true,
        true,
        "a duration: estimate minus logged time",
    ),
    field(
        "progress",
        &[],
        FieldType::Percent,
        NUMBER_OPS,
        true,
        true,
        "a percentage, 50 or 50%",
    ),
    field(
        "author",
        &["creator"],
        FieldType::Author,
        NAME_OPS,
        false,
        true,
        "me, agent, or an agent's name",
    ),
    field(
        "claimed",
        &[],
        FieldType::Claimed,
        ONE_OP,
        false,
        false,
        "an agent's name, or any",
    ),
    field(
        "comments",
        &[],
        FieldType::Count(CountOf::Comments),
        NUMBER_OPS,
        true,
        true,
        "a number",
    ),
    field(
        "files",
        &[],
        FieldType::Count(CountOf::Files),
        NUMBER_OPS,
        true,
        true,
        "a number",
    ),
    field(
        "subtasks",
        &[],
        FieldType::Count(CountOf::Subtasks),
        NUMBER_OPS,
        true,
        true,
        "a number",
    ),
    with_choices(
        field("is", &[], FieldType::Is, ONE_OP, false, false, "a flag"),
        IS_FLAGS,
    ),
    with_choices(
        field(
            "has",
            &[],
            FieldType::Has,
            ONE_OP,
            false,
            false,
            "a part of the task",
        ),
        HAS_PARTS,
    ),
];

/// Sort keys that are not fields.
pub const EXTRA_SORT_KEYS: &[&str] = &["rank", "relevance"];

/// The field named `name` (case-insensitive), by name or alias.
#[must_use]
pub fn find(name: &str) -> Option<&'static Field> {
    let lower = name.to_ascii_lowercase();
    FIELDS
        .iter()
        .find(|f| f.name == lower || f.aliases.contains(&lower.as_str()))
}

/// Every name and alias, for suggestions.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    FIELDS
        .iter()
        .flat_map(|f| std::iter::once(f.name).chain(f.aliases.iter().copied()))
}

/// Every sort key: the sortable fields, then `rank` and `relevance`.
pub fn sort_keys() -> impl Iterator<Item = &'static str> {
    FIELDS
        .iter()
        .filter(|f| f.sortable)
        .map(|f| f.name)
        .chain(EXTRA_SORT_KEYS.iter().copied())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_and_readable_by_the_parser() {
        let mut seen = HashSet::new();
        for name in all_names().chain(EXTRA_SORT_KEYS.iter().copied()) {
            assert!(seen.insert(name), "{name} twice");
            assert!(name.starts_with(|c: char| c.is_ascii_lowercase()), "{name}");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{name}"
            );
        }
    }

    #[test]
    fn every_field_takes_colon() {
        for f in FIELDS {
            assert!(f.ops.contains(&Op::Is), "{}", f.name);
        }
    }
}
