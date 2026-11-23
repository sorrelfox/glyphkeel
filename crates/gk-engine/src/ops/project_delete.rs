// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Deleting a project for good, and the project's brief for agents.
//!
//! Deleting removes the project and its sub-projects with everything in
//! them: tasks (trashed ones too), their links (also those from other
//! projects' tasks), comments and their earlier texts, attachments, time
//! entries, reminders, drafts, description revisions, statuses, types,
//! labels, layouts, saved views and wiki pages. Stored files no other
//! attachment uses are removed as well. It is a permanent operation: the
//! person confirms it and agents need the full profile. History keeps one
//! row saying what was deleted; earlier rows about those items lose their
//! values, so the deleted text is gone from history too, and can't be
//! reverted.

use gk_api::OpError;
use gk_api::ops::project::{
    AgentBrief, BriefSource, ProjectBrief, ProjectDelete, ProjectDeleteInput, ProjectDeleted,
    ProjectRef,
};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::json;

use super::desc;
use super::fields::PROJECT;
use super::lookup::{self, b, id_at};
use crate::failure::Failure;
use crate::registry::Registry;
use crate::write::WriteCtx;

pub(super) fn register(r: &mut Registry) {
    r.query::<ProjectBrief, _>(desc::<ProjectBrief>(), |q, input: ProjectRef| {
        let p = lookup::project(q.tx, "project", &input.project)?;
        brief(q.tx, p.id, p.key)
    });
    r.command::<ProjectDelete, _>(desc::<ProjectDelete>(), delete);
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
fn clear_task(w: &WriteCtx<'_>, task: Id, files: &mut Vec<String>) -> Result<(), Failure> {
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

fn delete(w: &mut WriteCtx<'_>, input: ProjectDeleteInput) -> Result<ProjectDeleted, Failure> {
    let root = lookup::project(w.tx(), "project", &input.project)?;
    let projects = whole_subtree(w.tx(), root.id)?;
    let tasks = count(
        w.tx(),
        "SELECT count(*) FROM tasks WHERE project_id = ?1",
        &projects,
    )?;
    if let Some(expected) = input.expected_task_count
        && expected != tasks
    {
        return Err(OpError::conflict(format!(
            "{} and its sub-projects hold {tasks} tasks, not {expected}; nothing was deleted.",
            root.key
        ))
        .with_detail("tasks", tasks)
        .with_hint("Check the project again and repeat with the right count.")
        .into());
    }
    let comments = count(
        w.tx(),
        "SELECT count(*) FROM comments c JOIN tasks t ON t.id = c.task_id WHERE t.project_id = ?1",
        &projects,
    )?;
    let mut keys = Vec::with_capacity(projects.len());
    let mut names = Vec::with_capacity(projects.len());
    for p in &projects {
        let (key, name): (String, String) = w.tx().query_row(
            "SELECT key, name FROM projects WHERE id = ?1",
            [b(*p)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        keys.push(key);
        names.push(name);
    }

    let mut files = Vec::new();
    let mut file_count = 0u64;
    for p in projects.iter().rev() {
        for task in ids(w.tx(), "SELECT id FROM tasks WHERE project_id = ?1", *p)? {
            file_count += count(
                w.tx(),
                "SELECT count(*) FROM attachments WHERE owner_id = ?1
                    AND owner_kind = 'task'",
                &[task],
            )?;
            file_count += count(
                w.tx(),
                "SELECT count(*) FROM attachments a JOIN comments c ON c.id = a.owner_id
                  WHERE a.owner_kind = 'comment' AND c.task_id = ?1",
                &[task],
            )?;
            clear_task(w, task, &mut files)?;
        }
        run(w, "DELETE FROM tasks WHERE project_id = ?1", *p)?;
        for page in ids(
            w.tx(),
            "SELECT id FROM wiki_pages WHERE project_id = ?1",
            *p,
        )? {
            file_count += collect_files(w.tx(), "wiki_page", page, &mut files)?;
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
        file_count += collect_files(w.tx(), "project", *p, &mut files)?;
        for sql in [
            "DELETE FROM wiki_redirects WHERE project_id = ?1",
            "DELETE FROM wiki_pages WHERE project_id = ?1",
            "DELETE FROM attachments WHERE owner_kind = 'project' AND owner_id = ?1",
            "DELETE FROM text_revisions WHERE owner_kind = 'project' AND owner_id = ?1",
            "DELETE FROM drafts WHERE owner_kind IN ('project', 'new_wiki_page') AND owner_id = ?1",
            "DELETE FROM statuses WHERE project_id = ?1",
            "DELETE FROM task_types WHERE project_id = ?1",
            "DELETE FROM labels WHERE project_id = ?1",
            "DELETE FROM project_key_aliases WHERE project_id = ?1",
            "DELETE FROM saved_views WHERE project_id = ?1",
            "DELETE FROM search_docs WHERE project_id = ?1",
        ] {
            run(w, sql, *p)?;
        }
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
        run(w, "DELETE FROM projects WHERE id = ?1", *p)?;
    }
    super::file::drop_unused(w, &files)?;
    w.record_field(
        PROJECT,
        root.id,
        Some(root.id),
        "deleted",
        Some(json!({ "key": keys[0], "name": names[0], "projects": keys, "tasks": tasks })),
        None,
    );
    Ok(ProjectDeleted {
        project: root.key,
        projects: keys,
        tasks,
        comments,
        files: file_count,
        group: w.group().to_string(),
    })
}
