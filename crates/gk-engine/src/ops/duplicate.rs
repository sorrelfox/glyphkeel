// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Duplicating a task.
//!
//! The copy gets the original's title (or a new one), description with its
//! checklist as it stands, type, priority, labels, dates, estimate and
//! progress, its version, its sprint unless that sprint is closed, and its
//! files (sharing the stored files). Its subtasks are
//! copied too, in order, unless left out. The copy sits next to the
//! original (same parent), starts in the project's first to-do status (or
//! the one given) at the bottom of the column, and is linked to the
//! original as its clone. Comments, time and links are not copied.

use std::collections::HashMap;

use gk_api::ops::task::{TaskChange, TaskDuplicate, TaskDuplicateInput, TaskPart};
use gk_core::{Id, LinkKind};
use gk_store::rusqlite::params;
use serde_json::json;

use super::desc;
use super::lookup::{self, List, Target, b, opt_id_at};
use super::task::{NewTask, descendants, first_status, invalid, label_ids};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

pub(super) fn register(r: &mut Registry) {
    r.command::<TaskDuplicate, _>(desc::<TaskDuplicate>(), duplicate);
}

/// The fields copied from one task.
struct Source {
    title: String,
    description: String,
    task_type: Id,
    priority: i64,
    parent: Option<Id>,
    start_date: Option<String>,
    due_date: Option<String>,
    estimate: Option<i64>,
    progress: Option<i64>,
}

fn source(w: &WriteCtx<'_>, id: Id) -> Result<Source, Failure> {
    Ok(w.tx().query_row(
        "SELECT title, description, type_id, priority, parent_id, start_date, due_date,
                estimate_min, progress_override
           FROM tasks WHERE id = ?1",
        [b(id)],
        |r| {
            Ok(Source {
                title: r.get(0)?,
                description: r.get(1)?,
                task_type: lookup::id_at(r, 2)?,
                priority: r.get(3)?,
                parent: opt_id_at(r, 4)?,
                start_date: r.get(5)?,
                due_date: r.get(6)?,
                estimate: r.get(7)?,
                progress: r.get(8)?,
            })
        },
    )?)
}

fn duplicate(w: &mut WriteCtx<'_>, input: TaskDuplicateInput) -> Result<TaskChange, Failure> {
    let original = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), original.project)?.ok_or_else(|| {
        Failure::Op(messages::not_found(
            "task-not-found",
            &[("task", input.key.clone())],
        ))
    })?;
    lookup::ensure_active(&project)?;
    let title = input
        .title
        .as_deref()
        .map(gk_core::check_title)
        .transpose()
        .map_err(|e| invalid("title", e))?;
    let fresh = first_status(w.tx(), &project)?;
    let root_status = match &input.status {
        Some(s) => lookup::named(w.tx(), List::Statuses, &project, "status", s)?.id,
        None => fresh,
    };
    let tasks = if input.without_subtasks {
        vec![original.id]
    } else {
        descendants(w.tx(), original.id)?
    };
    let mut copies: HashMap<Id, Id> = HashMap::new();
    for old in tasks {
        let src = source(w, old)?;
        let is_root = old == original.id;
        let parent = if is_root {
            src.parent
        } else {
            src.parent.and_then(|p| copies.get(&p).copied())
        };
        let new = super::task::insert(
            w,
            NewTask {
                project: project.id,
                project_key: project.key.clone(),
                title: match (&title, is_root) {
                    (Some(t), true) => t.clone(),
                    _ => src.title,
                },
                description: src.description,
                status: if is_root { root_status } else { fresh },
                task_type: src.task_type,
                priority: src.priority,
                parent,
                start_date: src.start_date,
                due_date: src.due_date,
                estimate: src.estimate,
                progress: src.progress,
                labels: label_ids(w.tx(), old)?,
                target: Target::Bottom,
            },
        )?;
        super::version::copy_to(w, old, new, project.id)?;
        super::sprint::copy_to(w, old, new, project.id)?;
        super::file::copy_to(w, old, new, project.id)?;
        super::custom_field::copy_values(w, old, new)?;
        copies.insert(old, new);
    }
    let copy = copies
        .get(&original.id)
        .copied()
        .ok_or_else(|| Failure::Op(messages::storage("storage-copy-not-made", &[])))?;
    let (relation, from, to) = LinkKind::Clones.stored(copy, original.id);
    let link = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO task_relations (id, from_task, to_task, kind, created_at, created_by_kind,
                                     created_by_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            b(link),
            b(from),
            b(to),
            relation.as_str(),
            w.now(),
            caller.actor.kind.as_str(),
            caller.actor.name
        ],
    )?;
    w.record_field(
        super::relation::ENTITY,
        link,
        Some(project.id),
        "link",
        None,
        Some(json!({ "from": from.to_string(), "to": to.to_string(), "kind": relation.as_str() })),
    );
    super::task::changed_with(w, copy, Vec::new(), &[TaskPart::Subtasks, TaskPart::Links])
}
