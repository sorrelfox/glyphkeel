// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Purging a project for good, and the project's brief for agents.
//!
//! A project is deleted into the trash (see `trash.rs`); purging it removes
//! it and its sub-projects with everything in them: tasks (trashed ones
//! too), their links (also those from other projects' tasks), comments and
//! their earlier texts, attachments, time entries, reminders, drafts,
//! description revisions, statuses, types, labels, layouts, saved views and
//! wiki pages. History keeps who and when of earlier rows about those
//! items, not their values.

use gk_api::ops::project::{AgentBrief, BriefSource, ProjectBrief, ProjectRef};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};

use super::desc;
use super::lookup::{self, b, id_at};
use crate::failure::Failure;
use crate::registry::Registry;
use crate::write::WriteCtx;

pub(super) fn register(r: &mut Registry) {
    r.query::<ProjectBrief, _>(desc::<ProjectBrief>(), |q, input: ProjectRef| {
        let p = lookup::project(q.tx, "project", &input.project)?;
        brief(q.tx, p.id, p.key)
    });
}

fn brief(tx: &Transaction<'_>, id: Id, key: String) -> Result<AgentBrief, Failure> {
    let page: Option<(String, String)> = tx
        .query_row(
            "SELECT w.title, r.body FROM wiki_pages w JOIN wiki_revisions r ON r.id = w.current_rev
              WHERE w.project_id = ?1 AND w.is_agent_brief = 1 AND w.deleted_at IS NULL",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((title, body)) = page {
        return Ok(AgentBrief {
            project: key,
            source: BriefSource::WikiPage,
            title: Some(title),
            body,
        });
    }
    let description: String = tx.query_row(
        "SELECT description FROM projects WHERE id = ?1",
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(AgentBrief {
        project: key,
        source: if description.trim().is_empty() {
            BriefSource::None
        } else {
            BriefSource::Description
        },
        title: None,
        body: description,
    })
}

fn ids(tx: &Transaction<'_>, sql: &str, owner: Id) -> Result<Vec<Id>, Failure> {
    let mut stmt = tx.prepare_cached(sql)?;
    let out = stmt
        .query_map([b(owner)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// Every project below `id` (and it), parents first, trashed ones too.
fn whole_subtree(tx: &Transaction<'_>, id: Id) -> Result<Vec<Id>, Failure> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let children = ids(
            tx,
            "SELECT id FROM projects WHERE parent_id = ?1 ORDER BY rank, id",
            out[i],
        )?;
        out.extend(children);
        i += 1;
    }
    Ok(out)
}

fn run(w: &WriteCtx<'_>, sql: &str, id: Id) -> Result<usize, Failure> {
    Ok(w.tx().execute(sql, [b(id)])?)
}

/// Removes task `task` and everything that hangs off it, except the task
/// row itself (tasks of a project go in one statement, as they refer to
/// each other). The stored files its attachments used are added to
/// `files`.
pub(crate) fn clear_task(
    w: &WriteCtx<'_>,
    task: Id,
    files: &mut Vec<String>,
) -> Result<(), Failure> {
    super::links::forget_task(w.tx(), task)?;
    let comments = ids(w.tx(), "SELECT id FROM comments WHERE task_id = ?1", task)?;
    for c in &comments {
        collect_files(w.tx(), "comment", *c, files)?;
        run(
            w,
            "DELETE FROM attachments WHERE owner_kind = 'comment' AND owner_id = ?1",
            *c,
        )?;
        run(w, "DELETE FROM comment_revisions WHERE comment_id = ?1", *c)?;
        run(
            w,
            "DELETE FROM drafts WHERE owner_kind = 'comment' AND owner_id = ?1",
            *c,
        )?;
    }
    run(w, "DELETE FROM comments WHERE task_id = ?1", task)?;
    collect_files(w.tx(), "task", task, files)?;
    for sql in [
        "DELETE FROM attachments WHERE owner_kind = 'task' AND owner_id = ?1",
        "DELETE FROM task_relations WHERE from_task = ?1 OR to_task = ?1",
        "DELETE FROM task_labels WHERE task_id = ?1",
        "DELETE FROM task_key_aliases WHERE task_id = ?1",
        "DELETE FROM time_entries WHERE task_id = ?1",
        "DELETE FROM timer WHERE task_id = ?1",
        "DELETE FROM reminders WHERE task_id = ?1",
        "DELETE FROM reminder_log WHERE task_id = ?1",
        "DELETE FROM text_revisions WHERE owner_kind = 'task' AND owner_id = ?1",
        "DELETE FROM drafts WHERE owner_kind IN ('task', 'new_comment') AND owner_id = ?1",
    ] {
        run(w, sql, task)?;
    }
    Ok(())
}

fn collect_files(
    tx: &Transaction<'_>,
    kind: &str,
    owner: Id,
    files: &mut Vec<String>,
) -> Result<u64, Failure> {
    let mut stmt = tx
        .prepare_cached("SELECT sha256 FROM attachments WHERE owner_kind = ?1 AND owner_id = ?2")?;
    let shas = stmt
        .query_map(params![kind, b(owner)], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let n = shas.len() as u64;
    for sha in shas {
        if !files.contains(&sha) {
            files.push(sha);
        }
    }
    Ok(n)
}

fn count(tx: &Transaction<'_>, sql: &str, projects: &[Id]) -> Result<u64, Failure> {
    let mut total = 0u64;
    for p in projects {
        let n: i64 = tx.query_row(sql, [b(*p)], |r| r.get(0))?;
        total += u64::try_from(n).unwrap_or(0);
    }
    Ok(total)
}

/// How much projects hold.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Counts {
    pub(crate) tasks: u64,
    pub(crate) comments: u64,
    pub(crate) files: u64,
}

/// The tasks (trashed ones too), comments and attachments of `projects`.
pub(crate) fn counts(tx: &Transaction<'_>, projects: &[Id]) -> Result<Counts, Failure> {
    Ok(Counts {
        tasks: count(
            tx,
            "SELECT count(*) FROM tasks WHERE project_id = ?1",
            projects,
        )?,
        comments: count(
            tx,
            "SELECT count(*) FROM comments c JOIN tasks t ON t.id = c.task_id
              WHERE t.project_id = ?1",
            projects,
        )?,
        files: count(
            tx,
            "SELECT count(*) FROM attachments a
              WHERE (a.owner_kind = 'task'
                     AND a.owner_id IN (SELECT id FROM tasks WHERE project_id = ?1))
                 OR (a.owner_kind = 'comment'
                     AND a.owner_id IN (SELECT c.id FROM comments c JOIN tasks t
                                          ON t.id = c.task_id WHERE t.project_id = ?1))
                 OR (a.owner_kind = 'wiki_page'
                     AND a.owner_id IN (SELECT id FROM wiki_pages WHERE project_id = ?1))
                 OR (a.owner_kind = 'project' AND a.owner_id = ?1)",
            projects,
        )?,
    })
}

/// Deletes project `root` and every project below it for good, with
/// everything in them. The stored files their attachments used are added
/// to `files`. Returns how many projects and tasks went.
pub(crate) fn purge_projects(
    w: &WriteCtx<'_>,
    root: Id,
    files: &mut Vec<String>,
) -> Result<u64, Failure> {
    let projects = whole_subtree(w.tx(), root)?;
    let mut gone = 0u64;
    for p in projects.iter().rev() {
        for task in ids(w.tx(), "SELECT id FROM tasks WHERE project_id = ?1", *p)? {
            clear_task(w, task, files)?;
            gone += 1;
        }
        w.tx().execute(
            "UPDATE tasks SET parent_id = NULL WHERE project_id = ?1",
            [b(*p)],
        )?;
        run(w, "DELETE FROM tasks WHERE project_id = ?1", *p)?;
        for page in ids(
            w.tx(),
            "SELECT id FROM wiki_pages WHERE project_id = ?1",
            *p,
        )? {
            collect_files(w.tx(), "wiki_page", page, files)?;
            super::links::forget(w.tx(), super::links::Source::Wiki(page))?;
            for sql in [
                "DELETE FROM attachments WHERE owner_kind = 'wiki_page' AND owner_id = ?1",
                "DELETE FROM wiki_redirects WHERE page_id = ?1",
                "DELETE FROM wiki_revisions WHERE page_id = ?1",
                "DELETE FROM drafts WHERE owner_kind IN ('wiki_page', 'new_wiki_page')
                   AND owner_id = ?1",
            ] {
                run(w, sql, page)?;
            }
        }
        collect_files(w.tx(), "project", *p, files)?;
        w.tx().execute(
            "UPDATE wiki_pages SET parent_id = NULL WHERE project_id = ?1",
            [b(*p)],
        )?;
        for sql in [
            "DELETE FROM wiki_redirects WHERE project_id = ?1",
            "DELETE FROM wiki_pages WHERE project_id = ?1",
            "DELETE FROM attachments WHERE owner_kind = 'project' AND owner_id = ?1",
            "DELETE FROM text_revisions WHERE owner_kind = 'project' AND owner_id = ?1",
            "DELETE FROM drafts WHERE owner_kind IN ('project', 'new_wiki_page') AND owner_id = ?1",
            "DELETE FROM statuses WHERE project_id = ?1",
            "DELETE FROM task_types WHERE project_id = ?1",
            "DELETE FROM labels WHERE project_id = ?1",
            "UPDATE tasks SET fix_version_id = NULL
              WHERE fix_version_id IN (SELECT id FROM versions WHERE project_id = ?1)",
            "DELETE FROM versions WHERE project_id = ?1",
            "UPDATE tasks SET sprint_id = NULL
              WHERE sprint_id IN (SELECT id FROM sprints WHERE project_id = ?1)",
            "DELETE FROM sprints WHERE project_id = ?1",
            "DELETE FROM project_key_aliases WHERE project_id = ?1",
            "DELETE FROM saved_views WHERE project_id = ?1",
            "DELETE FROM templates WHERE project_id = ?1",
            "DELETE FROM task_series WHERE project_id = ?1",
            "DELETE FROM search_docs WHERE project_id = ?1",
        ] {
            run(w, sql, *p)?;
        }
        super::automation::purge_project(w.tx(), *p)?;
        w.tx().execute(
            "DELETE FROM view_settings WHERE scope = ?1",
            [p.to_string()],
        )?;
        // Earlier history of these items keeps who and when, not what.
        w.tx().execute(
            "UPDATE activity SET changes = '{\"fields\":{},\"purged\":true}', inverse = NULL
              WHERE project_id = ?1",
            [b(*p)],
        )?;
        w.tx().execute(
            "UPDATE projects SET parent_id = NULL WHERE parent_id = ?1",
            [b(*p)],
        )?;
        run(w, "DELETE FROM projects WHERE id = ?1", *p)?;
        gone += 1;
    }
    Ok(gone)
}
