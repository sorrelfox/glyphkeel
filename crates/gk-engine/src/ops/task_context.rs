// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `task.context`: everything an agent needs to work on one task, in one
//! read.
//!
//! It puts together what other reads give: the task as `task.get` shows it
//! (with its checklist, links, comments and references), the brief as
//! `project.brief` gives it, the changes as `history.feed` lists them for
//! the task and the waiting proposals as `proposal.list` shows them, plus
//! the parents, the subtasks down to a depth, the project's statuses with
//! their rules and the openings of the wiki pages the task links to.
//!
//! **Size.** The context stays within `max_bytes` of JSON. Over it, it
//! leaves out the oldest comments first, then the pages' openings (the last
//! linked first), the oldest changes and proposals, and then cuts the brief
//! and the description; `trimmed` counts what went. What can't be trimmed
//! (fields, links, subtasks) may still be larger, and `bytes` then says so.
//!
//! **Scope.** A caller limited to a project gets the same reads as each
//! part would give it: links and page links to other projects are left out.

use std::collections::HashSet;

use gk_api::ops::common::Category;
use gk_api::ops::history::ActivityQuery;
use gk_api::ops::link::ReferenceKind;
use gk_api::ops::task::{Task, TaskPart, TaskRef};
use gk_api::ops::task_context::{
    CONTEXT_ACTIVITY, CONTEXT_COMMENTS, CONTEXT_DEFAULT_BYTES, CONTEXT_DEFAULT_DEPTH,
    CONTEXT_EXCERPT_BYTES, CONTEXT_MAX_BYTES, CONTEXT_MAX_DEPTH, CONTEXT_MIN_BYTES, CONTEXT_PAGES,
    CONTEXT_PROPOSALS, CONTEXT_SUBTASKS, ContextPage, ContextStatus, ContextSubtask,
    ContextTrimmed, TaskContext, TaskContextGet, TaskContextInput,
};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction};

use super::desc;
use super::lookup::{self, b, id_at};
use super::project::parse_enum;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};

/// Most levels above a task: subtasks go at most this deep.
const MAX_PARENTS: usize = super::task::MAX_LEVELS;

pub(super) fn register(r: &mut Registry) {
    r.query::<TaskContextGet, _>(desc::<TaskContextGet>(), get);
}

fn get(q: &QueryCtx<'_>, input: TaskContextInput) -> Result<TaskContext, Failure> {
    let depth = input.depth.unwrap_or(CONTEXT_DEFAULT_DEPTH);
    if depth > CONTEXT_MAX_DEPTH {
        return Err(messages::invalid(
            "depth",
            "task-context-depth",
            &[("max", CONTEXT_MAX_DEPTH.to_string())],
        ));
    }
    let max_bytes = input.max_bytes.unwrap_or(CONTEXT_DEFAULT_BYTES);
    if !(CONTEXT_MIN_BYTES..=CONTEXT_MAX_BYTES).contains(&max_bytes) {
        return Err(messages::invalid(
            "max_bytes",
            "task-context-size",
            &[
                ("min", CONTEXT_MIN_BYTES.to_string()),
                ("max", CONTEXT_MAX_BYTES.to_string()),
            ],
        ));
    }
    let t = lookup::task(q.tx, "key", &input.key)?;
    let units = super::task::units(q.tx)?;
    let mut task = super::task::load(q.tx, t.id, units, q.now)?;
    super::task::add_parts(
        q.tx,
        &q.env.zone,
        &mut task,
        t.id,
        &[
            TaskPart::Checklist,
            TaskPart::Links,
            TaskPart::Comments,
            TaskPart::References,
        ],
    )?;
    let mut trimmed = ContextTrimmed::default();
    if let Some(comments) = task.comments.as_mut()
        && comments.len() > CONTEXT_COMMENTS
    {
        let over = comments.len() - CONTEXT_COMMENTS;
        comments.drain(..over);
        trimmed.comments = over as u64;
    }
    let brief_page = brief_page(q.tx, t.project)?;
    let (pages, unlisted) = pages(q.tx, &task, brief_page)?;
    trimmed.pages = unlisted;
    let (subtasks, unlisted) = subtasks(q.tx, t.id, &task.key, depth)?;
    trimmed.subtasks = unlisted;
    let activity = super::history::feed(
        q,
        ActivityQuery {
            task: Some(t.id.to_string()),
            limit: Some(CONTEXT_ACTIVITY),
            ..ActivityQuery::default()
        },
    )?;
    if activity.total > activity.items.len() as u64 {
        trimmed.activity = activity.total - activity.items.len() as u64;
    }
    let mut context = TaskContext {
        parents: parents(q.tx, t.parent)?,
        subtasks,
        statuses: statuses(q.tx, t.project, t.status)?,
        agent_rules: super::rules::agent_rules(q.tx, t.project)?,
        brief: super::project_delete::brief(q.tx, t.project, task.project.clone())?,
        pages,
        activity: activity.items,
        proposals: super::proposal::pending_on_task(q, t.id, CONTEXT_PROPOSALS)?,
        task,
        trimmed,
        bytes: 0,
        max_bytes: u64::from(max_bytes),
    };
    fit(&mut context, max_bytes as usize);
    Ok(context)
}

/// The size of `c` as JSON, with `bytes` set to that size.
fn measure(c: &mut TaskContext) -> usize {
    // The size is part of the output: settle it in a few rounds, as its
    // own digits change the size.
    let mut size = 0;
    for _ in 0..3 {
        size = serde_json::to_vec(c).map_or(0, |v| v.len());
        if c.bytes == size as u64 {
            break;
        }
        c.bytes = size as u64;
    }
    size
}

/// Trims `c` until it is at most `max` bytes of JSON, or nothing more can
/// go, in the order the module explains.
fn fit(c: &mut TaskContext, max: usize) {
    if measure(c) <= max {
        return;
    }
    while measure(c) > max {
        let Some(comments) = c.task.comments.as_mut() else {
            break;
        };
        if comments.is_empty() {
            break;
        }
        comments.remove(0);
        c.trimmed.comments += 1;
    }
    for i in (0..c.pages.len()).rev() {
        if measure(c) <= max {
            return;
        }
        let page = &mut c.pages[i];
        if page.excerpt.take().is_some() {
            page.cut = true;
            c.trimmed.page_excerpts += 1;
        }
    }
    while measure(c) > max && c.activity.pop().is_some() {
        c.trimmed.activity += 1;
    }
    while measure(c) > max && c.proposals.pop().is_some() {
        c.trimmed.proposals += 1;
    }
    let size = measure(c);
    if size > max {
        c.trimmed.brief = shorten(&mut c.brief.body, size - max);
    }
    let size = measure(c);
    if size > max {
        c.trimmed.description = shorten(&mut c.task.description, size - max);
    }
    let mut size = measure(c);
    // JSON escapes make text longer than its bytes: cut a little more until
    // it fits or the texts are gone.
    while size > max && !(c.brief.body.is_empty() && c.task.description.is_empty()) {
        let over = size - max;
        if !c.brief.body.is_empty() {
            c.trimmed.brief |= shorten(&mut c.brief.body, over);
        } else {
            c.trimmed.description |= shorten(&mut c.task.description, over);
        }
        size = measure(c);
    }
}

/// Cuts `over` bytes or more off the end of `text`, at a character
/// boundary; whether anything was cut.
fn shorten(text: &mut String, over: usize) -> bool {
    if text.is_empty() {
        return false;
    }
    let keep = text.len().saturating_sub(over.max(16));
    text.truncate(boundary(text, keep));
    true
}

/// The largest character boundary of `text` at or before `at`.
fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The opening of a page's text, at most [`CONTEXT_EXCERPT_BYTES`], cut at
/// the last line or word break when there is one near; whether more
/// follows.
fn excerpt(body: &str) -> (String, bool) {
    if body.len() <= CONTEXT_EXCERPT_BYTES {
        return (body.to_owned(), false);
    }
    let end = boundary(body, CONTEXT_EXCERPT_BYTES);
    let head = &body[..end];
    let near = end.saturating_sub(CONTEXT_EXCERPT_BYTES / 4);
    let cut = head
        .rfind('\n')
        .filter(|i| *i >= near)
        .or_else(|| head.rfind(' ').filter(|i| *i >= near))
        .unwrap_or(end);
    (head[..cut].trim_end().to_owned(), true)
}

/// The tasks above the task whose parent is `parent`, nearest first.
fn parents(tx: &Transaction<'_>, mut parent: Option<Id>) -> Result<Vec<TaskRef>, Failure> {
    let mut out = Vec::new();
    while let Some(id) = parent {
        if out.len() >= MAX_PARENTS {
            break;
        }
        out.push(super::task::task_ref(tx, id)?);
        parent = tx
            .prepare_cached("SELECT parent_id FROM tasks WHERE id = ?1")?
            .query_row([b(id)], |r| lookup::opt_id_at(r, 0))
            .optional()?
            .flatten();
    }
    Ok(out)
}

/// The live subtasks of `id` (key `key`) down to `depth` levels, each
/// after its parent, at most [`CONTEXT_SUBTASKS`]; and how many more of
/// the levels reached there were.
fn subtasks(
    tx: &Transaction<'_>,
    id: Id,
    key: &str,
    depth: u32,
) -> Result<(Vec<ContextSubtask>, u64), Failure> {
    let mut out = Vec::new();
    let mut unlisted = 0;
    walk(tx, id, key, 1, depth, &mut out, &mut unlisted)?;
    Ok((out, unlisted))
}

fn walk(
    tx: &Transaction<'_>,
    parent: Id,
    parent_key: &str,
    level: u32,
    depth: u32,
    out: &mut Vec<ContextSubtask>,
    unlisted: &mut u64,
) -> Result<(), Failure> {
    if level > depth {
        return Ok(());
    }
    let children = {
        let mut stmt = tx.prepare_cached(
            "SELECT t.id FROM tasks t JOIN statuses s ON s.id = t.status_id
              WHERE t.parent_id = ?1 AND t.deleted_at IS NULL
              ORDER BY t.subtask_rank, s.rank, t.rank, t.id",
        )?;
        stmt.query_map([b(parent)], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    for child in children {
        if out.len() >= CONTEXT_SUBTASKS {
            *unlisted += 1;
            continue;
        }
        let task = super::task::task_ref(tx, child)?;
        let key = task.key.clone();
        out.push(ContextSubtask {
            level,
            parent: parent_key.to_owned(),
            task,
        });
        walk(tx, child, &key, level + 1, depth, out, unlisted)?;
    }
    Ok(())
}

/// The statuses of project `project` in board order, marking `current`.
fn statuses(tx: &Transaction<'_>, project: Id, current: Id) -> Result<Vec<ContextStatus>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, name, category, rules FROM statuses WHERE project_id = ?1
          ORDER BY rank, id",
    )?;
    let rows = stmt
        .query_map([b(project)], |r| {
            Ok((
                id_at(r, 0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(id, name, category, rules)| ContextStatus {
            current: id == current,
            id: id.to_string(),
            name,
            category: parse_enum(&category).unwrap_or(Category::Todo),
            rules: super::rules::parse(rules.as_deref()),
        })
        .collect())
}

/// The live wiki page marked as project `project`'s brief, if any.
fn brief_page(tx: &Transaction<'_>, project: Id) -> Result<Option<Id>, Failure> {
    Ok(tx
        .prepare_cached(
            "SELECT id FROM wiki_pages
              WHERE project_id = ?1 AND is_agent_brief = 1 AND deleted_at IS NULL",
        )?
        .query_row([b(project)], |r| id_at(r, 0))
        .optional()?)
}

/// The pages the task's references name, once each, without the brief's
/// page, at most [`CONTEXT_PAGES`]; and how many more there were.
fn pages(
    tx: &Transaction<'_>,
    task: &Task,
    brief: Option<Id>,
) -> Result<(Vec<ContextPage>, u64), Failure> {
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(brief) = brief {
        seen.insert(brief.to_string());
    }
    let mut out = Vec::new();
    let mut unlisted = 0;
    for r in task.references.iter().flatten() {
        if r.kind != ReferenceKind::Page || !r.found {
            continue;
        }
        let (Some(page), Some(project)) = (&r.page, &r.project) else {
            continue;
        };
        if !seen.insert(page.id.clone()) {
            continue;
        }
        if out.len() >= CONTEXT_PAGES {
            unlisted += 1;
            continue;
        }
        let Ok(id) = page.id.parse::<Id>() else {
            continue;
        };
        let body: Option<String> = tx
            .prepare_cached(
                "SELECT r.body FROM wiki_pages w JOIN wiki_revisions r ON r.id = w.current_rev
                  WHERE w.id = ?1 AND w.deleted_at IS NULL",
            )?
            .query_row([b(id)], |r| r.get(0))
            .optional()?;
        let Some(body) = body else {
            continue;
        };
        let (text, cut) = excerpt(&body);
        out.push(ContextPage {
            page: page.clone(),
            project: project.clone(),
            excerpt: Some(text),
            cut,
        });
    }
    Ok((out, unlisted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_stop_at_a_break_near_the_limit() {
        let short = "A short page.";
        assert_eq!(excerpt(short), (short.to_owned(), false));
        let long = format!("{}\nsecond line goes on", "word ".repeat(400));
        let (text, cut) = excerpt(&long);
        assert!(cut);
        assert!(text.len() <= CONTEXT_EXCERPT_BYTES);
        assert!(text.ends_with("word"), "{text:?}");
    }

    #[test]
    fn shortening_keeps_characters_whole() {
        let mut text = "ééééé".to_owned();
        assert!(shorten(&mut text, 1));
        assert!(text.is_empty() || text.chars().all(|c| c == 'é'));
        let mut empty = String::new();
        assert!(!shorten(&mut empty, 10));
    }
}
