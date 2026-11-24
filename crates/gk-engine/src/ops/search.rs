// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Global search over the full-text index (`search_docs` with its two FTS5
//! tables): task titles and descriptions, comments, wiki pages and file
//! names.
//!
//! Matching: every word must occur, as a whole word or a word's beginning
//! (`fts_words`, with English word forms); a "quoted phrase" as written.
//! When the words find nothing in a document, fragments of three or more
//! characters still match anywhere in it (`fts_grams`), so `CONN` finds
//! `ERR_CONN`.
//!
//! Ranking, best first, in groups: a task key or project key typed in
//! full; titles of tasks and pages holding every word; the text of tasks and
//! pages; comments and file names; fragments only. Within a group the
//! better BM25 score (titles weigh ten times the text), then the newer
//! change, then the older document.

use std::collections::{HashMap, HashSet};

use gk_api::ops::common::Category;
use gk_api::ops::query::{TaskSearchInput, TextRange};
use gk_api::ops::search::{
    MatchedIn, SearchHit, SearchInput, SearchKind, SearchQuery, SearchResult,
};
use gk_core::{Id, ProjectKey, TaskKey};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};

use super::desc;
use super::lookup::{self, b, id_at, opt_id_at};
use super::project::parse_enum;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;

/// Results per page unless asked otherwise.
const DEFAULT_LIMIT: u32 = 50;
/// The most results per page.
const MAX_LIMIT: u32 = 200;
/// Marks the start of a match in highlighted text.
pub(crate) const OPEN: char = '\u{1}';
/// Marks the end of a match.
pub(crate) const CLOSE: char = '\u{2}';
/// Words of context in a snippet.
const SNIPPET_WORDS: u32 = 16;

pub(super) fn register(r: &mut Registry) {
    r.query::<SearchQuery, _>(desc::<SearchQuery>(), search);
}

// ---------------------------------------------------------------------
// Search text

/// One word or phrase of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Term {
    pub(crate) text: String,
    pub(crate) phrase: bool,
}

/// The words and "phrases" of `text`; pieces without a letter or digit are
/// left out.
pub(crate) fn terms(text: &str) -> Vec<Term> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix('"') {
            let (phrase, next) = after.split_once('"').unwrap_or((after, ""));
            out.push(Term {
                text: phrase.trim().to_owned(),
                phrase: true,
            });
            rest = next;
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let (word, next) = rest.split_at(end);
            out.push(Term {
                text: word.trim_matches('"').to_owned(),
                phrase: false,
            });
            rest = next;
        }
    }
    out.retain(|t| t.text.chars().any(char::is_alphanumeric));
    out
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The `fts_words` expression: every term, words as prefixes.
pub(crate) fn words_expr(terms: &[Term]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    let parts: Vec<String> = terms
        .iter()
        .map(|t| {
            let q = quoted(&t.text);
            if t.phrase { q } else { format!("{q}*") }
        })
        .collect();
    Some(parts.join(" AND "))
}

/// The `fts_grams` expression: every term as a fragment. `None` when a
/// term is shorter than three characters, which fragments can't find.
pub(crate) fn grams_expr(terms: &[Term]) -> Option<String> {
    if terms.is_empty() || terms.iter().any(|t| t.text.chars().count() < 3) {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|t| quoted(&t.text))
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

/// `expr` limited to one column.
pub(crate) fn in_column(column: &str, expr: &str) -> String {
    format!("{{{column}}} : ({expr})")
}

/// The UTF-16 length of `text`, as the page's editor counts.
fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// `text` without the match marks, on one line (every run of whitespace one
/// space, none at the ends), and where the marked matches are.
pub(crate) fn unmark(text: &str) -> (String, Vec<TextRange>) {
    let mut out = String::with_capacity(text.len());
    let mut ranges = Vec::new();
    let mut start = None;
    let mut space = false;
    for c in text.chars() {
        match c {
            c if c.is_whitespace() => space = !out.is_empty(),
            CLOSE => {
                if let Some(from) = start.take() {
                    let to = utf16_len(&out);
                    if to > from {
                        ranges.push(TextRange { from, to });
                    }
                }
            }
            c => {
                if space {
                    out.push(' ');
                    space = false;
                }
                if c == OPEN {
                    start = Some(utf16_len(&out));
                } else {
                    out.push(c);
                }
            }
        }
    }
    (out, ranges)
}

/// Where `needles` occur in `hay`, ignoring case, in UTF-16 code units;
/// overlapping matches merged.
pub(crate) fn find_ranges(hay: &str, needles: &[&str]) -> Vec<TextRange> {
    let lower: Vec<(u32, char)> = {
        let mut at = 0u32;
        hay.chars()
            .map(|c| {
                let here = at;
                at += u32::try_from(c.len_utf16()).unwrap_or(1);
                (here, lowered(c))
            })
            .collect()
    };
    let end = utf16_len(hay);
    let mut ranges: Vec<TextRange> = Vec::new();
    for needle in needles {
        let n: Vec<char> = needle.chars().map(lowered).collect();
        if n.is_empty() || n.len() > lower.len() {
            continue;
        }
        for i in 0..=lower.len() - n.len() {
            if lower[i..i + n.len()]
                .iter()
                .map(|(_, c)| *c)
                .eq(n.iter().copied())
            {
                let from = lower[i].0;
                let to = lower.get(i + n.len()).map_or(end, |(at, _)| *at);
                ranges.push(TextRange { from, to });
            }
        }
    }
    ranges.sort_by_key(|r| (r.from, r.to));
    let mut merged: Vec<TextRange> = Vec::new();
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.from <= last.to => last.to = last.to.max(r.to),
            _ => merged.push(r),
        }
    }
    merged
}

fn lowered(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// A page size and an offset from `limit` and `cursor`.
pub(crate) fn paging(
    limit: Option<u32>,
    cursor: Option<&str>,
    default: u32,
) -> Result<(u32, usize), Failure> {
    let limit = limit.unwrap_or(default);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(messages::invalid(
            "limit",
            "list-limit",
            &[("max", MAX_LIMIT.to_string())],
        ));
    }
    let offset = match cursor {
        Some(c) => c
            .parse()
            .map_err(|_| messages::invalid("cursor", "list-cursor", &[]))?,
        None => 0,
    };
    Ok((limit, offset))
}

// ---------------------------------------------------------------------
// Matching

/// One document that matched, before the page is built.
#[derive(Debug, Clone)]
struct Found {
    docid: i64,
    kind: String,
    entity: Id,
    task: Option<Id>,
    project: Id,
    tier: u8,
    score: f64,
    updated_at: i64,
    fragment: bool,
}

/// What a result list holds before it is turned into hits.
#[derive(Debug, Clone)]
enum Entry {
    Project(Id),
    TaskKey(Id),
    Doc(Found),
}

/// The documents matching `words` (and `grams`, the fragment expression),
/// unfiltered, best first.
fn matches(tx: &Transaction<'_>, words: &str, grams: Option<&str>) -> Result<Vec<Found>, Failure> {
    let title = in_column("title", words);
    let body = in_column("body", words);
    let grams_all = grams.unwrap_or("\"\u{1}\u{1}\u{1}\"");
    let grams_body = in_column("body", grams_all);
    // A fragment expression that can match nothing stands in when there is
    // none, so the statement stays the same.
    let sql = "WITH
        w(docid, score) AS MATERIALIZED (
          SELECT rowid, bm25(fts_words, 10.0, 1.0) FROM fts_words WHERE fts_words MATCH ?1),
        g(docid, score) AS MATERIALIZED (
          SELECT rowid, bm25(fts_grams, 10.0, 1.0) FROM fts_grams WHERE fts_grams MATCH ?4),
        c(docid) AS (SELECT docid FROM w UNION SELECT docid FROM g)
        SELECT d.docid, d.kind, d.entity_id, d.task_id, d.project_id, d.updated_at,
               w.score, g.score,
               d.docid IN (SELECT rowid FROM fts_words WHERE fts_words MATCH ?2),
               d.docid IN (SELECT rowid FROM fts_words WHERE fts_words MATCH ?3),
               d.docid IN (SELECT rowid FROM fts_grams WHERE fts_grams MATCH ?5)
          FROM c JOIN search_docs d ON d.docid = c.docid
          LEFT JOIN w ON w.docid = d.docid LEFT JOIN g ON g.docid = d.docid";
    let mut stmt = tx.prepare(sql)?;
    let rows = stmt
        .query_map(params![words, title, body, grams_all, grams_body], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                id_at(r, 2)?,
                opt_id_at(r, 3)?,
                id_at(r, 4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<f64>>(7)?,
                r.get::<_, bool>(8)?,
                r.get::<_, bool>(9)?,
                r.get::<_, bool>(10)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (docid, kind, entity, task, project, updated_at, w, g, in_title, in_body, g_body) in rows {
        let titled = matches!(kind.as_str(), "task" | "wiki");
        // Comments carry their task's title for display; only their own
        // text counts.
        let (tier, score, fragment) = match (kind.as_str(), w, g) {
            (_, Some(w), _) if titled && in_title => (0, w, false),
            (_, Some(w), _) if titled => (1, w, false),
            ("comment", Some(w), _) if in_body => (2, w, false),
            ("file", Some(w), _) => (2, w, false),
            ("comment", _, Some(g)) if g_body => (3, g, true),
            ("comment", _, _) => continue,
            (_, _, Some(g)) => (3, g, true),
            _ => continue,
        };
        out.push(Found {
            docid,
            kind,
            entity,
            task,
            project,
            tier,
            score,
            updated_at,
            fragment,
        });
    }
    out.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(a.score.total_cmp(&b.score))
            .then(b.updated_at.cmp(&a.updated_at))
            .then(a.docid.cmp(&b.docid))
    });
    Ok(out)
}

/// Whether search results from the project `id` may be listed: live, and
/// active unless the scope names it.
fn visible(
    tx: &Transaction<'_>,
    cache: &mut HashMap<Id, bool>,
    scope: Option<&HashSet<Id>>,
    id: Id,
) -> Result<bool, Failure> {
    if let Some(scope) = scope {
        return Ok(scope.contains(&id));
    }
    if let Some(v) = cache.get(&id) {
        return Ok(*v);
    }
    let v = lookup::project_by_id(tx, id)?.is_some_and(|p| !p.archived);
    cache.insert(id, v);
    Ok(v)
}

/// Whether the item behind a document is live: its task (and the comment
/// or file), or its page; a file only while what it is attached to is.
fn live(tx: &Transaction<'_>, f: &Found) -> Result<bool, Failure> {
    let sql = match f.kind.as_str() {
        "task" => "SELECT deleted_at IS NULL FROM tasks WHERE id = ?1",
        "comment" => {
            "SELECT c.deleted_at IS NULL AND t.deleted_at IS NULL FROM comments c
               JOIN tasks t ON t.id = c.task_id WHERE c.id = ?1"
        }
        "wiki" => "SELECT deleted_at IS NULL FROM wiki_pages WHERE id = ?1",
        "file" => {
            "SELECT a.deleted_at IS NULL AND CASE a.owner_kind
                 WHEN 'task' THEN EXISTS (SELECT 1 FROM tasks t
                                           WHERE t.id = a.owner_id AND t.deleted_at IS NULL)
                 WHEN 'comment' THEN EXISTS (SELECT 1 FROM comments c
                                              JOIN tasks t ON t.id = c.task_id
                                             WHERE c.id = a.owner_id AND c.deleted_at IS NULL
                                               AND t.deleted_at IS NULL)
                 WHEN 'wiki_page' THEN EXISTS (SELECT 1 FROM wiki_pages w
                                                WHERE w.id = a.owner_id AND w.deleted_at IS NULL)
                 WHEN 'project' THEN EXISTS (SELECT 1 FROM projects p
                                              WHERE p.id = a.owner_id AND p.deleted_at IS NULL)
                 ELSE 0 END
               FROM attachments a WHERE a.id = ?1"
        }
        _ => return Ok(false),
    };
    Ok(tx
        .query_row(sql, [b(f.entity)], |r| r.get::<_, bool>(0))
        .optional()?
        .unwrap_or(false))
}

fn kind_of(kind: &str) -> SearchKind {
    match kind {
        "comment" => SearchKind::Comment,
        "wiki" => SearchKind::Wiki,
        "file" => SearchKind::File,
        _ => SearchKind::Task,
    }
}

fn search(q: &QueryCtx<'_>, input: SearchInput) -> Result<SearchResult, Failure> {
    let tx = q.tx;
    let (limit, offset) = paging(input.limit, input.cursor.as_deref(), DEFAULT_LIMIT)?;
    let text = input.text.trim();
    let terms = terms(text);
    let jump_key = text.parse::<TaskKey>().is_ok();
    let jump_project = ProjectKey::parse_lenient(text).is_ok();
    if terms.is_empty() && !jump_key && !jump_project {
        return Err(messages::invalid("text", "search-empty", &[]));
    }
    let wanted = |k: SearchKind| input.kinds.is_empty() || input.kinds.contains(&k);
    let scope = match &input.project {
        Some(p) => {
            let project = lookup::project(tx, "project", p)?;
            let ids = if input.include_subprojects {
                super::project::subtree(tx, project.id)?
            } else {
                vec![project.id]
            };
            Some(ids.into_iter().collect::<HashSet<Id>>())
        }
        None => None,
    };
    let tasks: Option<HashSet<Id>> = match &input.query {
        Some(query) if !query.trim().is_empty() => {
            let search = TaskSearchInput {
                query: query.clone(),
                project: input.project.clone(),
                include_subprojects: input.include_subprojects,
                ..TaskSearchInput::default()
            };
            let (_, compiled) = super::query::compile_search(tx, &q.env.zone, q.now, &search)?;
            Some(
                super::query::matching_ids(tx, &compiled)?
                    .into_iter()
                    .collect(),
            )
        }
        _ => None,
    };

    let mut entries = Vec::new();
    let mut seen_task = None;
    let mut projects = HashMap::new();
    if jump_key
        && wanted(SearchKind::Task)
        && let Ok(task) = lookup::task(tx, "text", text)
        && visible(tx, &mut projects, scope.as_ref(), task.project)?
        && tasks.as_ref().is_none_or(|t| t.contains(&task.id))
    {
        entries.push(Entry::TaskKey(task.id));
        seen_task = Some(task.id);
    }
    if jump_project
        && tasks.is_none()
        && wanted(SearchKind::Project)
        && let Ok(project) = lookup::project(tx, "text", text)
        && visible(tx, &mut projects, scope.as_ref(), project.id)?
    {
        entries.push(Entry::Project(project.id));
    }
    if let Some(words) = words_expr(&terms) {
        let grams = grams_expr(&terms);
        for found in matches(tx, &words, grams.as_deref())? {
            let kind = kind_of(&found.kind);
            if !wanted(kind)
                || (kind == SearchKind::Task && seen_task == Some(found.entity))
                || !visible(tx, &mut projects, scope.as_ref(), found.project)?
            {
                continue;
            }
            if let Some(tasks) = &tasks {
                match found.task {
                    Some(t) if tasks.contains(&t) => {}
                    _ => continue,
                }
            }
            if !live(tx, &found)? {
                continue;
            }
            entries.push(Entry::Doc(found));
        }
    }

    let total = entries.len();
    let page: Vec<&Entry> = entries.iter().skip(offset).take(limit as usize).collect();
    let mut items = Vec::with_capacity(page.len());
    for entry in &page {
        items.push(hit(tx, entry, &terms)?);
    }
    let end = offset + page.len();
    Ok(SearchResult {
        items,
        total: total as u64,
        next_cursor: (end < total && !page.is_empty()).then(|| end.to_string()),
    })
}

// ---------------------------------------------------------------------
// Building results

/// A task's key, title, status and category.
fn task_facts(
    tx: &Transaction<'_>,
    id: Id,
) -> Result<(String, String, String, Category, i64), Failure> {
    let (project, number, title, status, category, updated): (
        Vec<u8>,
        i64,
        String,
        String,
        String,
        i64,
    ) = tx.query_row(
        "SELECT t.project_id, t.number, t.title, s.name, s.category, t.updated_at
           FROM tasks t JOIN statuses s ON s.id = t.status_id WHERE t.id = ?1",
        [b(id)],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    let project = Id::from_slice(&project)
        .map_err(|e| Failure::Store(gk_store::StoreError::Corrupt(e.to_string())))?;
    Ok((
        lookup::task_key(tx, project, number)?,
        title,
        status,
        parse_enum(&category).unwrap_or(Category::Todo),
        updated,
    ))
}

fn project_key(tx: &Transaction<'_>, id: Id) -> Result<String, Failure> {
    Ok(
        tx.query_row("SELECT key FROM projects WHERE id = ?1", [b(id)], |r| {
            r.get(0)
        })?,
    )
}

/// The highlighted title and the snippet of document `docid`.
fn marks(
    tx: &Transaction<'_>,
    docid: i64,
    terms: &[Term],
    fragment: bool,
) -> Result<Option<(String, String)>, Failure> {
    let (table, expr) = if fragment {
        ("fts_grams", grams_expr(terms))
    } else {
        ("fts_words", words_expr(terms))
    };
    let Some(expr) = expr else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT highlight({table}, 0, char(1), char(2)),
                snippet({table}, 1, char(1), char(2), '…', {SNIPPET_WORDS})
           FROM {table} WHERE {table} MATCH ?1 AND rowid = ?2"
    );
    Ok(tx
        .query_row(&sql, params![expr, docid], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

fn hit(tx: &Transaction<'_>, entry: &Entry, terms: &[Term]) -> Result<SearchHit, Failure> {
    let words: Vec<&str> = terms.iter().map(|t| t.text.as_str()).collect();
    match entry {
        Entry::Project(id) => {
            let (key, name, updated): (String, String, i64) = tx.query_row(
                "SELECT key, name, updated_at FROM projects WHERE id = ?1",
                [b(*id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            Ok(SearchHit {
                kind: SearchKind::Project,
                matched_in: MatchedIn::Key,
                project: key,
                task: None,
                title: name,
                title_matches: Vec::new(),
                snippet: None,
                snippet_matches: Vec::new(),
                status: None,
                category: None,
                slug: None,
                updated_at: rfc3339(updated),
                id: id.to_string(),
            })
        }
        Entry::TaskKey(id) => {
            let (key, title, status, category, updated) = task_facts(tx, *id)?;
            let project = key
                .rsplit_once('-')
                .map_or_else(String::new, |(p, _)| p.to_owned());
            Ok(SearchHit {
                kind: SearchKind::Task,
                matched_in: MatchedIn::Key,
                project,
                task: Some(key),
                title,
                title_matches: Vec::new(),
                snippet: None,
                snippet_matches: Vec::new(),
                status: Some(status),
                category: Some(category),
                slug: None,
                updated_at: rfc3339(updated),
                id: id.to_string(),
            })
        }
        Entry::Doc(f) => {
            let kind = kind_of(&f.kind);
            let stored_title: String = tx.query_row(
                "SELECT title FROM search_docs WHERE docid = ?1",
                [f.docid],
                |r| r.get(0),
            )?;
            let (mut title, mut title_matches, mut snippet, mut snippet_matches) =
                (stored_title, Vec::new(), None, Vec::new());
            if let Some((t, s)) = marks(tx, f.docid, terms, f.fragment)? {
                let (t, tm) = unmark(&t);
                title = t;
                title_matches = tm;
                let (s, sm) = unmark(&s);
                if !sm.is_empty() {
                    snippet = Some(s);
                    snippet_matches = sm;
                }
            }
            if title_matches.is_empty() && f.fragment {
                title_matches = find_ranges(&title, &words);
            }
            if kind == SearchKind::Comment {
                // The title is the task's; the match is in the comment.
                title_matches.clear();
            }
            let (task, status, category) = match f.task {
                Some(task) => {
                    let (key, _, status, category, _) = task_facts(tx, task)?;
                    (Some(key), Some(status), Some(category))
                }
                None => (None, None, None),
            };
            let slug = if kind == SearchKind::Wiki {
                tx.query_row(
                    "SELECT slug FROM wiki_pages WHERE id = ?1",
                    [b(f.entity)],
                    |r| r.get(0),
                )
                .optional()?
            } else {
                None
            };
            Ok(SearchHit {
                kind,
                matched_in: match (f.tier, kind) {
                    (3, _) => MatchedIn::Fragment,
                    (0, _) | (_, SearchKind::File) => MatchedIn::Title,
                    _ => MatchedIn::Text,
                },
                project: project_key(tx, f.project)?,
                task,
                title,
                title_matches,
                snippet,
                snippet_matches,
                status,
                category,
                slug,
                updated_at: rfc3339(f.updated_at),
                id: f.entity.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str, phrase: bool) -> Term {
        Term {
            text: text.to_owned(),
            phrase,
        }
    }

    #[test]
    fn terms_are_words_and_phrases() {
        assert_eq!(
            terms(r#"  timeout "connection reset" -- user_id "#),
            [
                t("timeout", false),
                t("connection reset", true),
                t("user_id", false)
            ]
        );
        assert_eq!(terms(r#""open"#), [t("open", true)]);
        assert_eq!(
            words_expr(&terms(r#"a "b c""#)).as_deref(),
            Some(r#""a"* AND "b c""#)
        );
        assert_eq!(
            words_expr(&terms(r#"say "hi""#)).as_deref(),
            Some(r#""say"* AND "hi""#)
        );
        assert_eq!(grams_expr(&terms("conn ab")), None);
        assert_eq!(grams_expr(&terms("conn")).as_deref(), Some(r#""conn""#));
    }

    #[test]
    fn marks_become_utf16_ranges() {
        let (text, ranges) = unmark("\n a \u{1}😀b\u{2} c\n\n\u{1}d\u{2}\n");
        assert_eq!(text, "a 😀b c d");
        assert_eq!(
            ranges,
            [TextRange { from: 2, to: 5 }, TextRange { from: 8, to: 9 }]
        );
    }

    #[test]
    fn ranges_ignore_case_and_merge() {
        assert_eq!(
            find_ranges("ERR_CONN and conn", &["conn", "rr_c"]),
            [TextRange { from: 1, to: 8 }, TextRange { from: 13, to: 17 }]
        );
    }
}
