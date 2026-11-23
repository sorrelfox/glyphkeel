// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Comments and their earlier texts.
//!
//! Rules: a comment is Markdown, not empty, at most 256 KB; an answer
//! belongs to the same task as the comment it answers; editing keeps the
//! earlier text as a revision and raises the comment's version; deleting
//! moves the comment to the trash (in the command's group), and reverting
//! that brings it back. Comments in archived projects can't change.
//!
//! History records a comment's `created` (its life), `trashed` and `body`.

use gk_api::OpError;
use gk_api::ops::comment::{
    Comment, CommentAdd, CommentAddInput, CommentChange, CommentDelete, CommentList,
    CommentListInput, CommentListOp, CommentRef, CommentRevision, CommentUpdate,
    CommentUpdateInput,
};
use gk_api::ops::common::{Actor, ActorKind, Via};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::text;
use super::lookup::{self, b, id_at, opt_id_at};
use super::relation::bump;
use crate::failure::Failure;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for comments.
pub(crate) const ENTITY: &str = "comment";

pub(super) fn register(r: &mut Registry) {
    r.query::<CommentListOp, _>(desc::<CommentListOp>(), |q, input: CommentListInput| {
        let task = lookup::task(q.tx, "key", &input.key)?;
        let items = list(q.tx, task.id, input.revisions)?;
        Ok(CommentList {
            total: items.len() as u64,
            items,
        })
    });
    r.command::<CommentAdd, _>(desc::<CommentAdd>(), |w, input: CommentAddInput| {
        let id = add_comment(w, &input, "body")?;
        changed(w, id)
    });
    r.command::<CommentUpdate, _>(desc::<CommentUpdate>(), update);
    r.command::<CommentDelete, _>(desc::<CommentDelete>(), |w, input: CommentRef| {
        let c = find(w.tx(), &input.comment)?;
        ensure_active(w.tx(), c.task)?;
        set_live(w, c.id, Some(c.project), "trashed", Some(Value::Bool(true)))?;
        changed(w, c.id)
    });
}

fn actor(kind: &str, name: String) -> Actor {
    Actor {
        kind: if kind == "agent" {
            ActorKind::Agent
        } else {
            ActorKind::User
        },
        name,
    }
}

fn via(text: &str) -> Via {
    match text {
        "cli" => Via::Cli,
        "mcp" => Via::Mcp,
        _ => Via::App,
    }
}

const COLUMNS: &str = "c.id, p.key || '-' || t.number, c.body, c.reply_to, c.actor_kind,
    c.actor_name, c.via, c.created_at, c.edited_at, c.version,
    (SELECT count(*) FROM comment_revisions v WHERE v.comment_id = c.id)";

fn row(r: &Row<'_>) -> gk_store::rusqlite::Result<Comment> {
    Ok(Comment {
        id: id_at(r, 0)?.to_string(),
        task: r.get(1)?,
        body: r.get(2)?,
        reply_to: opt_id_at(r, 3)?.map(|id| id.to_string()),
        author: actor(&r.get::<_, String>(4)?, r.get(5)?),
        via: via(&r.get::<_, String>(6)?),
        created_at: rfc3339(r.get(7)?),
        edited_at: r.get::<_, Option<i64>>(8)?.map(rfc3339),
        version: r.get(9)?,
        revision_count: u32::try_from(r.get::<_, i64>(10)?).unwrap_or(0),
        revisions: None,
        files: Vec::new(),
    })
}

/// The earlier texts of comment `id`, newest first.
fn revisions(tx: &Transaction<'_>, id: Id) -> Result<Vec<CommentRevision>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT body, actor_kind, actor_name, created_at FROM comment_revisions
          WHERE comment_id = ?1 ORDER BY created_at DESC, id DESC",
    )?;
    let items = stmt
        .query_map([b(id)], |r| {
            Ok(CommentRevision {
                body: r.get(0)?,
                replaced_by: actor(&r.get::<_, String>(1)?, r.get(2)?),
                replaced_at: rfc3339(r.get(3)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// One comment as outputs carry it, trashed or not.
fn load(tx: &Transaction<'_>, id: Id, with_revisions: bool) -> Result<Comment, Failure> {
    let mut c = tx.query_row(
        &format!(
            "SELECT {COLUMNS} FROM comments c JOIN tasks t ON t.id = c.task_id
               JOIN projects p ON p.id = t.project_id WHERE c.id = ?1"
        ),
        [b(id)],
        row,
    )?;
    if with_revisions {
        c.revisions = Some(revisions(tx, id)?);
    }
    c.files = super::file::of_comment(tx, id)?;
    Ok(c)
}

/// The live comments of task `task`, oldest first.
pub(crate) fn list(
    tx: &Transaction<'_>,
    task: Id,
    with_revisions: bool,
) -> Result<Vec<Comment>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT id FROM comments WHERE task_id = ?1 AND deleted_at IS NULL
          ORDER BY created_at, id",
    )?;
    let ids = stmt
        .query_map([b(task)], |r| id_at(r, 0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| load(tx, id, with_revisions))
        .collect()
}

/// What commands need of a comment row.
pub(crate) struct CommentRow {
    pub(crate) id: Id,
    pub(crate) task: Id,
    pub(crate) project: Id,
    version: i64,
}

/// The live comment `text` names (its ID).
pub(crate) fn find(tx: &Transaction<'_>, text: &str) -> Result<CommentRow, Failure> {
    let missing = || {
        Failure::Op(
            OpError::not_found(format!("There is no comment \"{text}\"."), &[])
                .with_detail("field", "comment")
                .with_hint("Comment IDs are listed by get_task with include [\"comments\"]."),
        )
    };
    let id: Id = text.trim().parse().map_err(|_| missing())?;
    tx.query_row(
        "SELECT c.id, c.task_id, t.project_id, c.version FROM comments c
           JOIN tasks t ON t.id = c.task_id
          WHERE c.id = ?1 AND c.deleted_at IS NULL AND t.deleted_at IS NULL",
        [b(id)],
        |r| {
            Ok(CommentRow {
                id: id_at(r, 0)?,
                task: id_at(r, 1)?,
                project: id_at(r, 2)?,
                version: r.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(missing)
}

fn ensure_active(tx: &Transaction<'_>, task: Id) -> Result<(), Failure> {
    let t = lookup::task_by_id(tx, task)?
        .ok_or_else(|| Failure::Op(OpError::not_found("The comment's task is gone.", &[])))?;
    if let Some(p) = lookup::project_by_id(tx, t.project)? {
        lookup::ensure_active(&p)?;
    }
    Ok(())
}

fn invalid(field: &str, e: impl std::fmt::Display) -> Failure {
    super::task::invalid(field, e)
}

/// Keeps the comment's search entry in step: indexed while live, removed
/// while trashed.
fn reindex(w: &WriteCtx<'_>, id: Id) -> Result<(), Failure> {
    type Indexed = (Vec<u8>, Vec<u8>, String, String, bool);
    let row: Option<Indexed> = w
        .tx()
        .query_row(
            "SELECT c.task_id, t.project_id, t.title, c.body, c.deleted_at IS NULL
               FROM comments c JOIN tasks t ON t.id = c.task_id WHERE c.id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    match row {
        Some((task, project, title, body, true)) => {
            w.tx().execute(
                "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                                          updated_at)
                 VALUES ('comment', ?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (kind, entity_id) DO UPDATE SET title = excluded.title,
                   body = excluded.body, updated_at = excluded.updated_at",
                params![b(id), task, project, title, body, w.now()],
            )?;
        }
        _ => {
            w.tx().execute(
                "DELETE FROM search_docs WHERE kind = 'comment' AND entity_id = ?1",
                [b(id)],
            )?;
        }
    }
    Ok(())
}

/// Adds a comment to the task `input.key` names and records it; `field`
/// names the body's input field in errors. Returns the comment's ID.
pub(crate) fn add_comment(
    w: &mut WriteCtx<'_>,
    input: &CommentAddInput,
    field: &str,
) -> Result<Id, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    ensure_active(w.tx(), task.id)?;
    gk_core::check_comment(&input.body).map_err(|e| invalid(field, e))?;
    let reply_to = match &input.reply_to {
        Some(text) => {
            let c = find(w.tx(), text).map_err(|_| {
                Failure::Op(OpError::not_found(
                    format!("There is no comment \"{text}\" on this task."),
                    &[],
                ))
            })?;
            if c.task != task.id {
                return Err(OpError::invalid_at(
                    "reply_to",
                    "An answer must be on the same task as the comment it answers.",
                )
                .into());
            }
            Some(c.id)
        }
        None => None,
    };
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO comments (id, task_id, body, reply_to, actor_kind, actor_name, session_id,
                               via, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            b(id),
            b(task.id),
            input.body,
            reply_to.map(b),
            caller.actor.kind.as_str(),
            caller.actor.name,
            caller.session.map(b),
            caller.via.as_str(),
            w.now()
        ],
    )?;
    reindex(w, id)?;
    bump(w, task.id)?;
    let key = lookup::task_key(w.tx(), task.project, task.number)?;
    let pid = Some(task.project);
    w.record_field(ENTITY, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(ENTITY, id, pid, "task", None, text(key));
    w.record_field(ENTITY, id, pid, "body", None, text(input.body.as_str()));
    Ok(id)
}

fn changed(w: &WriteCtx<'_>, id: Id) -> Result<CommentChange, Failure> {
    let deleted: bool = w.tx().query_row(
        "SELECT deleted_at IS NOT NULL FROM comments WHERE id = ?1",
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(CommentChange {
        comment: load(w.tx(), id, false)?,
        deleted,
        group: w.group_if_changed(),
    })
}

/// The comment's body, as history records it.
pub(crate) fn raw_body(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    Ok(tx
        .query_row("SELECT body FROM comments WHERE id = ?1", [b(id)], |r| {
            r.get::<_, String>(0)
        })
        .optional()?
        .map(Value::String))
}

/// Replaces the comment's body, keeping the earlier one as a revision, and
/// records the change. Returns whether it changed.
pub(crate) fn set_body(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    body: &str,
) -> Result<bool, Failure> {
    let before = raw_body(w.tx(), id)?;
    let Some(Value::String(old)) = &before else {
        return Err(OpError::not_found("The comment is gone.", &[]).into());
    };
    if old == body {
        return Ok(false);
    }
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO comment_revisions (id, comment_id, body, actor_kind, actor_name, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            b(Id::generate_at(u64::try_from(w.now()).unwrap_or(0))),
            b(id),
            old,
            caller.actor.kind.as_str(),
            caller.actor.name,
            w.now()
        ],
    )?;
    w.tx().execute(
        "UPDATE comments SET body = ?2, edited_at = ?3, version = version + 1 WHERE id = ?1",
        params![b(id), body, w.now()],
    )?;
    reindex(w, id)?;
    let task: Vec<u8> =
        w.tx()
            .query_row("SELECT task_id FROM comments WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })?;
    if let Ok(task) = Id::from_slice(&task) {
        bump(w, task)?;
    }
    w.record_field(ENTITY, id, project, "body", before, text(body));
    Ok(true)
}

fn update(w: &mut WriteCtx<'_>, input: CommentUpdateInput) -> Result<CommentChange, Failure> {
    let c = find(w.tx(), &input.comment)?;
    ensure_active(w.tx(), c.task)?;
    if let Some(expected) = input.expected_version
        && expected != c.version
    {
        return Err(OpError::conflict(format!(
            "The comment changed since you read it: it has version {}, not {expected}.",
            c.version
        ))
        .with_detail("version", c.version)
        .with_hint("Read the comment again and repeat the change.")
        .into());
    }
    gk_core::check_comment(&input.body).map_err(|e| invalid("body", e))?;
    set_body(w, c.id, Some(c.project), &input.body)?;
    changed(w, c.id)
}

/// Whether comment `id` is live: as `created` records it (`true` while
/// live) or as `trashed` does (`true` while in the trash).
pub(crate) fn raw_life(
    tx: &Transaction<'_>,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM comments WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(live
        .filter(|l| *l == (field == "created"))
        .map(|_| Value::Bool(true)))
}

/// Moves comment `id` to the trash or back, as `field` (`created` or
/// `trashed`) set to `value` asks, and records the change.
pub(crate) fn set_live(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    let live = (field == "created") == value.is_some();
    let before = raw_life(w.tx(), id, field)?;
    if before == value {
        return Ok(());
    }
    if live {
        w.tx().execute(
            "UPDATE comments SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1",
            [b(id)],
        )?;
    } else {
        w.tx().execute(
            "UPDATE comments SET deleted_at = ?2, deleted_group = ?3 WHERE id = ?1",
            params![b(id), w.now(), b(w.group())],
        )?;
    }
    reindex(w, id)?;
    let task: Vec<u8> =
        w.tx()
            .query_row("SELECT task_id FROM comments WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })?;
    if let Ok(task) = Id::from_slice(&task) {
        bump(w, task)?;
    }
    w.record_field(ENTITY, id, project, field, before, value);
    Ok(())
}
