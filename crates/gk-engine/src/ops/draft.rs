// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Drafts: an editor's unsaved text, kept per person in `drafts` so that it
//! survives a crash or a restart.
//!
//! Rules: one draft per person and editor (the person is the caller's typed
//! actor, so people sharing a workspace never see each other's drafts);
//! nothing is recorded in history and no change event is sent; saving an
//! empty text discards the draft; saving the edited text through its own
//! operation (a description, a comment, a wiki page) discards the caller's
//! draft of it; a draft not saved for [`DRAFT_EXPIRY_DAYS`] days is no
//! longer offered, and is deleted by the next save.

use gk_api::ops::draft::{
    DRAFT_EXPIRY_DAYS, Draft, DraftDiscard, DraftDiscarded, DraftGet, DraftKind, DraftList,
    DraftListInput, DraftListOp, DraftOutput, DraftRef, DraftSave, DraftSaveInput,
};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};

use super::desc;
use super::lookup::{self, b, id_at, opt_id_at};
use crate::context::Context;
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// How long a draft is kept, in milliseconds.
const EXPIRY_MS: i64 = DRAFT_EXPIRY_DAYS * 24 * 60 * 60 * 1000;
/// The most drafts one list returns.
const LIST_MAX: i64 = 200;

pub(super) fn register(r: &mut Registry) {
    r.query::<DraftGet, _>(desc::<DraftGet>(), |q, input: DraftRef| {
        let owner = resolve(q.tx, input.kind, &input.target)?;
        Ok(DraftOutput {
            draft: one(q.tx, q.caller, &owner, q.now)?,
        })
    });
    r.query::<DraftListOp, _>(desc::<DraftListOp>(), |q, input: DraftListInput| {
        let task = input
            .task
            .as_deref()
            .map(|t| lookup::task(q.tx, "task", t))
            .transpose()?;
        Ok(DraftList {
            items: list(q.tx, q.caller, task.map(|t| t.id), q.now)?,
        })
    });
    r.command::<DraftSave, _>(desc::<DraftSave>(), save);
    r.command::<DraftDiscard, _>(desc::<DraftDiscard>(), |w, input: DraftRef| {
        let owner = resolve(w.tx(), input.kind, &input.target)?;
        Ok(DraftDiscarded {
            discarded: clear(w, owner.kind, owner.id)?,
        })
    });
}

/// What a draft is about: the editor's kind, the ID of what it edits and,
/// for drafts about a task, the task's key.
struct Owner {
    kind: DraftKind,
    id: Id,
    task: Option<String>,
}

/// Finds what `target` names for a draft of `kind`.
fn resolve(tx: &Transaction<'_>, kind: DraftKind, target: &str) -> Result<Owner, Failure> {
    let (id, task) = match kind {
        DraftKind::Task | DraftKind::NewComment => {
            let t = lookup::task(tx, "target", target)?;
            (t.id, Some(lookup::task_key(tx, t.project, t.number)?))
        }
        DraftKind::Comment => {
            let c = super::comment::find(tx, target)?;
            let key = match lookup::task_by_id(tx, c.task)? {
                Some(t) => Some(lookup::task_key(tx, t.project, t.number)?),
                None => None,
            };
            (c.id, key)
        }
        DraftKind::Project | DraftKind::NewWikiPage => {
            (lookup::project(tx, "target", target)?.id, None)
        }
        DraftKind::WikiPage => (super::wiki::find_page(tx, None, "target", target)?.id, None),
    };
    Ok(Owner { kind, id, task })
}

fn kind_of(name: &str) -> Option<DraftKind> {
    [
        DraftKind::Task,
        DraftKind::Comment,
        DraftKind::NewComment,
        DraftKind::Project,
        DraftKind::WikiPage,
        DraftKind::NewWikiPage,
    ]
    .into_iter()
    .find(|k| k.as_str() == name)
}

/// The caller's draft of `owner`, unless it has expired.
fn one(
    tx: &Transaction<'_>,
    caller: &Context,
    owner: &Owner,
    now: i64,
) -> Result<Option<Draft>, Failure> {
    let row = tx
        .query_row(
            "SELECT title, body, base_rev, updated_at FROM drafts
              WHERE actor_kind = ?1 AND actor_name = ?2 AND owner_kind = ?3 AND owner_id = ?4
                AND updated_at >= ?5",
            params![
                caller.actor.kind.as_str(),
                caller.actor.name,
                owner.kind.as_str(),
                b(owner.id),
                now - EXPIRY_MS
            ],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, String>(1)?,
                    opt_id_at(r, 2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(title, body, base, at)| Draft {
        kind: owner.kind,
        target: owner.id.to_string(),
        task: owner.task.clone(),
        body,
        title,
        base_revision: base.map(|r| r.to_string()),
        updated_at: rfc3339(at),
    }))
}

/// The caller's drafts that haven't expired, newest first; with `task`,
/// only those about that task and its live comments.
fn list(
    tx: &Transaction<'_>,
    caller: &Context,
    task: Option<Id>,
    now: i64,
) -> Result<Vec<Draft>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT d.owner_kind, d.owner_id, d.title, d.body, d.base_rev, d.updated_at,
                p.key || '-' || coalesce(t1.number, t2.number)
           FROM drafts d
           LEFT JOIN tasks t1 ON d.owner_kind IN ('task', 'new_comment') AND t1.id = d.owner_id
           LEFT JOIN comments c ON d.owner_kind = 'comment' AND c.id = d.owner_id
                               AND c.deleted_at IS NULL
           LEFT JOIN tasks t2 ON t2.id = c.task_id
           LEFT JOIN projects p ON p.id = coalesce(t1.project_id, t2.project_id)
          WHERE d.actor_kind = ?1 AND d.actor_name = ?2 AND d.updated_at >= ?3
            AND (?4 IS NULL OR coalesce(t1.id, t2.id) = ?4)
          ORDER BY d.updated_at DESC, d.owner_kind, d.owner_id
          LIMIT ?5",
    )?;
    let rows = stmt.query_map(
        params![
            caller.actor.kind.as_str(),
            caller.actor.name,
            now - EXPIRY_MS,
            task.map(b),
            LIST_MAX
        ],
        |r: &Row<'_>| {
            Ok((
                r.get::<_, String>(0)?,
                id_at(r, 1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                opt_id_at(r, 4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    let mut items = Vec::new();
    for row in rows {
        let (kind, id, title, body, base, at, key) = row?;
        let Some(kind) = kind_of(&kind) else { continue };
        items.push(Draft {
            kind,
            target: id.to_string(),
            task: key,
            body,
            title,
            base_revision: base.map(|r| r.to_string()),
            updated_at: rfc3339(at),
        });
    }
    Ok(items)
}

fn save(w: &mut WriteCtx<'_>, input: DraftSaveInput) -> Result<DraftOutput, Failure> {
    let owner = resolve(w.tx(), input.kind, &input.target)?;
    gk_core::check_markdown("body", &input.body).map_err(|e| super::task::invalid("body", e))?;
    if let Some(title) = &input.title
        && title.chars().count() > gk_core::text::TITLE_MAX_CHARS
    {
        return Err(messages::invalid(
            "title",
            "draft-title-long",
            &[("max", gk_core::text::TITLE_MAX_CHARS.to_string())],
        ));
    }
    let base = match input.base_revision.as_deref() {
        Some(text) => Some(text.trim().parse::<Id>().map_err(|_| {
            messages::invalid(
                "base_revision",
                "input-not-an-id",
                &[("text", text.to_owned())],
            )
        })?),
        None => None,
    };
    w.tx().execute(
        "DELETE FROM drafts WHERE updated_at < ?1",
        [w.now() - EXPIRY_MS],
    )?;
    if input.body.is_empty() && input.title.as_deref().is_none_or(str::is_empty) {
        clear(w, owner.kind, owner.id)?;
        return Ok(DraftOutput { draft: None });
    }
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO drafts (actor_kind, actor_name, owner_kind, owner_id, title, body, base_rev,
                             updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (actor_kind, actor_name, owner_kind, owner_id) DO UPDATE SET
           title = excluded.title, body = excluded.body, base_rev = excluded.base_rev,
           updated_at = excluded.updated_at",
        params![
            caller.actor.kind.as_str(),
            caller.actor.name,
            owner.kind.as_str(),
            b(owner.id),
            input.title,
            input.body,
            base.map(b),
            w.now()
        ],
    )?;
    Ok(DraftOutput {
        draft: one(w.tx(), &caller, &owner, w.now())?,
    })
}

/// Deletes the caller's draft of the editor `kind` for `owner`. Returns
/// whether there was one. Operations that save an edited text call it, so
/// the saved text isn't offered again as unsaved.
pub(crate) fn clear(w: &WriteCtx<'_>, kind: DraftKind, owner: Id) -> Result<bool, Failure> {
    let caller = w.caller();
    let deleted = w.tx().execute(
        "DELETE FROM drafts
          WHERE actor_kind = ?1 AND actor_name = ?2 AND owner_kind = ?3 AND owner_id = ?4",
        params![
            caller.actor.kind.as_str(),
            caller.actor.name,
            kind.as_str(),
            b(owner)
        ],
    )?;
    Ok(deleted > 0)
}
