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

use gk_core::{Day, DurationUnits, fold, parse_minutes};

use crate::dates::{Calendar, Days, parse_query_date};
use crate::error::{QueryError, closest};
use crate::fields::{
    self, CATEGORIES, CountOf, DateColumn, DurationColumn, Field, FieldType, HAS_PARTS, IS_FLAGS,
    LinkField, MomentColumn, PRIORITIES, TextScope,
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
    /// Has any of these labels.
    Labels(Vec<NameRef>),
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
}

/// What a sort key sorts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// A field.
    Field(&'static Field),
    /// The place on the board.
    Rank,
    /// How well the text terms match.
    Relevance,
}

/// One checked sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Cond::Labels(n) => names(NameKind::Label, n, out),
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
}

impl std::fmt::Debug for CheckEnv<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckEnv")
            .field("today", &self.calendar.today())
            .field("units", &self.units)
            .finish()
    }
}

/// Checks `query`.
///
/// # Errors
/// The first field, operator or value that doesn't fit.
pub fn check(query: &Query, env: CheckEnv<'_>) -> Result<Checked, QueryError> {
    let filter = query.filter.as_ref().map(|e| expr(e, env)).transpose()?;
    let mut sort = Vec::new();
    if let Some(clause) = &query.sort {
        for key in &clause.keys {
            let lower = key.field.to_ascii_lowercase();
            let by = match lower.as_str() {
                "rank" => SortBy::Rank,
                "relevance" => SortBy::Relevance,
                _ => match fields::find(&lower).filter(|f| f.sortable) {
                    Some(f) => SortBy::Field(f),
                    None => {
                        return Err(match closest(&lower, fields::sort_keys()) {
                            Some(s) => QueryError::new("query-sort-unknown", key.span)
                                .arg("field", key.field.clone())
                                .arg("suggestion", s),
                            None => QueryError::new("query-sort-unknown-plain", key.span)
                                .arg("field", key.field.clone())
                                .arg(
                                    "choices",
                                    fields::sort_keys().collect::<Vec<_>>().join(", "),
                                ),
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
    let Some(field) = fields::find(&p.field) else {
        return Err(match closest(&p.field, fields::all_names()) {
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

fn word(v: &Value) -> &str {
    match &v.kind {
        ValueKind::Word { text, .. } => text,
        ValueKind::Range(a, _) => a,
    }
}

fn is_none(v: &Value) -> bool {
    matches!(&v.kind, ValueKind::Word { text, quoted: false } if text.eq_ignore_ascii_case("none"))
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
        FieldType::Label => Cond::Labels(
            values
                .iter()
                .map(|v| name_ref(v, field))
                .collect::<Result<_, _>>()?,
        ),
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
                let (first, last) = days_of(v, env)?;
                let days = bounds(op, first.days_since_epoch(), last.days_since_epoch());
                let start = |d: i64| {
                    Day::from_days_since_epoch(d)
                        .map(|d| env.calendar.start_of(d))
                        .unwrap_or(if d < 0 { i64::MIN } else { i64::MAX })
                };
                b.push(Bounds {
                    lo: days.lo.map(start),
                    hi: days
                        .hi
                        .map(|d| start(d.saturating_add(1)).saturating_sub(1)),
                });
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
        FieldType::Count(of) => Cond::Count(
            of,
            values
                .iter()
                .map(|v| whole_number(v, false).map(|(lo, hi)| bounds(op, lo, hi)))
                .collect::<Result<_, _>>()?,
        ),
        FieldType::Author => Cond::Author(
            values
                .iter()
                .map(|v| {
                    let text = v.text();
                    match text.to_lowercase().as_str() {
                        "me" => AuthorRef::Me,
                        "agent" => AuthorRef::Agent,
                        _ => AuthorRef::Named(text),
                    }
                })
                .collect(),
        ),
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
        FieldType::Has => Cond::Has(
            values
                .iter()
                .map(|v| choice(v, field, HAS_PARTS))
                .collect::<Result<_, _>>()?,
        ),
    };
    Ok(Filter::Cond(cond))
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

    fn run(src: &str) -> Result<Checked, QueryError> {
        let c = cal();
        check(
            &parse(src)?,
            CheckEnv {
                calendar: &c,
                units: DurationUnits::default(),
            },
        )
    }

    fn err(src: &str) -> QueryError {
        run(src).expect_err(src)
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
