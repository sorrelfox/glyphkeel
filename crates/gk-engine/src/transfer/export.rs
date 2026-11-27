// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Exporting: read the records in one snapshot, then write the archive.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use gk_api::ops::transfer::{
    ArchiveCounts, ArchiveFormat, ExportInput, ExportResult, TransferPhase,
};
use gk_api::{ErrorCode, OpError};
use gk_core::Id;
use gk_store::rusqlite::{Row, Transaction};
use serde::Serialize;
use serde_json::Value;

use super::model::{
    Activity, AgentSession, Attachment, AutomationLine, Claim, Comment, CommentRevision, Counts,
    DashboardLine, FolderLine, Label, Layout, Manifest, ManifestProject, Modules, Project,
    ProjectFile, Relation, Revision, SavedView, SeriesLine, Sprint, Status, Task, TaskType,
    TemplateLine, TimeEntry, Version, WikiPage,
};
use super::sink::{Seal, Sink};
use super::{FORMAT_NAME, FORMAT_VERSION, Progress, project_dir, report};
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::ops::lookup;

type Sql<T> = gk_store::rusqlite::Result<T>;

fn text_id(r: &Row<'_>, i: usize) -> Sql<String> {
    let bytes: Vec<u8> = r.get(i)?;
    Ok(Id::from_slice(&bytes).map_or_else(|_| String::new(), |id| id.to_string()))
}

fn opt_id(r: &Row<'_>, i: usize) -> Sql<Option<String>> {
    let bytes: Option<Vec<u8>> = r.get(i)?;
    Ok(bytes
        .and_then(|b| Id::from_slice(&b).ok())
        .map(|id| id.to_string()))
}

fn json_col(r: &Row<'_>, i: usize) -> Sql<Option<Value>> {
    let text: Option<String> = r.get(i)?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

fn all<T>(
    tx: &Transaction<'_>,
    sql: &str,
    arg: &[u8],
    f: impl FnMut(&Row<'_>) -> Sql<T>,
) -> Result<Vec<T>, Failure> {
    let mut stmt = tx.prepare_cached(sql)?;
    let rows = stmt.query_map([arg], f)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// One project as read from the workspace.
struct Out {
    file: ProjectFile,
    tasks: Vec<Task>,
    descriptions: Vec<(i64, String)>,
    comments: Vec<Comment>,
    time: Vec<TimeEntry>,
    revisions: Vec<Revision>,
    wiki: Vec<(WikiPage, String)>,
    files: Vec<Attachment>,
    activity: Vec<Activity>,
}

/// The projects to export: those named with everything below them, or
/// all of them; parents first.
fn chosen(tx: &Transaction<'_>, input: &ExportInput) -> Result<Vec<Id>, Failure> {
    let mut roots = Vec::new();
    if input.projects.is_empty() {
        let mut stmt =
            tx.prepare_cached("SELECT id FROM projects WHERE parent_id IS NULL ORDER BY rank, id")?;
        roots = stmt
            .query_map([], |r| lookup::id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?;
    } else {
        for (i, p) in input.projects.iter().enumerate() {
            let row = lookup::project(tx, &format!("projects[{i}]"), p)?;
            if !roots.contains(&row.id) {
                roots.push(row.id);
            }
        }
    }
    let mut out: Vec<Id> = Vec::new();
    let mut queue = roots;
    while let Some(p) = queue.first().copied() {
        queue.remove(0);
        if out.contains(&p) {
            continue;
        }
        out.push(p);
        let children = all(
            tx,
            "SELECT id FROM projects WHERE parent_id = ?1 ORDER BY rank, id",
            p.as_bytes(),
            |r| lookup::id_at(r, 0),
        )?;
        queue.extend(children);
    }
    Ok(out)
}

/// The folder top-level project `id` is in, as the archive carries it.
fn folder_line(tx: &Transaction<'_>, id: Id) -> Result<Option<FolderLine>, Failure> {
    let Some(folder) = crate::ops::folder::of_project(tx, id)? else {
        return Ok(None);
    };
    Ok(crate::ops::folder::rows(tx)?
        .into_iter()
        .find(|(f, _)| *f == folder)
        .map(|(f, r)| FolderLine {
            id: f.to_string(),
            name: r.name,
            hue: r.hue,
            rank: r.rank,
            created_at: r.created_at,
            created_by_kind: r.created_by_kind,
            created_by_name: r.created_by_name,
            updated_at: r.updated_at,
        }))
}

#[allow(clippy::too_many_lines)]
fn read_project(tx: &Transaction<'_>, id: Id, tasks_in: &HashSet<String>) -> Result<Out, Failure> {
    let pid = id.as_bytes().to_vec();
    let mut project = tx.query_row(
        "SELECT id, parent_id, rank, key, name, description, description_rev, hue, icon, state,
                modules, next_number, created_at, updated_at, deleted_at, deleted_group, agent_rules
           FROM projects WHERE id = ?1",
        [&pid],
        |r| {
            let modules: String = r.get(10)?;
            Ok(Project {
                id: text_id(r, 0)?,
                parent_id: opt_id(r, 1)?,
                rank: r.get(2)?,
                key: r.get(3)?,
                name: r.get(4)?,
                description: r.get(5)?,
                description_rev: opt_id(r, 6)?,
                hue: r.get(7)?,
                icon: r.get(8)?,
                state: r.get(9)?,
                modules: modules_of(&modules),
                next_number: r.get(11)?,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
                deleted_at: r.get(14)?,
                deleted_group: opt_id(r, 15)?,
                agent_rules: json_of(r.get::<_, Option<String>>(16)?),
                folder: None,
            })
        },
    )?;
    project.folder = folder_line(tx, id)?;
    let statuses = all(
        tx,
        "SELECT id, name, category, hue, rank, wip_limit, rules, wip_min FROM statuses
          WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(Status {
                id: text_id(r, 0)?,
                name: r.get(1)?,
                category: r.get(2)?,
                hue: r.get(3)?,
                rank: r.get(4)?,
                wip_limit: r.get(5)?,
                wip_min: r.get(7)?,
                rules: json_of(r.get::<_, Option<String>>(6)?),
            })
        },
    )?;
    let types = all(
        tx,
        "SELECT id, name, glyph, hue, rank, is_default, due_offset_days FROM task_types
          WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(TaskType {
                id: text_id(r, 0)?,
                name: r.get(1)?,
                glyph: r.get(2)?,
                hue: r.get(3)?,
                rank: r.get(4)?,
                is_default: r.get(5)?,
                due_offset_days: r.get(6)?,
            })
        },
    )?;
    let labels = all(
        tx,
        "SELECT id, name, hue, rank FROM labels WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(Label {
                id: text_id(r, 0)?,
                name: r.get(1)?,
                hue: r.get(2)?,
                rank: r.get(3)?,
            })
        },
    )?;
    let versions = all(
        tx,
        "SELECT id, name, description, status, start_date, due_date, sharing, rank,
                created_at, updated_at
           FROM versions WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(Version {
                id: text_id(r, 0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                status: r.get(3)?,
                start_date: r.get(4)?,
                due_date: r.get(5)?,
                sharing: r.get(6)?,
                rank: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        },
    )?;
    let sprints = all(
        tx,
        "SELECT id, name, goal, state, start_date, end_date, started_at, completed_at, rank,
                created_at, updated_at
           FROM sprints WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(Sprint {
                id: text_id(r, 0)?,
                name: r.get(1)?,
                goal: r.get(2)?,
                state: r.get(3)?,
                start_date: r.get(4)?,
                end_date: r.get(5)?,
                started_at: r.get(6)?,
                completed_at: r.get(7)?,
                rank: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        },
    )?;
    let views = all(
        tx,
        "SELECT id, view_kind, name, description, query, columns, sort, group_by, totals,
                options, is_default, rank, created_at, updated_at
           FROM saved_views WHERE project_id = ?1 ORDER BY rank, id",
        &pid,
        |r| {
            Ok(SavedView {
                id: text_id(r, 0)?,
                view_kind: r.get(1)?,
                name: r.get(2)?,
                description: r.get(3)?,
                query: r.get(4)?,
                columns: json_col(r, 5)?,
                sort: json_col(r, 6)?,
                group_by: json_col(r, 7)?,
                totals: json_col(r, 8)?,
                options: json_col(r, 9)?,
                is_default: r.get(10)?,
                rank: r.get(11)?,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            })
        },
    )?;
    let scope = id.to_string();
    let layouts = {
        let mut stmt = tx.prepare_cached(
            "SELECT view_kind, settings, updated_at FROM view_settings WHERE scope = ?1
              ORDER BY view_kind",
        )?;
        stmt.query_map([&scope], |r| {
            Ok(Layout {
                view_kind: r.get(0)?,
                settings: json_col(r, 1)?.unwrap_or(Value::Null),
                updated_at: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let key_aliases = all(
        tx,
        "SELECT key FROM project_key_aliases WHERE project_id = ?1 ORDER BY key",
        &pid,
        |r| r.get(0),
    )?;
    let key = project.key.clone();
    let templates = crate::ops::template::all(tx)?
        .into_iter()
        .filter(|(_, t)| t.project.as_deref() == Some(id.to_string().as_str()))
        .map(|(tid, t)| template_line(tid, t))
        .collect();
    let series = crate::ops::recurrence::of_project(tx, id)?
        .into_iter()
        .map(|(sid, s)| SeriesLine {
            id: sid.to_string(),
            rule: serde_json::to_value(&s.rule).unwrap_or(Value::Null),
            pattern: serde_json::to_value(&s.pattern).unwrap_or(Value::Null),
            current_task: s.current,
            next_date: s.next_date,
            made: s.made,
            ended_at: s.ended_at,
            created_at: s.created_at,
            created_by_kind: s.created_by_kind,
            created_by_name: s.created_by_name,
            updated_at: s.updated_at,
        })
        .collect();
    let automations = crate::ops::automation::of_scope(tx, Some(id))?
        .into_iter()
        .map(|(rid, r)| automation_line(rid, r))
        .collect();
    let dashboards = crate::ops::dashboard::of_scope(tx, Some(id))?
        .into_iter()
        .map(|(did, d)| dashboard_line(did, d))
        .collect();

    let mut descriptions = Vec::new();
    let mut tasks = all(
        tx,
        "SELECT t.id, t.number, t.title, t.description, t.description_rev, t.status_id,
                t.status_entered_at, t.type_id, t.priority, t.parent_id, t.rank, t.start_date,
                t.due_date, t.estimate_min, t.progress_override, t.checklist_done,
                t.checklist_total, t.spent_min, t.claim_session, t.claim_name, t.claim_until,
                t.created_at, t.created_by_kind, t.created_by_name, t.edited_at, t.updated_at,
                t.closed_at, t.deleted_at, t.deleted_group, t.version, t.subtask_rank,
                t.fix_version_id, t.series_id, t.sprint_id, t.cover_id
           FROM tasks t WHERE t.project_id = ?1 ORDER BY t.number",
        &pid,
        |r| {
            let number: i64 = r.get(1)?;
            let description: String = r.get(3)?;
            let claim = match (opt_id(r, 18)?, r.get::<_, Option<i64>>(20)?) {
                (Some(session_id), Some(until)) => Some(Claim {
                    session_id,
                    name: r.get::<_, Option<String>>(19)?.unwrap_or_default(),
                    until,
                }),
                _ => None,
            };
            Ok((
                Task {
                    id: text_id(r, 0)?,
                    project_id: id.to_string(),
                    number,
                    key: format!("{key}-{number}"),
                    title: r.get(2)?,
                    description_rev: opt_id(r, 4)?,
                    status_id: text_id(r, 5)?,
                    status_entered_at: r.get(6)?,
                    type_id: text_id(r, 7)?,
                    priority: r.get(8)?,
                    parent_id: opt_id(r, 9)?,
                    rank: r.get(10)?,
                    start_date: r.get(11)?,
                    due_date: r.get(12)?,
                    estimate_min: r.get(13)?,
                    progress_override: r.get(14)?,
                    checklist_done: r.get(15)?,
                    checklist_total: r.get(16)?,
                    spent_min: r.get(17)?,
                    claim,
                    labels: Vec::new(),
                    relations: Vec::new(),
                    created_at: r.get(21)?,
                    created_by_kind: r.get(22)?,
                    created_by_name: r.get(23)?,
                    edited_at: r.get(24)?,
                    updated_at: r.get(25)?,
                    closed_at: r.get(26)?,
                    deleted_at: r.get(27)?,
                    deleted_group: opt_id(r, 28)?,
                    version: r.get(29)?,
                    subtask_rank: r.get(30)?,
                    key_aliases: Vec::new(),
                    fix_version_id: opt_id(r, 31)?,
                    custom_values: Vec::new(),
                    series_id: opt_id(r, 32)?,
                    sprint_id: opt_id(r, 33)?,
                    cover_id: opt_id(r, 34)?,
                },
                description,
            ))
        },
    )?
    .into_iter()
    .map(|(t, d)| {
        if !d.is_empty() {
            descriptions.push((t.number, d));
        }
        t
    })
    .collect::<Vec<_>>();
    for t in &mut tasks {
        let tid: Id = t.id.parse().unwrap_or_else(|_| Id::generate());
        let tb = tid.as_bytes().to_vec();
        t.labels = all(
            tx,
            "SELECT l.id FROM task_labels x JOIN labels l ON l.id = x.label_id
              WHERE x.task_id = ?1 ORDER BY l.rank, l.id",
            &tb,
            |r| text_id(r, 0),
        )?;
        t.relations = all(
            tx,
            "SELECT id, to_task, kind, lag_days, created_at, created_by_kind, created_by_name
               FROM task_relations WHERE from_task = ?1 ORDER BY id",
            &tb,
            |r| {
                Ok(Relation {
                    id: text_id(r, 0)?,
                    to_task: text_id(r, 1)?,
                    kind: r.get(2)?,
                    lag_days: r.get(3)?,
                    created_at: r.get(4)?,
                    created_by_kind: r.get(5)?,
                    created_by_name: r.get(6)?,
                })
            },
        )?
        .into_iter()
        .filter(|rel| tasks_in.contains(&rel.to_task))
        .collect();
        t.key_aliases = all(
            tx,
            "SELECT project_key || '-' || number FROM task_key_aliases WHERE task_id = ?1
              ORDER BY project_key, number",
            &tb,
            |r| r.get(0),
        )?;
        t.custom_values = super::custom_fields::export_values(tx, &tb)?;
    }
    let task_ids: Vec<Vec<u8>> = tasks
        .iter()
        .filter_map(|t| t.id.parse::<Id>().ok())
        .map(|id| id.as_bytes().to_vec())
        .collect();
    let custom_fields = super::custom_fields::export_fields(tx, &pid, &task_ids)?;

    let mut comments = all(
        tx,
        "SELECT c.id, c.task_id, c.body, c.reply_to, c.actor_kind, c.actor_name, c.session_id,
                c.via, c.created_at, c.edited_at, c.deleted_at, c.deleted_group, c.version
           FROM comments c JOIN tasks t ON t.id = c.task_id
          WHERE t.project_id = ?1 ORDER BY c.created_at, c.id",
        &pid,
        |r| {
            Ok(Comment {
                id: text_id(r, 0)?,
                task_id: text_id(r, 1)?,
                body: r.get(2)?,
                reply_to: opt_id(r, 3)?,
                actor_kind: r.get(4)?,
                actor_name: r.get(5)?,
                session_id: opt_id(r, 6)?,
                via: r.get(7)?,
                created_at: r.get(8)?,
                edited_at: r.get(9)?,
                deleted_at: r.get(10)?,
                deleted_group: opt_id(r, 11)?,
                version: r.get(12)?,
                revisions: Vec::new(),
            })
        },
    )?;
    for c in &mut comments {
        let cid: Id = c.id.parse().unwrap_or_else(|_| Id::generate());
        c.revisions = all(
            tx,
            "SELECT id, body, actor_kind, actor_name, created_at FROM comment_revisions
              WHERE comment_id = ?1 ORDER BY created_at, id",
            cid.as_bytes(),
            |r| {
                Ok(CommentRevision {
                    id: text_id(r, 0)?,
                    body: r.get(1)?,
                    actor_kind: r.get(2)?,
                    actor_name: r.get(3)?,
                    created_at: r.get(4)?,
                })
            },
        )?;
    }
    let time = all(
        tx,
        "SELECT e.id, e.task_id, e.work_date, e.minutes, e.note, coalesce(k.name, e.activity),
                e.actor_kind, e.actor_name, e.session_id, e.created_at, e.edited_at, e.deleted_at,
                e.deleted_group, e.version
           FROM time_entries e JOIN tasks t ON t.id = e.task_id
           LEFT JOIN work_kinds k ON k.id = e.work_kind_id
          WHERE t.project_id = ?1 ORDER BY e.work_date, e.id",
        &pid,
        |r| {
            Ok(TimeEntry {
                id: text_id(r, 0)?,
                task_id: text_id(r, 1)?,
                work_date: r.get(2)?,
                minutes: r.get(3)?,
                note: r.get(4)?,
                activity: r.get(5)?,
                actor_kind: r.get(6)?,
                actor_name: r.get(7)?,
                session_id: opt_id(r, 8)?,
                created_at: r.get(9)?,
                edited_at: r.get(10)?,
                deleted_at: r.get(11)?,
                deleted_group: opt_id(r, 12)?,
                version: r.get(13)?,
            })
        },
    )?;
    let revision = |owner_kind: &'static str| {
        move |r: &Row<'_>| -> Sql<Revision> {
            Ok(Revision {
                id: text_id(r, 0)?,
                owner_kind: owner_kind.to_owned(),
                owner_id: text_id(r, 1)?,
                body: r.get(2)?,
                base_rev: opt_id(r, 3)?,
                kind: r.get(4)?,
                message: r.get(5)?,
                actor_kind: r.get(6)?,
                actor_name: r.get(7)?,
                session_id: opt_id(r, 8)?,
                created_at: r.get(9)?,
                resolved_at: r.get(10)?,
            })
        }
    };
    let mut revisions = all(
        tx,
        "SELECT id, owner_id, body, base_rev, kind, message, actor_kind, actor_name, session_id,
                created_at, resolved_at
           FROM text_revisions WHERE owner_kind = 'project' AND owner_id = ?1
          ORDER BY created_at, id",
        &pid,
        revision("project"),
    )?;
    revisions.extend(all(
        tx,
        "SELECT r.id, r.owner_id, r.body, r.base_rev, r.kind, r.message, r.actor_kind,
                r.actor_name, r.session_id, r.created_at, r.resolved_at
           FROM text_revisions r JOIN tasks t ON t.id = r.owner_id
          WHERE r.owner_kind = 'task' AND t.project_id = ?1 ORDER BY r.created_at, r.id",
        &pid,
        revision("task"),
    )?);
    revisions.extend(all(
        tx,
        "SELECT r.id, r.page_id, r.body, r.base_rev, r.kind, r.message, r.actor_kind,
                r.actor_name, r.session_id, r.created_at, r.resolved_at
           FROM wiki_revisions r JOIN wiki_pages w ON w.id = r.page_id
          WHERE w.project_id = ?1 ORDER BY r.created_at, r.id",
        &pid,
        revision("wiki_page"),
    )?);

    let pages = all(
        tx,
        "SELECT w.id, w.parent_id, w.rank, w.title, w.slug, w.current_rev, w.is_agent_brief,
                w.created_at, w.updated_at, w.deleted_at, w.deleted_group,
                coalesce((SELECT body FROM wiki_revisions r WHERE r.id = w.current_rev), ''),
                w.is_home, w.is_template
           FROM wiki_pages w WHERE w.project_id = ?1 ORDER BY w.rank, w.id",
        &pid,
        |r| {
            Ok((
                WikiPage {
                    id: text_id(r, 0)?,
                    project_id: id.to_string(),
                    parent_id: opt_id(r, 1)?,
                    rank: r.get(2)?,
                    title: r.get(3)?,
                    slug: r.get(4)?,
                    path: String::new(),
                    current_rev: opt_id(r, 5)?.unwrap_or_default(),
                    is_agent_brief: r.get(6)?,
                    is_home: r.get(12)?,
                    is_template: r.get(13)?,
                    created_at: r.get(7)?,
                    updated_at: r.get(8)?,
                    deleted_at: r.get(9)?,
                    deleted_group: opt_id(r, 10)?,
                    redirects: Vec::new(),
                },
                r.get::<_, String>(11)?,
            ))
        },
    )?;
    // Each page's file name: its slug made valid on every system, unique
    // among the pages next to it ignoring case.
    let mut taken = HashSet::new();
    let slugs: BTreeMap<String, (Option<String>, String)> = pages
        .iter()
        .map(|(p, _)| {
            let name = gk_core::unique_file_name(&p.slug, |name| {
                taken.insert((p.parent_id.clone(), name.to_lowercase()))
            });
            (p.id.clone(), (p.parent_id.clone(), name))
        })
        .collect();
    let mut wiki = Vec::new();
    for (mut page, body) in pages {
        let mut parts = vec![
            slugs
                .get(&page.id)
                .map_or_else(|| page.slug.clone(), |(_, name)| name.clone()),
        ];
        let mut at = page.parent_id.clone();
        while let Some(parent) = at {
            match slugs.get(&parent) {
                Some((up, name)) if parts.len() < 16 => {
                    parts.push(name.clone());
                    at = up.clone();
                }
                _ => break,
            }
        }
        parts.reverse();
        page.path = format!("{}.md", parts.join("/"));
        let pb: Id = page.id.parse().unwrap_or_else(|_| Id::generate());
        page.redirects = all(
            tx,
            "SELECT slug FROM wiki_redirects WHERE page_id = ?1 ORDER BY slug",
            pb.as_bytes(),
            |r| r.get(0),
        )?;
        wiki.push((page, body));
    }

    let attachment = |r: &Row<'_>| -> Sql<Attachment> {
        Ok(Attachment {
            id: text_id(r, 0)?,
            owner_kind: r.get(1)?,
            owner_id: text_id(r, 2)?,
            file_name: r.get(3)?,
            mime: r.get(4)?,
            size: u64::try_from(r.get::<_, i64>(5)?).unwrap_or(0),
            sha256: r.get(6)?,
            width: r.get(7)?,
            height: r.get(8)?,
            actor_kind: r.get(9)?,
            actor_name: r.get(10)?,
            created_at: r.get(11)?,
            deleted_at: r.get(12)?,
            deleted_group: opt_id(r, 13)?,
        })
    };
    let files = all(
        tx,
        "SELECT a.id, a.owner_kind, a.owner_id, a.file_name, a.mime, a.size, a.sha256, a.width,
                a.height, a.actor_kind, a.actor_name, a.created_at, a.deleted_at, a.deleted_group
           FROM attachments a
          WHERE (a.owner_kind = 'task'
                 AND a.owner_id IN (SELECT id FROM tasks WHERE project_id = ?1))
             OR (a.owner_kind = 'comment'
                 AND a.owner_id IN (SELECT c.id FROM comments c JOIN tasks t ON t.id = c.task_id
                                     WHERE t.project_id = ?1))
             OR (a.owner_kind = 'wiki_page'
                 AND a.owner_id IN (SELECT id FROM wiki_pages WHERE project_id = ?1))
             OR (a.owner_kind = 'project' AND a.owner_id = ?1)
          ORDER BY a.created_at, a.id",
        &pid,
        attachment,
    )?;
    let activity = all(
        tx,
        "SELECT seq, group_id, at, op, actor_kind, actor_name, via, session_id, project_id,
                entity_kind, entity_id, changes, inverse, reverted_by
           FROM activity WHERE project_id = ?1 ORDER BY seq",
        &pid,
        |r| {
            Ok(Activity {
                seq: r.get(0)?,
                group_id: text_id(r, 1)?,
                at: r.get(2)?,
                op: r.get(3)?,
                actor_kind: r.get(4)?,
                actor_name: r.get(5)?,
                via: r.get(6)?,
                session_id: opt_id(r, 7)?,
                project_id: opt_id(r, 8)?,
                entity_kind: r.get(9)?,
                entity_id: text_id(r, 10)?,
                changes: json_col(r, 11)?.unwrap_or(Value::Null),
                inverse: json_col(r, 12)?,
                reverted_by: opt_id(r, 13)?,
            })
        },
    )?;
    Ok(Out {
        file: ProjectFile {
            project,
            statuses,
            types,
            labels,
            versions,
            sprints,
            views,
            layouts,
            key_aliases,
            templates,
            series,
            automations,
            dashboards,
            custom_fields,
        },
        tasks,
        descriptions,
        comments,
        time,
        revisions,
        wiki,
        files,
        activity,
    })
}

fn automation_line(id: Id, r: crate::ops::automation::Record) -> AutomationLine {
    AutomationLine {
        id: id.to_string(),
        name: r.name,
        enabled: r.enabled,
        trigger: r.trigger,
        condition: r.condition,
        actions: r.actions,
        rank: r.rank,
        created_at: r.created_at,
        created_by_kind: r.created_by_kind,
        created_by_name: r.created_by_name,
        updated_at: r.updated_at,
        updated_by_kind: r.updated_by_kind,
    }
}

fn dashboard_line(id: Id, d: crate::ops::dashboard::Record) -> DashboardLine {
    DashboardLine {
        id: id.to_string(),
        name: d.name,
        description: d.description,
        widgets: d
            .widgets
            .iter()
            .filter_map(|w| serde_json::to_value(w).ok())
            .collect(),
        rank: d.rank,
        created_at: d.created_at,
        created_by_kind: d.created_by_kind,
        created_by_name: d.created_by_name,
        updated_at: d.updated_at,
    }
}

fn template_line(id: Id, t: crate::ops::template::Record) -> TemplateLine {
    TemplateLine {
        id: id.to_string(),
        kind: match t.kind {
            gk_api::ops::template::TemplateKind::Task => "task".to_owned(),
            gk_api::ops::template::TemplateKind::Project => "project".to_owned(),
        },
        name: t.name,
        description: t.description,
        content: t.content,
        rank: t.rank,
        created_at: t.created_at,
        created_by_kind: t.created_by_kind,
        created_by_name: t.created_by_name,
        updated_at: t.updated_at,
    }
}

/// Stored JSON text as a value; none when there is none or it can't be read.
fn json_of(text: Option<String>) -> Option<serde_json::Value> {
    text.and_then(|t| serde_json::from_str(&t).ok())
}

fn modules_of(text: &str) -> Modules {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let flag = |k: &str| -> u8 {
        match v.get(k) {
            Some(Value::Bool(b)) => u8::from(*b),
            Some(Value::Number(n)) => u8::from(n.as_u64().unwrap_or(1) != 0),
            _ => 1,
        }
    };
    Modules {
        board: flag("board"),
        list: flag("list"),
        gantt: flag("gantt"),
        wiki: flag("wiki"),
        time: flag("time"),
    }
}

fn sessions(
    tx: &Transaction<'_>,
    wanted: Option<&BTreeSet<String>>,
) -> Result<Vec<AgentSession>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, client_id, client_version, display_name, profile, scope_project, pid,
                started_at, last_seen_at, ended_at, reads, writes
           FROM agent_sessions ORDER BY started_at, id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AgentSession {
                id: text_id(r, 0)?,
                client_id: r.get(1)?,
                client_version: r.get(2)?,
                display_name: r.get(3)?,
                profile: r.get(4)?,
                scope_project: opt_id(r, 5)?,
                pid: r.get(6)?,
                started_at: r.get(7)?,
                last_seen_at: r.get(8)?,
                ended_at: r.get(9)?,
                reads: r.get(10)?,
                writes: r.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|s| wanted.is_none_or(|w| w.contains(&s.id)))
        .map(|mut s| {
            // A scope outside the archive means nothing in another
            // workspace.
            s.scope_project = None;
            s
        })
        .collect())
}

fn line<T: Serialize>(out: &mut Vec<u8>, record: &T) -> Result<(), OpError> {
    serde_json::to_writer(&mut *out, record)
        .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
    out.push(b'\n');
    Ok(())
}

fn jsonl<T: Serialize>(records: &[T]) -> Result<Vec<u8>, OpError> {
    let mut out = Vec::new();
    for r in records {
        line(&mut out, r)?;
    }
    Ok(out)
}

/// Exports to `target`.
pub(crate) fn export(
    engine: &Engine,
    input: &ExportInput,
    target: &Path,
    format: ArchiveFormat,
    seal: Option<&Seal>,
    progress: &Progress,
) -> Result<ExportResult, OpError> {
    report(progress, TransferPhase::Reading, 0, 1);
    let whole = input.projects.is_empty();
    let (projects, sessions, templates, automations, dashboards) = engine
        .store()
        .read(|tx| {
            let ids = chosen(tx, input)?;
            let mut tasks_in = HashSet::new();
            for p in &ids {
                for t in all(
                    tx,
                    "SELECT id FROM tasks WHERE project_id = ?1",
                    p.as_bytes(),
                    |r| text_id(r, 0),
                )? {
                    tasks_in.insert(t);
                }
            }
            let mut out = Vec::new();
            for (i, p) in ids.iter().enumerate() {
                report(progress, TransferPhase::Reading, i as u64, ids.len() as u64);
                out.push(read_project(tx, *p, &tasks_in)?);
            }
            let wanted: Option<BTreeSet<String>> = (!whole).then(|| {
                let mut w = BTreeSet::new();
                for o in &out {
                    w.extend(o.activity.iter().filter_map(|a| a.session_id.clone()));
                    w.extend(o.comments.iter().filter_map(|c| c.session_id.clone()));
                    w.extend(o.time.iter().filter_map(|e| e.session_id.clone()));
                    w.extend(o.revisions.iter().filter_map(|r| r.session_id.clone()));
                    w.extend(
                        o.tasks
                            .iter()
                            .filter_map(|t| t.claim.as_ref().map(|c| c.session_id.clone())),
                    );
                }
                w
            });
            let sessions = sessions(tx, wanted.as_ref())?;
            // Project templates belong to the workspace: only a whole
            // export takes them.
            let templates: Vec<TemplateLine> = if whole {
                crate::ops::template::all(tx)?
                    .into_iter()
                    .filter(|(_, t)| t.project.is_none())
                    .map(|(tid, t)| template_line(tid, t))
                    .collect()
            } else {
                Vec::new()
            };
            // So do the workspace's own automation rules.
            let automations: Vec<AutomationLine> = if whole {
                crate::ops::automation::of_scope(tx, None)?
                    .into_iter()
                    .map(|(rid, r)| automation_line(rid, r))
                    .collect()
            } else {
                Vec::new()
            };
            // And the workspace's own dashboards.
            let dashboards: Vec<DashboardLine> = if whole {
                crate::ops::dashboard::of_scope(tx, None)?
                    .into_iter()
                    .map(|(did, d)| dashboard_line(did, d))
                    .collect()
            } else {
                Vec::new()
            };
            Ok::<_, Failure>((out, sessions, templates, automations, dashboards))
        })
        .map_err(OpError::from)?;

    let cant_write = |e: std::io::Error| {
        messages::error(
            ErrorCode::Storage,
            "transfer-write-failed",
            &[
                ("path", target.display().to_string()),
                ("error", e.to_string()),
            ],
        )
    };
    let mut sink = Sink::create(target, format, seal).map_err(cant_write)?;
    let written = write_all(
        engine,
        &mut sink,
        &projects,
        &sessions,
        &Own {
            templates: &templates,
            automations: &automations,
            dashboards: &dashboards,
        },
        progress,
    );
    let counts = match written {
        Ok(counts) => counts,
        Err(e) => {
            sink.abandon();
            return Err(e);
        }
    };
    let size = sink.finish().map_err(cant_write)?;
    report(progress, TransferPhase::Done, 1, 1);
    Ok(ExportResult {
        path: target.to_string_lossy().into_owned(),
        format,
        projects: projects
            .iter()
            .map(|p| p.file.project.key.clone())
            .collect(),
        counts,
        size,
        encrypted: seal.is_some(),
    })
}

/// What an export takes of the workspace itself, besides its projects.
struct Own<'a> {
    templates: &'a [TemplateLine],
    automations: &'a [AutomationLine],
    dashboards: &'a [DashboardLine],
}

fn write_all(
    engine: &Engine,
    sink: &mut Sink,
    projects: &[Out],
    sessions: &[AgentSession],
    own: &Own<'_>,
    progress: &Progress,
) -> Result<ArchiveCounts, OpError> {
    let Own {
        templates,
        automations,
        dashboards,
    } = *own;
    let io = |e: std::io::Error| {
        messages::error(
            ErrorCode::Storage,
            "transfer-archive-write-failed",
            &[("error", e.to_string())],
        )
    };
    let mut counts = Counts::default();
    let mut blobs = BTreeSet::new();
    let mut manifest_projects = Vec::new();
    sink.put("agent_sessions.jsonl", &jsonl(sessions)?)
        .map_err(io)?;
    counts.agent_sessions = sessions.len() as u64;
    if !templates.is_empty() {
        sink.put(super::TEMPLATES, &jsonl(templates)?).map_err(io)?;
    }
    if !automations.is_empty() {
        sink.put(super::AUTOMATIONS, &jsonl(automations)?)
            .map_err(io)?;
    }
    if !dashboards.is_empty() {
        sink.put(super::DASHBOARDS, &jsonl(dashboards)?)
            .map_err(io)?;
    }
    for (i, p) in projects.iter().enumerate() {
        report(
            progress,
            TransferPhase::Writing,
            i as u64,
            projects.len() as u64,
        );
        let key = &p.file.project.key;
        let dir = project_dir(key);
        let project_json = serde_json::to_vec_pretty(&p.file)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        sink.put(&format!("{dir}/project.json"), &project_json)
            .map_err(io)?;
        sink.put(&format!("{dir}/tasks.jsonl"), &jsonl(&p.tasks)?)
            .map_err(io)?;
        for (number, text) in &p.descriptions {
            sink.put(&format!("{dir}/tasks/{key}-{number}.md"), text.as_bytes())
                .map_err(io)?;
        }
        sink.put(&format!("{dir}/comments.jsonl"), &jsonl(&p.comments)?)
            .map_err(io)?;
        sink.put(&format!("{dir}/time.jsonl"), &jsonl(&p.time)?)
            .map_err(io)?;
        sink.put(&format!("{dir}/revisions.jsonl"), &jsonl(&p.revisions)?)
            .map_err(io)?;
        let pages: Vec<&WikiPage> = p.wiki.iter().map(|(w, _)| w).collect();
        sink.put(&format!("{dir}/wiki.jsonl"), &jsonl(&pages)?)
            .map_err(io)?;
        for (page, body) in &p.wiki {
            sink.put(&format!("{dir}/wiki/{}", page.path), body.as_bytes())
                .map_err(io)?;
        }
        sink.put(&format!("{dir}/files.jsonl"), &jsonl(&p.files)?)
            .map_err(io)?;
        sink.put(&format!("{dir}/activity.jsonl"), &jsonl(&p.activity)?)
            .map_err(io)?;
        counts.projects += 1;
        counts.tasks += p.tasks.len() as u64;
        counts.open_tasks += p
            .tasks
            .iter()
            .filter(|t| t.closed_at.is_none() && t.deleted_at.is_none())
            .count() as u64;
        counts.dated_tasks += p
            .tasks
            .iter()
            .filter(|t| t.start_date.is_some() || t.due_date.is_some())
            .count() as u64;
        for t in &p.tasks {
            counts.relations += t.relations.len() as u64;
            counts.blocks_relations +=
                t.relations.iter().filter(|r| r.kind == "blocks").count() as u64;
        }
        counts.comments += p.comments.len() as u64;
        counts.time_entries += p.time.len() as u64;
        counts.revisions += p.revisions.len() as u64;
        counts.wiki_pages += p.wiki.len() as u64;
        counts.attachments += p.files.len() as u64;
        counts.activity += p.activity.len() as u64;
        blobs.extend(p.files.iter().map(|a| a.sha256.clone()));
        manifest_projects.push(ManifestProject {
            id: p.file.project.id.clone(),
            key: key.clone(),
            name: p.file.project.name.clone(),
            path: dir,
        });
    }
    let layout = engine.store().layout();
    let total = blobs.len() as u64;
    for (i, sha) in blobs.iter().enumerate() {
        report(progress, TransferPhase::Files, i as u64, total);
        gk_store::file_path(layout, sha).ok_or_else(|| {
            messages::error(
                ErrorCode::Storage,
                "transfer-stored-file-missing",
                &[("sha256", sha.clone())],
            )
        })?;
        sink.put_file(&format!("files/{sha}"), sha, layout)
            .map_err(io)?;
    }
    counts.blobs = total;
    let manifest = Manifest {
        format: FORMAT_NAME.to_owned(),
        format_version: FORMAT_VERSION,
        app_version: engine.env().program_version.clone(),
        workspace_id: engine.env().workspace_id.to_string(),
        exported_at: engine.now_ms(),
        generator: None,
        projects: manifest_projects,
        counts: counts.clone(),
        hashes: sink.hashes.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
    sink.put_manifest(&bytes).map_err(io)?;
    Ok(ArchiveCounts {
        projects: counts.projects,
        tasks: counts.tasks,
        comments: counts.comments,
        time_entries: counts.time_entries,
        revisions: counts.revisions,
        wiki_pages: counts.wiki_pages,
        attachments: counts.attachments,
        files: counts.blobs,
        activity: counts.activity,
        agent_sessions: counts.agent_sessions,
    })
}
