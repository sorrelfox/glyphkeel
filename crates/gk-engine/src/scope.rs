// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Limiting a caller to one project and its sub-projects.
//!
//! An agent started with `--project TM` sees only TM and the projects below
//! it. The limit holds for the whole of one operation's transaction, on the
//! thread that runs it, so every lookup of a project, task, page, comment,
//! file, time entry or view can check it without being handed the caller:
//!
//! - lookups treat an item of another project as missing;
//! - lists and searches that take a project get the limit's project when
//!   none is given ([`narrow_input`]); the others filter by [`allows`] or
//!   [`sees`], sub-projects included;
//! - references to tasks of other projects (links, blockers) are left out,
//!   and the projects above the limit's project are not named;
//! - every change a command records must belong to an allowed project, or
//!   the whole command is refused.

use std::cell::RefCell;
use std::collections::HashSet;

use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction};
use serde_json::{Value, json};

use crate::context::Context;
use crate::failure::Failure;
use crate::messages;
use crate::ops::lookup::{self, b};

/// The project a caller is limited to, as resolved for one operation.
#[derive(Debug, Clone)]
struct Scope {
    /// The project's key, for messages.
    key: String,
    /// The project and every project below it when the operation started.
    projects: HashSet<Id>,
    /// The project itself.
    root: Id,
}

thread_local! {
    static CURRENT: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

/// Puts the previous limit back when dropped.
#[derive(Debug)]
#[must_use]
pub(crate) struct Guard {
    previous: Option<Scope>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// Applies `ctx`'s project limit, if any, to the operation that runs in
/// `tx` on this thread until the guard is dropped.
///
/// # Errors
/// `not_found` when the project of the limit doesn't exist.
pub(crate) fn enter(tx: &Transaction<'_>, ctx: &Context) -> Result<Guard, Failure> {
    // The project of the limit itself is looked up without a limit.
    let previous = CURRENT.with(|c| c.borrow_mut().take());
    let guard = Guard { previous };
    let Some(text) = ctx.project_scope.as_deref() else {
        return Ok(guard);
    };
    let root = lookup::project(tx, "project", text).map_err(|_| {
        Failure::from(messages::hint(
            messages::not_found("scope-project-not-found", &[("project", text.to_owned())]),
            "scope-project-not-found-hint",
            &[],
        ))
    })?;
    let mut stmt = tx.prepare_cached(
        "WITH RECURSIVE below (id) AS (
           SELECT ?1
           UNION SELECT p.id FROM projects p JOIN below ON p.parent_id = below.id
         )
         SELECT id FROM below",
    )?;
    let projects = stmt
        .query_map([b(root.id)], |r| lookup::id_at(r, 0))?
        .collect::<Result<HashSet<_>, _>>()?;
    CURRENT.with(|c| {
        *c.borrow_mut() = Some(Scope {
            key: root.key.clone(),
            projects,
            root: root.id,
        });
    });
    Ok(guard)
}

/// Whether a limit applies to the operation running on this thread.
pub(crate) fn active() -> bool {
    CURRENT.with(|c| c.borrow().is_some())
}

/// The limit's project, while a limit applies. Outputs show it as a
/// top-level project: nothing above it is named.
pub(crate) fn root() -> Option<Id> {
    CURRENT.with(|c| c.borrow().as_ref().map(|s| s.root))
}

/// The projects the limit allows as the operation started (the limit's
/// project and all below it, trashed ones too), while a limit applies.
pub(crate) fn projects() -> Option<Vec<Id>> {
    CURRENT.with(|c| {
        c.borrow().as_ref().map(|s| {
            let mut ids: Vec<Id> = s.projects.iter().copied().collect();
            ids.sort_unstable();
            ids
        })
    })
}

/// Whether the project `project` may be seen and changed: always without
/// a limit; with one, when it is the limit's project or below it (also a
/// project made below it during this operation).
pub(crate) fn allows(tx: &Transaction<'_>, project: Id) -> Result<bool, Failure> {
    let Some((root, known)) = CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| (s.root, s.projects.contains(&project)))
    }) else {
        return Ok(true);
    };
    if known {
        return Ok(true);
    }
    let found: Option<i64> = tx
        .query_row(
            "WITH RECURSIVE above (id) AS (
               SELECT parent_id FROM projects WHERE id = ?1
               UNION SELECT p.parent_id FROM projects p JOIN above ON p.id = above.id
             )
             SELECT 1 FROM above WHERE id = ?2",
            [b(project), b(root)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Whether the project `project` may be seen, for filtering rows read in
/// one go: only the projects that existed when the operation started.
pub(crate) fn sees(project: Id) -> bool {
    CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .is_none_or(|s| s.projects.contains(&project))
    })
}

/// The refusal for a command that changes something outside the limit.
pub(crate) fn outside() -> Failure {
    let key = CURRENT
        .with(|c| c.borrow().as_ref().map(|s| s.key.clone()))
        .unwrap_or_default();
    messages::hint(
        messages::forbidden("scope-outside", &[("project", key)]),
        "scope-outside-hint",
        &[],
    )
    .into()
}

/// The input of operation `op` for a caller limited to a project: lists
/// and searches over every project get the limit's project instead. Other
/// inputs are returned as they are.
pub(crate) fn narrow_input(ctx: &Context, op: &str, mut input: Value) -> Value {
    let Some(project) = ctx.project_scope.as_deref() else {
        return input;
    };
    let Some(fields) = input.as_object_mut() else {
        return input;
    };
    let unset = |fields: &serde_json::Map<String, Value>, name: &str| {
        fields.get(name).is_none_or(Value::is_null)
    };
    match op {
        // These take one project and include its sub-projects.
        "search.query" | "task.search" | "schedule.get" => {
            if unset(fields, "project") {
                fields.insert("project".to_owned(), json!(project));
                fields.insert("include_subprojects".to_owned(), json!(true));
            }
        }
        "history.feed" | "time.report" | "today.get" => {
            if unset(fields, "project") {
                fields.insert("project".to_owned(), json!(project));
            }
        }
        "view.list" => {
            fields.insert("all".to_owned(), json!(false));
            if unset(fields, "project") {
                fields.insert("project".to_owned(), json!(project));
            }
        }
        "transfer.export"
            if fields
                .get("projects")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty) =>
        {
            fields.insert("projects".to_owned(), json!([project]));
        }
        _ => {}
    }
    input
}
