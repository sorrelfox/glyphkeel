// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task queries: `query.parse` for the app's query field and `task.search`
//! for every list. Both go through [`compile`]: parse, check against the
//! field registry (relative dates in the workspace's time zone, weeks
//! starting on the workspace's first day), then look up the names the
//! query uses, so an unknown status is an error with the choices rather
//! than an empty list.

use std::collections::BTreeMap;

use gk_api::OpError;
use gk_api::ops::common::{Actor, ActorKind, Category, Glyph, Priority};
use gk_api::ops::query::{
    ListGroup, ListGrouping, ListTotals, ListedTask, ParsedQuery, QueryChip, QueryField,
    QueryParse, QueryParseInput, QueryProblem, QueryToken, QueryTokenKind, TaskSearch,
    TaskSearchInput, TaskSearchResult, TextRange,
};
use gk_api::ops::settings::WeekStart;
use gk_api::ops::task::{Checklist, StatusRef, TypeRef};
use gk_core::{Day, Id, Weekday, fold};
use gk_query::check::{NameKind, NameUse};
use gk_query::{
    Calendar, CheckEnv, Checked, ChipKind, Compiled, Param, Query, QueryError, Scope, Span, SqlEnv,
    TokenKind,
};
use gk_store::rusqlite::types::Value as Sql;
use gk_store::rusqlite::{OptionalExtension, Transaction, params_from_iter};

use super::desc;
use super::fields::priority_name;
use super::lookup::{self, b, id_at, opt_id_at};
use super::project::parse_enum;
use super::task::{
    BLOCKED_SQL, CLOSED_SUBTASKS_SQL, COMMENTS_SQL, FILES_SQL, SUBTASKS_SQL, live_claim,
    task_labels,
};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::{Zone, rfc3339};

/// Tasks per page unless asked otherwise.
const DEFAULT_LIMIT: u32 = 50;
/// The most tasks per page.
const MAX_LIMIT: u32 = 200;
/// The most names listed as choices in an error.
const MAX_CHOICES: usize = 30;

pub(super) fn register(r: &mut Registry) {
    r.query::<QueryParse, _>(desc::<QueryParse>(), |q, input: QueryParseInput| {
        parse_for_field(q, &input)
    });
    r.query::<TaskSearch, _>(desc::<TaskSearch>(), search);
}

/// Today, the local days and the first day of the week of the workspace.
pub(crate) struct WorkspaceCalendar<'a> {
    zone: &'a Zone,
    now: i64,
    week_start: Weekday,
}

impl Calendar for WorkspaceCalendar<'_> {
    fn today(&self) -> Day {
        self.zone.day_of(self.now)
    }

    fn start_of(&self, day: Day) -> i64 {
        self.zone.moment(day, 0)
    }

    fn week_start(&self) -> Weekday {
        self.week_start
    }
}

/// The workspace's calendar at `now`.
pub(crate) fn calendar<'a>(
    tx: &Transaction<'_>,
    zone: &'a Zone,
    now: i64,
) -> Result<WorkspaceCalendar<'a>, Failure> {
    let settings = super::settings::load(tx)?;
    Ok(WorkspaceCalendar {
        zone,
        now,
        week_start: match settings.week_start {
            WeekStart::Monday => Weekday::Monday,
            WeekStart::Saturday => Weekday::Saturday,
            WeekStart::Sunday => Weekday::Sunday,
        },
    })
}

/// The UTF-16 offset of byte `at` in `src`, as the page's editor counts.
fn utf16(src: &str, at: usize) -> u32 {
    let at = at.min(src.len());
    let prefix = src.get(..at).unwrap_or(src);
    u32::try_from(prefix.encode_utf16().count()).unwrap_or(u32::MAX)
}

fn range(src: &str, span: Span) -> TextRange {
    TextRange {
        from: utf16(src, span.start),
        to: utf16(src, span.end),
    }
}

/// A query mistake as an operation error: the English message, its ID and
/// arguments for translation, and where it is (in characters).
pub(crate) fn query_error(e: &QueryError, src: &str, field: &str) -> Failure {
    let chars = |at: usize| {
        src.get(..at.min(src.len()))
            .map_or(0, |s| s.chars().count())
    };
    let args: serde_json::Map<String, serde_json::Value> = e
        .args
        .iter()
        .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(v.clone())))
        .collect();
    let mut error = OpError::invalid_at(field, e.message())
        .with_detail("message_id", e.id)
        .with_detail("from", chars(e.span.start))
        .with_detail("to", chars(e.span.end))
        .with_detail("text", e.span.of(src));
    if !args.is_empty() {
        error = error.with_detail("args", serde_json::Value::Object(args));
    }
    if let Some(choices) = e.get("choices") {
        error = error.with_detail(
            "choices",
            choices.split(", ").map(str::to_owned).collect::<Vec<_>>(),
        );
    }
    Failure::Op(error)
}

/// Every live name of `kind`, sorted, without repeats.
fn live_names(tx: &Transaction<'_>, kind: NameKind) -> Result<Vec<String>, Failure> {
    let sql = match kind {
        NameKind::Status => {
            "SELECT DISTINCT x.name FROM statuses x JOIN projects p ON p.id = x.project_id
              WHERE p.deleted_at IS NULL ORDER BY x.name_fold"
        }
        NameKind::Type => {
            "SELECT DISTINCT x.name FROM task_types x JOIN projects p ON p.id = x.project_id
              WHERE p.deleted_at IS NULL ORDER BY x.name_fold"
        }
        NameKind::Label => {
            "SELECT DISTINCT x.name FROM labels x JOIN projects p ON p.id = x.project_id
              WHERE p.deleted_at IS NULL ORDER BY x.name_fold"
        }
        NameKind::Project => "SELECT key FROM projects WHERE deleted_at IS NULL ORDER BY key",
        NameKind::Task => return Ok(Vec::new()),
    };
    let mut stmt = tx.prepare_cached(sql)?;
    let mut names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    names.dedup_by(|a, b| fold(a) == fold(b));
    Ok(names)
}

/// Looks up every name the query uses; the first unknown one is an error
/// with the names there are.
fn check_names(tx: &Transaction<'_>, uses: &[NameUse]) -> Result<Option<QueryError>, Failure> {
    let mut cache: BTreeMap<u8, Vec<String>> = BTreeMap::new();
    for u in uses {
        let (slot, id, none) = match u.kind {
            NameKind::Status => (0, "query-unknown-status", "query-unknown-status-none"),
            NameKind::Type => (1, "query-unknown-type", "query-unknown-type-none"),
            NameKind::Label => (2, "query-unknown-label", "query-unknown-label-none"),
            NameKind::Project => (3, "query-unknown-project", "query-unknown-project-none"),
            NameKind::Task => {
                if lookup::task(tx, "query", &u.text).is_err() {
                    return Ok(Some(
                        QueryError::new("query-unknown-task", u.span).arg("value", u.text.clone()),
                    ));
                }
                continue;
            }
        };
        let names = match cache.entry(slot) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => e.insert(live_names(tx, u.kind)?),
        };
        let found = if u.kind == NameKind::Project {
            names.iter().any(|n| n == &u.text)
                || tx
                    .query_row(
                        "SELECT 1 FROM project_key_aliases WHERE key = ?1",
                        [u.text.as_str()],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some()
        } else {
            let wanted = fold(&u.text);
            names.iter().any(|n| fold(n) == wanted)
        };
        if !found && names.is_empty() {
            return Ok(Some(
                QueryError::new(none, u.span).arg("value", u.text.clone()),
            ));
        }
        if !found {
            let mut choices: Vec<&str> =
                names.iter().map(String::as_str).take(MAX_CHOICES).collect();
            if names.len() > MAX_CHOICES {
                choices.push("…");
            }
            return Ok(Some(
                QueryError::new(id, u.span)
                    .arg("value", u.text.clone())
                    .arg("choices", choices.join(", ")),
            ));
        }
    }
    Ok(None)
}

/// Parses and checks `src` and looks up its names. The outer error is a
/// storage problem; the inner one a mistake in the query.
pub(crate) fn try_compile(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    src: &str,
) -> Result<Result<(Query, Checked), QueryError>, Failure> {
    let cal = calendar(tx, zone, now)?;
    let units = super::task::units(tx)?;
    let custom = super::custom_field::query_fields(tx)?;
    let compiled = gk_query::compile(
        src,
        CheckEnv {
            calendar: &cal,
            units,
            custom: &custom,
        },
    );
    let (query, checked) = match compiled {
        Ok(c) => c,
        Err(e) => return Ok(Err(e)),
    };
    if let Some(e) = check_names(tx, &checked.names())? {
        return Ok(Err(e));
    }
    Ok(Ok((query, checked)))
}

/// Like [`try_compile`], with a mistake as an `invalid` error on input
/// field `field`.
pub(crate) fn compile(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    src: &str,
    field: &str,
) -> Result<(Query, Checked), Failure> {
    try_compile(tx, zone, now, src)?.map_err(|e| query_error(&e, src, field))
}

/// The sort keys of `text` (`due,-priority`), checked.
pub(crate) fn sort_keys(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    text: &str,
    field: &str,
) -> Result<Vec<gk_query::check::Sort>, Failure> {
    let src = format!("sort:{}", text.trim());
    let (query, checked) = compile(tx, zone, now, &src, field)?;
    debug_assert!(query.filter.is_none());
    Ok(checked.sort)
}

fn sql_params(params: &[Param]) -> impl Iterator<Item = Sql> + '_ {
    params.iter().map(|p| match p {
        Param::Int(n) => Sql::Integer(*n),
        Param::Text(t) => Sql::Text(t.clone()),
    })
}

/// The IDs of the tasks `compiled` matches, in order.
pub(crate) fn matching_ids(tx: &Transaction<'_>, compiled: &Compiled) -> Result<Vec<Id>, Failure> {
    let sql = compiled.ids();
    let mut stmt = tx.prepare(&sql.text)?;
    let ids = stmt
        .query_map(params_from_iter(sql_params(&sql.params)), |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    if !crate::scope::active() {
        return Ok(ids);
    }
    let mut kept = Vec::with_capacity(ids.len());
    for id in ids {
        if lookup::task_by_id(tx, id)?.is_some() {
            kept.push(id);
        }
    }
    Ok(kept)
}

fn token_kind(k: TokenKind) -> QueryTokenKind {
    match k {
        TokenKind::Field => QueryTokenKind::Field,
        TokenKind::Operator => QueryTokenKind::Operator,
        TokenKind::Value => QueryTokenKind::Value,
        TokenKind::Comma => QueryTokenKind::Comma,
        TokenKind::Keyword => QueryTokenKind::Keyword,
        TokenKind::Negation => QueryTokenKind::Negation,
        TokenKind::Paren => QueryTokenKind::Paren,
        TokenKind::Text => QueryTokenKind::Text,
        TokenKind::Key => QueryTokenKind::Key,
        TokenKind::Sort => QueryTokenKind::Sort,
        TokenKind::Error => QueryTokenKind::Error,
    }
}

fn problem(e: &QueryError, src: &str) -> QueryProblem {
    QueryProblem {
        message: e.message(),
        message_id: e.id.to_owned(),
        args: e
            .args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        range: range(src, e.span),
    }
}

/// The fields of the query language: the built-in ones, then the
/// workspace's custom fields as `cf.<name>`.
fn registry(tx: &Transaction<'_>) -> Result<Vec<QueryField>, Failure> {
    let mut out: Vec<QueryField> = gk_query::fields::FIELDS
        .iter()
        .map(|f| QueryField {
            name: f.name.to_owned(),
            aliases: f.aliases.iter().map(|a| (*a).to_owned()).collect(),
            operators: f.ops.iter().map(|o| o.as_str().to_owned()).collect(),
            ranges: f.ranges,
            sortable: f.sortable,
            values: f.values.to_owned(),
            choices: f.choices.iter().map(|c| (*c).to_owned()).collect(),
        })
        .collect();
    for f in super::custom_field::query_fields(tx)? {
        out.push(QueryField {
            name: f.written(),
            aliases: Vec::new(),
            operators: f.ops().iter().map(|o| o.as_str().to_owned()).collect(),
            ranges: f.ranges(),
            sortable: true,
            values: f.values().to_owned(),
            choices: match f.kind {
                gk_core::FieldKind::Boolean => vec!["yes".to_owned(), "no".to_owned()],
                _ => f.options.clone(),
            },
        });
    }
    Ok(out)
}

fn parse_for_field(q: &QueryCtx<'_>, input: &QueryParseInput) -> Result<ParsedQuery, Failure> {
    let src = input.query.as_str();
    let (parsed, tokens) = gk_query::parse_with_tokens(src);
    let tokens = tokens
        .iter()
        .map(|t| QueryToken {
            kind: token_kind(t.kind),
            range: range(src, t.span),
        })
        .collect();
    let fields = if input.fields {
        Some(registry(q.tx)?)
    } else {
        None
    };
    let query = match parsed {
        Ok(query) => query,
        Err(e) => {
            return Ok(ParsedQuery {
                valid: false,
                normalized: None,
                tokens,
                chips: None,
                advanced: false,
                sort: Vec::new(),
                error: Some(problem(&e, src)),
                fields,
            });
        }
    };
    let chips = gk_query::chips(&query).map(|chips| {
        chips
            .into_iter()
            .map(|c| {
                let ranges = c.spans.iter().map(|s| range(src, *s)).collect();
                match c.kind {
                    ChipKind::Condition { field, op, values } => QueryChip {
                        field: Some(field),
                        op: Some(op.as_str().to_owned()),
                        values,
                        negated: c.negated,
                        ranges,
                    },
                    ChipKind::Text { words } => QueryChip {
                        field: None,
                        op: None,
                        values: words,
                        negated: c.negated,
                        ranges,
                    },
                }
            })
            .collect()
    });
    let error = try_compile(q.tx, &q.env.zone, q.now, src)?.err();
    Ok(ParsedQuery {
        valid: error.is_none(),
        normalized: Some(gk_query::print(&query)),
        tokens,
        advanced: chips.is_none(),
        chips,
        sort: query
            .sort
            .iter()
            .flat_map(|s| s.keys.iter())
            .map(|k| format!("{}{}", if k.descending { "-" } else { "" }, k.field))
            .collect(),
        error: error.map(|e| problem(&e, src)),
        fields,
    })
}

/// The compiled query of a search input, with its normal form.
pub(crate) fn compile_search(
    tx: &Transaction<'_>,
    zone: &Zone,
    now: i64,
    input: &TaskSearchInput,
) -> Result<(String, Compiled), Failure> {
    let (query, checked) = compile(tx, zone, now, &input.query, "query")?;
    let scope = match &input.project {
        Some(p) => Some(Scope {
            project: lookup::project(tx, "project", p)?.key,
            subprojects: input.include_subprojects,
        }),
        None => None,
    };
    let default_sort = match &input.sort {
        Some(s) if !s.trim().is_empty() => sort_keys(tx, zone, now, s, "sort")?,
        _ => Vec::new(),
    };
    let compiled = Compiled::new(
        &checked,
        &SqlEnv {
            now,
            scope,
            default_sort,
        },
    );
    Ok((gk_query::print(&query), compiled))
}

fn search(q: &QueryCtx<'_>, input: TaskSearchInput) -> Result<TaskSearchResult, Failure> {
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(messages::invalid(
            "limit",
            "query-task-limit",
            &[("max", MAX_LIMIT.to_string())],
        ));
    }
    let offset: u64 = match &input.cursor {
        Some(c) => c.parse().map_err(|_| {
            Failure::Op(messages::hint(
                messages::invalid_at("cursor", "list-cursor", &[]),
                "list-cursor-hint",
                &[],
            ))
        })?,
        None => 0,
    };
    let (normal, mut compiled) = compile_search(q.tx, &q.env.zone, q.now, &input)?;
    let groups = match input.group_by {
        Some(by) => {
            let week_start = super::settings::load(q.tx)?.week_start;
            let g = grouping(by, week_start);
            compiled.group_first(&g.rows);
            let sql = compiled.groups(
                &format!("{} AS g, {}, {}, {}", g.key, g.value, g.title, g.hue),
                "g",
                &g.order,
            );
            let mut stmt = q.tx.prepare(&sql.text)?;
            stmt.query_map(params_from_iter(sql_params(&sql.params)), |r| {
                Ok(ListGroup {
                    value: r.get(1)?,
                    title: r.get(2)?,
                    hue: r.get::<_, Option<String>>(3)?.and_then(|h| parse_enum(&h)),
                    count: u64::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        }
        None => Vec::new(),
    };
    let sql = compiled.totals();
    let totals =
        q.tx.query_row(&sql.text, params_from_iter(sql_params(&sql.params)), |r| {
            Ok(ListTotals {
                count: u64::try_from(r.get::<_, i64>(0)?).unwrap_or(0),
                estimate_minutes: r.get(1)?,
                spent_minutes: r.get(2)?,
                remaining_minutes: r.get(3)?,
                open: u64::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
            })
        })?;
    let sql = compiled.page(limit, offset);
    let ids = {
        let mut stmt = q.tx.prepare(&sql.text)?;
        stmt.query_map(params_from_iter(sql_params(&sql.params)), |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let items = ids
        .iter()
        .map(|id| listed(q.tx, *id, q.now))
        .collect::<Result<Vec<_>, _>>()?;
    let end = offset + items.len() as u64;
    Ok(TaskSearchResult {
        next_cursor: (end < totals.count && !items.is_empty()).then(|| end.to_string()),
        total: totals.count,
        items,
        query: normal,
        sort: compiled.sort().to_vec(),
        totals,
        groups,
    })
}

/// The SQL of a grouping, over the tables `t`, `p`, `s` and `y` of a
/// query: the key each task is grouped by, the row order that keeps the
/// groups together, a group's value, title and colour, and the order of
/// the groups. Tasks without a value (no label, parent or due date) come
/// last, as one group.
struct GroupSql {
    key: String,
    rows: String,
    value: String,
    title: String,
    hue: String,
    order: String,
}

fn grouping(by: ListGrouping, week_start: WeekStart) -> GroupSql {
    let text = |s: &str| s.to_owned();
    match by {
        ListGrouping::Status => {
            // Statuses of the same name in several projects are one group,
            // in the place the first of them has on its board.
            let place = "(SELECT min(x.rank) FROM statuses x WHERE x.name_fold = s.name_fold)";
            GroupSql {
                key: text("s.name_fold"),
                rows: format!("{place} ASC, s.name_fold ASC"),
                value: text("min(s.name)"),
                title: text("NULL"),
                hue: text("min(s.hue)"),
                order: format!("min({place}) ASC, g ASC"),
            }
        }
        ListGrouping::Priority => {
            let names = gk_query::fields::PRIORITIES
                .iter()
                .enumerate()
                .map(|(n, name)| format!("WHEN {n} THEN '{name}'"))
                .collect::<Vec<_>>()
                .join(" ");
            GroupSql {
                key: text("t.priority"),
                rows: text("t.priority DESC"),
                value: format!("CASE t.priority {names} END"),
                title: text("NULL"),
                hue: text("NULL"),
                order: text("g DESC"),
            }
        }
        ListGrouping::Label => {
            let first = |column: &str| {
                format!(
                    "(SELECT l.{column} FROM task_labels tl JOIN labels l ON l.id = tl.label_id \
                     WHERE tl.task_id = t.id ORDER BY l.name_fold, l.name LIMIT 1)"
                )
            };
            GroupSql {
                key: first("name_fold"),
                rows: format!("{} ASC NULLS LAST", first("name_fold")),
                value: format!("min({})", first("name")),
                title: text("NULL"),
                hue: format!("min({})", first("hue")),
                order: text("g IS NULL, g ASC"),
            }
        }
        ListGrouping::Parent => {
            let parent = |column: &str| {
                format!(
                    "(SELECT {column} FROM tasks k JOIN projects pp ON pp.id = k.project_id \
                     WHERE k.id = t.parent_id)"
                )
            };
            let key = parent("pp.key || '-' || k.number");
            GroupSql {
                rows: format!(
                    "{} ASC NULLS LAST, {} ASC NULLS LAST",
                    parent("pp.key"),
                    parent("k.number")
                ),
                value: format!("min({key})"),
                title: format!("min({})", parent("k.title")),
                hue: text("NULL"),
                order: format!(
                    "g IS NULL, min({}) ASC, min({}) ASC",
                    parent("pp.key"),
                    parent("k.number")
                ),
                key,
            }
        }
        ListGrouping::DueWeek => {
            let first_day = match week_start {
                WeekStart::Sunday => 0,
                WeekStart::Monday => 1,
                WeekStart::Saturday => 6,
            };
            // `%w` counts from Sunday (0); step back to the week's first day.
            let week = format!(
                "date(t.due_date, '-' || ((CAST(strftime('%w', t.due_date) AS INTEGER) \
                 - {first_day} + 7) % 7) || ' days')"
            );
            GroupSql {
                rows: format!("{week} ASC NULLS LAST"),
                value: week.clone(),
                title: text("NULL"),
                hue: text("NULL"),
                order: text("g IS NULL, g ASC"),
                key: week,
            }
        }
        ListGrouping::Project => GroupSql {
            key: text("p.key"),
            rows: text("p.key ASC"),
            value: text("p.key"),
            title: text("min(p.name)"),
            hue: text("NULL"),
            order: text("g ASC"),
        },
    }
}

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

/// One task as lists show it.
pub(crate) fn listed(tx: &Transaction<'_>, id: Id, now: i64) -> Result<ListedTask, Failure> {
    let sql = format!(
        "SELECT p.key, t.number, t.title, s.id, s.name, s.category, y.id, y.name, y.glyph,
                t.priority, t.parent_id, t.start_date, t.due_date, t.estimate_min, t.spent_min,
                t.checklist_done, t.checklist_total, {SUBTASKS_SQL}, {CLOSED_SUBTASKS_SQL},
                {BLOCKED_SQL}, {COMMENTS_SQL}, {FILES_SQL}, t.claim_session, t.claim_name,
                t.claim_until, t.rank, t.created_by_kind, t.created_by_name, t.created_at,
                t.updated_at, t.closed_at, t.version
           FROM tasks t JOIN projects p ON p.id = t.project_id
           JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id
          WHERE t.id = ?1"
    );
    let (mut task, parent) = tx.query_row(&sql, [b(id)], |r| {
        let project: String = r.get(0)?;
        let estimate: Option<i64> = r.get(13)?;
        let spent: i64 = r.get(14)?;
        Ok((
            ListedTask {
                key: format!("{project}-{}", r.get::<_, i64>(1)?),
                title: r.get(2)?,
                project,
                status: StatusRef {
                    id: id_at(r, 3)?.to_string(),
                    name: r.get(4)?,
                    category: parse_enum(&r.get::<_, String>(5)?).unwrap_or(Category::Todo),
                },
                task_type: TypeRef {
                    id: id_at(r, 6)?.to_string(),
                    name: r.get(7)?,
                    glyph: parse_enum(&r.get::<_, String>(8)?).unwrap_or(Glyph::Task),
                },
                priority: parse_enum(priority_name(r.get(9)?)).unwrap_or(Priority::None),
                labels: Vec::new(),
                fields: BTreeMap::new(),
                parent: None,
                start_date: r.get(11)?,
                due_date: r.get(12)?,
                estimate_minutes: estimate,
                spent_minutes: spent,
                remaining_minutes: estimate.map(|e| e - spent),
                progress_percent: 0,
                checklist: Checklist {
                    done: count(r.get(15)?),
                    total: count(r.get(16)?),
                },
                subtask_count: Checklist {
                    done: count(r.get(18)?),
                    total: count(r.get(17)?),
                },
                blocked: r.get(19)?,
                comment_count: count(r.get(20)?),
                file_count: count(r.get(21)?),
                claim: live_claim(opt_id_at(r, 22)?, r.get(23)?, r.get(24)?, now),
                rank: r.get(25)?,
                created_by: Actor {
                    kind: if r.get::<_, String>(26)? == "agent" {
                        ActorKind::Agent
                    } else {
                        ActorKind::User
                    },
                    name: r.get(27)?,
                },
                created_at: rfc3339(r.get(28)?),
                updated_at: rfc3339(r.get(29)?),
                closed_at: r.get::<_, Option<i64>>(30)?.map(rfc3339),
                version: r.get(31)?,
                id: id.to_string(),
            },
            opt_id_at(r, 10)?,
        ))
    })?;
    if let Some(parent) = parent {
        task.parent = tx
            .query_row(
                "SELECT p.key || '-' || t.number FROM tasks t JOIN projects p ON p.id = t.project_id
                  WHERE t.id = ?1",
                [b(parent)],
                |r| r.get(0),
            )
            .optional()?;
    }
    task.labels = task_labels(tx, id)?;
    task.fields = super::custom_field::listed_values(tx, id)?;
    task.progress_percent = super::rollup::progress(tx, id)?;
    Ok(task)
}
