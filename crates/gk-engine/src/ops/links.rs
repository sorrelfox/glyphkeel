// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The links that texts make: wiki links (`[[Page]]`, `[[Page#Heading]]`,
//! `[[KEY:Page]]`) and task keys (`TM-41`) in wiki pages, task
//! descriptions and comments.
//!
//! Rules:
//! - `text_links` holds each source's links as written, rewritten whenever
//!   the source's text changes (from the same places that keep the search
//!   index in step) and removed only when the source is purged. A source in
//!   the trash keeps its rows; they count again once it is restored.
//! - A link is resolved when it is read: a page by title, slug, old slug or
//!   ID in the source's project, and `[[KEY:Page]]` in project KEY when the
//!   source's project has no page of that whole name; a task by its key,
//!   current or old. Rows only narrow down the candidates; each one is
//!   resolved before it counts, so a renamed, moved, deleted or restored
//!   target needs no change to the rows.
//! - Sources in the trash, in a trashed project or outside the caller's
//!   project limit are left out. A text that links to itself is not its own
//!   backlink.

use std::collections::HashSet;

use gk_api::ops::link::{
    Backlink, BacklinkList, Reference, ReferenceKind, ReferenceList, TaskBacklinks, TaskLinksInput,
    TaskReferences, WikiBacklinks, WikiLinksInput, WikiReferences,
};
use gk_core::{Id, ProjectKey, TaskKey, fold};
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use gk_store::{Store, StoreError};

use super::desc;
use super::description::{self, Owner};
use super::lookup::{self, TaskRow, b, id_at};
use super::wiki::{self, PageRow};
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};

/// The `meta` key a schema upgrade sets when the texts written before it
/// have to be read.
const PENDING: &str = "text_links_pending";

pub(super) fn register(r: &mut Registry) {
    r.query::<WikiBacklinks, _>(desc::<WikiBacklinks>(), |q, input: WikiLinksInput| {
        let page = wiki::find_page(q.tx, input.project.as_deref(), "page", &input.page)?;
        Ok(backlink_list(page_backlinks(q.tx, &page)?))
    });
    r.query::<WikiReferences, _>(desc::<WikiReferences>(), |q, input: WikiLinksInput| {
        let page = wiki::find_page(q.tx, input.project.as_deref(), "page", &input.page)?;
        Ok(reference_list(page_references(q.tx, &page)?))
    });
    r.query::<TaskBacklinks, _>(desc::<TaskBacklinks>(), task_backlinks_op);
    r.query::<TaskReferences, _>(desc::<TaskReferences>(), task_references_op);
}

fn task_backlinks_op(q: &QueryCtx<'_>, input: TaskLinksInput) -> Result<BacklinkList, Failure> {
    let task = lookup::task(q.tx, "key", &input.key)?;
    Ok(backlink_list(task_backlinks(q.tx, task.id)?))
}

fn task_references_op(q: &QueryCtx<'_>, input: TaskLinksInput) -> Result<ReferenceList, Failure> {
    let task = lookup::task(q.tx, "key", &input.key)?;
    Ok(reference_list(task_references(q.tx, task.id)?))
}

fn backlink_list(items: Vec<Backlink>) -> BacklinkList {
    BacklinkList {
        total: items.len() as u64,
        items,
    }
}

fn reference_list(items: Vec<Reference>) -> ReferenceList {
    ReferenceList {
        total: items.len() as u64,
        items,
    }
}

// ---------------------------------------------------------------------
// Keeping the rows

/// A text that links: a page's current text, a task's description or a
/// comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Source {
    Wiki(Id),
    Task(Id),
    Comment(Id),
}

impl Source {
    const fn kind(self) -> &'static str {
        match self {
            Self::Wiki(_) => "wiki",
            Self::Task(_) => "task",
            Self::Comment(_) => "comment",
        }
    }

    const fn id(self) -> Id {
        match self {
            Self::Wiki(id) | Self::Task(id) | Self::Comment(id) => id,
        }
    }

    fn parse(kind: &str, id: Id) -> Option<Self> {
        match kind {
            "wiki" => Some(Self::Wiki(id)),
            "task" => Some(Self::Task(id)),
            "comment" => Some(Self::Comment(id)),
            _ => None,
        }
    }
}

/// `KEY` and `Page` of a link written `KEY:Page`, when `KEY` has the shape
/// of a project key.
fn split_key(name: &str) -> Option<(&str, &str)> {
    let (key, tail) = name.split_once(':')?;
    let (key, tail) = (key.trim(), tail.trim());
    (!tail.is_empty() && key.parse::<ProjectKey>().is_ok()).then_some((key, tail))
}

/// Writes the rows of `source` for `refs`, replacing its old ones.
fn write_refs(
    tx: &Transaction<'_>,
    source: Source,
    refs: &gk_markdown::References,
) -> Result<(), Failure> {
    forget(tx, source)?;
    let mut stmt = tx.prepare_cached(
        "INSERT INTO text_links (source_kind, source_id, position, target_kind, target, heading,
                                 name_fold, name_slug, tail_fold, tail_slug, task_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    let mut position = 0i64;
    for link in &refs.wiki_links {
        // `[[#Heading]]` is a link within the same text.
        if link.page.is_empty() {
            continue;
        }
        let (tail_fold, tail_slug) = split_key(&link.page)
            .map(|(_, tail)| (fold(tail), gk_core::slug(tail)))
            .unwrap_or_default();
        stmt.execute(params![
            source.kind(),
            b(source.id()),
            position,
            "wiki",
            link.page,
            link.heading,
            fold(&link.page),
            gk_core::slug(&link.page),
            tail_fold,
            tail_slug,
            None::<i64>,
        ])?;
        position += 1;
    }
    for key in &refs.task_keys {
        let Ok(parsed) = key.parse::<TaskKey>() else {
            continue;
        };
        stmt.execute(params![
            source.kind(),
            b(source.id()),
            position,
            "task",
            key,
            None::<String>,
            "",
            "",
            "",
            "",
            i64::from(parsed.number()),
        ])?;
        position += 1;
    }
    Ok(())
}

/// Rewrites the rows of `source` from its text `text`.
pub(crate) fn index_text(tx: &Transaction<'_>, source: Source, text: &str) -> Result<(), Failure> {
    if text.is_empty() {
        return forget(tx, source);
    }
    write_refs(tx, source, &gk_markdown::references(text))
}

/// Rewrites the rows of `source` from references already worked out.
pub(crate) fn index_refs(
    tx: &Transaction<'_>,
    source: Source,
    refs: &gk_markdown::References,
) -> Result<(), Failure> {
    write_refs(tx, source, refs)
}

/// Rewrites the rows of `source` from its text as stored now; removes them
/// when the source is gone.
pub(crate) fn index(tx: &Transaction<'_>, source: Source) -> Result<(), Failure> {
    let text: Option<String> = match source {
        Source::Wiki(id) => {
            let exists: Option<i64> = tx
                .query_row("SELECT 1 FROM wiki_pages WHERE id = ?1", [b(id)], |r| {
                    r.get(0)
                })
                .optional()?;
            match exists {
                Some(_) => Some(description::current_text(tx, Owner::Wiki(id))?.0),
                None => None,
            }
        }
        Source::Task(id) => tx
            .query_row(
                "SELECT description FROM tasks WHERE id = ?1",
                [b(id)],
                |r| r.get(0),
            )
            .optional()?,
        Source::Comment(id) => tx
            .query_row("SELECT body FROM comments WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })
            .optional()?,
    };
    match text {
        Some(text) => index_text(tx, source, &text),
        None => forget(tx, source),
    }
}

/// Removes the rows of `source`, as when it is purged.
pub(crate) fn forget(tx: &Transaction<'_>, source: Source) -> Result<(), Failure> {
    tx.execute(
        "DELETE FROM text_links WHERE source_kind = ?1 AND source_id = ?2",
        params![source.kind(), b(source.id())],
    )?;
    Ok(())
}

/// Removes the rows of task `task` and of its comments.
pub(crate) fn forget_task(tx: &Transaction<'_>, task: Id) -> Result<(), Failure> {
    tx.execute(
        "DELETE FROM text_links WHERE source_kind = 'comment'
            AND source_id IN (SELECT id FROM comments WHERE task_id = ?1)",
        [b(task)],
    )?;
    forget(tx, Source::Task(task))
}

/// Rewrites the rows of every text of project `project`: its pages, tasks
/// and comments, live or in the trash.
pub(crate) fn index_project(tx: &Transaction<'_>, project: Id) -> Result<(), Failure> {
    for (sql, make) in [
        (
            "SELECT id FROM wiki_pages WHERE project_id = ?1",
            Source::Wiki as fn(Id) -> Source,
        ),
        ("SELECT id FROM tasks WHERE project_id = ?1", Source::Task),
        (
            "SELECT c.id FROM comments c JOIN tasks t ON t.id = c.task_id
              WHERE t.project_id = ?1",
            Source::Comment,
        ),
    ] {
        let ids = {
            let mut stmt = tx.prepare_cached(sql)?;
            stmt.query_map([b(project)], |r| id_at(r, 0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for id in ids {
            index(tx, make(id))?;
        }
    }
    Ok(())
}

/// Reads the texts written before the link index existed, once, after the
/// schema upgrade that added it. Does nothing when there is nothing to do;
/// when another process is busy writing it is left for the next start.
pub(crate) fn index_pending(store: &Store) {
    let pending = store.read(|tx| {
        tx.query_row("SELECT 1 FROM meta WHERE key = ?1", [PENDING], |_| Ok(()))
            .optional()
            .map_err(StoreError::from)
    });
    if !matches!(pending, Ok(Some(()))) {
        return;
    }
    let done = store.write("links.index", |tx| {
        let still: Option<i64> = tx
            .query_row("SELECT 1 FROM meta WHERE key = ?1", [PENDING], |r| r.get(0))
            .optional()?;
        if still.is_none() {
            return Ok::<_, Failure>(0);
        }
        let projects = {
            let mut stmt = tx.prepare("SELECT id FROM projects")?;
            stmt.query_map([], |r| id_at(r, 0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for p in &projects {
            index_project(tx, *p)?;
        }
        tx.execute("DELETE FROM meta WHERE key = ?1", [PENDING])?;
        Ok(projects.len())
    });
    match done {
        Ok(n) if n > 0 => tracing::info!(projects = n, "wiki and task links indexed"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = ?e, "wiki and task links not indexed yet"),
    }
}

// ---------------------------------------------------------------------
// Reading

/// One stored link.
struct LinkRow {
    source: Source,
    target_kind: String,
    target: String,
    heading: Option<String>,
}

/// A source as backlinks show it: live, in a live project the caller sees.
struct Seen {
    project: Id,
    project_key: String,
    title: String,
    page: Option<PageRow>,
    task: Option<(Id, String)>,
    comment: Option<(Id, i64)>,
    /// For sorting: pages, then descriptions, then comments.
    order: (u8, String, i64, i64),
}

fn project_key(tx: &Transaction<'_>, project: Id) -> Result<String, Failure> {
    Ok(tx.query_row(
        "SELECT key FROM projects WHERE id = ?1",
        [b(project)],
        |r| r.get(0),
    )?)
}

fn task_title(tx: &Transaction<'_>, task: Id) -> Result<String, Failure> {
    Ok(
        tx.query_row("SELECT title FROM tasks WHERE id = ?1", [b(task)], |r| {
            r.get(0)
        })?,
    )
}

/// `source` if it is live and the caller sees it.
fn seen(tx: &Transaction<'_>, source: Source) -> Result<Option<Seen>, Failure> {
    match source {
        Source::Wiki(id) => {
            let Some(page) = wiki::by_id(tx, id)? else {
                return Ok(None);
            };
            let key = project_key(tx, page.project)?;
            Ok(Some(Seen {
                project: page.project,
                project_key: key,
                title: page.title.clone(),
                order: (0, fold(&page.title), 0, 0),
                page: Some(page),
                task: None,
                comment: None,
            }))
        }
        Source::Task(id) => {
            let Some(task) = lookup::task_by_id(tx, id)? else {
                return Ok(None);
            };
            let key = project_key(tx, task.project)?;
            Ok(Some(Seen {
                project: task.project,
                title: task_title(tx, id)?,
                task: Some((id, format!("{key}-{}", task.number))),
                order: (1, key.clone(), task.number, 0),
                project_key: key,
                page: None,
                comment: None,
            }))
        }
        Source::Comment(id) => {
            let row: Option<(Id, i64)> = tx
                .query_row(
                    "SELECT task_id, created_at FROM comments WHERE id = ?1 AND deleted_at IS NULL",
                    [b(id)],
                    |r| Ok((id_at(r, 0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((task_id, at)) = row else {
                return Ok(None);
            };
            let Some(task) = lookup::task_by_id(tx, task_id)? else {
                return Ok(None);
            };
            let key = project_key(tx, task.project)?;
            Ok(Some(Seen {
                project: task.project,
                title: task_title(tx, task_id)?,
                task: Some((task_id, format!("{key}-{}", task.number))),
                comment: Some((id, at)),
                order: (2, String::new(), at, 0),
                project_key: key,
                page: None,
            }))
        }
    }
}

/// The page a link written `target` in a text of `project` names.
pub(crate) fn resolve_page(
    tx: &Transaction<'_>,
    project: Id,
    target: &str,
) -> Result<Option<PageRow>, Failure> {
    if let Some(p) = wiki::page_in(tx, project, target)? {
        return Ok(Some(p));
    }
    if let Some((key, tail)) = split_key(target)
        && let Ok(other) = lookup::project(tx, "page", key)
        && other.id != project
    {
        return wiki::page_in(tx, other.id, tail);
    }
    Ok(None)
}

/// The task a key names, current or old.
fn resolve_task(tx: &Transaction<'_>, key: &str) -> Option<TaskRow> {
    lookup::task(tx, "task", key).ok()
}

/// The stored links whose `column` is one of `values`, of `target_kind`
/// (`wiki` or `task`). The kind is written into the SQL, not bound, so the
/// partial indexes for that kind serve the query.
fn candidates(
    tx: &Transaction<'_>,
    target_kind: &'static str,
    condition: &str,
    values: &serde_json::Value,
) -> Result<Vec<LinkRow>, Failure> {
    debug_assert!(target_kind.bytes().all(|c| c.is_ascii_lowercase()));
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT source_kind, source_id, target_kind, target, heading FROM text_links
          WHERE target_kind = '{target_kind}' AND ({condition})
          ORDER BY source_kind, source_id, position"
    ))?;
    let rows = stmt
        .query_map(params![values.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                id_at(r, 1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(kind, id, target_kind, target, heading)| {
            Some(LinkRow {
                source: Source::parse(&kind, id)?,
                target_kind,
                target,
                heading,
            })
        })
        .collect())
}

/// Turns the links that resolve to the target into backlinks: one per
/// source, with the heading of its first link, in order.
fn collect_backlinks(
    tx: &Transaction<'_>,
    rows: Vec<LinkRow>,
    skip: impl Fn(Source, &Seen) -> bool,
    hits: impl Fn(&Seen, &LinkRow) -> Result<bool, Failure>,
) -> Result<Vec<Backlink>, Failure> {
    let mut done: HashSet<Source> = HashSet::new();
    let mut out: Vec<(Seen, Option<String>)> = Vec::new();
    for row in rows {
        if done.contains(&row.source) {
            continue;
        }
        let Some(s) = seen(tx, row.source)? else {
            done.insert(row.source);
            continue;
        };
        if skip(row.source, &s) {
            done.insert(row.source);
            continue;
        }
        if hits(&s, &row)? {
            done.insert(row.source);
            out.push((s, row.heading));
        }
    }
    out.sort_by(|a, b| a.0.order.cmp(&b.0.order));
    Ok(out
        .into_iter()
        .map(|(s, heading)| Backlink {
            task: s.task.map(|(_, key)| key),
            comment: s.comment.map(|(id, _)| id.to_string()),
            page: s.page.as_ref().map(wiki::page_ref),
            project: s.project_key,
            title: s.title,
            heading,
        })
        .collect())
}

const NAME_MATCH: &str = "name_fold IN (SELECT value FROM json_each(?1))
    OR name_slug IN (SELECT value FROM json_each(?1))
    OR tail_fold IN (SELECT value FROM json_each(?1))
    OR tail_slug IN (SELECT value FROM json_each(?1))";

/// The pages, task descriptions and comments that link to `page`.
pub(crate) fn page_backlinks(
    tx: &Transaction<'_>,
    page: &PageRow,
) -> Result<Vec<Backlink>, Failure> {
    let mut names: Vec<String> = vec![
        fold(&page.title),
        gk_core::slug(&page.title),
        page.slug.clone(),
        fold(&page.id.to_string()),
    ];
    {
        let mut stmt = tx.prepare_cached("SELECT slug FROM wiki_redirects WHERE page_id = ?1")?;
        let old = stmt
            .query_map([b(page.id)], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        names.extend(old);
    }
    names.sort();
    names.dedup();
    let rows = candidates(tx, "wiki", NAME_MATCH, &serde_json::json!(names))?;
    collect_backlinks(
        tx,
        rows,
        |source, _| source == Source::Wiki(page.id),
        |s, row| Ok(resolve_page(tx, s.project, &row.target)?.is_some_and(|p| p.id == page.id)),
    )
}

/// The pages, other tasks' descriptions and comments on other tasks that
/// mention task `task`.
pub(crate) fn task_backlinks(tx: &Transaction<'_>, task: Id) -> Result<Vec<Backlink>, Failure> {
    let mut numbers: Vec<i64> =
        vec![
            tx.query_row("SELECT number FROM tasks WHERE id = ?1", [b(task)], |r| {
                r.get(0)
            })?,
        ];
    {
        let mut stmt =
            tx.prepare_cached("SELECT number FROM task_key_aliases WHERE task_id = ?1")?;
        let old = stmt
            .query_map([b(task)], |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        numbers.extend(old);
    }
    numbers.sort_unstable();
    numbers.dedup();
    let rows = candidates(
        tx,
        "task",
        "task_number IN (SELECT value FROM json_each(?1))",
        &serde_json::json!(numbers),
    )?;
    collect_backlinks(
        tx,
        rows,
        |source, s| source == Source::Task(task) || s.task.as_ref().is_some_and(|t| t.0 == task),
        |_, row| Ok(resolve_task(tx, &row.target).is_some_and(|t| t.id == task)),
    )
}

/// The stored links of `source`, in order.
fn rows_of(tx: &Transaction<'_>, source: Source) -> Result<Vec<LinkRow>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT target_kind, target, heading FROM text_links
          WHERE source_kind = ?1 AND source_id = ?2 ORDER BY position",
    )?;
    let rows = stmt
        .query_map(params![source.kind(), b(source.id())], |r| {
            Ok(LinkRow {
                source,
                target_kind: r.get(0)?,
                target: r.get(1)?,
                heading: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// `rows`, resolved as links in a text of `project`.
fn resolve_rows(
    tx: &Transaction<'_>,
    project: Id,
    rows: Vec<LinkRow>,
    comment: Option<Id>,
    out: &mut Vec<Reference>,
) -> Result<(), Failure> {
    for row in rows {
        let mut r = Reference {
            kind: ReferenceKind::Page,
            target: row.target,
            heading: row.heading,
            found: false,
            page: None,
            task: None,
            project: None,
            title: None,
            comment: comment.map(|c| c.to_string()),
        };
        if row.target_kind == "task" {
            r.kind = ReferenceKind::Task;
            if let Some(t) = resolve_task(tx, &r.target) {
                let key = project_key(tx, t.project)?;
                r.found = true;
                r.task = Some(format!("{key}-{}", t.number));
                r.title = Some(task_title(tx, t.id)?);
                r.project = Some(key);
            }
        } else if let Some(p) = resolve_page(tx, project, &r.target)? {
            r.found = true;
            r.project = Some(project_key(tx, p.project)?);
            r.title = Some(p.title.clone());
            r.page = Some(wiki::page_ref(&p));
        }
        out.push(r);
    }
    Ok(())
}

/// What page `page`'s current text links to.
pub(crate) fn page_references(
    tx: &Transaction<'_>,
    page: &PageRow,
) -> Result<Vec<Reference>, Failure> {
    let mut out = Vec::new();
    let rows = rows_of(tx, Source::Wiki(page.id))?;
    resolve_rows(tx, page.project, rows, None, &mut out)?;
    Ok(out)
}

/// What task `task`'s description and live comments link to.
pub(crate) fn task_references(tx: &Transaction<'_>, task: Id) -> Result<Vec<Reference>, Failure> {
    let project: Id = tx.query_row(
        "SELECT project_id FROM tasks WHERE id = ?1",
        [b(task)],
        |r| id_at(r, 0),
    )?;
    let mut out = Vec::new();
    let rows = rows_of(tx, Source::Task(task))?;
    resolve_rows(tx, project, rows, None, &mut out)?;
    let comments = {
        let mut stmt = tx.prepare_cached(
            "SELECT id FROM comments WHERE task_id = ?1 AND deleted_at IS NULL
              ORDER BY created_at, id",
        )?;
        stmt.query_map([b(task)], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for c in comments {
        let rows = rows_of(tx, Source::Comment(c))?;
        resolve_rows(tx, project, rows, Some(c), &mut out)?;
    }
    Ok(out)
}
