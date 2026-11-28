// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The field registry: every field a query can filter or sort by, with its
//! aliases, type, operators and values. The query field's autocomplete,
//! the condition builder and the reference documentation are made from
//! this one list. Custom fields come from the workspace and are listed
//! next to it ([`CustomField`]), as `cf.<name>`.

use gk_core::{FieldKind, fold};

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
    /// The names of versions (releases), or `none`.
    Version,
    /// The names of sprints, `active`, or `none`.
    Sprint,
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
    /// A value the task had at any time: `was:status:Review`.
    Was,
    /// A field that changed, at any time or within dates:
    /// `changed:due>-7d`.
    Changed,
    /// Who changed the task: `me`, `agent` or an agent's name.
    ChangedBy,
    /// Where the query's words are looked for: `in:files` only inside
    /// attached files, `-in:files` never there.
    In,
    /// Total slack on the critical path, in working days.
    Slack,
    /// How many days later a task ends than in a baseline.
    Slip,
    /// A baseline of the task's project, by name: the task is in it, and
    /// `slip` in the same query compares with it.
    Baseline,
}

/// Which text a full-text field searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextScope {
    /// Title, description, comments and the text inside the files attached
    /// to the task or its comments.
    All,
    /// Only the text inside attached files (`in:files`).
    Files,
    /// Title, description and comments, not attached files (`-in:files`).
    NoFiles,
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
    "critical",
];
/// What `has:` checks.
pub const HAS_PARTS: &[&str] = &[
    "due",
    "start",
    "estimate",
    "description",
    "labels",
    "version",
    "sprint",
    "files",
    "comments",
    "subtasks",
    "parent",
    "links",
    "time",
    "reminder",
    "attachment-text",
];
/// Where `in:` looks for the query's words.
pub const IN_PLACES: &[&str] = &["files"];

/// The fields `was:` looks up in the history.
pub const WAS_FIELDS: &[&str] = &[
    "status", "category", "type", "priority", "label", "version", "sprint",
];
/// The fields `changed:` looks up in the history.
pub const CHANGED_FIELDS: &[&str] = &[
    "status",
    "type",
    "priority",
    "title",
    "description",
    "start",
    "due",
    "estimate",
    "progress",
    "parent",
    "label",
    "version",
    "sprint",
    "project",
    "closed",
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
        "words in the title, description, comments or attached files",
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
        "label names, or none",
    ),
    field(
        "version",
        &["fix-version"],
        FieldType::Version,
        NAME_OPS,
        false,
        true,
        "version names, or none",
    ),
    field(
        "sprint",
        &["sprints"],
        FieldType::Sprint,
        NAME_OPS,
        false,
        true,
        "sprint names, active, or none",
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
        "slack",
        &[],
        FieldType::Slack,
        NUMBER_OPS,
        true,
        false,
        "working days of slack on the critical path, such as 2d; negative when late",
    ),
    field(
        "slip",
        &[],
        FieldType::Slip,
        NUMBER_OPS,
        true,
        false,
        "days later than in the newest baseline, or the one baseline: names, such as 3d or -1d",
    ),
    field(
        "baseline",
        &["baselines"],
        FieldType::Baseline,
        ONE_OP,
        false,
        false,
        "a baseline name of the task's project",
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
    with_choices(
        field(
            "was",
            &[],
            FieldType::Was,
            ONE_OP,
            false,
            false,
            "a field and a value the task had at any time: was:status:Review",
        ),
        WAS_FIELDS,
    ),
    with_choices(
        field(
            "changed",
            &[],
            FieldType::Changed,
            ONE_OP,
            false,
            false,
            "a field that changed; with dates, when: changed:due>-7d",
        ),
        CHANGED_FIELDS,
    ),
    with_choices(
        field(
            "in",
            &[],
            FieldType::In,
            ONE_OP,
            false,
            false,
            "where the words are looked for: in:files only inside attached files, -in:files never there",
        ),
        IN_PLACES,
    ),
    field(
        "changed-by",
        &[],
        FieldType::ChangedBy,
        NAME_OPS,
        false,
        false,
        "me, agent, or an agent's name: who changed the task's fields",
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

/// How a condition on the history starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum History {
    /// `was:<field>…`: the task had a value at any time.
    Was,
    /// `changed:<field>…`: the field changed, within dates.
    Changed,
}

impl History {
    /// How it is written, without the colon.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Was => "was",
            Self::Changed => "changed",
        }
    }
}

/// The history condition a field name starts, with the field it looks at:
/// `was:status` is `was` and `status`.
#[must_use]
pub fn split_history(written: &str) -> Option<(History, &str)> {
    let (head, inner) = written.split_once(':')?;
    let kind = if head.eq_ignore_ascii_case("was") {
        History::Was
    } else if head.eq_ignore_ascii_case("changed") {
        History::Changed
    } else {
        return None;
    };
    Some((kind, inner))
}

/// Whether `written` (any case) starts a history condition when an
/// operator follows it: `was` or `changed`.
#[must_use]
pub fn is_history_prefix(written: &str) -> bool {
    written.eq_ignore_ascii_case("was") || written.eq_ignore_ascii_case("changed")
}

/// The field whose values a condition holds: the field itself, or for
/// `was:status` the status. `changed:due` holds dates, not values of the
/// due field, so it has none.
#[must_use]
pub fn find_target(written: &str) -> Option<&'static Field> {
    match split_history(written) {
        Some((History::Was, inner)) => find(inner),
        Some((History::Changed, _)) => None,
        None => find(written),
    }
}

/// The history's name for the changes of a field (`due` is `due_date`), for
/// the fields whose changes the history keeps and queries can search.
#[must_use]
pub const fn history_key(ty: FieldType) -> Option<&'static str> {
    Some(match ty {
        FieldType::Status => "status",
        FieldType::Type => "type",
        FieldType::Priority => "priority",
        FieldType::Title => "title",
        FieldType::FullText(TextScope::Description) => "description",
        FieldType::Date(DateColumn::Start) => "start_date",
        FieldType::Date(DateColumn::Due) => "due_date",
        FieldType::Duration(DurationColumn::Estimate) => "estimate",
        FieldType::Percent => "progress",
        FieldType::Parent => "parent",
        FieldType::Label => "labels",
        FieldType::Version => "fix_version",
        FieldType::Sprint => "sprint",
        FieldType::Project => "project",
        FieldType::Moment(MomentColumn::Closed) => "closed_at",
        _ => return None,
    })
}

/// What custom fields are written with in queries: `cf.points>3`.
pub const CUSTOM_PREFIX: &str = "cf.";

/// A custom field as queries see it. Fields of several projects that have
/// the same query name are one field here: a condition on it looks at each
/// task's own field of that name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomField {
    /// Its name, as people wrote it.
    pub name: String,
    /// Its name in queries, after `cf.` (`story-points`).
    pub query_name: String,
    /// What it holds.
    pub kind: FieldKind,
    /// The options of a select field, of every project's field of this
    /// name.
    pub options: Vec<String>,
}

impl CustomField {
    /// How the field is written in queries: `cf.story-points`.
    #[must_use]
    pub fn written(&self) -> String {
        format!("{CUSTOM_PREFIX}{}", self.query_name)
    }

    /// The operators it takes.
    #[must_use]
    pub const fn ops(&self) -> &'static [Op] {
        match self.kind {
            FieldKind::Integer | FieldKind::Decimal | FieldKind::Date => ORDERED_OPS,
            FieldKind::Text | FieldKind::Markdown | FieldKind::Url => TITLE_OPS,
            FieldKind::Boolean | FieldKind::Select | FieldKind::MultiSelect => NAME_OPS,
        }
    }

    /// Whether `a..b` ranges work.
    #[must_use]
    pub const fn ranges(&self) -> bool {
        self.kind.is_ordered()
    }

    /// The values it takes, for people (English, as for built-in fields).
    #[must_use]
    pub const fn values(&self) -> &'static str {
        match self.kind {
            FieldKind::Text | FieldKind::Markdown => "words in the text; = the exact text; none",
            FieldKind::Url => "part of the link; = the exact link; none",
            FieldKind::Integer => "a whole number, a range such as 1..5, or none",
            FieldKind::Decimal => "a number such as 2.5, a range, or none",
            FieldKind::Date => "a date, today, +7d, this-week, or none",
            FieldKind::Boolean => "yes or no",
            FieldKind::Select | FieldKind::MultiSelect => "option names, or none",
        }
    }
}

/// The custom field a condition names: `cf.<query name>` (any case), or the
/// bare query name when no built-in field has it.
#[must_use]
pub fn find_custom<'a>(written: &str, custom: &'a [CustomField]) -> Option<&'a CustomField> {
    let (name, prefixed) = match strip_custom_prefix(written) {
        Some(rest) => (rest, true),
        None => (written, false),
    };
    if !prefixed && find(written).is_some() {
        return None;
    }
    let wanted = fold(name);
    custom.iter().find(|f| f.query_name == wanted)
}

/// The text after `cf.` (any case), if `written` starts with it.
#[must_use]
pub fn strip_custom_prefix(written: &str) -> Option<&str> {
    written
        .get(..CUSTOM_PREFIX.len())
        .filter(|p| p.eq_ignore_ascii_case(CUSTOM_PREFIX))
        .and_then(|_| written.get(CUSTOM_PREFIX.len()..))
}

/// The arguments of the message with each field's values in words
/// (`query-values-<name>`): the query's own words and examples, which stay
/// as they are in every language.
const VALUES_ARGS: &[(&str, &[(&str, &str)])] = &[
    ("key", &[("example", "TM-41")]),
    ("project", &[("example", "TM/*")]),
    ("title", &[("equals", "=")]),
    ("text", &[]),
    ("description", &[]),
    ("comment", &[]),
    ("status", &[]),
    (
        "category",
        &[
            ("categories", "todo, in_progress, done, cancelled"),
            ("groups", "open, closed"),
        ],
    ),
    ("type", &[]),
    (
        "priority",
        &[("order", "none < low < normal < high < urgent")],
    ),
    ("label", &[("none", "none")]),
    ("version", &[("none", "none")]),
    ("sprint", &[("active", "active"), ("none", "none")]),
    ("parent", &[("none", "none")]),
    ("under", &[]),
    ("blocks", &[("any", "any")]),
    ("blocked-by", &[("any", "any")]),
    ("relates", &[("any", "any")]),
    ("duplicates", &[("any", "any")]),
    (
        "start",
        &[
            ("today", "today"),
            ("ahead", "+7d"),
            ("week", "this-week"),
            ("none", "none"),
        ],
    ),
    (
        "due",
        &[
            ("today", "today"),
            ("ahead", "+7d"),
            ("week", "this-week"),
            ("none", "none"),
        ],
    ),
    (
        "during",
        &[
            ("week", "this-week"),
            ("range", "2026-11-01..2026-11-30"),
            ("start", "start"),
            ("due", "due"),
        ],
    ),
    (
        "created",
        &[("today", "today"), ("back", "-7d"), ("month", "this-month")],
    ),
    ("edited", &[]),
    ("updated", &[]),
    ("closed", &[("none", "none")]),
    (
        "estimate",
        &[("examples", "90m, 1.5h, 1h30m, 2d"), ("none", "none")],
    ),
    ("spent", &[]),
    ("remaining", &[]),
    ("progress", &[("plain", "50"), ("percent", "50%")]),
    ("slack", &[("example", "2d")]),
    (
        "slip",
        &[("baseline", "baseline:"), ("late", "3d"), ("early", "-1d")],
    ),
    ("baseline", &[]),
    ("author", &[("me", "me"), ("agent", "agent")]),
    ("claimed", &[("any", "any")]),
    ("comments", &[]),
    ("files", &[]),
    ("subtasks", &[]),
    ("is", &[]),
    ("has", &[]),
    ("was", &[("example", "was:status:Review")]),
    ("changed", &[("example", "changed:due>-7d")]),
    ("in", &[("only", "in:files"), ("never", "-in:files")]),
    ("changed-by", &[("me", "me"), ("agent", "agent")]),
];

/// The same for custom fields, by kind (`query-values-custom-<kind>`).
const CUSTOM_VALUES_ARGS: &[(&str, &[(&str, &str)])] = &[
    ("text", &[("equals", "="), ("none", "none")]),
    ("url", &[("equals", "="), ("none", "none")]),
    ("integer", &[("range", "1..5"), ("none", "none")]),
    ("decimal", &[("example", "2.5"), ("none", "none")]),
    (
        "date",
        &[
            ("today", "today"),
            ("ahead", "+7d"),
            ("week", "this-week"),
            ("none", "none"),
        ],
    ),
    ("boolean", &[("yes", "yes"), ("no", "no")]),
    ("select", &[("none", "none")]),
];

/// A message ID with its arguments, for the app to show in the person's
/// language.
pub type Message = (String, Vec<(&'static str, String)>);

fn args_of(
    table: &[(&str, &'static [(&'static str, &'static str)])],
    key: &str,
) -> Vec<(&'static str, String)> {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, args)| args.iter().map(|(n, v)| (*n, (*v).to_owned())).collect())
        .unwrap_or_default()
}

impl Field {
    /// The message with [`Field::values`] in words: its ID and arguments.
    #[must_use]
    pub fn values_message(&self) -> Message {
        (
            format!("query-values-{}", self.name),
            args_of(VALUES_ARGS, self.name),
        )
    }
}

impl CustomField {
    /// The message with [`CustomField::values`] in words: its ID and
    /// arguments.
    #[must_use]
    pub fn values_message(&self) -> Message {
        let kind = match self.kind {
            FieldKind::Text | FieldKind::Markdown => "text",
            FieldKind::Url => "url",
            FieldKind::Integer => "integer",
            FieldKind::Decimal => "decimal",
            FieldKind::Date => "date",
            FieldKind::Boolean => "boolean",
            FieldKind::Select | FieldKind::MultiSelect => "select",
        };
        (
            format!("query-values-custom-{kind}"),
            args_of(CUSTOM_VALUES_ARGS, kind),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_fields_values_have_a_message_with_the_same_english() {
        let check = |values: &str, (id, args): Message| {
            assert!(crate::error::pattern(&id).is_some(), "no message {id}");
            assert_eq!(crate::error::render(&id, &args), values, "{id}");
        };
        for f in FIELDS {
            check(f.values, f.values_message());
        }
        for kind in [
            FieldKind::Text,
            FieldKind::Markdown,
            FieldKind::Url,
            FieldKind::Integer,
            FieldKind::Decimal,
            FieldKind::Date,
            FieldKind::Boolean,
            FieldKind::Select,
            FieldKind::MultiSelect,
        ] {
            let f = CustomField {
                name: "Area".to_owned(),
                query_name: "area".to_owned(),
                kind,
                options: Vec::new(),
            };
            check(f.values(), f.values_message());
        }
        // Every message of the kind belongs to a field.
        let ids: Vec<String> = FIELDS
            .iter()
            .map(|f| f.values_message().0)
            .chain(
                CUSTOM_VALUES_ARGS
                    .iter()
                    .map(|(k, _)| format!("query-values-custom-{k}")),
            )
            .collect();
        for line in crate::error::MESSAGES_FTL.lines() {
            if let Some((id, _)) = line.split_once(" = ")
                && id.starts_with("query-values-")
            {
                assert!(ids.iter().any(|i| i == id), "{id} belongs to no field");
            }
        }
    }

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
    fn custom_fields_are_found_with_or_without_the_prefix() {
        let custom = vec![
            CustomField {
                name: "Story points".to_owned(),
                query_name: "story-points".to_owned(),
                kind: FieldKind::Integer,
                options: Vec::new(),
            },
            CustomField {
                name: "Due".to_owned(),
                query_name: "due".to_owned(),
                kind: FieldKind::Text,
                options: Vec::new(),
            },
        ];
        assert_eq!(
            find_custom("CF.Story-Points", &custom).map(|f| f.name.as_str()),
            Some("Story points")
        );
        assert_eq!(
            find_custom("story-points", &custom).map(|f| f.name.as_str()),
            Some("Story points")
        );
        assert_eq!(find_custom("due", &custom), None, "a built-in field first");
        assert_eq!(
            find_custom("cf.due", &custom).map(|f| f.name.as_str()),
            Some("Due")
        );
        assert_eq!(find_custom("cf.nothing", &custom), None);
    }

    #[test]
    fn history_fields_are_known_fields() {
        for name in WAS_FIELDS {
            assert!(find(name).is_some(), "{name}");
        }
        for name in CHANGED_FIELDS {
            let f = find(name).expect(name);
            assert!(history_key(f.ty).is_some(), "{name}");
        }
        assert_eq!(split_history("WAS:status"), Some((History::Was, "status")));
        assert_eq!(
            split_history("changed:due"),
            Some((History::Changed, "due"))
        );
        assert_eq!(split_history("status"), None);
        assert_eq!(find_target("was:prio").map(|f| f.name), Some("priority"));
        assert_eq!(find_target("changed:status"), None);
    }

    #[test]
    fn every_field_takes_colon() {
        for f in FIELDS {
            assert!(f.ops.contains(&Op::Is), "{}", f.name);
        }
    }
}
