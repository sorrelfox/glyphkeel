// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki pages: the tree per project, finding pages and their headings,
//! reading, creating and changing them.
//!
//! Rules:
//! - A title is unique among a project's live pages ignoring case (the
//!   schema enforces it too) and can't hold `[`, `]`, `#` or `|`.
//! - The slug follows the title. A slug stays taken by its page while the
//!   page is in the trash, so a new page gets `-2`, `-3`… A renamed page
//!   leaves its old slug as a redirect.
//! - Pages nest at most [`MAX_DEPTH`] deep, within one project.
//! - The text is saved as revisions through `description.rs`, with the
//!   same base revision, merge and conflict-copy rules as descriptions.
//! - History records `created`, `title`, `slug`, `parent`, `rank`,
//!   `agent_brief`, `home` and the text as `body` (`{"revision": ID}`), so
//!   Undo and revert work on all of them.
//! - At most one page per project is the home page, which the wiki opens
//!   on. A page in the trash keeps the mark (only live pages show it), so
//!   it is the home page again when it comes back.
//! - Any number of pages are templates (`template`, recorded for Undo). A
//!   new page can start from any page's text (`wiki.create` with
//!   `template`); the app offers the pages marked as templates.

use std::collections::HashMap;

use gk_api::ops::description::{RevisionList, SaveOutcome};
use gk_api::ops::wiki::{
    WikiChange, WikiCreate, WikiCreateInput, WikiExport, WikiField, WikiGet, WikiGetInput,
    WikiHeading, WikiHistory, WikiHistoryInput, WikiImport, WikiList, WikiListInput, WikiMatch,
    WikiMove, WikiMoveInput, WikiPage, WikiPageRef, WikiPageSummary, WikiPart, WikiSearch,
    WikiSearchHit, WikiSearchInput, WikiSearchResult, WikiTree, WikiUpdate, WikiUpdateInput,
};
use gk_core::{Id, PageTitleError, TextError, fold};
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::Value;

use super::desc;
use super::description::{self, Owner, WIKI_CONFLICTS_SQL};
use super::fields::{self, WIKI, id_value, text};
use super::lookup::{self, ProjectRow, Target, b, id_at, opt_id_at};
use super::search::{find_ranges, in_column, terms, unmark, words_expr};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// How deep pages nest: a page at the top has depth 0.
pub(crate) const MAX_DEPTH: u32 = 8;
/// Results of a page search unless asked otherwise.
const DEFAULT_SEARCH_LIMIT: u32 = 20;
/// The most pages whose headings one search reads.
const MAX_HEADING_PAGES: usize = 2000;
/// Characters of context around a text match.
const SNIPPET_WORDS: u32 = 12;

pub(super) fn register(r: &mut Registry) {
    r.query::<WikiList, _>(desc::<WikiList>(), list);
    r.query::<WikiSearch, _>(desc::<WikiSearch>(), search);
    r.query::<WikiGet, _>(desc::<WikiGet>(), get);
    r.command::<WikiCreate, _>(desc::<WikiCreate>(), create);
    r.command::<WikiUpdate, _>(desc::<WikiUpdate>(), update);
    r.command::<WikiMove, _>(desc::<WikiMove>(), move_op);
    r.engine_op::<WikiExport, _>(desc::<WikiExport>(), |engine, ctx, input| {
        engine.export_wiki_page(ctx, &input)
    });
    r.engine_op::<WikiImport, _>(desc::<WikiImport>(), |engine, ctx, input| {
        engine.import_wiki_folder(ctx, &input)
    });
    r.query::<WikiHistory, _>(desc::<WikiHistory>(), |q, input: WikiHistoryInput| {
        let page = find(q.tx, input.project.as_deref(), "page", &input.page)?;
        let items = description::revisions(q.tx, Owner::Wiki(page.id))?;
        Ok(RevisionList {
            total: items.len() as u64,
            items,
        })
    });
}

// ---------------------------------------------------------------------
// Rows

/// What the engine needs of a page row.
#[derive(Debug, Clone)]
pub(crate) struct PageRow {
    pub(crate) id: Id,
    pub(crate) project: Id,
    pub(crate) parent: Option<Id>,
    pub(crate) title: String,
    pub(crate) slug: String,
    rank: String,
    agent_brief: bool,
    home: bool,
    template: bool,
    created_at: i64,
    updated_at: i64,
}

const PAGE_COLUMNS: &str = "id, project_id, parent_id, title, slug, rank, is_agent_brief,
    created_at, updated_at, is_home, is_template";

fn page_row(r: &Row<'_>) -> gk_store::rusqlite::Result<PageRow> {
    Ok(PageRow {
        id: id_at(r, 0)?,
        project: id_at(r, 1)?,
        parent: opt_id_at(r, 2)?,
        title: r.get(3)?,
        slug: r.get(4)?,
        rank: r.get(5)?,
        agent_brief: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        home: r.get(9)?,
        template: r.get(10)?,
    })
}

/// The live page `id`.
pub(crate) fn by_id(tx: &Transaction<'_>, id: Id) -> Result<Option<PageRow>, Failure> {
    Ok(tx
        .query_row(
            &format!("SELECT {PAGE_COLUMNS} FROM wiki_pages WHERE id = ?1 AND deleted_at IS NULL"),
            [b(id)],
            page_row,
        )
        .optional()?
        .filter(|p| crate::scope::sees(p.project)))
}

/// The live page of project `project` that `text` names: its title (any
/// case), slug, an old slug, or its ID; `None` when none does. Wiki links
/// resolve the same way.
pub(crate) fn page_in(
    tx: &Transaction<'_>,
    project: Id,
    text: &str,
) -> Result<Option<PageRow>, Failure> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if let Ok(id) = text.parse::<Id>()
        && let Some(p) = by_id(tx, id)?
        && p.project == project
    {
        return Ok(Some(p));
    }
    let one = |sql: &str, arg: &str| -> Result<Option<PageRow>, Failure> {
        Ok(tx
            .query_row(
                &format!(
                    "SELECT {PAGE_COLUMNS} FROM wiki_pages
                      WHERE project_id = ?1 AND {sql} AND deleted_at IS NULL"
                ),
                params![b(project), arg],
                page_row,
            )
            .optional()?
            .filter(|p| crate::scope::sees(p.project)))
    };
    if let Some(p) = one("title_fold = ?2", &fold(text))? {
        return Ok(Some(p));
    }
    if let Some(p) = one("slug = ?2", &text.to_lowercase())? {
        return Ok(Some(p));
    }
    let redirect: Option<Vec<u8>> = tx
        .query_row(
            "SELECT page_id FROM wiki_redirects WHERE project_id = ?1 AND slug = ?2",
            params![b(project), gk_core::slug(text)],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = redirect.and_then(|v| Id::from_slice(&v).ok())
        && let Some(p) = by_id(tx, id)?
    {
        return Ok(Some(p));
    }
    Ok(None)
}

/// The live page of `project` that `text` names, as [`page_in`]; an error
/// that lists the project's pages when none does.
fn in_project(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    field: &str,
    text: &str,
) -> Result<PageRow, Failure> {
    let text = text.trim();
    if let Some(p) = page_in(tx, project.id, text)? {
        return Ok(p);
    }
    let mut stmt = tx.prepare_cached(
        "SELECT title FROM wiki_pages WHERE project_id = ?1 AND deleted_at IS NULL
          ORDER BY title_fold LIMIT 30",
    )?;
    let titles = stmt
        .query_map([b(project.id)], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let args = [("project", project.key.clone()), ("page", text.to_owned())];
    Err(messages::not_found("wiki-page-not-found", &args)
        .with_detail("field", field)
        .with_detail(
            "choices",
            titles.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .with_hint(messages::text("wiki-list-hint", &[]))
        .with_detail("hint_id", "wiki-list-hint")
        .into())
}

/// The live page `text` names; `project` is needed unless `text` is an ID.
fn find(
    tx: &Transaction<'_>,
    project: Option<&str>,
    field: &str,
    text: &str,
) -> Result<PageRow, Failure> {
    match project {
        Some(p) => {
            let project = lookup::project(tx, "project", p)?;
            in_project(tx, &project, field, text)
        }
        None => {
            if let Ok(id) = text.trim().parse::<Id>() {
                return by_id(tx, id)?.ok_or_else(|| {
                    messages::not_found("wiki-page-id-not-found", &[("page", id.to_string())])
                        .with_detail("field", field)
                        .into()
                });
            }
            Err(messages::invalid(
                "project",
                "wiki-project-needed",
                &[("page", text.trim().to_owned())],
            ))
        }
    }
}

/// The live page `text` names, as [`find`], for other modules.
pub(crate) fn find_page(
    tx: &Transaction<'_>,
    project: Option<&str>,
    field: &str,
    text: &str,
) -> Result<PageRow, Failure> {
    find(tx, project, field, text)
}

/// Refuses to bring page `id` back while another live page of its project
/// has its title.
pub(crate) fn ensure_title_free(tx: &Transaction<'_>, id: Id) -> Result<(), Failure> {
    let (project, title): (Vec<u8>, String) = tx.query_row(
        "SELECT project_id, title FROM wiki_pages WHERE id = ?1",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let project = Id::from_slice(&project)
        .map_err(|e| Failure::Store(gk_store::StoreError::Corrupt(e.to_string())))?;
    if let Some(p) = lookup::project_by_id(tx, project)? {
        ensure_free(tx, &p, &title, Some(id))?;
    }
    Ok(())
}

fn project_of(tx: &Transaction<'_>, page: &PageRow) -> Result<ProjectRow, Failure> {
    lookup::project_by_id(tx, page.project)?
        .ok_or_else(|| Failure::Op(messages::not_found("wiki-project-gone", &[])))
}

/// How many pages `id` is nested in.
pub(crate) fn depth(tx: &Transaction<'_>, mut parent: Option<Id>) -> Result<u32, Failure> {
    let mut depth = 0;
    while let Some(p) = parent {
        depth += 1;
        if depth > MAX_DEPTH * 2 {
            break;
        }
        parent = tx
            .query_row(
                "SELECT parent_id FROM wiki_pages WHERE id = ?1",
                [b(p)],
                |r| opt_id_at(r, 0),
            )
            .optional()?
            .flatten();
    }
    Ok(depth)
}

/// The IDs of the pages above `parent`, outermost first.
pub(crate) fn ancestor_ids(tx: &Transaction<'_>, parent: Option<Id>) -> Result<Vec<Id>, Failure> {
    Ok(ancestors(tx, parent)?.iter().map(|p| p.id).collect())
}

/// The pages above `parent`, outermost first.
fn ancestors(tx: &Transaction<'_>, mut parent: Option<Id>) -> Result<Vec<PageRow>, Failure> {
    let mut out = Vec::new();
    while let Some(p) = parent {
        let Some(row) = by_id(tx, p)? else { break };
        parent = row.parent;
        out.push(row);
        if out.len() > (MAX_DEPTH * 2) as usize {
            break;
        }
    }
    out.reverse();
    Ok(out)
}

/// The live pages directly inside `parent` (`None`: the top) of
/// `project`, in order.
pub(crate) fn children(
    tx: &Transaction<'_>,
    project: Id,
    parent: Option<Id>,
) -> Result<Vec<PageRow>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {PAGE_COLUMNS} FROM wiki_pages
          WHERE project_id = ?1 AND parent_id IS ?2 AND deleted_at IS NULL ORDER BY rank, id"
    ))?;
    let rows = stmt
        .query_map(params![b(project), parent.map(b)], page_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// How many levels of live pages hang below `id`.
fn height(tx: &Transaction<'_>, id: Id) -> Result<u32, Failure> {
    let mut level = vec![id];
    let mut height = 0;
    while height <= MAX_DEPTH * 2 {
        let mut next = Vec::new();
        for p in &level {
            let mut stmt = tx.prepare_cached(
                "SELECT id FROM wiki_pages WHERE parent_id = ?1 AND deleted_at IS NULL",
            )?;
            let ids = stmt
                .query_map([b(*p)], |r| id_at(r, 0))?
                .collect::<Result<Vec<_>, _>>()?;
            next.extend(ids);
        }
        if next.is_empty() {
            break;
        }
        height += 1;
        level = next;
    }
    Ok(height)
}

pub(crate) fn page_ref(p: &PageRow) -> WikiPageRef {
    WikiPageRef {
        title: p.title.clone(),
        slug: p.slug.clone(),
        id: p.id.to_string(),
    }
}

fn summary(p: &PageRow, project: &str, depth: u32, children: u32) -> WikiPageSummary {
    WikiPageSummary {
        title: p.title.clone(),
        slug: p.slug.clone(),
        project: project.to_owned(),
        parent: p.parent.map(|id| id.to_string()),
        depth,
        children,
        agent_brief: p.agent_brief,
        home: p.home,
        template: p.template,
        updated_at: rfc3339(p.updated_at),
        id: p.id.to_string(),
    }
}

/// The summary of one page, with its depth and children counted.
fn summary_of(tx: &Transaction<'_>, p: &PageRow) -> Result<WikiPageSummary, Failure> {
    let key: String = tx.query_row(
        "SELECT key FROM projects WHERE id = ?1",
        [b(p.project)],
        |r| r.get(0),
    )?;
    let children: i64 = tx.query_row(
        "SELECT count(*) FROM wiki_pages WHERE parent_id = ?1 AND deleted_at IS NULL",
        [b(p.id)],
        |r| r.get(0),
    )?;
    Ok(summary(
        p,
        &key,
        depth(tx, p.parent)?,
        u32::try_from(children).unwrap_or(0),
    ))
}

fn headings_of(body: &str) -> Vec<WikiHeading> {
    gk_markdown::headings(body)
        .into_iter()
        .map(|h| WikiHeading {
            level: h.level,
            text: h.text,
            line: u32::try_from(h.line).unwrap_or(u32::MAX),
        })
        .collect()
}

// ---------------------------------------------------------------------
// The search index

/// Keeps the page's search entry in step (indexed while live, removed while
/// deleted) and its links, which are kept while it is in the trash.
pub(crate) fn reindex(w: &WriteCtx<'_>, id: Id) -> Result<(), Failure> {
    super::links::index(w.tx(), super::links::Source::Wiki(id))?;
    match by_id(w.tx(), id)? {
        Some(p) => {
            let (body, _) = description::current_text(w.tx(), Owner::Wiki(id))?;
            w.tx().execute(
                "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                                          updated_at)
                 VALUES ('wiki', ?1, NULL, ?2, ?3, ?4, ?5)
                 ON CONFLICT (kind, entity_id) DO UPDATE SET project_id = excluded.project_id,
                   title = excluded.title, body = excluded.body, updated_at = excluded.updated_at",
                params![
                    b(id),
                    b(p.project),
                    p.title,
                    gk_markdown::plain_text(&body),
                    w.now()
                ],
            )?;
        }
        None => {
            w.tx().execute(
                "DELETE FROM search_docs WHERE kind = 'wiki' AND entity_id = ?1",
                [b(id)],
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Reading

fn list(q: &QueryCtx<'_>, input: WikiListInput) -> Result<WikiTree, Failure> {
    let project = lookup::project(q.tx, "project", &input.project)?;
    let mut stmt = q.tx.prepare_cached(&format!(
        "SELECT {PAGE_COLUMNS} FROM wiki_pages
          WHERE project_id = ?1 AND deleted_at IS NULL ORDER BY rank, id"
    ))?;
    let rows = stmt
        .query_map([b(project.id)], page_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let live: std::collections::HashSet<Id> = rows.iter().map(|p| p.id).collect();
    let mut by_parent: HashMap<Option<Id>, Vec<&PageRow>> = HashMap::new();
    for p in &rows {
        // A page whose parent is gone shows at the top.
        let parent = p.parent.filter(|id| live.contains(id));
        by_parent.entry(parent).or_default().push(p);
    }
    let mut items = Vec::with_capacity(rows.len());
    let mut stack: Vec<(&PageRow, u32)> = by_parent
        .get(&None)
        .map(|v| v.iter().rev().map(|p| (*p, 0)).collect())
        .unwrap_or_default();
    while let Some((p, d)) = stack.pop() {
        let kids = by_parent.get(&Some(p.id)).map_or(&[][..], Vec::as_slice);
        items.push(summary(
            p,
            &project.key,
            d,
            u32::try_from(kids.len()).unwrap_or(u32::MAX),
        ));
        for k in kids.iter().rev() {
            stack.push((k, d + 1));
        }
    }
    Ok(WikiTree {
        total: items.len() as u64,
        items,
    })
}

/// The page `p` as `wiki.get` returns it, at its current text or at
/// revision `revision`.
fn page_out(
    tx: &Transaction<'_>,
    p: &PageRow,
    revision: Option<&str>,
    include: &[WikiPart],
) -> Result<WikiPage, Failure> {
    let owner = Owner::Wiki(p.id);
    let (current_body, current_rev) = description::current_text(tx, owner)?;
    let (body, rev) = match revision {
        Some(r) => {
            let (id, body) = description::revision_text(tx, owner, "revision", r)?;
            (body, Some(id))
        }
        None => (current_body, current_rev),
    };
    let project: String = tx.query_row(
        "SELECT key FROM projects WHERE id = ?1",
        [b(p.project)],
        |r| r.get(0),
    )?;
    let conflicts: i64 = tx.query_row(
        &format!("SELECT {WIKI_CONFLICTS_SQL} FROM wiki_pages p WHERE p.id = ?1"),
        [b(p.id)],
        |r| r.get(0),
    )?;
    let path = ancestors(tx, p.parent)?;
    let checklist_items = include
        .contains(&WikiPart::Checklist)
        .then(|| description::checklist_items(&body));
    Ok(WikiPage {
        title: p.title.clone(),
        slug: p.slug.clone(),
        project,
        parent: path.last().map(page_ref),
        path: path.iter().map(page_ref).collect(),
        children: children(tx, p.project, Some(p.id))?
            .iter()
            .map(page_ref)
            .collect(),
        headings: headings_of(&body),
        current: rev == current_rev,
        revision: rev.map(|r| r.to_string()),
        body,
        conflict_copies: u32::try_from(conflicts).unwrap_or(0),
        agent_brief: p.agent_brief,
        home: p.home,
        template: p.template,
        backlinks: if include.contains(&WikiPart::Backlinks) {
            Some(super::links::page_backlinks(tx, p)?)
        } else {
            None
        },
        references: if include.contains(&WikiPart::References) {
            Some(super::links::page_references(tx, p)?)
        } else {
            None
        },
        checklist_items,
        files: if include.contains(&WikiPart::Files) {
            Some(super::file::of_page(tx, p.id)?)
        } else {
            None
        },
        created_at: rfc3339(p.created_at),
        updated_at: rfc3339(p.updated_at),
        id: p.id.to_string(),
    })
}

fn get(q: &QueryCtx<'_>, input: WikiGetInput) -> Result<WikiPage, Failure> {
    let page = find(q.tx, input.project.as_deref(), "page", &input.page)?;
    page_out(q.tx, &page, input.revision.as_deref(), &input.include)
}

// ---------------------------------------------------------------------
// Finding pages

/// The live pages of the scope: one project's, or every active project's;
/// the most recently changed first.
fn pages_in(tx: &Transaction<'_>, project: Option<&ProjectRow>) -> Result<Vec<PageRow>, Failure> {
    let sql = format!(
        "SELECT w.id, w.project_id, w.parent_id, w.title, w.slug, w.rank, w.is_agent_brief,
                w.created_at, w.updated_at, w.is_home, w.is_template
           FROM wiki_pages w JOIN projects p ON p.id = w.project_id
          WHERE w.deleted_at IS NULL AND p.deleted_at IS NULL AND {}
          ORDER BY w.updated_at DESC, w.id",
        if project.is_some() {
            "w.project_id = ?1"
        } else {
            "p.state = 'active' AND ?1 IS NULL"
        }
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt
        .query_map([project.map(|p| b(p.id))], page_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// How well a title matches the words: 0 when it starts with the whole
/// text, 1 when every word starts a word of it, 2 when every word is in it;
/// `None` otherwise.
fn title_rank(title: &str, text: &str, words: &[String]) -> Option<u8> {
    let t = fold(title);
    if words.is_empty() || !words.iter().all(|w| t.contains(w.as_str())) {
        return None;
    }
    if t.starts_with(&fold(text.trim())) {
        return Some(0);
    }
    let starts: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    if words
        .iter()
        .all(|w| starts.iter().any(|s| s.starts_with(w.as_str())))
    {
        Some(1)
    } else {
        Some(2)
    }
}

fn search(q: &QueryCtx<'_>, input: WikiSearchInput) -> Result<WikiSearchResult, Failure> {
    let tx = q.tx;
    let (limit, _) = super::search::paging(input.limit, None, DEFAULT_SEARCH_LIMIT)?;
    let project = input
        .project
        .as_deref()
        .map(|p| lookup::project(tx, "project", p))
        .transpose()?;
    let mut pages = pages_in(tx, project.as_ref())?;
    // A caller limited to a project searches it and its sub-projects.
    pages.retain(|p| crate::scope::sees(p.project));
    let text = input.text.trim();
    let words_of = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(fold)
            .filter(|w| !w.is_empty())
            .collect()
    };
    let mut hits: Vec<WikiSearchHit> = Vec::new();

    if text.is_empty() {
        for p in &pages {
            hits.push(WikiSearchHit {
                page: summary_of(tx, p)?,
                matched_in: WikiMatch::Recent,
                title_matches: Vec::new(),
                heading: None,
                heading_matches: Vec::new(),
                snippet: None,
                snippet_matches: Vec::new(),
            });
            if hits.len() >= limit as usize {
                break;
            }
        }
        return Ok(WikiSearchResult {
            total: pages.len() as u64,
            items: hits,
        });
    }

    let (page_part, heading_part) = match text.split_once('#') {
        Some((p, h)) => (p.trim(), Some(h.trim())),
        None => (text, None),
    };
    let page_words = words_of(page_part);
    let raw_page_words: Vec<&str> = page_part.split_whitespace().collect();

    // Titles.
    let mut titled: Vec<(u8, &PageRow)> = pages
        .iter()
        .filter_map(|p| {
            if page_words.is_empty() {
                heading_part.map(|_| (0, p))
            } else {
                title_rank(&p.title, page_part, &page_words).map(|r| (r, p))
            }
        })
        .collect();
    titled.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.title.chars().count().cmp(&b.1.title.chars().count()))
            .then_with(|| fold(&a.1.title).cmp(&fold(&b.1.title)))
    });

    if let Some(heading_part) = heading_part {
        // `Page#Heading`: the headings of the matching pages.
        let heading_words = words_of(heading_part);
        let raw: Vec<&str> = heading_part.split_whitespace().collect();
        for (_, p) in titled.iter().take(MAX_HEADING_PAGES) {
            let (body, _) = description::current_text(tx, Owner::Wiki(p.id))?;
            let mut summary = None;
            for h in headings_of(&body) {
                let folded = fold(&h.text);
                if heading_words.iter().all(|w| folded.contains(w.as_str())) {
                    let page = match &summary {
                        Some(s) => s,
                        None => summary.insert(summary_of(tx, p)?),
                    };
                    hits.push(WikiSearchHit {
                        page: page.clone(),
                        matched_in: WikiMatch::Heading,
                        title_matches: find_ranges(&p.title, &raw_page_words),
                        heading_matches: find_ranges(&h.text, &raw),
                        heading: Some(h),
                        snippet: None,
                        snippet_matches: Vec::new(),
                    });
                }
            }
        }
    } else {
        for (_, p) in &titled {
            hits.push(WikiSearchHit {
                page: summary_of(tx, p)?,
                matched_in: WikiMatch::Title,
                title_matches: find_ranges(&p.title, &raw_page_words),
                heading: None,
                heading_matches: Vec::new(),
                snippet: None,
                snippet_matches: Vec::new(),
            });
        }
        // Headings of every page in scope.
        for p in pages.iter().take(MAX_HEADING_PAGES) {
            let (body, _) = description::current_text(tx, Owner::Wiki(p.id))?;
            if !body.contains('#') && !body.contains("\n==") && !body.contains("\n--") {
                continue;
            }
            for h in headings_of(&body) {
                let folded = fold(&h.text);
                if page_words.iter().all(|w| folded.contains(w.as_str())) {
                    hits.push(WikiSearchHit {
                        page: summary_of(tx, p)?,
                        matched_in: WikiMatch::Heading,
                        title_matches: Vec::new(),
                        heading_matches: find_ranges(&h.text, &raw_page_words),
                        heading: Some(h),
                        snippet: None,
                        snippet_matches: Vec::new(),
                    });
                }
            }
        }
        // The text, through the full-text index.
        let terms = terms(page_part);
        if let Some(expr) = words_expr(&terms) {
            let shown: std::collections::HashSet<Id> = titled.iter().map(|(_, p)| p.id).collect();
            let in_scope: HashMap<Id, &PageRow> = pages.iter().map(|p| (p.id, p)).collect();
            let mut stmt = tx.prepare(&format!(
                "SELECT d.entity_id, snippet(fts_words, 1, char(1), char(2), '…', {SNIPPET_WORDS})
                   FROM fts_words JOIN search_docs d ON d.docid = fts_words.rowid
                  WHERE fts_words MATCH ?1 AND d.kind = 'wiki'
                  ORDER BY bm25(fts_words, 0.0, 1.0), d.updated_at DESC"
            ))?;
            let rows = stmt
                .query_map([in_column("body", &expr)], |r| {
                    Ok((id_at(r, 0)?, r.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, snippet) in rows {
                let Some(p) = in_scope.get(&id) else { continue };
                if shown.contains(&id) {
                    continue;
                }
                let (snippet, ranges) = unmark(&snippet);
                hits.push(WikiSearchHit {
                    page: summary_of(tx, p)?,
                    matched_in: WikiMatch::Text,
                    title_matches: Vec::new(),
                    heading: None,
                    heading_matches: Vec::new(),
                    snippet: Some(snippet),
                    snippet_matches: ranges,
                });
            }
        }
    }
    let total = hits.len() as u64;
    hits.truncate(limit as usize);
    Ok(WikiSearchResult { items: hits, total })
}

// ---------------------------------------------------------------------
// Changing

fn title_error(e: PageTitleError) -> Failure {
    match e {
        PageTitleError::Reserved(c) => {
            messages::invalid("title", "wiki-title-reserved", &[("char", c.to_string())])
        }
        PageTitleError::Text(TextError::Empty(_)) => {
            messages::invalid("title", "wiki-title-empty", &[])
        }
        PageTitleError::Text(TextError::TooLong { len, max, .. }) => messages::invalid(
            "title",
            "wiki-title-too-long",
            &[("length", len.to_string()), ("max", max.to_string())],
        ),
    }
}

/// Refuses `title` when another live page of `project` has it, ignoring
/// case.
fn ensure_free(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    title: &str,
    except: Option<Id>,
) -> Result<(), Failure> {
    let other: Option<String> = tx
        .query_row(
            "SELECT title FROM wiki_pages
              WHERE project_id = ?1 AND title_fold = ?2 AND deleted_at IS NULL AND id IS NOT ?3",
            params![b(project.id), fold(title), except.map(b)],
            |r| r.get(0),
        )
        .optional()?;
    match other {
        Some(other) => Err(messages::conflict(
            "wiki-title-taken",
            &[("project", project.key.clone()), ("title", other)],
        )
        .with_detail("field", "title")
        .into()),
        None => Ok(()),
    }
}

/// A slug for `title` in `project` that no other page has, live or deleted.
fn free_slug(tx: &Transaction<'_>, project: Id, title: &str, page: Id) -> Result<String, Failure> {
    let base = gk_core::slug(title);
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let taken: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM wiki_pages WHERE project_id = ?1 AND slug = ?2 AND id <> ?3",
                params![b(project), candidate, b(page)],
                |r| r.get(0),
            )
            .optional()?;
        if taken.is_none() {
            return Ok(candidate);
        }
        n += 1;
    }
}

/// Lets a live page take `slug`: a redirect from a renamed page gives way to
/// a page of that name. A redirect to a page in the trash stays, so that the
/// old links find that page again once it is restored and the slug is free
/// (a live page with the slug comes first while it exists).
fn release_redirect(tx: &Transaction<'_>, project: Id, slug: &str) -> Result<(), Failure> {
    tx.execute(
        "DELETE FROM wiki_redirects
          WHERE project_id = ?1 AND slug = ?2
            AND page_id IN (SELECT id FROM wiki_pages WHERE deleted_at IS NULL)",
        params![b(project), slug],
    )?;
    Ok(())
}

/// The parent page `text` names in `project`, checked for loops and depth
/// when `moving` (with the pages below it) goes inside it.
fn parent_for(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    text: &str,
    moving: Option<Id>,
) -> Result<PageRow, Failure> {
    let parent = match text.trim().parse::<Id>() {
        Ok(id) => match by_id(tx, id)? {
            Some(p) if p.project != project.id => {
                return Err(messages::invalid(
                    "parent",
                    "wiki-parent-other-project",
                    &[("page", p.title)],
                ));
            }
            _ => in_project(tx, project, "parent", text)?,
        },
        Err(_) => in_project(tx, project, "parent", text)?,
    };
    if let Some(moving) = moving
        && (parent.id == moving || ancestors(tx, parent.parent)?.iter().any(|a| a.id == moving))
    {
        return Err(messages::invalid("parent", "wiki-parent-loop", &[]));
    }
    let below = match moving {
        Some(id) => height(tx, id)?,
        None => 0,
    };
    if depth(tx, Some(parent.id))? + below > MAX_DEPTH {
        return Err(messages::invalid(
            "parent",
            "wiki-too-deep",
            &[("max", MAX_DEPTH.to_string())],
        ));
    }
    Ok(parent)
}

/// Where `before`/`after`/`at` put a page among `siblings`.
fn target(
    tx: &Transaction<'_>,
    project: &ProjectRow,
    siblings: &[PageRow],
    before: Option<&str>,
    after: Option<&str>,
    at: Option<gk_api::ops::common::Edge>,
) -> Result<Option<Target>, Failure> {
    let sibling = |field: &str, text: &str| -> Result<Id, Failure> {
        let p = in_project(tx, project, field, text)?;
        if siblings.iter().any(|s| s.id == p.id) {
            Ok(p.id)
        } else {
            Err(messages::invalid(
                field,
                "wiki-sibling-not-found",
                &[("page", text.trim().to_owned())],
            ))
        }
    };
    let before = before.map(|t| sibling("before", t)).transpose()?;
    let after = after.map(|t| sibling("after", t)).transpose()?;
    Target::from_parts(before, after, at)
}

/// Gives `page` its place among `siblings` and records the new ranks.
fn place(
    w: &mut WriteCtx<'_>,
    project: Id,
    page: Id,
    siblings: &[PageRow],
    target: Target,
) -> Result<(), Failure> {
    let list: Vec<(Id, String)> = siblings.iter().map(|s| (s.id, s.rank.clone())).collect();
    for (id, rank) in lookup::place(&list, page, target)? {
        fields::set(w, WIKI, id, Some(project), "rank", text(rank.as_str()))?;
    }
    Ok(())
}

fn create(w: &mut WriteCtx<'_>, input: WikiCreateInput) -> Result<WikiChange, Failure> {
    let project = input.project.clone();
    let change = create_page(w, input)?;
    let project = lookup::project(w.tx(), "project", &project)?;
    super::draft::clear(w, gk_api::ops::draft::DraftKind::NewWikiPage, project.id)?;
    Ok(change)
}

/// Creates a page as `wiki.create` does, leaving the person's draft of a
/// new page alone.
pub(crate) fn create_page(
    w: &mut WriteCtx<'_>,
    input: WikiCreateInput,
) -> Result<WikiChange, Failure> {
    let project = lookup::project(w.tx(), "project", &input.project)?;
    lookup::ensure_active(&project)?;
    lookup::ensure_wiki_on(w.tx(), &project)?;
    let title = gk_core::check_page_title(&input.title).map_err(title_error)?;
    ensure_free(w.tx(), &project, &title, None)?;
    let parent = input
        .parent
        .as_deref()
        .map(|p| parent_for(w.tx(), &project, p, None))
        .transpose()?;
    let body = match (&input.template, input.body) {
        (Some(_), Some(_)) => {
            return Err(messages::invalid("template", "wiki-template-and-body", &[]));
        }
        (Some(template), None) => {
            let source =
                super::links::resolve_page(w.tx(), project.id, template)?.ok_or_else(|| {
                    Failure::Op(
                        messages::not_found(
                            "wiki-template-not-found",
                            &[("page", template.trim().to_owned())],
                        )
                        .with_detail("field", "template"),
                    )
                })?;
            description::current_text(w.tx(), Owner::Wiki(source.id))?.0
        }
        (None, body) => body.unwrap_or_default(),
    };
    gk_core::check_markdown("body", &body).map_err(|e| super::task::invalid("body", e))?;
    let siblings = children(w.tx(), project.id, parent.as_ref().map(|p| p.id))?;
    let target = target(
        w.tx(),
        &project,
        &siblings,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?
    .unwrap_or(Target::Bottom);

    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let slug = free_slug(w.tx(), project.id, &title, id)?;
    release_redirect(w.tx(), project.id, &slug)?;
    let list: Vec<(Id, String)> = siblings.iter().map(|s| (s.id, s.rank.clone())).collect();
    let ranks = lookup::place(&list, id, target)?;
    let own = ranks
        .iter()
        .find(|(p, _)| *p == id)
        .map(|(_, r)| r.as_str().to_owned())
        .ok_or_else(|| Failure::Op(messages::storage("wiki-no-place", &[])))?;
    w.tx().execute(
        "INSERT INTO wiki_pages (id, project_id, parent_id, rank, title, title_fold, slug,
                                 created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            b(id),
            b(project.id),
            parent.as_ref().map(|p| b(p.id)),
            own,
            title,
            fold(&title),
            slug,
            w.now()
        ],
    )?;
    for (other, rank) in ranks.iter().filter(|(p, _)| *p != id) {
        fields::set(
            w,
            WIKI,
            *other,
            Some(project.id),
            "rank",
            text(rank.as_str()),
        )?;
    }
    description::initial(w, Owner::Wiki(id), &body)?;
    if let Some(message) = input.message.as_deref().filter(|m| !m.trim().is_empty())
        && let (_, Some(rev)) = description::current_text(w.tx(), Owner::Wiki(id))?
    {
        description::set_message(w, Owner::Wiki(id), &rev.to_string(), message.trim())?;
    }
    w.record_field(
        WIKI,
        id,
        Some(project.id),
        "created",
        None,
        Some(Value::Bool(true)),
    );
    reindex(w, id)?;
    changed(w, id, None)
}

fn update(w: &mut WriteCtx<'_>, input: WikiUpdateInput) -> Result<WikiChange, Failure> {
    let page = find(w.tx(), input.project.as_deref(), "page", &input.page)?;
    let project = project_of(w.tx(), &page)?;
    lookup::ensure_active(&project)?;
    lookup::ensure_wiki_on(w.tx(), &project)?;
    let owner = Owner::Wiki(page.id);
    if input.body.is_some() && !input.edits.is_empty() {
        return Err(messages::invalid("edits", "wiki-body-and-edits", &[]));
    }
    let clears_parent = input.clear.contains(&WikiField::Parent);
    if clears_parent && input.parent.is_some() {
        return Err(messages::invalid("clear", "wiki-parent-and-clear", &[]));
    }

    if let Some(title) = &input.title {
        let title = gk_core::check_page_title(title).map_err(title_error)?;
        ensure_free(w.tx(), &project, &title, Some(page.id))?;
        if title != page.title {
            fields::set(w, WIKI, page.id, Some(project.id), "title", text(&title))?;
            let slug = free_slug(w.tx(), project.id, &title, page.id)?;
            if slug != page.slug {
                w.tx().execute(
                    "INSERT OR REPLACE INTO wiki_redirects (project_id, slug, page_id)
                     VALUES (?1, ?2, ?3)",
                    params![b(project.id), page.slug, b(page.id)],
                )?;
                release_redirect(w.tx(), project.id, &slug)?;
                fields::set(w, WIKI, page.id, Some(project.id), "slug", text(&slug))?;
            }
            reindex(w, page.id)?;
        }
    }

    let new_parent = match (&input.parent, clears_parent) {
        (Some(p), _) => Some(Some(p.as_str())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    move_page(
        w,
        &project,
        &page,
        new_parent,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?;

    if let Some(brief) = input.agent_brief {
        set_brief(w, &project, &page, brief)?;
    }
    if let Some(home) = input.home {
        set_home(w, &project, &page, home)?;
    }
    if let Some(template) = input.template.filter(|t| *t != page.template) {
        fields::set(
            w,
            WIKI,
            page.id,
            Some(project.id),
            "template",
            Some(Value::Bool(template)),
        )?;
    }

    let mut save = None;
    if let Some(body) = &input.body {
        gk_core::check_markdown("body", body).map_err(|e| super::task::invalid("body", e))?;
        save = Some(description::save(
            w,
            owner,
            Some(project.id),
            body,
            input.base_revision.as_deref(),
            "normal",
        )?);
        super::draft::clear(w, gk_api::ops::draft::DraftKind::WikiPage, page.id)?;
    } else if !input.edits.is_empty() {
        save = Some(description::save_edits(
            w,
            owner,
            Some(project.id),
            &input.edits,
        )?);
    }
    if let Some(r) = &input.resolve_conflict {
        save = Some(description::resolve(
            w,
            owner,
            Some(project.id),
            &r.copy,
            r.action,
            r.text.as_deref(),
            input
                .base_revision
                .as_deref()
                .filter(|_| input.body.is_none()),
        )?);
    }
    if let (Some(message), Some(s)) = (
        input
            .message
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty()),
        &save,
    ) {
        let rev = match s.outcome {
            SaveOutcome::Saved | SaveOutcome::Merged => s.revision.as_deref(),
            SaveOutcome::Conflict => s.conflict_copy.as_deref(),
            SaveOutcome::Unchanged => None,
        };
        if let Some(rev) = rev {
            description::set_message(w, owner, rev, message)?;
        }
    }
    changed(w, page.id, save)
}

/// Marks `page` as its project's brief for agents, unmarking the page
/// marked before, or unmarks it. Agents can't change the brief, which
/// instructs them.
fn set_brief(
    w: &mut WriteCtx<'_>,
    project: &ProjectRow,
    page: &PageRow,
    brief: bool,
) -> Result<(), Failure> {
    if w.caller().is_agent() {
        return Err(Failure::Op(messages::hint(
            messages::tag(
                gk_api::OpError::forbidden(messages::text("wiki-brief-agents", &[])),
                "wiki-brief-agents",
                &[],
            ),
            "wiki-brief-agents-hint",
            &[],
        )));
    }
    if brief == page.agent_brief {
        return Ok(());
    }
    if brief {
        let before: Option<Vec<u8>> = w
            .tx()
            .query_row(
                "SELECT id FROM wiki_pages
                  WHERE project_id = ?1 AND is_agent_brief = 1 AND id <> ?2",
                params![b(project.id), b(page.id)],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(before) = before.and_then(|id| Id::from_slice(&id).ok()) {
            fields::set(
                w,
                WIKI,
                before,
                Some(project.id),
                "agent_brief",
                Some(Value::Bool(false)),
            )?;
        }
    }
    fields::set(
        w,
        WIKI,
        page.id,
        Some(project.id),
        "agent_brief",
        Some(Value::Bool(brief)),
    )?;
    Ok(())
}

/// Gives page `id`, made in this change without a text, its first text,
/// and indexes it.
pub(crate) fn first_text(w: &mut WriteCtx<'_>, id: Id, body: &str) -> Result<(), Failure> {
    gk_core::check_markdown("body", body).map_err(|e| super::task::invalid("body", e))?;
    description::initial(w, Owner::Wiki(id), body)?;
    reindex(w, id)
}

/// Makes `page` its project's home page, so that the page that was it
/// before stops being it, or makes it an ordinary page again.
fn set_home(
    w: &mut WriteCtx<'_>,
    project: &ProjectRow,
    page: &PageRow,
    home: bool,
) -> Result<(), Failure> {
    if home == page.home {
        return Ok(());
    }
    if home {
        let before: Option<Id> = w
            .tx()
            .query_row(
                "SELECT id FROM wiki_pages WHERE project_id = ?1 AND is_home = 1 AND id <> ?2",
                params![b(project.id), b(page.id)],
                |r| id_at(r, 0),
            )
            .optional()?;
        if let Some(before) = before {
            fields::set(
                w,
                WIKI,
                before,
                Some(project.id),
                "home",
                Some(Value::Bool(false)),
            )?;
        }
    }
    fields::set(
        w,
        WIKI,
        page.id,
        Some(project.id),
        "home",
        Some(Value::Bool(home)),
    )?;
    Ok(())
}

/// Moves `page` (with the pages inside it) into `new_parent` (`Some(None)`:
/// to the top; `None`: it stays) and to the place `before`/`after`/`at`
/// names. A page that changes parent goes last unless placed. Refuses a
/// parent that is the page or inside it, or that would nest too deep.
#[allow(clippy::option_option)]
fn move_page(
    w: &mut WriteCtx<'_>,
    project: &ProjectRow,
    page: &PageRow,
    new_parent: Option<Option<&str>>,
    before: Option<&str>,
    after: Option<&str>,
    at: Option<gk_api::ops::common::Edge>,
) -> Result<(), Failure> {
    let new_parent = match new_parent {
        Some(Some(p)) => Some(Some(parent_for(w.tx(), project, p, Some(page.id))?.id)),
        Some(None) => Some(None),
        None => None,
    };
    let moved = new_parent.is_some_and(|p| p != page.parent);
    let parent = new_parent.unwrap_or(page.parent);
    let siblings = children(w.tx(), project.id, parent)?;
    let placed = target(w.tx(), project, &siblings, before, after, at)?;
    if moved {
        fields::set(
            w,
            WIKI,
            page.id,
            Some(project.id),
            "parent",
            parent.and_then(id_value),
        )?;
    }
    if moved || placed.is_some() {
        place(
            w,
            project.id,
            page.id,
            &siblings,
            placed.unwrap_or(Target::Bottom),
        )?;
    }
    Ok(())
}

fn move_op(w: &mut WriteCtx<'_>, input: WikiMoveInput) -> Result<WikiChange, Failure> {
    let page = find(w.tx(), input.project.as_deref(), "page", &input.page)?;
    let project = project_of(w.tx(), &page)?;
    lookup::ensure_active(&project)?;
    lookup::ensure_wiki_on(w.tx(), &project)?;
    if input.top_level && input.parent.is_some() {
        return Err(messages::invalid("top_level", "wiki-parent-and-top", &[]));
    }
    if !input.top_level
        && input.parent.is_none()
        && input.before.is_none()
        && input.after.is_none()
        && input.at.is_none()
    {
        return Err(messages::invalid("parent", "wiki-move-nowhere", &[]));
    }
    let new_parent = match (&input.parent, input.top_level) {
        (Some(p), _) => Some(Some(p.as_str())),
        (None, true) => Some(None),
        (None, false) => None,
    };
    move_page(
        w,
        &project,
        &page,
        new_parent,
        input.before.as_deref(),
        input.after.as_deref(),
        input.at,
    )?;
    changed(w, page.id, None)
}

fn changed(
    w: &WriteCtx<'_>,
    id: Id,
    save: Option<gk_api::ops::description::DescriptionSave>,
) -> Result<WikiChange, Failure> {
    let page = by_id(w.tx(), id)?
        .ok_or_else(|| Failure::Op(messages::not_found("wiki-page-gone", &[])))?;
    Ok(WikiChange {
        page: page_out(w.tx(), &page, None, &[])?,
        group: w.group_if_changed(),
        save,
    })
}

// ---------------------------------------------------------------------
// History

/// Whether page `id` is live, as `created` records it (`true` while live).
pub(crate) fn raw_life(
    tx: &Transaction<'_>,
    id: Id,
) -> Result<Option<Value>, gk_store::StoreError> {
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM wiki_pages WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(live.filter(|l| *l).map(Value::Bool))
}

/// Brings page `id` back (`value` true) or takes it away, as revert and
/// Undo of its creation do.
pub(crate) fn set_live(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    value: Option<Value>,
) -> Result<(), Failure> {
    if value.is_none() {
        let inside: i64 = w.tx().query_row(
            "SELECT count(*) FROM wiki_pages WHERE parent_id = ?1 AND deleted_at IS NULL",
            [b(id)],
            |r| r.get(0),
        )?;
        if inside > 0 {
            return Err(messages::conflict("wiki-holds-pages", &[]).into());
        }
        w.tx().execute(
            "UPDATE wiki_pages SET deleted_at = ?2, deleted_group = ?3, is_agent_brief = 0,
                                   is_home = 0
              WHERE id = ?1",
            params![b(id), w.now(), b(w.group())],
        )?;
    } else {
        let (project_id, title): (Vec<u8>, String) = w.tx().query_row(
            "SELECT project_id, title FROM wiki_pages WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let project_id = Id::from_slice(&project_id)
            .map_err(|e| Failure::Store(gk_store::StoreError::Corrupt(e.to_string())))?;
        if let Some(p) = lookup::project_by_id(w.tx(), project_id)? {
            ensure_free(w.tx(), &p, &title, Some(id))?;
        }
        w.tx().execute(
            "UPDATE wiki_pages SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1",
            [b(id)],
        )?;
    }
    let before = value.is_none().then_some(Value::Bool(true));
    w.record_field(WIKI, id, project, "created", before, value);
    reindex(w, id)
}

/// Writes a recorded column field back, as revert and Undo do, keeping
/// titles unique and the search entry in step.
pub(crate) fn apply_field(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    if field == "title"
        && let Some(Value::String(title)) = &value
        && let Some(page) = by_id(w.tx(), id)?
        && let Some(p) = lookup::project_by_id(w.tx(), page.project)?
    {
        ensure_free(w.tx(), &p, title, Some(id))?;
    }
    fields::set(w, WIKI, id, project, field, value)?;
    if field == "title" {
        reindex(w, id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use gk_api::ops::query::TextRange;

    use super::*;

    #[test]
    fn titles_rank_by_how_they_match() {
        let words = |s: &str| s.split_whitespace().map(fold).collect::<Vec<_>>();
        assert_eq!(title_rank("Release plan", "rel", &words("rel")), Some(0));
        assert_eq!(
            title_rank("The release plan", "rel pl", &words("rel pl")),
            Some(1)
        );
        assert_eq!(title_rank("Prerelease", "rel", &words("rel")), Some(2));
        assert_eq!(title_rank("Setup", "rel", &words("rel")), None);
        assert_eq!(title_rank("ÜBER uns", "über", &words("über")), Some(0));
    }

    #[test]
    fn heading_ranges_ignore_case() {
        assert_eq!(
            find_ranges("Install steps", &["STEP"]),
            [TextRange { from: 8, to: 12 }]
        );
    }
}
