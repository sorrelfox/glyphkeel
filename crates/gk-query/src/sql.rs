// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! SQL for a checked query, over the workspace schema (version 4).
//!
//! Every value is a bound parameter; the SQL text is made only of fixed
//! pieces. A condition is wrapped in `coalesce(…, 0)`, so it is always 0
//! or 1: a negation then includes tasks where the field is empty, as the
//! language promises (`-label:bug` includes tasks without labels).
//!
//! The tables are `tasks t`, `projects p`, `statuses s` and `task_types y`.
//! Text terms use the full-text index (`fts_words` over `search_docs`, plain
//! text of titles, descriptions and comments).

use std::fmt::Write as _;

use gk_core::Day;

use crate::check::{
    AuthorRef, Bounds, Checked, Cond, CustomCond, CustomTest, Filter, KeyRef, ProjectRef,
    RealBounds, Sort, SortBy,
};
use crate::fields::{
    CountOf, DateColumn, DurationColumn, FieldType, LinkField, MomentColumn, TextScope,
};

/// A bound parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Param {
    /// A whole number.
    Int(i64),
    /// Text.
    Text(String),
}

/// SQL text with its parameters, in order (`?` placeholders).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sql {
    /// The statement.
    pub text: String,
    /// The parameters for its `?`s, in order.
    pub params: Vec<Param>,
}

impl Sql {
    fn push(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn param(&mut self, p: Param) {
        self.text.push('?');
        self.params.push(p);
    }

    fn int(&mut self, n: i64) {
        self.param(Param::Int(n));
    }

    fn str(&mut self, s: &str) {
        self.param(Param::Text(s.to_owned()));
    }

    fn append(&mut self, other: Sql) {
        self.text.push_str(&other.text);
        self.params.extend(other.params);
    }
}

/// Where the list looks besides the query: a project, with or without its
/// sub-projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The project's key (current or old), upper case.
    pub project: String,
    /// Its sub-projects too.
    pub subprojects: bool,
}

/// What the SQL needs besides the query.
#[derive(Debug, Clone)]
pub struct SqlEnv {
    /// The current moment, for claims (milliseconds since 1970 UTC).
    pub now: i64,
    /// A project to look in, if any.
    pub scope: Option<Scope>,
    /// The order when the query has no `sort:` and no text terms; status
    /// order, then rank, when empty.
    pub default_sort: Vec<Sort>,
}

/// The SQL of one checked query.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// `WITH` clause (relevance), if any.
    with: Sql,
    /// The `FROM` clause with joins.
    from: String,
    /// The `WHERE` condition.
    filter: Sql,
    /// The `ORDER BY` list.
    order: Sql,
    /// The sort keys used, `-` for descending.
    sort: Vec<String>,
}

const FROM: &str = "tasks t JOIN projects p ON p.id = t.project_id \
JOIN statuses s ON s.id = t.status_id JOIN task_types y ON y.id = t.type_id";

impl Compiled {
    /// Builds the SQL of `checked`.
    #[must_use]
    pub fn new(checked: &Checked, env: &SqlEnv) -> Self {
        let today = checked.today;
        let mut filter = Sql::default();
        filter.push("t.deleted_at IS NULL AND p.deleted_at IS NULL AND ");
        // Tasks of archived projects only when the query names the project.
        let mut named = checked.named_projects();
        if let Some(scope) = &env.scope {
            named.push(ProjectRef {
                key: scope.project.clone(),
                subtree: scope.subprojects,
                span: crate::syntax::Span::default(),
            });
        }
        if named.is_empty() {
            filter.push("p.state = 'active'");
        } else {
            filter.push("(p.state = 'active' OR ");
            filter.append(projects(&named));
            filter.push(")");
        }
        if let Some(scope) = &env.scope {
            filter.push(" AND ");
            filter.append(projects(&[ProjectRef {
                key: scope.project.clone(),
                subtree: scope.subprojects,
                span: crate::syntax::Span::default(),
            }]));
        }
        if let Some(f) = &checked.filter {
            filter.push(" AND ");
            filter.append(cond_tree(f, today, env.now));
        }

        let terms = checked.text_terms();
        let mut with = Sql::default();
        let relevance = !terms.is_empty();
        let mut from = FROM.to_owned();
        if relevance {
            let expr: Vec<String> = terms
                .iter()
                .filter_map(|(_, text, phrase)| fts_expr(text, *phrase))
                .collect();
            if expr.is_empty() {
                with.push("WITH rel(task_id, score) AS (SELECT NULL, NULL WHERE 0) ");
            } else {
                with.push(
                    "WITH m(docid, score) AS MATERIALIZED (SELECT rowid, bm25(fts_words) \
                     FROM fts_words WHERE fts_words MATCH ",
                );
                with.str(&expr.join(" OR "));
                with.push(
                    "), rel(task_id, score) AS (SELECT d.task_id, min(m.score) \
                     FROM m JOIN search_docs d ON d.docid = m.docid",
                );
                with.push(" WHERE d.kind IN ('task', 'comment') GROUP BY d.task_id) ");
            }
            from.push_str(" LEFT JOIN rel ON rel.task_id = t.id");
        }

        let sorts: Vec<Sort> = if !checked.sort.is_empty() {
            checked.sort.clone()
        } else if relevance {
            vec![Sort {
                by: SortBy::Relevance,
                descending: false,
            }]
        } else {
            env.default_sort.clone()
        };
        let mut order = Sql::default();
        let mut keys = sorts;
        if keys.is_empty() {
            let status = crate::fields::find("status").map(SortBy::Field);
            keys.extend(status.map(|by| Sort {
                by,
                descending: false,
            }));
            keys.push(Sort {
                by: SortBy::Rank,
                descending: false,
            });
        }
        let mut names = Vec::with_capacity(keys.len());
        for key in &keys {
            let name = match &key.by {
                SortBy::Field(f) => f.name.to_owned(),
                SortBy::Custom(q) => format!("{}{q}", crate::fields::CUSTOM_PREFIX),
                SortBy::Rank => "rank".to_owned(),
                SortBy::Relevance => "relevance".to_owned(),
            };
            names.push(format!("{}{name}", if key.descending { "-" } else { "" }));
            for e in sort_exprs(&key.by, relevance) {
                let dir = if key.descending { "DESC" } else { "ASC" };
                order.append(e);
                let _ = write!(order.text, " {dir} NULLS LAST, ");
            }
        }
        order.push("p.key ASC, t.number ASC");
        Self {
            with,
            from,
            filter,
            order,
            sort: names,
        }
    }

    /// The sort keys the rows are ordered by (`-` for descending); ties go
    /// by project key and task number.
    #[must_use]
    pub fn sort(&self) -> &[String] {
        &self.sort
    }

    /// The matching task IDs in order, one page: `LIMIT ? OFFSET ?`.
    #[must_use]
    pub fn page(&self, limit: u32, offset: u64) -> Sql {
        let mut sql = self.ids();
        sql.push(" LIMIT ");
        sql.int(i64::from(limit));
        sql.push(" OFFSET ");
        sql.int(i64::try_from(offset).unwrap_or(i64::MAX));
        sql
    }

    /// Every matching task ID, in order.
    #[must_use]
    pub fn ids(&self) -> Sql {
        let mut sql = self.with.clone();
        let _ = write!(sql.text, "SELECT t.id FROM {} WHERE ", self.from);
        sql.append(self.filter.clone());
        sql.push(" ORDER BY ");
        sql.append(self.order.clone());
        sql
    }

    /// One row: how many tasks match, and their estimates, logged time and
    /// remaining time added up (minutes).
    #[must_use]
    pub fn totals(&self) -> Sql {
        let mut sql = Sql::default();
        let _ = write!(
            sql.text,
            "SELECT count(*), coalesce(sum(t.estimate_min), 0), coalesce(sum(t.spent_min), 0), \
             coalesce(sum(t.estimate_min - t.spent_min), 0), \
             coalesce(sum(s.category IN ('todo', 'in_progress')), 0) FROM {FROM} WHERE "
        );
        sql.append(self.filter.clone());
        sql
    }

    /// Keeps groups of rows together: `order` (an `ORDER BY` list over the
    /// tables `t`, `p`, `s` and `y`) comes before the query's own order.
    pub fn group_first(&mut self, order: &str) {
        self.order.text = format!("{order}, {}", self.order.text);
    }

    /// One row per group of the matching tasks: `SELECT {select}, count(*)
    /// … GROUP BY {group} ORDER BY {order}`, over the same tables as the
    /// rows.
    #[must_use]
    pub fn groups(&self, select: &str, group: &str, order: &str) -> Sql {
        let mut sql = Sql::default();
        let _ = write!(sql.text, "SELECT {select}, count(*) FROM {FROM} WHERE ");
        sql.append(self.filter.clone());
        let _ = write!(sql.text, " GROUP BY {group} ORDER BY {order}");
        sql
    }

    /// Whether the task `?` matches: `SELECT 1 … WHERE t.id = ? AND …`.
    #[must_use]
    pub fn matches_one(&self) -> Sql {
        let mut sql = Sql::default();
        let _ = write!(sql.text, "SELECT 1 FROM {FROM} WHERE t.id = ? AND ");
        sql.append(self.filter.clone());
        sql
    }
}

/// The FTS5 expression for a word (as a prefix) or a phrase; `None` when
/// the text has no letters or digits to search for.
fn fts_expr(text: &str, phrase: bool) -> Option<String> {
    if !text.chars().any(char::is_alphanumeric) {
        return None;
    }
    let quoted = format!("\"{}\"", text.replace('"', "\"\""));
    Some(if phrase { quoted } else { format!("{quoted}*") })
}

fn like_pattern(word: &str) -> String {
    let mut out = String::with_capacity(word.len() + 2);
    out.push('%');
    for c in word.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

fn cond_tree(f: &Filter, today: Day, now: i64) -> Sql {
    let mut sql = Sql::default();
    match f {
        Filter::And(items) | Filter::Or(items) => {
            let joint = if matches!(f, Filter::And(_)) {
                " AND "
            } else {
                " OR "
            };
            sql.push("(");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    sql.push(joint);
                }
                sql.append(cond_tree(item, today, now));
            }
            sql.push(")");
        }
        Filter::Not(inner) => {
            sql.push("NOT ");
            sql.append(cond_tree(inner, today, now));
        }
        Filter::Cond(c) => {
            sql.push("coalesce((");
            sql.append(cond(c, today, now));
            sql.push("), 0)");
        }
    }
    sql
}

/// `a OR b OR …`, or a false condition for none.
fn any(parts: Vec<Sql>) -> Sql {
    let mut sql = Sql::default();
    if parts.is_empty() {
        sql.push("0");
        return sql;
    }
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            sql.push(" OR ");
        }
        sql.append(p);
    }
    sql
}

fn bounds_on(column: &str, b: &[Bounds], as_day: bool) -> Vec<Sql> {
    b.iter()
        .map(|b| {
            let mut sql = Sql::default();
            let value = |sql: &mut Sql, n: i64| {
                if as_day {
                    let day = Day::from_days_since_epoch(n).map_or_else(
                        |_| {
                            if n < 0 {
                                "0000-00-00".to_owned()
                            } else {
                                "9999-99-99".to_owned()
                            }
                        },
                        |d| d.to_string(),
                    );
                    sql.str(&day);
                } else {
                    sql.int(n);
                }
            };
            match (b.lo, b.hi) {
                (Some(lo), Some(hi)) if lo == hi => {
                    let _ = write!(sql.text, "{column} = ");
                    value(&mut sql, lo);
                }
                (Some(lo), Some(hi)) => {
                    let _ = write!(sql.text, "{column} BETWEEN ");
                    value(&mut sql, lo);
                    sql.push(" AND ");
                    value(&mut sql, hi);
                }
                (Some(lo), None) => {
                    let _ = write!(sql.text, "{column} >= ");
                    value(&mut sql, lo);
                }
                (None, Some(hi)) => {
                    let _ = write!(sql.text, "{column} <= ");
                    value(&mut sql, hi);
                }
                (None, None) => {
                    let _ = write!(sql.text, "{column} IS NOT NULL");
                }
            }
            sql
        })
        .collect()
}

/// A task whose days, from its start to its due date, share a day with
/// `b`. A task with one of the dates is that one day; without either it
/// never matches (the comparisons are NULL).
fn span_overlaps(b: &Bounds) -> Sql {
    const FIRST: &str = "coalesce(t.start_date, t.due_date)";
    const LAST: &str = "coalesce(t.due_date, t.start_date)";
    let mut parts = Vec::new();
    // The task ends on or after the first day…
    if let Some(lo) = b.lo {
        parts.extend(bounds_on(
            LAST,
            &[Bounds {
                lo: Some(lo),
                hi: None,
            }],
            true,
        ));
    }
    // …and starts on or before the last one.
    if let Some(hi) = b.hi {
        parts.extend(bounds_on(
            FIRST,
            &[Bounds {
                lo: None,
                hi: Some(hi),
            }],
            true,
        ));
    }
    if parts.is_empty() {
        parts.extend(bounds_on(FIRST, &[Bounds { lo: None, hi: None }], true));
    }
    let mut sql = Sql::default();
    sql.push("(");
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            sql.push(" AND ");
        }
        sql.append(p);
    }
    sql.push(")");
    sql
}

fn with_none(column: &str, mut parts: Vec<Sql>, none: bool) -> Sql {
    if none {
        let mut sql = Sql::default();
        let _ = write!(sql.text, "{column} IS NULL");
        parts.push(sql);
    }
    any(parts)
}

/// The IDs of the tasks with these keys (current or old keys, old project
/// keys too).
fn key_ids(keys: &[KeyRef]) -> Sql {
    let mut sql = Sql::default();
    sql.push("(");
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            sql.push(" UNION ALL ");
        }
        let n = i64::from(k.number);
        sql.push(
            "SELECT k.id FROM tasks k JOIN projects kp ON kp.id = k.project_id WHERE kp.key = ",
        );
        sql.str(&k.project);
        sql.push(" AND k.number = ");
        sql.int(n);
        sql.push(" UNION ALL SELECT a.task_id FROM task_key_aliases a WHERE a.project_key = ");
        sql.str(&k.project);
        sql.push(" AND a.number = ");
        sql.int(n);
        sql.push(
            " UNION ALL SELECT k.id FROM tasks k JOIN project_key_aliases pa \
             ON pa.project_id = k.project_id WHERE pa.key = ",
        );
        sql.str(&k.project);
        sql.push(" AND k.number = ");
        sql.int(n);
    }
    if keys.is_empty() {
        sql.push("SELECT NULL WHERE 0");
    }
    sql.push(")");
    sql
}

/// `t.project_id IN (…)` for these projects (and their sub-projects where
/// asked).
fn projects(refs: &[ProjectRef]) -> Sql {
    let mut sql = Sql::default();
    sql.push("t.project_id IN (");
    for (i, r) in refs.iter().enumerate() {
        if i > 0 {
            sql.push(" UNION ALL ");
        }
        if r.subtree {
            sql.push(
                "SELECT * FROM (WITH RECURSIVE sub(id) AS (SELECT id FROM projects WHERE key = ",
            );
            sql.str(&r.key);
            sql.push(" OR id IN (SELECT project_id FROM project_key_aliases WHERE key = ");
            sql.str(&r.key);
            sql.push(
                ") UNION SELECT c.id FROM projects c JOIN sub ON c.parent_id = sub.id) \
                 SELECT id FROM sub)",
            );
        } else {
            sql.push("SELECT id FROM projects WHERE key = ");
            sql.str(&r.key);
            sql.push(" UNION ALL SELECT project_id FROM project_key_aliases WHERE key = ");
            sql.str(&r.key);
        }
    }
    sql.push(")");
    sql
}

fn names_in(column: &str, names: &[crate::check::NameRef]) -> Sql {
    let mut sql = Sql::default();
    let _ = write!(sql.text, "{column} IN (");
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            sql.push(", ");
        }
        sql.str(&n.folded);
    }
    sql.push(")");
    sql
}

const OPEN: &str = "s.category IN ('todo', 'in_progress')";
const BLOCKED: &str = "EXISTS (SELECT 1 FROM task_relations r JOIN tasks o ON o.id = r.from_task \
WHERE r.to_task = t.id AND r.kind = 'blocks' AND o.deleted_at IS NULL AND o.closed_at IS NULL)";
const HAS_SUBTASKS: &str =
    "EXISTS (SELECT 1 FROM tasks c WHERE c.parent_id = t.id AND c.deleted_at IS NULL)";
const COMMENTS: &str =
    "(SELECT count(*) FROM comments c WHERE c.task_id = t.id AND c.deleted_at IS NULL)";
const FILES: &str = "(SELECT count(*) FROM attachments a WHERE a.deleted_at IS NULL \
AND ((a.owner_kind = 'task' AND a.owner_id = t.id) OR (a.owner_kind = 'comment' AND a.owner_id IN \
(SELECT c.id FROM comments c WHERE c.task_id = t.id AND c.deleted_at IS NULL))))";
const SUBTASKS: &str =
    "(SELECT count(*) FROM tasks c WHERE c.parent_id = t.id AND c.deleted_at IS NULL)";
/// Progress in percent: set by hand, else 100 when closed, else the ticked
/// share of the checklist, else the average of the subtasks' own progress.
const PROGRESS: &str = "(CASE WHEN t.progress_override IS NOT NULL THEN t.progress_override \
WHEN t.closed_at IS NOT NULL THEN 100 \
WHEN t.checklist_total > 0 THEN t.checklist_done * 100 / t.checklist_total \
ELSE coalesce((SELECT CAST(round(avg(CASE WHEN c.progress_override IS NOT NULL THEN c.progress_override \
WHEN c.closed_at IS NOT NULL THEN 100 WHEN c.checklist_total > 0 \
THEN c.checklist_done * 100.0 / c.checklist_total ELSE 0 END)) AS INTEGER) \
FROM tasks c WHERE c.parent_id = t.id AND c.deleted_at IS NULL), 0) END)";

fn date_column(c: DateColumn) -> &'static str {
    match c {
        DateColumn::Start => "t.start_date",
        DateColumn::Due => "t.due_date",
    }
}

fn moment_column(c: MomentColumn) -> &'static str {
    match c {
        MomentColumn::Created => "t.created_at",
        MomentColumn::Edited => "t.edited_at",
        MomentColumn::Updated => "t.updated_at",
        MomentColumn::Closed => "t.closed_at",
    }
}

fn duration_column(c: DurationColumn) -> &'static str {
    match c {
        DurationColumn::Estimate => "t.estimate_min",
        DurationColumn::Spent => "t.spent_min",
        DurationColumn::Remaining => "(t.estimate_min - t.spent_min)",
    }
}

fn count_expr(c: CountOf) -> &'static str {
    match c {
        CountOf::Comments => COMMENTS,
        CountOf::Files => FILES,
        CountOf::Subtasks => SUBTASKS,
    }
}

fn day_text(d: Day, offset: i64) -> String {
    d.add_days(offset).unwrap_or(d).to_string()
}

#[allow(clippy::too_many_lines)]
fn cond(c: &Cond, today: Day, now: i64) -> Sql {
    let mut sql = Sql::default();
    match c {
        Cond::Custom(c) => sql.append(custom(c)),
        Cond::Keys(keys) => {
            sql.push("t.id IN ");
            sql.append(key_ids(keys));
        }
        Cond::BareKey(key) => {
            sql.push("t.id IN ");
            sql.append(key_ids(std::slice::from_ref(key)));
        }
        Cond::Projects(refs) => sql.append(projects(refs)),
        Cond::TitleWords(words) => {
            for (i, w) in words.iter().enumerate() {
                if i > 0 {
                    sql.push(" AND ");
                }
                sql.push("t.title_fold LIKE ");
                sql.str(&like_pattern(w));
                sql.push(" ESCAPE '\\'");
            }
        }
        Cond::TitleExact(title) => {
            sql.push("t.title_fold = ");
            sql.str(title);
        }
        Cond::Text {
            scope,
            text,
            phrase,
        } => match fts_expr(text, *phrase) {
            Some(expr) => {
                let (column, kinds) = match scope {
                    TextScope::All => ("", "('task', 'comment')"),
                    TextScope::Description => ("body : ", "('task')"),
                    TextScope::Comments => ("body : ", "('comment')"),
                };
                sql.push(
                    "t.id IN (SELECT d.task_id FROM fts_words JOIN search_docs d \
                     ON d.docid = fts_words.rowid WHERE fts_words MATCH ",
                );
                sql.str(&format!("{column}{expr}"));
                let _ = write!(sql.text, " AND d.kind IN {kinds})");
            }
            None => {
                // Nothing the index can find (only punctuation): look in
                // the title.
                sql.push("t.title_fold LIKE ");
                sql.str(&like_pattern(&gk_core::fold(text)));
                sql.push(" ESCAPE '\\'");
            }
        },
        Cond::Statuses(names) => sql.append(names_in("s.name_fold", names)),
        Cond::Types(names) => sql.append(names_in("y.name_fold", names)),
        Cond::Labels(names) => {
            sql.push(
                "EXISTS (SELECT 1 FROM task_labels tl JOIN labels l ON l.id = tl.label_id \
                 WHERE tl.task_id = t.id AND ",
            );
            sql.append(names_in("l.name_fold", names));
            sql.push(")");
        }
        Cond::Categories(cats) => {
            sql.push("s.category IN (");
            for (i, c) in cats.iter().enumerate() {
                if i > 0 {
                    sql.push(", ");
                }
                sql.str(c);
            }
            sql.push(")");
        }
        Cond::Priority(b) => sql.append(any(bounds_on("t.priority", b, false))),
        Cond::Parent { keys, none } => {
            let mut parts = Vec::new();
            if !keys.is_empty() {
                let mut p = Sql::default();
                p.push("t.parent_id IN ");
                p.append(key_ids(keys));
                parts.push(p);
            }
            sql.append(with_none("t.parent_id", parts, *none));
        }
        Cond::Under(keys) => {
            sql.push(
                "t.id IN (SELECT * FROM (WITH RECURSIVE below(id) AS \
                 (SELECT c.id FROM tasks c WHERE c.parent_id IN ",
            );
            sql.append(key_ids(keys));
            sql.push(
                " UNION SELECT c.id FROM tasks c JOIN below ON c.parent_id = below.id) \
                 SELECT id FROM below))",
            );
        }
        Cond::Link { link, keys } => {
            let (this, other, kind) = match link {
                LinkField::Blocks => ("r.from_task", "r.to_task", "blocks"),
                LinkField::BlockedBy => ("r.to_task", "r.from_task", "blocks"),
                LinkField::Duplicates => ("r.from_task", "r.to_task", "duplicates"),
                LinkField::Relates => ("", "", "relates"),
            };
            let others = |sql: &mut Sql, column: &str| {
                if !keys.is_empty() {
                    let _ = write!(sql.text, " AND {column} IN ");
                    sql.append(key_ids(keys));
                }
            };
            if this.is_empty() {
                // `relates` is stored once, either way round.
                sql.push(
                    "EXISTS (SELECT 1 FROM task_relations r JOIN tasks o ON o.id = r.to_task \
                     WHERE r.kind = 'relates' AND r.from_task = t.id AND o.deleted_at IS NULL",
                );
                others(&mut sql, "r.to_task");
                sql.push(
                    ") OR EXISTS (SELECT 1 FROM task_relations r JOIN tasks o ON o.id = r.from_task \
                     WHERE r.kind = 'relates' AND r.to_task = t.id AND o.deleted_at IS NULL",
                );
                others(&mut sql, "r.from_task");
                sql.push(")");
            } else {
                let _ = write!(
                    sql.text,
                    "EXISTS (SELECT 1 FROM task_relations r JOIN tasks o ON o.id = {other} \
                     WHERE r.kind = '{kind}' AND {this} = t.id AND o.deleted_at IS NULL"
                );
                others(&mut sql, other);
                sql.push(")");
            }
        }
        Cond::Date {
            column,
            bounds,
            none,
        } => {
            let col = date_column(*column);
            sql.append(with_none(col, bounds_on(col, bounds, true), *none));
        }
        Cond::Span(bounds) => sql.append(any(bounds.iter().map(span_overlaps).collect())),
        Cond::Moment {
            column,
            bounds,
            none,
        } => {
            let col = moment_column(*column);
            sql.append(with_none(col, bounds_on(col, bounds, false), *none));
        }
        Cond::Duration {
            column,
            bounds,
            none,
        } => {
            let col = duration_column(*column);
            let none_col = if *column == DurationColumn::Spent {
                // Logged time is never empty; `none` means nothing logged.
                "nullif(t.spent_min, 0)"
            } else {
                col
            };
            sql.append(with_none(none_col, bounds_on(col, bounds, false), *none));
        }
        Cond::Progress(b) => sql.append(any(bounds_on(PROGRESS, b, false))),
        Cond::Count(of, b) => sql.append(any(bounds_on(count_expr(*of), b, false))),
        Cond::Author(who) => {
            let parts = who
                .iter()
                .map(|a| {
                    let mut p = Sql::default();
                    match a {
                        AuthorRef::Me => p.push("t.created_by_kind = 'user'"),
                        AuthorRef::Agent => p.push("t.created_by_kind = 'agent'"),
                        AuthorRef::Named(name) => {
                            p.push("t.created_by_name = ");
                            p.str(name);
                            p.push(" COLLATE NOCASE");
                        }
                    }
                    p
                })
                .collect();
            sql.append(any(parts));
        }
        Cond::Claimed(names) => {
            sql.push("t.claim_session IS NOT NULL AND t.claim_until > ");
            sql.int(now);
            if let Some(names) = names {
                sql.push(" AND t.claim_name COLLATE NOCASE IN (");
                for (i, n) in names.iter().enumerate() {
                    if i > 0 {
                        sql.push(", ");
                    }
                    sql.str(n);
                }
                sql.push(")");
            }
        }
        Cond::Is(flags) => {
            let parts = flags
                .iter()
                .map(|flag| {
                    let mut p = Sql::default();
                    match *flag {
                        "open" => p.push(OPEN),
                        "closed" => p.push("s.category IN ('done', 'cancelled')"),
                        "overdue" => {
                            let _ = write!(p.text, "{OPEN} AND t.due_date < ");
                            p.str(&today.to_string());
                        }
                        "due-soon" => {
                            let _ = write!(p.text, "{OPEN} AND t.due_date BETWEEN ");
                            p.str(&today.to_string());
                            p.push(" AND ");
                            p.str(&day_text(today, 3));
                        }
                        "blocked" => p.push(BLOCKED),
                        "claimed" => {
                            p.push("t.claim_session IS NOT NULL AND t.claim_until > ");
                            p.int(now);
                        }
                        "subtask" => p.push("t.parent_id IS NOT NULL"),
                        "parent" => p.push(HAS_SUBTASKS),
                        "unscheduled" => p.push("t.start_date IS NULL AND t.due_date IS NULL"),
                        "milestone" => p.push("y.glyph = 'milestone'"),
                        "over-estimate" => {
                            p.push("t.estimate_min IS NOT NULL AND t.spent_min > t.estimate_min");
                        }
                        _ => p.push("0"),
                    }
                    p
                })
                .map(|p| {
                    let mut w = Sql::default();
                    w.push("(");
                    w.append(p);
                    w.push(")");
                    w
                })
                .collect();
            sql.append(any(parts));
        }
        Cond::Has(parts) => {
            let parts = parts
                .iter()
                .map(|part| {
                    let mut p = Sql::default();
                    p.push(match *part {
                        "due" => "t.due_date IS NOT NULL",
                        "start" => "t.start_date IS NOT NULL",
                        "estimate" => "t.estimate_min IS NOT NULL",
                        "description" => "t.description <> ''",
                        "labels" => "EXISTS (SELECT 1 FROM task_labels tl WHERE tl.task_id = t.id)",
                        "files" => "0 < (SELECT count(*) FROM attachments a WHERE a.deleted_at IS NULL \
                             AND ((a.owner_kind = 'task' AND a.owner_id = t.id) OR (a.owner_kind = 'comment' \
                             AND a.owner_id IN (SELECT c.id FROM comments c WHERE c.task_id = t.id \
                             AND c.deleted_at IS NULL))))",
                        "comments" => "EXISTS (SELECT 1 FROM comments c WHERE c.task_id = t.id \
                             AND c.deleted_at IS NULL)",
                        "subtasks" => HAS_SUBTASKS,
                        "parent" => "t.parent_id IS NOT NULL",
                        "links" => "EXISTS (SELECT 1 FROM task_relations r JOIN tasks o \
                             ON o.id = CASE WHEN r.from_task = t.id THEN r.to_task ELSE r.from_task END \
                             WHERE (r.from_task = t.id OR r.to_task = t.id) AND o.deleted_at IS NULL)",
                        "time" => "(t.spent_min > 0 OR EXISTS (SELECT 1 FROM time_entries e \
                             WHERE e.task_id = t.id AND e.deleted_at IS NULL))",
                        "reminder" => "EXISTS (SELECT 1 FROM reminders m WHERE m.task_id = t.id)",
                        _ => "0",
                    });
                    p
                })
                .collect();
            sql.append(any(parts));
        }
    }
    sql
}

/// A condition on a custom field: whether one of the task's values of the
/// fields with this query name passes the test, or the task has none.
fn custom(c: &CustomCond) -> Sql {
    let values = |sql: &mut Sql| {
        sql.push(
            "SELECT 1 FROM custom_values v JOIN custom_fields f ON f.id = v.field_id \
             WHERE v.task_id = t.id AND f.query_name = ",
        );
        sql.str(&c.query_name);
    };
    let mut parts = Vec::new();
    if c.none {
        let mut sql = Sql::default();
        sql.push("NOT EXISTS (");
        values(&mut sql);
        sql.push(")");
        parts.push(sql);
    }
    let test: Option<Vec<Sql>> = match &c.test {
        CustomTest::Nothing => None,
        CustomTest::Anything => {
            let mut sql = Sql::default();
            sql.push("1");
            parts.push(sql);
            None
        }
        CustomTest::Boolean(false) => {
            // No value counts as no.
            let mut sql = Sql::default();
            sql.push("NOT EXISTS (");
            values(&mut sql);
            sql.push(" AND v.value = 1)");
            parts.push(sql);
            None
        }
        CustomTest::Boolean(true) => Some(vec![Sql {
            text: "v.value = 1".to_owned(),
            params: Vec::new(),
        }]),
        CustomTest::Contains(texts) => Some(
            texts
                .iter()
                .map(|t| {
                    let mut sql = Sql::default();
                    sql.push("v.value LIKE ");
                    sql.str(&like_pattern(t));
                    sql.push(" ESCAPE '\\'");
                    sql
                })
                .collect(),
        ),
        CustomTest::Exact(texts) | CustomTest::Options(texts) => {
            let mut sql = Sql::default();
            sql.push("lower(v.value) IN (");
            for (i, t) in texts.iter().enumerate() {
                if i > 0 {
                    sql.push(", ");
                }
                sql.push("lower(");
                sql.str(t);
                sql.push(")");
            }
            sql.push(")");
            Some(vec![sql])
        }
        CustomTest::Integer(b) => Some(bounds_on("v.value", b, false)),
        CustomTest::Date(b) => Some(bounds_on("v.value", b, true)),
        CustomTest::Decimal(b) => Some(b.iter().map(real_bounds).collect()),
    };
    if let Some(tests) = test {
        let mut sql = Sql::default();
        sql.push("EXISTS (");
        values(&mut sql);
        sql.push(" AND (");
        sql.append(any(tests));
        sql.push("))");
        parts.push(sql);
    }
    any(parts)
}

fn real_bounds(b: &RealBounds) -> Sql {
    let mut sql = Sql::default();
    let mut first = true;
    for (bound, below) in [(&b.lo, false), (&b.hi, true)] {
        let Some(bound) = bound else { continue };
        if !first {
            sql.push(" AND ");
        }
        first = false;
        let op = match (below, bound.inclusive) {
            (false, true) => ">=",
            (false, false) => ">",
            (true, true) => "<=",
            (true, false) => "<",
        };
        let _ = write!(sql.text, "v.value {op} CAST(");
        sql.str(&bound.value);
        sql.push(" AS REAL)");
    }
    if first {
        sql.push("1");
    }
    sql
}

/// The expressions a sort key orders by.
fn sort_exprs(by: &SortBy, relevance: bool) -> Vec<Sql> {
    let fixed = |e: Vec<&str>| {
        e.into_iter()
            .map(|t| Sql {
                text: t.to_owned(),
                params: Vec::new(),
            })
            .collect()
    };
    if let SortBy::Custom(query_name) = by {
        let mut sql = Sql::default();
        sql.push(
            "(SELECT min(v.value) FROM custom_values v JOIN custom_fields f ON f.id = v.field_id \
             WHERE v.task_id = t.id AND f.query_name = ",
        );
        sql.str(query_name);
        sql.push(")");
        return vec![sql];
    }
    fixed(match *by {
        SortBy::Custom(_) => Vec::new(),
        SortBy::Rank => vec!["t.rank"],
        SortBy::Relevance => {
            if relevance {
                vec!["rel.score"]
            } else {
                Vec::new()
            }
        }
        SortBy::Field(f) => match f.ty {
            FieldType::Key => vec!["p.key", "t.number"],
            FieldType::Project => vec!["p.key"],
            FieldType::Title => vec!["t.title_fold"],
            FieldType::Status => vec!["s.rank"],
            FieldType::Category => vec![
                "CASE s.category WHEN 'todo' THEN 0 WHEN 'in_progress' THEN 1 WHEN 'done' THEN 2 ELSE 3 END",
            ],
            FieldType::Type => vec!["y.name_fold"],
            FieldType::Priority => vec!["t.priority"],
            FieldType::Parent => vec!["(SELECT k.number FROM tasks k WHERE k.id = t.parent_id)"],
            FieldType::Date(c) => vec![date_column(c)],
            FieldType::Moment(c) => vec![moment_column(c)],
            FieldType::Duration(c) => vec![duration_column(c)],
            FieldType::Percent => vec![PROGRESS],
            FieldType::Author => vec!["t.created_by_name"],
            FieldType::Count(c) => vec![count_expr(c)],
            FieldType::FullText(_)
            | FieldType::Span
            | FieldType::Label
            | FieldType::Under
            | FieldType::Link(_)
            | FieldType::Claimed
            | FieldType::Is
            | FieldType::Has => Vec::new(),
        },
    })
}
