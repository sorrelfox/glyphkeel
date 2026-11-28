// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Checking a parsed query against the field registry: every field exists,
//! every operator applies, every value fits its type. Relative dates are
//! evaluated here, against the calendar, so a checked query is tied to the
//! moment it runs.
//!
//! Names that live in the workspace (statuses, types, labels, projects,
//! task keys) are only collected here; the engine looks them up
//! ([`Checked::names`]).

use gk_core::{Day, DurationUnits, FieldKind, fold, parse_minutes};

use crate::dates::{Calendar, Days, parse_query_date};
use crate::error::{QueryError, closest};
use crate::fields::{
    self, CATEGORIES, CountOf, CustomField, DateColumn, DurationColumn, Field, FieldType,
    HAS_PARTS, History, IN_PLACES, IS_FLAGS, LinkField, MomentColumn, PRIORITIES, TextScope,
};
use crate::syntax::{Expr, Op, Predicate, Query, Span, Value, ValueKind};

/// A checked filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// All of these.
    And(Vec<Filter>),
    /// Any of these.
    Or(Vec<Filter>),
    /// Not this; tasks where the field is empty are included.
    Not(Box<Filter>),
    /// One condition.
    Cond(Cond),
}

/// Inclusive bounds on a number; `None` is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The smallest value that matches.
    pub lo: Option<i64>,
    /// The largest value that matches.
    pub hi: Option<i64>,
}

/// A task key in a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRef {
    /// The project key, upper case.
    pub project: String,
    /// The number.
    pub number: u32,
    /// Where it is written.
    pub span: Span,
}

impl KeyRef {
    /// `TM-41`.
    #[must_use]
    pub fn text(&self) -> String {
        format!("{}-{}", self.project, self.number)
    }
}

/// A project in a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRef {
    /// The key, upper case.
    pub key: String,
    /// With its sub-projects (`TM/*`).
    pub subtree: bool,
    /// Where it is written.
    pub span: Span,
}

/// A status, type or label name in a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRef {
    /// As written.
    pub text: String,
    /// Its matching key.
    pub folded: String,
    /// Where it is written.
    pub span: Span,
}

/// Who created a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorRef {
    /// The person.
    Me,
    /// Any agent.
    Agent,
    /// An agent by name.
    Named(String),
}

/// One checked condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    /// The task is one of these keys.
    Keys(Vec<KeyRef>),
    /// A bare key in the text: that task, if there is one.
    BareKey(KeyRef),
    /// In one of these projects.
    Projects(Vec<ProjectRef>),
    /// Every word appears in the title (folded words).
    TitleWords(Vec<String>),
    /// The title is exactly this (folded).
    TitleExact(String),
    /// Text through the search index.
    Text {
        /// Where to look.
        scope: TextScope,
        /// The word or phrase.
        text: String,
        /// A phrase in quotes rather than a word.
        phrase: bool,
    },
    /// In a status of one of these names.
    Statuses(Vec<NameRef>),
    /// In a status of one of these categories (database names).
    Categories(Vec<&'static str>),
    /// Of one of these types.
    Types(Vec<NameRef>),
    /// Has any of these labels, or (`none`) no label at all.
    Labels {
        /// The labels.
        names: Vec<NameRef>,
        /// Tasks without labels match.
        none: bool,
    },
    /// Planned for a version of one of these names, or (`none`) for none.
    Versions {
        /// The versions.
        names: Vec<NameRef>,
        /// Tasks without a version match.
        none: bool,
    },
    /// In a sprint of one of these names, (`active`) in its project's
    /// active sprint, or (`none`) in no sprint.
    Sprints {
        /// The sprints.
        names: Vec<NameRef>,
        /// Tasks in an active sprint match.
        active: bool,
        /// Tasks in no sprint match.
        none: bool,
    },
    /// Priority within any of these bounds (0 none to 4 urgent).
    Priority(Vec<Bounds>),
    /// A subtask of one of these tasks, or (`none`) of no task.
    Parent {
        /// The parents.
        keys: Vec<KeyRef>,
        /// Top-level tasks match.
        none: bool,
    },
    /// Below one of these tasks, at any depth.
    Under(Vec<KeyRef>),
    /// Linked to one of these tasks, or to any task when empty.
    Link {
        /// How.
        link: LinkField,
        /// The other tasks.
        keys: Vec<KeyRef>,
    },
    /// A day column within any of the bounds (days since 1970), or empty.
    Date {
        /// The column.
        column: DateColumn,
        /// The bounds.
        bounds: Vec<Bounds>,
        /// Empty matches.
        none: bool,
    },
    /// The days from the start date to the due date (either one when only
    /// one is set) overlap any of the bounds (days since 1970). Tasks
    /// without dates never match.
    Span(Vec<Bounds>),
    /// A moment column within any of the bounds (milliseconds), or empty.
    Moment {
        /// The column.
        column: MomentColumn,
        /// The bounds.
        bounds: Vec<Bounds>,
        /// Empty matches.
        none: bool,
    },
    /// A duration within any of the bounds (minutes), or empty.
    Duration {
        /// The column.
        column: DurationColumn,
        /// The bounds.
        bounds: Vec<Bounds>,
        /// Empty matches.
        none: bool,
    },
    /// Progress within any of the bounds (percent).
    Progress(Vec<Bounds>),
    /// A count within any of the bounds.
    Count(CountOf, Vec<Bounds>),
    /// Created by any of these.
    Author(Vec<AuthorRef>),
    /// Claimed by an agent: any (`None`) or one of these names.
    Claimed(Option<Vec<String>>),
    /// Any of these flags of `is:`.
    Is(Vec<&'static str>),
    /// Has any of these parts.
    Has(Vec<&'static str>),
    /// A condition on a custom field.
    Custom(CustomCond),
    /// The task has, or had at any time, one of these values.
    Was(WasTest),
    /// One of these fields (history names, such as `due_date`) changed
    /// after the task was created, at a moment within any of the bounds
    /// (milliseconds), or at any time when there are none.
    Changed {
        /// The fields, as the history names them.
        keys: Vec<&'static str>,
        /// Custom fields, by query name.
        custom: Vec<String>,
        /// When.
        at: Vec<Bounds>,
    },
    /// Any of these changed the task's fields after it was created.
    ChangedBy(Vec<AuthorRef>),
    /// `in:files`, where it is written. It never stays in a checked
    /// filter: checking turns it into the scope of the text terms.
    InFiles(Span),
    /// Total slack on the critical path within any of the bounds (working
    /// days).
    Slack(Vec<Bounds>),
    /// How many days later the task ends than in a baseline, within any of
    /// the bounds: the baseline named by the query's `baseline:`, else its
    /// project's newest.
    Slip {
        /// The bounds, in days.
        bounds: Vec<Bounds>,
        /// The baseline's folded name, from `baseline:`.
        baseline: Option<String>,
    },
    /// In a baseline of its project with one of these names.
    Baselines(Vec<NameRef>),
}

/// What `was:` looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasTest {
    /// Statuses of these names.
    Statuses(Vec<NameRef>),
    /// Statuses of these categories (database names).
    Categories(Vec<&'static str>),
    /// Types of these names.
    Types(Vec<NameRef>),
    /// These priorities, by name.
    Priorities(Vec<&'static str>),
    /// Labels of these names.
    Labels(Vec<NameRef>),
    /// Versions of these names.
    Versions(Vec<NameRef>),
    /// Sprints of these names.
    Sprints(Vec<NameRef>),
    /// A value of the custom field with this query name that passes the
    /// test.
    Custom {
        /// The field's query name.
        query_name: String,
        /// What the value must be.
        test: CustomTest,
    },
}

/// A condition on a custom field: the task's own field of that query name
/// passes the test, or (`none`) the task has no value for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCond {
    /// The field's query name.
    pub query_name: String,
    /// What a value must be.
    pub test: CustomTest,
    /// Tasks without a value match too.
    pub none: bool,
}

/// One bound of a decimal: the number as text and whether it is included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealBound {
    /// The number, as Rust writes an `f64`.
    pub value: String,
    /// Whether the bound itself matches.
    pub inclusive: bool,
}

/// Bounds on a decimal; `None` is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealBounds {
    /// The lower bound.
    pub lo: Option<RealBound>,
    /// The upper bound.
    pub hi: Option<RealBound>,
}

/// What a custom field's value must be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomTest {
    /// Nothing: only `none` matches.
    Nothing,
    /// Every task matches (`yes,no`).
    Anything,
    /// Contains any of these texts, ignoring case.
    Contains(Vec<String>),
    /// Is exactly one of these texts, ignoring case.
    Exact(Vec<String>),
    /// A whole number within any of the bounds.
    Integer(Vec<Bounds>),
    /// A decimal within any of the bounds.
    Decimal(Vec<RealBounds>),
    /// A day within any of the bounds (days since 1970).
    Date(Vec<Bounds>),
    /// Yes or no; a task without a value counts as no.
    Boolean(bool),
    /// Any of these options, as the field lists them.
    Options(Vec<String>),
}

/// What a sort key sorts by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortBy {
    /// A field.
    Field(&'static Field),
    /// A custom field, by its query name; several values (multi-select)
    /// sort by the first in order.
    Custom(String),
    /// The place on the board.
    Rank,
    /// How well the text terms match.
    Relevance,
}

/// One checked sort key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sort {
    /// What to sort by.
    pub by: SortBy,
    /// Descending.
    pub descending: bool,
}

/// The kinds of names that live in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameKind {
    /// A status.
    Status,
    /// A task type.
    Type,
    /// A label.
    Label,
    /// A version.
    Version,
    /// A sprint.
    Sprint,
    /// A project key.
    Project,
    /// A task key.
    Task,
}

/// A name the engine must find in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameUse {
    /// What kind of name.
    pub kind: NameKind,
    /// As written (a task key as `TM-41`, a project key upper case).
    pub text: String,
    /// Where it is written.
    pub span: Span,
}

/// A query checked against the registry, ready for SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The filter; `None` matches every task.
    pub filter: Option<Filter>,
    /// The sort keys the query gave; empty when it gave none.
    pub sort: Vec<Sort>,
    /// Today, in the workspace's time zone, when the query was checked.
    pub today: Day,
}

impl Checked {
    /// Whether the filter searches text outside a negation, which allows
    /// sorting by relevance.
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.filter
            .as_ref()
            .is_some_and(|f| positive_text(f, false))
    }

    /// The text terms outside negations.
    #[must_use]
    pub fn text_terms(&self) -> Vec<(TextScope, String, bool)> {
        let mut out = Vec::new();
        if let Some(f) = &self.filter {
            collect_text(f, false, &mut out);
        }
        out
    }

    /// The projects named outside negations: tasks of archived projects
    /// match only when the query names their project.
    #[must_use]
    pub fn named_projects(&self) -> Vec<ProjectRef> {
        let mut out = Vec::new();
        if let Some(f) = &self.filter {
            collect_projects(f, false, &mut out);
        }
        out
    }

    /// Whether the filter looks at the critical path (`is:critical`,
    /// `slack`), which the engine works out and hands to
    /// [`crate::Compiled::provide_schedule`].
    #[must_use]
    pub fn uses_schedule(&self) -> bool {
        fn uses(f: &Filter) -> bool {
            match f {
                Filter::And(items) | Filter::Or(items) => items.iter().any(uses),
                Filter::Not(inner) => uses(inner),
                Filter::Cond(Cond::Slack(_)) => true,
                Filter::Cond(Cond::Is(flags)) => flags.contains(&"critical"),
                Filter::Cond(_) => false,
            }
        }
        self.filter.as_ref().is_some_and(uses)
    }

    /// Every name the engine must find in the workspace.
    #[must_use]
    pub fn names(&self) -> Vec<NameUse> {
        let mut out = Vec::new();
        if let Some(f) = &self.filter {
            collect_names(f, &mut out);
        }
        out
    }
}

fn positive_text(f: &Filter, negated: bool) -> bool {
    match f {
        Filter::And(items) | Filter::Or(items) => items.iter().any(|i| positive_text(i, negated)),
        Filter::Not(inner) => positive_text(inner, !negated),
        Filter::Cond(Cond::Text { .. }) => !negated,
        Filter::Cond(_) => false,
    }
}

fn collect_text(f: &Filter, negated: bool, out: &mut Vec<(TextScope, String, bool)>) {
    match f {
        Filter::And(items) | Filter::Or(items) => {
            for i in items {
                collect_text(i, negated, out);
            }
        }
        Filter::Not(inner) => collect_text(inner, !negated, out),
        Filter::Cond(Cond::Text {
            scope,
            text,
            phrase,
        }) if !negated => out.push((*scope, text.clone(), *phrase)),
        Filter::Cond(_) => {}
    }
}

fn collect_projects(f: &Filter, negated: bool, out: &mut Vec<ProjectRef>) {
    match f {
        Filter::And(items) | Filter::Or(items) => {
            for i in items {
                collect_projects(i, negated, out);
            }
        }
        Filter::Not(inner) => collect_projects(inner, !negated, out),
        Filter::Cond(Cond::Projects(p)) if !negated => out.extend(p.iter().cloned()),
        Filter::Cond(_) => {}
    }
}

fn collect_names(f: &Filter, out: &mut Vec<NameUse>) {
    let names = |kind, refs: &[NameRef], out: &mut Vec<NameUse>| {
        out.extend(refs.iter().map(|n| NameUse {
            kind,
            text: n.text.clone(),
            span: n.span,
        }));
    };
    let keys = |refs: &[KeyRef], out: &mut Vec<NameUse>| {
        out.extend(refs.iter().map(|k| NameUse {
            kind: NameKind::Task,
            text: k.text(),
            span: k.span,
        }));
    };
    match f {
        Filter::And(items) | Filter::Or(items) => {
            for i in items {
                collect_names(i, out);
            }
        }
        Filter::Not(inner) => collect_names(inner, out),
        Filter::Cond(c) => match c {
            Cond::Statuses(n) => names(NameKind::Status, n, out),
            Cond::Types(n) => names(NameKind::Type, n, out),
            Cond::Labels { names: n, .. } => names(NameKind::Label, n, out),
            Cond::Versions { names: n, .. } => names(NameKind::Version, n, out),
            Cond::Sprints { names: n, .. } => names(NameKind::Sprint, n, out),
            Cond::Was(w) => match w {
                WasTest::Statuses(n) => names(NameKind::Status, n, out),
                WasTest::Types(n) => names(NameKind::Type, n, out),
                WasTest::Labels(n) => names(NameKind::Label, n, out),
                WasTest::Versions(n) => names(NameKind::Version, n, out),
                WasTest::Sprints(n) => names(NameKind::Sprint, n, out),
                WasTest::Categories(_) | WasTest::Priorities(_) | WasTest::Custom { .. } => {}
            },
            Cond::Projects(p) => out.extend(p.iter().map(|p| NameUse {
                kind: NameKind::Project,
                text: p.key.clone(),
                span: p.span,
            })),
            Cond::Keys(k) | Cond::Under(k) => keys(k, out),
            Cond::Parent { keys: k, .. } | Cond::Link { keys: k, .. } => keys(k, out),
            _ => {}
        },
    }
}

/// What the checker needs besides the query.
#[derive(Clone, Copy)]
pub struct CheckEnv<'a> {
    /// Today and the local days.
    pub calendar: &'a dyn Calendar,
    /// The working day and week, for durations.
    pub units: DurationUnits,
    /// The workspace's custom fields, one per query name.
    pub custom: &'a [CustomField],
}

impl std::fmt::Debug for CheckEnv<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckEnv")
            .field("today", &self.calendar.today())
            .field("units", &self.units)
            .field("custom", &self.custom.len())
            .finish()
    }
}

/// Checks `query`.
///
/// # Errors
/// The first field, operator or value that doesn't fit.
pub fn check(query: &Query, env: CheckEnv<'_>) -> Result<Checked, QueryError> {
    let mut filter = query.filter.as_ref().map(|e| expr(e, env)).transpose()?;
    if let Some(f) = &mut filter {
        bind_baseline(f);
    }
    let filter = match filter {
        Some(f) => scope_files(f)?,
        None => None,
    };
    let mut sort = Vec::new();
    if let Some(clause) = &query.sort {
        for key in &clause.keys {
            let lower = key.field.to_ascii_lowercase();
            let by = match lower.as_str() {
                "rank" => SortBy::Rank,
                "relevance" => SortBy::Relevance,
                _ => match fields::find(&lower).filter(|f| f.sortable) {
                    Some(f) if fields::strip_custom_prefix(&key.field).is_none() => {
                        SortBy::Field(f)
                    }
                    _ => {
                        if let Some(c) = fields::find_custom(&key.field, env.custom) {
                            sort.push(Sort {
                                by: SortBy::Custom(c.query_name.clone()),
                                descending: key.descending,
                            });
                            continue;
                        }
                        let custom: Vec<String> =
                            env.custom.iter().map(CustomField::written).collect();
                        let mut keys: Vec<&str> = fields::sort_keys().collect();
                        keys.extend(custom.iter().map(String::as_str));
                        let suggestion = closest(&lower, keys.iter().copied()).map(str::to_owned);
                        return Err(match suggestion {
                            Some(s) => QueryError::new("query-sort-unknown", key.span)
                                .arg("field", key.field.clone())
                                .arg("suggestion", s),
                            None => QueryError::new("query-sort-unknown-plain", key.span)
                                .arg("field", key.field.clone())
                                .arg("choices", keys.join(", ")),
                        });
                    }
                },
            };
            sort.push(Sort {
                by,
                descending: key.descending,
            });
        }
    }
    let checked = Checked {
        filter,
        sort,
        today: env.calendar.today(),
    };
    if !checked.has_text()
        && let Some(clause) = &query.sort
        && let Some(key) = clause
            .keys
            .iter()
            .find(|k| k.field.eq_ignore_ascii_case("relevance"))
    {
        return Err(QueryError::new("query-sort-relevance", key.span));
    }
    Ok(checked)
}

/// Where `in:files` and `-in:files` send the query's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilesIn {
    Only,
    Never,
}

/// Takes `in:files` and `-in:files` out of `f` and applies them to every
/// plain word and phrase: they say where the whole query's words are
/// looked for, so they can't be one side of an `OR`. Only the `-` written
/// on it counts: `-(word -in:files)` is the tasks without `word` outside
/// files. Returns `None` when nothing else is left.
fn scope_files(f: Filter) -> Result<Option<Filter>, QueryError> {
    let mut found = None;
    let f = take_in(f, false, &mut found)?;
    let Some(place) = found else {
        return Ok(f);
    };
    let scope = match place {
        FilesIn::Only => TextScope::Files,
        FilesIn::Never => TextScope::NoFiles,
    };
    Ok(f.map(|f| rescope(f, scope)))
}

fn take_in(
    f: Filter,
    in_or: bool,
    found: &mut Option<FilesIn>,
) -> Result<Option<Filter>, QueryError> {
    let place = match &f {
        Filter::Cond(Cond::InFiles(span)) => Some((*span, FilesIn::Only)),
        Filter::Not(inner) => match inner.as_ref() {
            Filter::Cond(Cond::InFiles(span)) => Some((*span, FilesIn::Never)),
            _ => None,
        },
        _ => None,
    };
    if let Some((span, place)) = place {
        if in_or {
            return Err(QueryError::new("query-in-files-or", span));
        }
        *found = Some(place);
        return Ok(None);
    }
    Ok(match f {
        Filter::And(items) => {
            let mut kept = Vec::new();
            for i in items {
                kept.extend(take_in(i, in_or, found)?);
            }
            match kept.len() {
                0 => None,
                1 => kept.pop(),
                _ => Some(Filter::And(kept)),
            }
        }
        Filter::Or(items) => {
            let mut kept = Vec::new();
            for i in items {
                kept.extend(take_in(i, true, found)?);
            }
            Some(Filter::Or(kept))
        }
        Filter::Not(inner) => take_in(*inner, in_or, found)?.map(|i| Filter::Not(Box::new(i))),
        other => Some(other),
    })
}

fn rescope(f: Filter, scope: TextScope) -> Filter {
    match f {
        Filter::And(items) => Filter::And(items.into_iter().map(|i| rescope(i, scope)).collect()),
        Filter::Or(items) => Filter::Or(items.into_iter().map(|i| rescope(i, scope)).collect()),
        Filter::Not(inner) => Filter::Not(Box::new(rescope(*inner, scope))),
        Filter::Cond(Cond::Text {
            scope: TextScope::All,
            text,
            phrase,
        }) => Filter::Cond(Cond::Text {
            scope,
            text,
            phrase,
        }),
        other => other,
    }
}

fn expr(e: &Expr, env: CheckEnv<'_>) -> Result<Filter, QueryError> {
    Ok(match e {
        Expr::And(items) => Filter::And(
            items
                .iter()
                .map(|i| expr(i, env))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Or(items) => Filter::Or(
            items
                .iter()
                .map(|i| expr(i, env))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Not(inner, _, _) => Filter::Not(Box::new(expr(inner, env)?)),
        Expr::Group(inner, _) => expr(inner, env)?,
        Expr::Text(t) => Filter::Cond(Cond::Text {
            scope: TextScope::All,
            text: t.text.clone(),
            phrase: t.quoted,
        }),
        Expr::Key(k) => Filter::Cond(Cond::BareKey(KeyRef {
            project: k.project.clone(),
            number: k.number,
            span: k.span,
        })),
        Expr::Pred(p) => predicate(p, env)?,
    })
}

fn usage(field: &Field) -> String {
    field
        .ops
        .iter()
        .filter(|op| **op != Op::Eq || matches!(field.ty, FieldType::Title))
        .map(|op| format!("{}{}", field.name, op.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn predicate(p: &Predicate, env: CheckEnv<'_>) -> Result<Filter, QueryError> {
    if let Some((kind, inner)) = fields::split_history(&p.field) {
        return history_predicate(p, kind, inner, env);
    }
    let prefixed = fields::strip_custom_prefix(&p.field).is_some();
    if let Some(custom) = fields::find_custom(&p.field, env.custom) {
        return custom_predicate(p, custom, env);
    }
    let custom_names: Vec<String> = env.custom.iter().map(CustomField::written).collect();
    if prefixed {
        return Err(unknown_custom(&p.field, p.field_span, env));
    }
    let Some(field) = fields::find(&p.field) else {
        let mut names: Vec<&str> = fields::all_names().collect();
        names.extend(custom_names.iter().map(String::as_str));
        let suggestion = closest(&p.field, names).map(str::to_owned);
        return Err(match suggestion {
            Some(s) => QueryError::new("query-unknown-field", p.field_span)
                .arg("field", p.field.clone())
                .arg("suggestion", s),
            None => QueryError::new("query-unknown-field-plain", p.field_span)
                .arg("field", p.field.clone()),
        });
    };
    if !field.ops.contains(&p.op) {
        return Err(if p.op == Op::Eq {
            QueryError::new("query-equals-text", p.op_span).arg("field", field.name)
        } else {
            QueryError::new("query-operator", p.op_span)
                .arg("op", p.op.as_str())
                .arg("field", field.name)
                .arg("usage", usage(field))
        });
    }
    for v in &p.values {
        if let ValueKind::Range(..) = v.kind {
            if !field.ranges {
                return Err(QueryError::new("query-no-range", v.span)
                    .arg("field", field.name)
                    .arg("usage", usage(field)));
            }
            if !matches!(p.op, Op::Is | Op::Eq) {
                return Err(
                    QueryError::new("query-range-operator", v.span).arg("field", field.name)
                );
            }
        }
        if let ValueKind::Word {
            text,
            quoted: false,
        } = &v.kind
            && text.eq_ignore_ascii_case("none")
            && !matches!(p.op, Op::Is | Op::Eq | Op::Ne)
        {
            return Err(QueryError::new("query-none-operator", v.span));
        }
    }
    // `!=` is "not `:`", and includes tasks where the field is empty.
    let (op, negate) = match p.op {
        Op::Ne => (Op::Is, true),
        Op::Eq if !matches!(field.ty, FieldType::Title) => (Op::Is, false),
        op => (op, false),
    };
    let cond = condition(field, op, &p.values, env)?;
    Ok(if negate {
        Filter::Not(Box::new(cond))
    } else {
        cond
    })
}

/// The error for `cf.<name>` naming no custom field.
fn unknown_custom(written: &str, span: Span, env: CheckEnv<'_>) -> QueryError {
    let custom_names: Vec<String> = env.custom.iter().map(CustomField::written).collect();
    let names = custom_names.iter().map(String::as_str);
    match closest(written, names) {
        Some(s) => QueryError::new("query-unknown-custom-field", span)
            .arg("field", written)
            .arg("suggestion", s),
        None if custom_names.is_empty() => {
            QueryError::new("query-no-custom-fields", span).arg("field", written)
        }
        None => QueryError::new("query-unknown-custom-field-plain", span)
            .arg("field", written)
            .arg("choices", custom_names.join(", ")),
    }
}

fn custom_usage(field: &CustomField) -> String {
    field
        .ops()
        .iter()
        .map(|op| format!("{}{}", field.written(), op.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn custom_predicate(
    p: &Predicate,
    field: &CustomField,
    env: CheckEnv<'_>,
) -> Result<Filter, QueryError> {
    let name = field.written();
    if !field.ops().contains(&p.op) {
        return Err(QueryError::new("query-operator", p.op_span)
            .arg("op", p.op.as_str())
            .arg("field", name)
            .arg("usage", custom_usage(field)));
    }
    for v in &p.values {
        if let ValueKind::Range(..) = v.kind {
            if !field.ranges() {
                return Err(QueryError::new("query-no-range", v.span)
                    .arg("field", name)
                    .arg("usage", custom_usage(field)));
            }
            if !matches!(p.op, Op::Is | Op::Eq) {
                return Err(QueryError::new("query-range-operator", v.span).arg("field", name));
            }
        }
        if is_none(v) && !matches!(p.op, Op::Is | Op::Eq | Op::Ne) {
            return Err(QueryError::new("query-none-operator", v.span));
        }
    }
    let (op, negate) = match p.op {
        Op::Ne => (Op::Is, true),
        op => (op, false),
    };
    let exact = p.op == Op::Eq;
    let none = p.values.iter().any(is_none);
    let values: Vec<&Value> = p.values.iter().filter(|v| !is_none(v)).collect();
    let test = if values.is_empty() {
        CustomTest::Nothing
    } else {
        custom_test(field, op, exact, &values, env)?
    };
    let cond = Filter::Cond(Cond::Custom(CustomCond {
        query_name: field.query_name.clone(),
        test,
        none,
    }));
    Ok(if negate {
        Filter::Not(Box::new(cond))
    } else {
        cond
    })
}

fn custom_test(
    field: &CustomField,
    op: Op,
    exact: bool,
    values: &[&Value],
    env: CheckEnv<'_>,
) -> Result<CustomTest, QueryError> {
    let name = field.written();
    Ok(match field.kind {
        FieldKind::Text | FieldKind::Markdown | FieldKind::Url => {
            let texts = values
                .iter()
                .map(|v| {
                    let text = v.text();
                    if text.trim().is_empty() {
                        Err(QueryError::new("query-empty-value", v.span).arg("field", name.clone()))
                    } else {
                        Ok(text)
                    }
                })
                .collect::<Result<_, _>>()?;
            if exact {
                CustomTest::Exact(texts)
            } else {
                CustomTest::Contains(texts)
            }
        }
        FieldKind::Integer => CustomTest::Integer(
            values
                .iter()
                .map(|v| {
                    let one = |t: &str| {
                        t.trim().parse::<i64>().map_err(|_| {
                            QueryError::new("query-bad-number", v.span).arg("value", t)
                        })
                    };
                    let (lo, hi) = match &v.kind {
                        ValueKind::Word { text, .. } => (one(text)?, one(text)?),
                        ValueKind::Range(a, b) => (one(a)?, one(b)?),
                    };
                    Ok(bounds(op, lo, hi))
                })
                .collect::<Result<_, QueryError>>()?,
        ),
        FieldKind::Decimal => CustomTest::Decimal(
            values
                .iter()
                .map(|v| {
                    let one = |t: &str| match t.trim().parse::<f64>() {
                        Ok(n) if n.is_finite() => Ok(n.to_string()),
                        _ => Err(QueryError::new("query-bad-decimal", v.span).arg("value", t)),
                    };
                    let (lo, hi) = match &v.kind {
                        ValueKind::Word { text, .. } => (one(text)?, one(text)?),
                        ValueKind::Range(a, b) => (one(a)?, one(b)?),
                    };
                    let bound = |value: String, inclusive| Some(RealBound { value, inclusive });
                    Ok(match op {
                        Op::Lt => RealBounds {
                            lo: None,
                            hi: bound(lo, false),
                        },
                        Op::Le => RealBounds {
                            lo: None,
                            hi: bound(hi, true),
                        },
                        Op::Gt => RealBounds {
                            lo: bound(hi, false),
                            hi: None,
                        },
                        Op::Ge => RealBounds {
                            lo: bound(lo, true),
                            hi: None,
                        },
                        Op::Is | Op::Eq | Op::Ne => RealBounds {
                            lo: bound(lo, true),
                            hi: bound(hi, true),
                        },
                    })
                })
                .collect::<Result<_, QueryError>>()?,
        ),
        FieldKind::Date => CustomTest::Date(
            values
                .iter()
                .map(|v| {
                    days_of(v, env).map(|(first, last)| {
                        bounds(op, first.days_since_epoch(), last.days_since_epoch())
                    })
                })
                .collect::<Result<_, _>>()?,
        ),
        FieldKind::Boolean => {
            let mut wanted = None;
            for v in values {
                let b = gk_core::custom_field::parse_bool(word(v)).ok_or_else(|| {
                    QueryError::new("query-bad-choice", v.span)
                        .arg("value", word(v))
                        .arg("field", name.clone())
                        .arg("choices", "yes, no")
                })?;
                if wanted.is_some_and(|w| w != b) {
                    // `yes,no` matches every task.
                    return Ok(CustomTest::Anything);
                }
                wanted = Some(b);
            }
            CustomTest::Boolean(wanted.unwrap_or(true))
        }
        FieldKind::Select | FieldKind::MultiSelect => {
            let mut out: Vec<String> = Vec::new();
            for v in values {
                let wanted = fold(&v.text());
                let Some(option) = field.options.iter().find(|o| fold(o) == wanted) else {
                    let text = v.text();
                    return Err(
                        match closest(&text, field.options.iter().map(String::as_str)) {
                            Some(s) => QueryError::new("query-bad-choice-suggest", v.span)
                                .arg("value", text)
                                .arg("field", name.clone())
                                .arg("suggestion", s),
                            None => QueryError::new("query-bad-choice", v.span)
                                .arg("value", text)
                                .arg("field", name.clone())
                                .arg("choices", field.options.join(", ")),
                        },
                    );
                };
                if !out.contains(option) {
                    out.push(option.clone());
                }
            }
            CustomTest::Options(out)
        }
    })
}

fn word(v: &Value) -> &str {
    match &v.kind {
        ValueKind::Word { text, .. } => text,
        ValueKind::Range(a, _) => a,
    }
}

fn is_none(v: &Value) -> bool {
    matches!(&v.kind, ValueKind::Word { text, quoted: false } if text.eq_ignore_ascii_case("none"))
}

/// The word `active` (not quoted): a sprint that is running.
fn is_active(v: &Value) -> bool {
    matches!(&v.kind, ValueKind::Word { text, quoted: false } if text.eq_ignore_ascii_case("active"))
}

fn is_any(v: &Value) -> bool {
    matches!(&v.kind, ValueKind::Word { text, quoted: false } if text.eq_ignore_ascii_case("any"))
}

/// The bounds `op` gives a value covering `lo..=hi`.
fn bounds(op: Op, lo: i64, hi: i64) -> Bounds {
    match op {
        Op::Lt => Bounds {
            lo: None,
            hi: Some(lo.saturating_sub(1)),
        },
        Op::Le => Bounds {
            lo: None,
            hi: Some(hi),
        },
        Op::Gt => Bounds {
            lo: Some(hi.saturating_add(1)),
            hi: None,
        },
        Op::Ge => Bounds {
            lo: Some(lo),
            hi: None,
        },
        Op::Is | Op::Eq | Op::Ne => Bounds {
            lo: Some(lo),
            hi: Some(hi),
        },
    }
}

fn key_ref(v: &Value) -> Result<KeyRef, QueryError> {
    let text = word(v).to_ascii_uppercase();
    crate::parse::task_key(&text)
        .map(|(project, number)| KeyRef {
            project,
            number,
            span: v.span,
        })
        .ok_or_else(|| QueryError::new("query-bad-key", v.span).arg("value", word(v)))
}

fn name_ref(v: &Value, field: &Field) -> Result<NameRef, QueryError> {
    let text = v.text();
    let folded = fold(&text);
    if folded.is_empty() {
        return Err(QueryError::new("query-empty-value", v.span).arg("field", field.name));
    }
    Ok(NameRef {
        text,
        folded,
        span: v.span,
    })
}

fn choice(
    v: &Value,
    field: &Field,
    choices: &'static [&'static str],
) -> Result<&'static str, QueryError> {
    let text = word(v).to_ascii_lowercase();
    if let Some(c) = choices.iter().find(|c| **c == text) {
        return Ok(c);
    }
    Err(match closest(&text, choices.iter().copied()) {
        Some(s) => QueryError::new("query-bad-choice-suggest", v.span)
            .arg("value", word(v))
            .arg("field", field.name)
            .arg("suggestion", s),
        None => QueryError::new("query-bad-choice", v.span)
            .arg("value", word(v))
            .arg("field", field.name)
            .arg("choices", choices.join(", ")),
    })
}

fn days_of(v: &Value, env: CheckEnv<'_>) -> Result<(Day, Day), QueryError> {
    let bad = || QueryError::new("query-bad-date", v.span);
    let one = |t: &str| parse_query_date(t, env.calendar).ok_or_else(bad);
    match &v.kind {
        ValueKind::Word { text, .. } => one(text).map(|d: Days| (d.first, d.last)),
        ValueKind::Range(a, b) => Ok((one(a)?.first, one(b)?.last)),
    }
}

fn minutes_of(v: &Value, env: CheckEnv<'_>) -> Result<(i64, i64), QueryError> {
    let one = |t: &str| {
        parse_minutes(t, env.units)
            .map_err(|_| QueryError::new("query-bad-duration", v.span).arg("value", t))
    };
    match &v.kind {
        ValueKind::Word { text, .. } => one(text).map(|m| (m, m)),
        ValueKind::Range(a, b) => Ok((one(a)?, one(b)?)),
    }
}

/// A number of days, `3d`, `3` or `-1d`.
fn days_value(v: &Value) -> Result<(i64, i64), QueryError> {
    let one = |t: &str| {
        let digits = t
            .strip_suffix('d')
            .or_else(|| t.strip_suffix('D'))
            .unwrap_or(t);
        digits
            .parse::<i32>()
            .ok()
            .filter(|n| n.unsigned_abs() <= 100_000)
            .map(i64::from)
            .ok_or_else(|| QueryError::new("query-bad-days", v.span).arg("value", t))
    };
    match &v.kind {
        ValueKind::Word { text, .. } => one(text).map(|n| (n, n)),
        ValueKind::Range(a, b) => Ok((one(a)?, one(b)?)),
    }
}

/// Gives the `slip` conditions the baseline named by the query's one
/// `baseline:` condition outside negations and alternatives, if it has
/// exactly one with one name.
fn bind_baseline(filter: &mut Filter) {
    fn named(f: &Filter, out: &mut Vec<String>) {
        match f {
            Filter::And(items) => items.iter().for_each(|i| named(i, out)),
            Filter::Cond(Cond::Baselines(names)) if names.len() == 1 => {
                out.push(names[0].folded.clone());
            }
            _ => {}
        }
    }
    fn bind(f: &mut Filter, name: &str) {
        match f {
            Filter::And(items) | Filter::Or(items) => items.iter_mut().for_each(|i| bind(i, name)),
            Filter::Not(inner) => bind(inner, name),
            Filter::Cond(Cond::Slip { baseline, .. }) => *baseline = Some(name.to_owned()),
            Filter::Cond(_) => {}
        }
    }
    let mut names = Vec::new();
    named(filter, &mut names);
    names.dedup();
    if let [name] = names.as_slice() {
        bind(filter, name);
    }
}

fn whole_number(v: &Value, percent: bool) -> Result<(i64, i64), QueryError> {
    let one = |t: &str| {
        let digits = if percent {
            t.strip_suffix('%').unwrap_or(t)
        } else {
            t
        };
        let n = digits
            .parse::<u32>()
            .ok()
            .filter(|n| !percent || *n <= 100)
            .map(i64::from);
        n.ok_or_else(|| {
            QueryError::new(
                if percent {
                    "query-bad-percent"
                } else {
                    "query-bad-number"
                },
                v.span,
            )
            .arg("value", t)
        })
    };
    match &v.kind {
        ValueKind::Word { text, .. } => one(text).map(|n| (n, n)),
        ValueKind::Range(a, b) => Ok((one(a)?, one(b)?)),
    }
}

#[allow(clippy::too_many_lines)]
fn condition(
    field: &'static Field,
    op: Op,
    values: &[Value],
    env: CheckEnv<'_>,
) -> Result<Filter, QueryError> {
    let cond = match field.ty {
        FieldType::Key => Cond::Keys(values.iter().map(key_ref).collect::<Result<_, _>>()?),
        FieldType::Project => Cond::Projects(
            values
                .iter()
                .map(|v| {
                    let text = word(v).to_ascii_uppercase();
                    let (key, subtree) = match text.strip_suffix("/*") {
                        Some(k) => (k.to_owned(), true),
                        None => (text, false),
                    };
                    let ok = key.len() >= 2
                        && key.len() <= 10
                        && key.starts_with(|c: char| c.is_ascii_uppercase())
                        && key
                            .chars()
                            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
                    if ok {
                        Ok(ProjectRef {
                            key,
                            subtree,
                            span: v.span,
                        })
                    } else {
                        Err(QueryError::new("query-bad-project", v.span).arg("value", word(v)))
                    }
                })
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Title => {
            if op == Op::Eq {
                let exact = values
                    .iter()
                    .map(|v| Filter::Cond(Cond::TitleExact(fold(&v.text()))))
                    .collect();
                return Ok(any_of(exact));
            }
            let each = values
                .iter()
                .map(|v| {
                    let words: Vec<String> = fold(&v.text())
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect();
                    if words.is_empty() {
                        Err(QueryError::new("query-empty-value", v.span).arg("field", field.name))
                    } else {
                        Ok(Filter::Cond(Cond::TitleWords(words)))
                    }
                })
                .collect::<Result<_, _>>()?;
            return Ok(any_of(each));
        }
        FieldType::FullText(scope) => {
            let each = values
                .iter()
                .map(|v| {
                    let (text, phrase) = match &v.kind {
                        ValueKind::Word { text, quoted } => (text.clone(), *quoted),
                        ValueKind::Range(..) => (v.text(), false),
                    };
                    if text.trim().is_empty() {
                        Err(QueryError::new("query-empty-value", v.span).arg("field", field.name))
                    } else {
                        Ok(Filter::Cond(Cond::Text {
                            scope,
                            text,
                            phrase,
                        }))
                    }
                })
                .collect::<Result<_, _>>()?;
            return Ok(any_of(each));
        }
        FieldType::Status => Cond::Statuses(
            values
                .iter()
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Type => Cond::Types(
            values
                .iter()
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Label => Cond::Labels {
            names: values
                .iter()
                .filter(|v| !is_none(v))
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
            none: values.iter().any(is_none),
        },
        FieldType::Version => Cond::Versions {
            names: values
                .iter()
                .filter(|v| !is_none(v))
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
            none: values.iter().any(is_none),
        },
        FieldType::Sprint => Cond::Sprints {
            names: values
                .iter()
                .filter(|v| !is_none(v) && !is_active(v))
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
            active: values.iter().any(is_active),
            none: values.iter().any(is_none),
        },
        FieldType::Category => {
            let mut cats = Vec::new();
            for v in values {
                match choice(v, field, CATEGORIES)? {
                    "open" => cats.extend(["todo", "in_progress"]),
                    "closed" => cats.extend(["done", "cancelled"]),
                    c => cats.push(c),
                }
            }
            cats.dedup();
            Cond::Categories(cats)
        }
        FieldType::Priority => Cond::Priority(
            values
                .iter()
                .map(|v| {
                    let c = choice(v, field, PRIORITIES)?;
                    let n = PRIORITIES
                        .iter()
                        .position(|p| *p == c)
                        .and_then(|i| i64::try_from(i).ok())
                        .unwrap_or(0);
                    Ok(bounds(op, n, n))
                })
                .collect::<Result<_, QueryError>>()?,
        ),
        FieldType::Parent => {
            let none = values.iter().any(is_none);
            Cond::Parent {
                keys: values
                    .iter()
                    .filter(|v| !is_none(v))
                    .map(key_ref)
                    .collect::<Result<_, _>>()?,
                none,
            }
        }
        FieldType::Under => Cond::Under(values.iter().map(key_ref).collect::<Result<_, _>>()?),
        FieldType::Link(link) => {
            let any = values.iter().any(is_any);
            Cond::Link {
                link,
                keys: if any {
                    Vec::new()
                } else {
                    values.iter().map(key_ref).collect::<Result<_, _>>()?
                },
            }
        }
        FieldType::Date(column) => {
            let none = values.iter().any(is_none);
            let mut b = Vec::new();
            for v in values.iter().filter(|v| !is_none(v)) {
                let (first, last) = days_of(v, env)?;
                b.push(bounds(
                    op,
                    first.days_since_epoch(),
                    last.days_since_epoch(),
                ));
            }
            Cond::Date {
                column,
                bounds: b,
                none,
            }
        }
        FieldType::Span => {
            if let Some(v) = values.iter().find(|v| is_none(v)) {
                return Err(QueryError::new("query-during-none", v.span));
            }
            Cond::Span(
                values
                    .iter()
                    .map(|v| {
                        days_of(v, env).map(|(first, last)| {
                            bounds(op, first.days_since_epoch(), last.days_since_epoch())
                        })
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        FieldType::Moment(column) => {
            let none = values.iter().any(is_none);
            let mut b = Vec::new();
            for v in values.iter().filter(|v| !is_none(v)) {
                b.push(moment_bounds(op, v, env)?);
            }
            Cond::Moment {
                column,
                bounds: b,
                none,
            }
        }
        FieldType::Duration(column) => {
            let none = values.iter().any(is_none);
            let mut b = Vec::new();
            for v in values.iter().filter(|v| !is_none(v)) {
                let (lo, hi) = minutes_of(v, env)?;
                b.push(bounds(op, lo, hi));
            }
            Cond::Duration {
                column,
                bounds: b,
                none,
            }
        }
        FieldType::Percent => Cond::Progress(
            values
                .iter()
                .map(|v| whole_number(v, true).map(|(lo, hi)| bounds(op, lo, hi)))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Slack => Cond::Slack(
            values
                .iter()
                .map(|v| days_value(v).map(|(lo, hi)| bounds(op, lo, hi)))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Slip => Cond::Slip {
            bounds: values
                .iter()
                .map(|v| days_value(v).map(|(lo, hi)| bounds(op, lo, hi)))
                .collect::<Result<_, _>>()?,
            baseline: None,
        },
        FieldType::Baseline => Cond::Baselines(
            values
                .iter()
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Count(of) => Cond::Count(
            of,
            values
                .iter()
                .map(|v| whole_number(v, false).map(|(lo, hi)| bounds(op, lo, hi)))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Author => Cond::Author(values.iter().map(author_ref).collect()),
        FieldType::Claimed => {
            if values.iter().any(is_any) {
                Cond::Claimed(None)
            } else {
                Cond::Claimed(Some(values.iter().map(Value::text).collect()))
            }
        }
        FieldType::Is => Cond::Is(
            values
                .iter()
                .map(|v| choice(v, field, IS_FLAGS))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Was => {
            return Err(QueryError::new("query-was-value", values[0].span));
        }
        FieldType::Changed => {
            let (mut keys, mut custom) = (Vec::new(), Vec::new());
            for v in values {
                if let Some(field) = fields::find_custom(word(v), env.custom) {
                    if !custom.contains(&field.query_name) {
                        custom.push(field.query_name.clone());
                    }
                    continue;
                }
                if fields::strip_custom_prefix(word(v)).is_some() {
                    return Err(unknown_custom(word(v), v.span, env));
                }
                let key = history_field(v.span, word(v), History::Changed)?;
                if !keys.contains(&key.1) {
                    keys.push(key.1);
                }
            }
            Cond::Changed {
                keys,
                custom,
                at: Vec::new(),
            }
        }
        FieldType::ChangedBy => Cond::ChangedBy(values.iter().map(author_ref).collect()),
        FieldType::Has => Cond::Has(
            values
                .iter()
                .map(|v| choice(v, field, HAS_PARTS))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::In => {
            for v in values {
                choice(v, field, IN_PLACES)?;
            }
            Cond::InFiles(values[0].span)
        }
    };
    Ok(Filter::Cond(cond))
}

fn author_ref(v: &Value) -> AuthorRef {
    let text = v.text();
    match text.to_lowercase().as_str() {
        "me" => AuthorRef::Me,
        "agent" => AuthorRef::Agent,
        _ => AuthorRef::Named(text),
    }
}

/// The moments of a day value under `op`: from the start of its first day
/// to the end of its last, in the workspace's time zone.
fn moment_bounds(op: Op, v: &Value, env: CheckEnv<'_>) -> Result<Bounds, QueryError> {
    let (first, last) = days_of(v, env)?;
    let days = bounds(op, first.days_since_epoch(), last.days_since_epoch());
    let start = |d: i64| {
        Day::from_days_since_epoch(d)
            .map(|d| env.calendar.start_of(d))
            .unwrap_or(if d < 0 { i64::MIN } else { i64::MAX })
    };
    Ok(Bounds {
        lo: days.lo.map(start),
        hi: days
            .hi
            .map(|d| start(d.saturating_add(1)).saturating_sub(1)),
    })
}

/// The field a history condition looks at, with its name in the history.
fn history_field(
    span: Span,
    written: &str,
    kind: History,
) -> Result<(&'static Field, &'static str), QueryError> {
    let choices = match kind {
        History::Was => fields::WAS_FIELDS,
        History::Changed => fields::CHANGED_FIELDS,
    };
    let found = fields::find(written)
        .filter(|f| choices.contains(&f.name))
        .and_then(|f| fields::history_key(f.ty).map(|k| (f, k)));
    // The category is read from the status.
    let found = found.or_else(|| {
        fields::find(written)
            .filter(|f| kind == History::Was && f.ty == FieldType::Category)
            .map(|f| (f, "status"))
    });
    found.ok_or_else(|| {
        let lower = written.to_ascii_lowercase();
        match closest(&lower, choices.iter().copied()) {
            Some(s) => QueryError::new("query-history-field-suggest", span)
                .arg("field", written)
                .arg("history", kind.as_str())
                .arg("suggestion", s),
            None => QueryError::new("query-history-field", span)
                .arg("field", written)
                .arg("history", kind.as_str())
                .arg("choices", choices.join(", ")),
        }
    })
}

/// `was:<field><op>values` or `changed:<field><op>dates`.
fn history_predicate(
    p: &Predicate,
    kind: History,
    inner: &str,
    env: CheckEnv<'_>,
) -> Result<Filter, QueryError> {
    let inner_span = Span::new(p.field_span.end - inner.len(), p.field_span.end);
    if let Some(custom) = fields::find_custom(inner, env.custom) {
        return custom_history_predicate(p, kind, custom, env);
    }
    if fields::strip_custom_prefix(inner).is_some() {
        return Err(unknown_custom(inner, inner_span, env));
    }
    let (field, key) = history_field(inner_span, inner, kind)?;
    let written = format!("{}:{}", kind.as_str(), field.name);
    let ops = history_ops(kind);
    if !ops.contains(&p.op) {
        return Err(history_operator_error(p, ops, &written));
    }
    for v in &p.values {
        if is_none(v) {
            return Err(QueryError::new("query-history-none", v.span)
                .arg("history", kind.as_str())
                .arg("field", field.name));
        }
        if let ValueKind::Range(..) = v.kind {
            if kind == History::Was {
                return Err(QueryError::new("query-no-range", v.span)
                    .arg("field", written.clone())
                    .arg("usage", format!("{written}:, {written}!=")));
            }
            if !matches!(p.op, Op::Is | Op::Eq) {
                return Err(
                    QueryError::new("query-range-operator", v.span).arg("field", written.clone())
                );
            }
        }
    }
    let (op, negate) = match p.op {
        Op::Ne => (Op::Is, true),
        Op::Eq => (Op::Is, false),
        op => (op, false),
    };
    let cond = match kind {
        History::Changed => Cond::Changed {
            keys: vec![key],
            custom: Vec::new(),
            at: p
                .values
                .iter()
                .map(|v| moment_bounds(op, v, env))
                .collect::<Result<_, _>>()?,
        },
        History::Was => Cond::Was(match field.ty {
            FieldType::Status => WasTest::Statuses(
                p.values
                    .iter()
                    .map(|v| name_ref(v, field))
                    .collect::<Result<_, _>>()?,
            ),
            FieldType::Type => WasTest::Types(
                p.values
                    .iter()
                    .map(|v| name_ref(v, field))
                    .collect::<Result<_, _>>()?,
            ),
            FieldType::Label => WasTest::Labels(
                p.values
                    .iter()
                    .map(|v| name_ref(v, field))
                    .collect::<Result<_, _>>()?,
            ),
            FieldType::Version => WasTest::Versions(
                p.values
                    .iter()
                    .map(|v| name_ref(v, field))
                    .collect::<Result<_, _>>()?,
            ),
            FieldType::Sprint => WasTest::Sprints(
                p.values
                    .iter()
                    .map(|v| name_ref(v, field))
                    .collect::<Result<_, _>>()?,
            ),
            FieldType::Category => {
                let mut cats = Vec::new();
                for v in &p.values {
                    match choice(v, field, CATEGORIES)? {
                        "open" => cats.extend(["todo", "in_progress"]),
                        "closed" => cats.extend(["done", "cancelled"]),
                        c => cats.push(c),
                    }
                }
                cats.dedup();
                WasTest::Categories(cats)
            }
            _ => WasTest::Priorities(
                p.values
                    .iter()
                    .map(|v| choice(v, field, PRIORITIES))
                    .collect::<Result<_, _>>()?,
            ),
        }),
    };
    let cond = Filter::Cond(cond);
    Ok(if negate {
        Filter::Not(Box::new(cond))
    } else {
        cond
    })
}

/// The operators a history condition takes.
fn history_ops(kind: History) -> &'static [Op] {
    match kind {
        History::Was => &[Op::Is, Op::Eq, Op::Ne],
        History::Changed => &[Op::Is, Op::Eq, Op::Ne, Op::Lt, Op::Le, Op::Gt, Op::Ge],
    }
}

fn history_operator_error(p: &Predicate, ops: &[Op], written: &str) -> QueryError {
    let usage = ops
        .iter()
        .filter(|op| **op != Op::Eq)
        .map(|op| format!("{written}{}", op.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    QueryError::new("query-operator", p.op_span)
        .arg("op", p.op.as_str())
        .arg("field", written)
        .arg("usage", usage)
}

/// `was:cf.<name><op>values` or `changed:cf.<name><op>dates`: values are
/// read as the field itself reads them (`:` finds words in a text, `=` the
/// exact text), without ranges or `none`.
fn custom_history_predicate(
    p: &Predicate,
    kind: History,
    field: &CustomField,
    env: CheckEnv<'_>,
) -> Result<Filter, QueryError> {
    let written = format!("{}:{}", kind.as_str(), field.written());
    let ops = history_ops(kind);
    if !ops.contains(&p.op) {
        return Err(history_operator_error(p, ops, &written));
    }
    for v in &p.values {
        if is_none(v) {
            return Err(QueryError::new("query-history-none", v.span)
                .arg("history", kind.as_str())
                .arg("field", field.written()));
        }
        if let ValueKind::Range(..) = v.kind {
            if kind == History::Was {
                return Err(QueryError::new("query-no-range", v.span)
                    .arg("field", written.clone())
                    .arg("usage", format!("{written}:, {written}!=")));
            }
            if !matches!(p.op, Op::Is | Op::Eq) {
                return Err(
                    QueryError::new("query-range-operator", v.span).arg("field", written.clone())
                );
            }
        }
    }
    let (op, negate) = match p.op {
        Op::Ne => (Op::Is, true),
        Op::Eq if kind == History::Changed => (Op::Is, false),
        op => (op, false),
    };
    let cond = match kind {
        History::Changed => Cond::Changed {
            keys: Vec::new(),
            custom: vec![field.query_name.clone()],
            at: p
                .values
                .iter()
                .map(|v| moment_bounds(op, v, env))
                .collect::<Result<_, _>>()?,
        },
        History::Was => {
            let values: Vec<&Value> = p.values.iter().collect();
            let exact = op == Op::Eq;
            let test = custom_test(field, Op::Is, exact, &values, env)?;
            Cond::Was(WasTest::Custom {
                query_name: field.query_name.clone(),
                test,
            })
        }
    };
    let cond = Filter::Cond(cond);
    Ok(if negate {
        Filter::Not(Box::new(cond))
    } else {
        cond
    })
}

fn any_of(mut items: Vec<Filter>) -> Filter {
    if items.len() == 1 {
        items.remove(0)
    } else {
        Filter::Or(items)
    }
}

#[cfg(test)]
mod tests {
    use gk_core::Weekday;

    use super::*;
    use crate::dates::FixedCalendar;
    use crate::parse::parse;

    fn cal() -> FixedCalendar {
        FixedCalendar {
            today: "2026-11-23".parse().expect("day"),
            offset_minutes: 0,
            week_start: Weekday::Monday,
        }
    }

    fn custom() -> Vec<CustomField> {
        vec![
            CustomField {
                name: "Points".to_owned(),
                query_name: "points".to_owned(),
                kind: FieldKind::Integer,
                options: Vec::new(),
            },
            CustomField {
                name: "Área".to_owned(),
                query_name: "área".to_owned(),
                kind: FieldKind::Select,
                options: vec!["Front end".to_owned(), "Back end".to_owned()],
            },
            CustomField {
                name: "Link".to_owned(),
                query_name: "link".to_owned(),
                kind: FieldKind::Url,
                options: Vec::new(),
            },
        ]
    }

    fn run(src: &str) -> Result<Checked, QueryError> {
        let c = cal();
        let custom = custom();
        check(
            &parse(src)?,
            CheckEnv {
                calendar: &c,
                units: DurationUnits::default(),
                custom: &custom,
            },
        )
    }

    #[test]
    fn custom_fields_take_the_operators_and_values_of_their_kind() {
        let ok = |src: &str| run(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        let first = |src: &str| match ok(src).filter {
            Some(Filter::Cond(Cond::Custom(c))) => c,
            other => panic!("{src}: {other:?}"),
        };
        let c = first("cf.points>3");
        assert_eq!(c.query_name, "points");
        assert_eq!(
            c.test,
            CustomTest::Integer(vec![Bounds {
                lo: Some(4),
                hi: None
            }])
        );
        assert!(first("points:none").none);
        assert_eq!(
            first("cf.ÁREA:\"back END\"").test,
            CustomTest::Options(vec!["Back end".to_owned()])
        );
        assert_eq!(err("cf.área:middle").id, "query-bad-choice");
        assert_eq!(err("cf.points:abc").id, "query-bad-number");
        assert_eq!(err("cf.link>3").id, "query-operator");
        assert_eq!(err("cf.link:a..b").id, "query-no-range");
        assert_eq!(err("cf.pionts:3").id, "query-unknown-custom-field");
        assert_eq!(err("cf.zzzzzzzz:3").id, "query-unknown-custom-field-plain");
        assert!(matches!(
            ok("is:open sort:-cf.points").sort.first().map(|s| &s.by),
            Some(SortBy::Custom(q)) if q == "points"
        ));
        assert_eq!(err("sort:cf.nothing").id, "query-sort-unknown-plain");
    }

    #[test]
    fn history_conditions_take_custom_fields() {
        let ok = |src: &str| run(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        assert_eq!(
            ok("changed:status,cf.points,points").filter,
            Some(Filter::Cond(Cond::Changed {
                keys: vec!["status"],
                custom: vec!["points".to_owned()],
                at: Vec::new(),
            }))
        );
        assert!(matches!(
            ok("changed:cf.points>-7d").filter,
            Some(Filter::Cond(Cond::Changed { keys, custom, at }))
                if keys.is_empty() && custom == ["points"] && at.len() == 1
        ));
        assert_eq!(
            ok("was:cf.ÁREA:\"back END\"").filter,
            Some(Filter::Cond(Cond::Was(WasTest::Custom {
                query_name: "área".to_owned(),
                test: CustomTest::Options(vec!["Back end".to_owned()]),
            })))
        );
        assert!(matches!(
            ok("was:cf.points:3").filter,
            Some(Filter::Cond(Cond::Was(WasTest::Custom {
                test: CustomTest::Integer(_),
                ..
            })))
        ));
        assert!(matches!(
            ok("was:cf.link=ABC-1").filter,
            Some(Filter::Cond(Cond::Was(WasTest::Custom {
                test: CustomTest::Exact(_),
                ..
            })))
        ));
        assert!(matches!(
            ok("was:cf.área!=\"front end\"").filter,
            Some(Filter::Not(_))
        ));
        assert_eq!(err("was:cf.points:none").id, "query-history-none");
        assert_eq!(err("was:cf.points:1..3").id, "query-no-range");
        assert_eq!(err("was:cf.points>3").id, "query-operator");
        assert_eq!(err("was:cf.área:middle").id, "query-bad-choice");
        assert_eq!(err("was:cf.pionts:3").id, "query-unknown-custom-field");
        assert_eq!(err("changed:cf.pionts").id, "query-unknown-custom-field");
    }

    fn err(src: &str) -> QueryError {
        run(src).expect_err(src)
    }

    #[test]
    fn slack_and_slip_take_days_and_slip_follows_the_one_baseline_named() {
        let slip = |src: &str| -> Vec<(Vec<Bounds>, Option<String>)> {
            fn walk(f: &Filter, out: &mut Vec<(Vec<Bounds>, Option<String>)>) {
                match f {
                    Filter::And(items) | Filter::Or(items) => {
                        items.iter().for_each(|i| walk(i, out))
                    }
                    Filter::Not(inner) => walk(inner, out),
                    Filter::Cond(Cond::Slip { bounds, baseline }) => {
                        out.push((bounds.clone(), baseline.clone()));
                    }
                    Filter::Cond(_) => {}
                }
            }
            let mut out = Vec::new();
            if let Some(f) = &run(src).unwrap_or_else(|e| panic!("{src}: {e:?}")).filter {
                walk(f, &mut out);
            }
            out
        };
        let above = |n: i64| Bounds {
            lo: Some(n),
            hi: None,
        };
        assert_eq!(slip("slip>3d"), vec![(vec![above(4)], None)]);
        assert_eq!(
            slip("slip<-1d"),
            vec![(
                vec![Bounds {
                    lo: None,
                    hi: Some(-2)
                }],
                None
            )]
        );
        assert_eq!(
            slip("baseline:\"Plan A\" slip>=2"),
            vec![(vec![above(2)], Some("plan a".to_owned()))]
        );
        // Two baselines, or one in an alternative: the newest.
        assert_eq!(slip("baseline:A baseline:B slip>2d")[0].1, None);
        assert_eq!(slip("(baseline:A OR is:open) slip>2d")[0].1, None);
        assert!(run("slack<2d").unwrap().uses_schedule());
        assert!(run("-is:critical").unwrap().uses_schedule());
        assert!(!run("slip>2d is:open").unwrap().uses_schedule());
        assert_eq!(err("slack<soon").id, "query-bad-days");
        assert_eq!(err("slip>2h").id, "query-bad-days");
    }

    #[test]
    fn none_on_labels_means_no_label_and_names_nothing() {
        let labels = |src: &str| match run(src).unwrap_or_else(|e| panic!("{src}: {e:?}")) {
            Checked {
                filter: Some(Filter::Cond(Cond::Labels { names, none })),
                ..
            } => (
                names.into_iter().map(|n| n.folded).collect::<Vec<_>>(),
                none,
            ),
            other => panic!("{src}: {other:?}"),
        };
        assert_eq!(labels("label:none"), (Vec::new(), true));
        assert_eq!(labels("labels:NONE"), (Vec::new(), true));
        assert_eq!(labels("label:bug,none"), (vec!["bug".to_owned()], true));
        assert_eq!(labels("label:bug"), (vec!["bug".to_owned()], false));
        // A quoted "none" is a label of that name.
        assert_eq!(labels("label:\"none\""), (vec!["none".to_owned()], false));
        assert!(run("label:none").unwrap().names().is_empty());
        assert!(matches!(
            run("-label:none").unwrap().filter,
            Some(Filter::Not(_))
        ));
    }

    #[test]
    fn the_planned_messages() {
        assert_eq!(
            err("stauts:Done").message(),
            "Unknown field \"stauts\". Did you mean \"status\"?"
        );
        assert_eq!(
            err("label<bug").message(),
            "\"<\" doesn't apply to label. Use label:, label!=."
        );
        assert_eq!(
            err("due:31-12-2026").message(),
            "Dates are written 2026-12-31, or as today, +7d, this-week."
        );
        assert_eq!(err("(a sort:due)").id, "query-sort-placement");
    }

    #[test]
    fn values_are_checked() {
        assert_eq!(err("estimate:lots").id, "query-bad-duration");
        assert_eq!(err("progress:120").id, "query-bad-percent");
        assert_eq!(err("comments:many").id, "query-bad-number");
        assert_eq!(err("is:opne").get("suggestion"), Some("open"));
        assert_eq!(err("priority:hihg").get("suggestion"), Some("high"));
        assert_eq!(err("parent:nonsense").id, "query-bad-key");
        assert_eq!(err("project:1X").id, "query-bad-project");
        assert_eq!(err("label:a..b").id, "query-no-range");
        assert_eq!(err("due<a..b").id, "query-range-operator");
        assert_eq!(err("due<none").id, "query-none-operator");
        assert_eq!(err("text=x").id, "query-equals-text");
        assert_eq!(err("sort:relevance").id, "query-sort-relevance");
        assert_eq!(err("sort:stauts").get("suggestion"), Some("status"));
        assert!(run("foo sort:relevance").is_ok());
        assert!(run("estimate:1h..4h due:none,today status=Done prio>=high").is_ok());
    }

    #[test]
    fn relative_dates_are_evaluated_against_the_calendar() {
        let c = run("due<+7d").expect("query");
        let today = cal().today.days_since_epoch();
        assert_eq!(
            c.filter,
            Some(Filter::Cond(Cond::Date {
                column: DateColumn::Due,
                bounds: vec![Bounds {
                    lo: None,
                    hi: Some(today + 6)
                }],
                none: false
            }))
        );
    }

    #[test]
    fn during_takes_days_periods_and_ranges() {
        let c = run("during:2026-11-01..2026-11-30").expect("query");
        let day = |s: &str| s.parse::<Day>().expect("day").days_since_epoch();
        assert_eq!(
            c.filter,
            Some(Filter::Cond(Cond::Span(vec![Bounds {
                lo: Some(day("2026-11-01")),
                hi: Some(day("2026-11-30")),
            }])))
        );
        let c = run("during:this-week").expect("query");
        assert_eq!(
            c.filter,
            Some(Filter::Cond(Cond::Span(vec![Bounds {
                lo: Some(day("2026-11-23")),
                hi: Some(day("2026-11-29")),
            }])))
        );
        assert_eq!(err("during:none").id, "query-during-none");
        assert_eq!(err("during<today").id, "query-operator");
        assert_eq!(err("during:soon").id, "query-bad-date");
        assert!(err("sort:during").id.starts_with("query-sort-unknown"));
    }

    #[test]
    fn not_equal_is_a_negation() {
        let c = run("label!=bug").expect("query");
        assert!(matches!(c.filter, Some(Filter::Not(_))));
    }

    #[test]
    fn history_conditions_are_checked() {
        let day = |d: &str| {
            let day: Day = d.parse().expect("day");
            cal().start_of(day)
        };
        let checked = run("changed:due>-7d").expect("query");
        assert_eq!(
            checked.filter,
            Some(Filter::Cond(Cond::Changed {
                keys: vec!["due_date"],
                custom: Vec::new(),
                at: vec![Bounds {
                    lo: Some(day("2026-11-17")),
                    hi: None
                }],
            }))
        );
        let checked = run("changed:due,labels,due").expect("query");
        assert_eq!(
            checked.filter,
            Some(Filter::Cond(Cond::Changed {
                keys: vec!["due_date", "labels"],
                custom: Vec::new(),
                at: Vec::new(),
            }))
        );
        assert!(matches!(
            run("was:cat:open").expect("query").filter,
            Some(Filter::Cond(Cond::Was(WasTest::Categories(c)))) if c == ["todo", "in_progress"]
        ));
        assert!(matches!(
            run("was:status!=Done").expect("query").filter,
            Some(Filter::Not(_))
        ));
        assert!(matches!(
            run("changed-by:agent").expect("query").filter,
            Some(Filter::Cond(Cond::ChangedBy(w))) if w == [AuthorRef::Agent]
        ));
        let names = run("was:status:Review was:label:bug")
            .expect("query")
            .names();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].kind, NameKind::Status);
        assert_eq!(err("was:status").id, "query-was-value");
        assert_eq!(err("was:stauts:Review").id, "query-history-field-suggest");
        assert_eq!(err("was:due:today").id, "query-history-field");
        assert_eq!(err("changed:stauts").id, "query-history-field-suggest");
        assert_eq!(err("was:status>Done").id, "query-operator");
        assert_eq!(err("was:status:none").id, "query-history-none");
        assert_eq!(err("was:priority:hgih").id, "query-bad-choice-suggest");
        assert_eq!(err("changed:due:soon").id, "query-bad-date");
        assert_eq!(err("was:status:a..b").id, "query-no-range");
        let e = err("was:stauts:Review");
        assert_eq!(e.span.of("was:stauts:Review"), "stauts");
    }

    #[test]
    fn names_are_collected_for_the_engine() {
        let c = run("status:Doing -label:x project:TM/* parent:TM-4 TM-5").expect("query");
        let names: Vec<(NameKind, String)> =
            c.names().into_iter().map(|n| (n.kind, n.text)).collect();
        assert_eq!(
            names,
            [
                (NameKind::Status, "Doing".to_owned()),
                (NameKind::Label, "x".to_owned()),
                (NameKind::Project, "TM".to_owned()),
                (NameKind::Task, "TM-4".to_owned()),
            ]
        );
        assert_eq!(c.named_projects().len(), 1);
    }
}
