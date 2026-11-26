// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Descriptions of tasks and projects, and the text of wiki pages:
//! revisions, merging, conflict copies, checklists, and the reader's HTML.
//!
//! Rules: every save of a description is a revision. A save that names the
//! revision it was edited from (its base) and finds another save in between
//! is merged line by line with that save; when both touched the same lines,
//! the stored text stays as it is and the late edit is kept as a conflict
//! copy until someone resolves it. A text saved before revisions existed
//! gets a revision the first time it changes, credited to whoever created
//! the item.
//!
//! History records a description as `{"revision": ID}`, never its text;
//! reverting brings the recorded revision back as a new one (`restored`).
//! A conflict copy is recorded as `conflict_copy`, its resolution as
//! `conflict_resolved`; neither can be reverted.
//!
//! Checklist items are named by number plus expected text: one tick or
//! clear changes exactly one byte. When the number now points at other text,
//! the one item with that text is used; with none or several, the change is
//! refused as stale.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use gk_api::OpError;
use gk_api::ops::common::{Actor, ActorKind};
use gk_api::ops::description::{
    ChecklistChange, ChecklistItem, ChecklistUpdate, ChecklistUpdateInput, ConflictAction,
    DescriptionHistory, DescriptionHistoryInput, DescriptionRevision, DescriptionRevisionInput,
    DescriptionSave, MarkdownRender, RenderInput, RenderOutput, RenderSource, Rendered,
    RevisionDetail, RevisionInfo, RevisionKind, RevisionList, SaveOutcome, TaskResolveConflict,
    TaskResolveInput,
};
use gk_api::ops::task::{Checklist, TaskChange};
use gk_core::{Id, Merge};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::{Value, json};

use super::desc;
use super::fields::{self, PROJECT, TASK, WIKI};
use super::lookup::{self, b, id_at, opt_id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// Most texts one render call takes.
const MAX_RENDER: usize = 200;

pub(super) fn register(r: &mut Registry) {
    r.command::<TaskResolveConflict, _>(desc::<TaskResolveConflict>(), resolve_command);
    r.command::<ChecklistUpdate, _>(desc::<ChecklistUpdate>(), checklist_update);
    r.query::<DescriptionHistory, _>(desc::<DescriptionHistory>(), history);
    r.query::<DescriptionRevision, _>(desc::<DescriptionRevision>(), revision_detail);
    r.query::<MarkdownRender, _>(desc::<MarkdownRender>(), render);
}

// ---------------------------------------------------------------------
// Owners and revisions

/// Whose text: a task's or project's description, or a wiki page's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner {
    Task(Id),
    Project(Id),
    Wiki(Id),
}

impl Owner {
    fn kind(self) -> &'static str {
        match self {
            Self::Task(_) => "task",
            Self::Project(_) => "project",
            Self::Wiki(_) => "wiki_page",
        }
    }

    fn id(self) -> Id {
        match self {
            Self::Task(id) | Self::Project(id) | Self::Wiki(id) => id,
        }
    }

    fn entity(self) -> &'static str {
        match self {
            Self::Task(_) => TASK,
            Self::Project(_) => PROJECT,
            Self::Wiki(_) => WIKI,
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Task(_) => "tasks",
            Self::Project(_) => "projects",
            Self::Wiki(_) => "wiki_pages",
        }
    }

    /// The column that holds the current revision.
    fn rev_column(self) -> &'static str {
        match self {
            Self::Task(_) | Self::Project(_) => "description_rev",
            Self::Wiki(_) => "current_rev",
        }
    }

    /// The text's name in history and in errors.
    pub(crate) fn field(self) -> &'static str {
        match self {
            Self::Task(_) | Self::Project(_) => "description",
            Self::Wiki(_) => "body",
        }
    }

    /// The input field of targeted edits.
    fn edits_field(self) -> &'static str {
        match self {
            Self::Task(_) | Self::Project(_) => "description_edits",
            Self::Wiki(_) => "edits",
        }
    }

    /// The owner of history's `entity`/`id`, for the kinds that have a
    /// text with revisions.
    pub(crate) fn of_entity(entity: &str, id: Id) -> Option<Self> {
        match entity {
            TASK => Some(Self::Task(id)),
            PROJECT => Some(Self::Project(id)),
            WIKI => Some(Self::Wiki(id)),
            _ => None,
        }
    }
}

/// The stored description and its revision.
#[derive(Debug, Clone)]
struct Stored {
    text: String,
    rev: Option<Id>,
}

fn stored(tx: &Transaction<'_>, owner: Owner) -> Result<Stored, Failure> {
    let sql = match owner {
        Owner::Wiki(_) => "SELECT coalesce(r.body, ''), p.current_rev FROM wiki_pages p
                             LEFT JOIN wiki_revisions r ON r.id = p.current_rev WHERE p.id = ?1"
            .to_owned(),
        _ => format!(
            "SELECT description, description_rev FROM {} WHERE id = ?1",
            owner.table()
        ),
    };
    let row = tx
        .query_row(&sql, [b(owner.id())], |r| {
            Ok(Stored {
                text: r.get(0)?,
                rev: opt_id_at(r, 1)?,
            })
        })
        .optional()?;
    row.ok_or_else(|| Failure::Op(messages::not_found("description-item-gone", &[])))
}

/// One row of `text_revisions`.
#[derive(Debug, Clone)]
pub(crate) struct RevisionRow {
    pub(crate) id: Id,
    owner_kind: String,
    owner_id: Id,
    pub(crate) body: String,
    base: Option<Id>,
    kind: String,
    message: Option<String>,
    actor_kind: String,
    actor_name: String,
    created_at: i64,
    resolved_at: Option<i64>,
}

const REVISION_COLUMNS: &str = "id, owner_kind, owner_id, body, base_rev, kind, message,
    actor_kind, actor_name, created_at, resolved_at";
/// The same columns from `wiki_revisions`.
const WIKI_REVISION_COLUMNS: &str = "id, 'wiki_page', page_id, body, base_rev, kind, message,
    actor_kind, actor_name, created_at, resolved_at";

fn revision_row(r: &Row<'_>) -> gk_store::rusqlite::Result<RevisionRow> {
    Ok(RevisionRow {
        id: id_at(r, 0)?,
        owner_kind: r.get(1)?,
        owner_id: id_at(r, 2)?,
        body: r.get(3)?,
        base: opt_id_at(r, 4)?,
        kind: r.get(5)?,
        message: r.get(6)?,
        actor_kind: r.get(7)?,
        actor_name: r.get(8)?,
        created_at: r.get(9)?,
        resolved_at: r.get(10)?,
    })
}

/// The revision `id`, of a description or of a wiki page.
fn revision(tx: &Transaction<'_>, id: Id) -> Result<Option<RevisionRow>, Failure> {
    let text = tx
        .query_row(
            &format!("SELECT {REVISION_COLUMNS} FROM text_revisions WHERE id = ?1"),
            [b(id)],
            revision_row,
        )
        .optional()?;
    if text.is_some() {
        return Ok(text);
    }
    Ok(tx
        .query_row(
            &format!("SELECT {WIKI_REVISION_COLUMNS} FROM wiki_revisions WHERE id = ?1"),
            [b(id)],
            revision_row,
        )
        .optional()?)
}

impl RevisionRow {
    fn owner(&self) -> Owner {
        match self.owner_kind.as_str() {
            "project" => Owner::Project(self.owner_id),
            "wiki_page" => Owner::Wiki(self.owner_id),
            _ => Owner::Task(self.owner_id),
        }
    }

    fn info(&self, current: Option<Id>) -> RevisionInfo {
        RevisionInfo {
            id: self.id.to_string(),
            kind: match self.kind.as_str() {
                "merged" => RevisionKind::Merged,
                "conflict" => RevisionKind::Conflict,
                "restored" => RevisionKind::Restored,
                "resolved" => RevisionKind::Resolved,
                _ => RevisionKind::Normal,
            },
            base: self.base.map(|b| b.to_string()),
            author: Actor {
                kind: if self.actor_kind == "agent" {
                    ActorKind::Agent
                } else {
                    ActorKind::User
                },
                name: self.actor_name.clone(),
            },
            created_at: rfc3339(self.created_at),
            message: self.message.clone(),
            resolved_at: self.resolved_at.map(rfc3339),
            current: current == Some(self.id),
            bytes: self.body.len() as u64,
        }
    }
}

/// A revision ID given by a caller, which must belong to `owner`.
fn revision_of(
    tx: &Transaction<'_>,
    owner: Owner,
    field: &str,
    text: &str,
) -> Result<RevisionRow, Failure> {
    let missing = || {
        Failure::Op(messages::hint(
            messages::not_found("revision-not-found", &[("revision", text.to_owned())])
                .with_detail("field", field),
            match owner {
                Owner::Wiki(_) => "revision-not-found-page-hint",
                _ => "revision-not-found-task-hint",
            },
            &[],
        ))
    };
    let id: Id = text.trim().parse().map_err(|_| missing())?;
    let row = revision(tx, id)?.ok_or_else(missing)?;
    if row.owner() != owner {
        return Err(missing());
    }
    Ok(row)
}

/// The current text of `owner` and its revision.
pub(crate) fn current_text(
    tx: &Transaction<'_>,
    owner: Owner,
) -> Result<(String, Option<Id>), Failure> {
    let s = stored(tx, owner)?;
    Ok((s.text, s.rev))
}

/// The text of revision `text` (an ID a caller gave) of `owner`.
pub(crate) fn revision_text(
    tx: &Transaction<'_>,
    owner: Owner,
    field: &str,
    text: &str,
) -> Result<(Id, String), Failure> {
    let row = revision_of(tx, owner, field, text)?;
    Ok((row.id, row.body))
}

/// Writes a revision of `owner`'s description by the caller.
fn insert_revision(
    w: &WriteCtx<'_>,
    owner: Owner,
    body: &str,
    base: Option<Id>,
    kind: &str,
) -> Result<Id, Failure> {
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller();
    if let Owner::Wiki(page) = owner {
        w.tx().execute(
            "INSERT INTO wiki_revisions (id, page_id, body, base_rev, kind, actor_kind,
                                         actor_name, session_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                b(id),
                b(page),
                body,
                base.map(b),
                kind,
                caller.actor.kind.as_str(),
                caller.actor.name,
                caller.session.map(b),
                w.now()
            ],
        )?;
        return Ok(id);
    }
    w.tx().execute(
        "INSERT INTO text_revisions (id, owner_kind, owner_id, body, base_rev, kind, actor_kind,
                                     actor_name, session_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            b(id),
            owner.kind(),
            b(owner.id()),
            body,
            base.map(b),
            kind,
            caller.actor.kind.as_str(),
            caller.actor.name,
            caller.session.map(b),
            w.now()
        ],
    )?;
    Ok(id)
}

/// Gives a new item's description its first revision, without history.
pub(crate) fn initial(w: &WriteCtx<'_>, owner: Owner, body: &str) -> Result<(), Failure> {
    if body.is_empty() {
        return Ok(());
    }
    let rev = insert_revision(w, owner, body, None, "normal")?;
    w.tx().execute(
        &format!(
            "UPDATE {} SET {} = ?2 WHERE id = ?1",
            owner.table(),
            owner.rev_column()
        ),
        params![b(owner.id()), b(rev)],
    )?;
    Ok(())
}

/// Writes a note on revision `rev`.
pub(crate) fn set_message(
    w: &WriteCtx<'_>,
    owner: Owner,
    rev: &str,
    message: &str,
) -> Result<(), Failure> {
    let Ok(rev) = rev.parse::<Id>() else {
        return Ok(());
    };
    let table = match owner {
        Owner::Wiki(_) => "wiki_revisions",
        _ => "text_revisions",
    };
    w.tx().execute(
        &format!("UPDATE {table} SET message = ?2 WHERE id = ?1"),
        params![b(rev), message],
    )?;
    Ok(())
}

/// The stored description, after giving a text saved before revisions
/// existed its revision (credited to whoever created the item).
fn with_revision(w: &WriteCtx<'_>, owner: Owner) -> Result<Stored, Failure> {
    let mut s = stored(w.tx(), owner)?;
    if s.rev.is_some() || s.text.is_empty() {
        return Ok(s);
    }
    let (kind, name, at): (String, String, i64) = match owner {
        // A page's text is always in a revision.
        Owner::Wiki(_) => return Ok(s),
        Owner::Task(id) => w.tx().query_row(
            "SELECT created_by_kind, created_by_name, edited_at FROM tasks WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?,
        Owner::Project(id) => w.tx().query_row(
            "SELECT 'user', '', updated_at FROM projects WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?,
    };
    let rev = Id::generate_at(u64::try_from(at).unwrap_or(0));
    w.tx().execute(
        "INSERT INTO text_revisions (id, owner_kind, owner_id, body, kind, actor_kind,
                                     actor_name, created_at)
         VALUES (?1, ?2, ?3, ?4, 'normal', ?5, ?6, ?7)",
        params![b(rev), owner.kind(), b(owner.id()), s.text, kind, name, at],
    )?;
    w.tx().execute(
        &format!(
            "UPDATE {} SET description_rev = ?2 WHERE id = ?1",
            owner.table()
        ),
        params![b(owner.id()), b(rev)],
    )?;
    s.rev = Some(rev);
    Ok(s)
}

/// A revision as history records it.
fn rev_value(rev: Option<Id>) -> Option<Value> {
    rev.map(|r| json!({ "revision": r.to_string() }))
}

/// The description of `entity`/`id` as history records it.
pub(crate) fn raw(tx: &Transaction<'_>, owner: Owner) -> Result<Option<Value>, StoreError> {
    let rev: Option<Option<Vec<u8>>> = tx
        .query_row(
            &format!(
                "SELECT {} FROM {} WHERE id = ?1",
                owner.rev_column(),
                owner.table()
            ),
            [b(owner.id())],
            |r| r.get(0),
        )
        .optional()?;
    Ok(rev_value(
        rev.flatten().and_then(|v| Id::from_slice(&v).ok()),
    ))
}

/// Makes `text` (with revision `rev`) the description and records it.
fn write_text(
    w: &mut WriteCtx<'_>,
    owner: Owner,
    project: Option<Id>,
    before: Option<Id>,
    text: &str,
    rev: Id,
) -> Result<(), Failure> {
    if let Owner::Wiki(page) = owner {
        w.tx().execute(
            "UPDATE wiki_pages SET current_rev = ?2 WHERE id = ?1",
            params![b(page), b(rev)],
        )?;
    } else {
        w.tx().execute(
            &format!(
                "UPDATE {} SET description = ?2, description_rev = ?3 WHERE id = ?1",
                owner.table()
            ),
            params![b(owner.id()), text, b(rev)],
        )?;
    }
    fields::touch(w, owner.entity(), owner.id(), owner.field())?;
    match owner {
        Owner::Task(id) => super::task::refresh_text(w, id)?,
        Owner::Wiki(page) => super::wiki::reindex(w, page)?,
        Owner::Project(_) => {}
    }
    w.record_field(
        owner.entity(),
        owner.id(),
        project,
        owner.field(),
        rev_value(before),
        rev_value(Some(rev)),
    );
    Ok(())
}

/// Marks `owner` as changed without changing its fields.
fn bump(w: &WriteCtx<'_>, owner: Owner) -> Result<(), Failure> {
    w.tx().execute(
        &format!("UPDATE {} SET updated_at = ?2 WHERE id = ?1", owner.table()),
        params![b(owner.id()), w.now()],
    )?;
    Ok(())
}

/// Saves `text` as `owner`'s description. With `base`, a revision other
/// than the stored one, the edit is merged with what was saved since, or
/// kept as a conflict copy. `kind` is the new revision's kind when it is
/// not a merge.
pub(crate) fn save(
    w: &mut WriteCtx<'_>,
    owner: Owner,
    project: Option<Id>,
    text: &str,
    base: Option<&str>,
    kind: &str,
) -> Result<DescriptionSave, Failure> {
    let base = base
        .map(|t| revision_of(w.tx(), owner, "base_revision", t))
        .transpose()?;
    let current = with_revision(w, owner)?;
    let unchanged = |rev: Option<Id>| DescriptionSave {
        outcome: SaveOutcome::Unchanged,
        revision: rev.map(|r| r.to_string()),
        conflict_copy: None,
    };
    match base {
        Some(base) if Some(base.id) != current.rev => {
            match gk_core::merge3(&base.body, &current.text, text) {
                Merge::Clean(merged) if merged == current.text => Ok(unchanged(current.rev)),
                Merge::Clean(merged) => {
                    gk_core::check_markdown(owner.field(), &merged)
                        .map_err(|e| super::task::invalid(owner.field(), e))?;
                    let rev = insert_revision(w, owner, &merged, current.rev, "merged")?;
                    write_text(w, owner, project, current.rev, &merged, rev)?;
                    Ok(DescriptionSave {
                        outcome: SaveOutcome::Merged,
                        revision: Some(rev.to_string()),
                        conflict_copy: None,
                    })
                }
                Merge::Conflict => {
                    let copy = insert_revision(w, owner, text, Some(base.id), "conflict")?;
                    bump(w, owner)?;
                    w.record_field(
                        owner.entity(),
                        owner.id(),
                        project,
                        "conflict_copy",
                        None,
                        rev_value(Some(copy)),
                    );
                    Ok(DescriptionSave {
                        outcome: SaveOutcome::Conflict,
                        revision: current.rev.map(|r| r.to_string()),
                        conflict_copy: Some(copy.to_string()),
                    })
                }
            }
        }
        _ if text == current.text => Ok(unchanged(current.rev)),
        _ => {
            let rev = insert_revision(w, owner, text, current.rev, kind)?;
            write_text(w, owner, project, current.rev, text, rev)?;
            Ok(DescriptionSave {
                outcome: SaveOutcome::Saved,
                revision: Some(rev.to_string()),
                conflict_copy: None,
            })
        }
    }
}

/// Applies targeted edits to the stored description and saves the result.
pub(crate) fn save_edits(
    w: &mut WriteCtx<'_>,
    owner: Owner,
    project: Option<Id>,
    edits: &[gk_api::ops::description::TextEdit],
) -> Result<DescriptionSave, Failure> {
    let current = with_revision(w, owner)?;
    let list: Vec<gk_core::Edit<'_>> = edits
        .iter()
        .map(|e| gk_core::Edit {
            old_text: &e.old_text,
            new_text: &e.new_text,
        })
        .collect();
    let text = gk_core::apply_edits(&current.text, &list).map_err(|e| {
        let field = format!("{}[{}].old_text", owner.edits_field(), e.index());
        let page = matches!(owner, Owner::Wiki(_));
        let (id, args) = match e {
            gk_core::EditError::Empty { .. } => ("edit-empty", vec![]),
            gk_core::EditError::NotFound { .. } if page => ("edit-not-in-page", vec![]),
            gk_core::EditError::NotFound { .. } => ("edit-not-in-description", vec![]),
            gk_core::EditError::Ambiguous { count, .. } => (
                if page {
                    "edit-ambiguous-page"
                } else {
                    "edit-ambiguous-description"
                },
                vec![("count", count.to_string())],
            ),
        };
        let hint = if page {
            "edit-page-hint"
        } else {
            "edit-task-hint"
        };
        Failure::Op(messages::hint(
            messages::invalid_at(&field, id, &args),
            hint,
            &[],
        ))
    })?;
    gk_core::check_markdown(owner.field(), &text)
        .map_err(|e| super::task::invalid(owner.field(), e))?;
    save(w, owner, project, &text, None, "normal")
}

/// Writes the recorded description `value` back, as history's revert does:
/// the recorded revision's text becomes a new revision.
pub(crate) fn restore(
    w: &mut WriteCtx<'_>,
    owner: Owner,
    project: Option<Id>,
    value: Option<&Value>,
) -> Result<(), Failure> {
    let text = match value {
        None | Some(Value::Null) => String::new(),
        // Changes recorded before revisions existed hold the text itself.
        Some(Value::String(text)) => text.clone(),
        Some(v) => {
            let id = v
                .get("revision")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<Id>().ok())
                .ok_or_else(|| Failure::Op(messages::storage("revision-unreadable", &[])))?;
            revision(w.tx(), id)?
                .ok_or_else(|| Failure::Op(messages::conflict("revision-gone", &[])))?
                .body
        }
    };
    save(w, owner, project, &text, None, "restored").map(drop)
}

// ---------------------------------------------------------------------
// Conflict copies

/// Resolves the conflict copy `copy` of `owner`'s text.
pub(crate) fn resolve(
    w: &mut WriteCtx<'_>,
    owner: Owner,
    project: Option<Id>,
    copy: &str,
    action: ConflictAction,
    text: Option<&str>,
    base: Option<&str>,
) -> Result<DescriptionSave, Failure> {
    let copy = revision_of(w.tx(), owner, "copy", copy)?;
    if copy.kind != "conflict" {
        return Err(messages::invalid("copy", "conflict-not-a-copy", &[]));
    }
    if copy.resolved_at.is_some() {
        return Err(messages::hint(
            messages::conflict("conflict-resolved-already", &[]),
            "conflict-read-task-hint",
            &[],
        )
        .into());
    }
    let current = with_revision(w, owner)?;
    if let Some(base) = base {
        let base = revision_of(w.tx(), owner, "base_revision", base)?;
        if Some(base.id) != current.rev {
            let id = match owner {
                Owner::Wiki(_) => "conflict-page-changed",
                _ => "conflict-description-changed",
            };
            return Err(messages::hint(
                messages::conflict(id, &[]).with_detail(
                    "revision",
                    current.rev.map(|r| r.to_string()).unwrap_or_default(),
                ),
                "conflict-changed-hint",
                &[],
            )
            .into());
        }
    }
    let saved = match action {
        ConflictAction::KeepCurrent => DescriptionSave {
            outcome: SaveOutcome::Unchanged,
            revision: current.rev.map(|r| r.to_string()),
            conflict_copy: None,
        },
        ConflictAction::UseCopy => save(w, owner, project, &copy.body, None, "resolved")?,
        ConflictAction::UseText => {
            let text =
                text.ok_or_else(|| messages::invalid("text", "conflict-use-text-needs-text", &[]))?;
            gk_core::check_markdown(owner.field(), text)
                .map_err(|e| super::task::invalid("text", e))?;
            save(w, owner, project, text, None, "resolved")?
        }
    };
    let table = match owner {
        Owner::Wiki(_) => "wiki_revisions",
        _ => "text_revisions",
    };
    w.tx().execute(
        &format!("UPDATE {table} SET resolved_at = ?2 WHERE id = ?1"),
        params![b(copy.id), w.now()],
    )?;
    bump(w, owner)?;
    w.record_field(
        owner.entity(),
        owner.id(),
        project,
        "conflict_resolved",
        None,
        rev_value(Some(copy.id)),
    );
    Ok(saved)
}

fn resolve_command(w: &mut WriteCtx<'_>, input: TaskResolveInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?
        .ok_or_else(|| Failure::Op(messages::not_found("task-project-gone", &[])))?;
    lookup::ensure_active(&project)?;
    super::claim::guard(w, task.id, &input.key, false)?;
    let saved = resolve(
        w,
        Owner::Task(task.id),
        Some(project.id),
        &input.copy,
        input.action,
        input.text.as_deref(),
        input.base_revision.as_deref(),
    )?;
    super::draft::clear(w, gk_api::ops::draft::DraftKind::Task, task.id)?;
    let mut change = super::task::changed(w, task.id, Vec::new())?;
    change.description_save = Some(saved);
    Ok(change)
}

// ---------------------------------------------------------------------
// Reading

/// The unresolved conflict copies of the task `t`.
pub(crate) const CONFLICTS_SQL: &str = "(SELECT count(*) FROM text_revisions v
     WHERE v.owner_kind = 'task' AND v.owner_id = t.id AND v.kind = 'conflict'
       AND v.resolved_at IS NULL)";

/// The unresolved conflict copies of the wiki page `p`.
pub(crate) const WIKI_CONFLICTS_SQL: &str = "(SELECT count(*) FROM wiki_revisions v
     WHERE v.page_id = p.id AND v.kind = 'conflict' AND v.resolved_at IS NULL)";

/// The revisions of `owner`'s text, newest first.
pub(crate) fn revisions(tx: &Transaction<'_>, owner: Owner) -> Result<Vec<RevisionInfo>, Failure> {
    let current = stored(tx, owner)?.rev;
    let rows = if let Owner::Wiki(page) = owner {
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT {WIKI_REVISION_COLUMNS} FROM wiki_revisions
              WHERE page_id = ?1 ORDER BY created_at DESC, rowid DESC"
        ))?;
        stmt.query_map([b(page)], revision_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = tx.prepare_cached(&format!(
            "SELECT {REVISION_COLUMNS} FROM text_revisions
              WHERE owner_kind = ?1 AND owner_id = ?2 ORDER BY created_at DESC, rowid DESC"
        ))?;
        stmt.query_map(params![owner.kind(), b(owner.id())], revision_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows.iter().map(|r| r.info(current)).collect())
}

fn history(q: &QueryCtx<'_>, input: DescriptionHistoryInput) -> Result<RevisionList, Failure> {
    let owner = match (&input.task, &input.project) {
        (Some(t), None) => Owner::Task(lookup::task(q.tx, "task", t)?.id),
        (None, Some(p)) => Owner::Project(lookup::project(q.tx, "project", p)?.id),
        _ => {
            return Err(
                messages::with("description-task-or-project", &[], OpError::invalid).into(),
            );
        }
    };
    let items = revisions(q.tx, owner)?;
    Ok(RevisionList {
        total: items.len() as u64,
        items,
    })
}

/// The key of a description's owner.
fn owner_key(tx: &Transaction<'_>, owner: Owner) -> Result<String, Failure> {
    Ok(match owner {
        Owner::Task(id) => {
            let (project, number): (Vec<u8>, i64) = tx.query_row(
                "SELECT project_id, number FROM tasks WHERE id = ?1",
                [b(id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let project = Id::from_slice(&project)
                .map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
            lookup::task_key(tx, project, number)?
        }
        Owner::Project(id) => {
            tx.query_row("SELECT key FROM projects WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })?
        }
        Owner::Wiki(id) => {
            tx.query_row("SELECT title FROM wiki_pages WHERE id = ?1", [b(id)], |r| {
                r.get(0)
            })?
        }
    })
}

fn revision_detail(
    q: &QueryCtx<'_>,
    input: DescriptionRevisionInput,
) -> Result<RevisionDetail, Failure> {
    let missing = || {
        Failure::Op(
            messages::not_found("revision-no-such", &[("revision", input.revision.clone())])
                .with_detail("field", "revision"),
        )
    };
    let id: Id = input.revision.trim().parse().map_err(|_| missing())?;
    let row = revision(q.tx, id)?.ok_or_else(missing)?;
    let owner = row.owner();
    if !owner_in_scope(q.tx, owner)? {
        return Err(missing());
    }
    let current = stored(q.tx, owner)?.rev;
    let diff = match &input.diff_from {
        Some(other) => {
            let from = revision_of(q.tx, owner, "diff_from", other)?;
            Some(gk_core::unified_diff(
                &from.body,
                &row.body,
                &format!("revision {}", from.id),
                &format!("revision {}", row.id),
            ))
        }
        None => None,
    };
    Ok(RevisionDetail {
        revision: row.info(current),
        owner: owner_key(q.tx, owner)?,
        body: row.body,
        diff,
    })
}

/// Whether the owner of a text is within the caller's project, for a
/// caller limited to one.
fn owner_in_scope(tx: &Transaction<'_>, owner: Owner) -> Result<bool, Failure> {
    if !crate::scope::active() {
        return Ok(true);
    }
    Ok(match owner {
        Owner::Project(id) => lookup::project_by_id(tx, id)?.is_some(),
        Owner::Task(id) => lookup::task_by_id(tx, id)?.is_some(),
        Owner::Wiki(id) => {
            let project: Option<Vec<u8>> = tx
                .query_row(
                    "SELECT project_id FROM wiki_pages WHERE id = ?1",
                    [lookup::b(id)],
                    |r| r.get(0),
                )
                .optional()?;
            project
                .and_then(|p| Id::from_slice(&p).ok())
                .is_some_and(crate::scope::sees)
        }
    })
}

/// The checklist of `text` as outputs carry it.
pub(crate) fn checklist_items(text: &str) -> Vec<ChecklistItem> {
    gk_markdown::checklist(text).iter().map(item).collect()
}

fn item(i: &gk_markdown::ChecklistItem) -> ChecklistItem {
    ChecklistItem {
        number: u32::try_from(i.number).unwrap_or(u32::MAX),
        checked: i.checked,
        text: i.text.clone(),
        depth: u32::try_from(i.depth).unwrap_or(u32::MAX),
    }
}

// ---------------------------------------------------------------------
// The reader's HTML

/// What one render request names.
struct Source {
    text: String,
    document_id: String,
    revision: Option<String>,
    version: Option<i64>,
}

fn source_of(tx: &Transaction<'_>, i: usize, s: &RenderSource) -> Result<Source, Failure> {
    let given = [
        s.source.is_some(),
        s.task.is_some(),
        s.comment.is_some(),
        s.project.is_some(),
        s.revision.is_some(),
    ]
    .iter()
    .filter(|g| **g)
    .count();
    let field = format!("documents[{i}]");
    // A page alone is its current text; with a text or a revision, it only
    // says whose text that is.
    let page_alone = s.page.is_some() && given == 0;
    let with_page = s.source.is_some() || s.revision.is_some();
    let valid = if page_alone {
        true
    } else {
        given == 1 && (s.page.is_none() || with_page)
    };
    if !valid {
        return Err(messages::invalid(&field, "render-one-source", &[]));
    }
    let mut out = if page_alone {
        let page = page_of(tx, &field, s.page.as_deref().unwrap_or_default())?;
        let (text, rev) = current_text(tx, Owner::Wiki(page.id))?;
        Source {
            text,
            document_id: format!("wiki-{}", page.id),
            revision: rev.map(|r| r.to_string()),
            version: None,
        }
    } else if let Some(text) = &s.source {
        Source {
            text: text.clone(),
            document_id: "preview".to_owned(),
            revision: None,
            version: None,
        }
    } else if let Some(key) = &s.task {
        let t = lookup::task(tx, &format!("{field}.task"), key)?;
        let st = stored(tx, Owner::Task(t.id))?;
        Source {
            text: st.text,
            document_id: t.id.to_string(),
            revision: st.rev.map(|r| r.to_string()),
            version: Some(t.version),
        }
    } else if let Some(comment) = &s.comment {
        let c = super::comment::find(tx, comment)?;
        let (body, version): (String, i64) = tx.query_row(
            "SELECT body, version FROM comments WHERE id = ?1",
            [b(c.id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Source {
            text: body,
            document_id: c.id.to_string(),
            revision: None,
            version: Some(version),
        }
    } else if let Some(project) = &s.project {
        let p = lookup::project(tx, &format!("{field}.project"), project)?;
        let st = stored(tx, Owner::Project(p.id))?;
        Source {
            text: st.text,
            document_id: p.id.to_string(),
            revision: st.rev.map(|r| r.to_string()),
            version: None,
        }
    } else {
        let text = s.revision.as_deref().unwrap_or_default();
        let missing = || {
            Failure::Op(
                messages::not_found("revision-no-such", &[("revision", text.to_owned())])
                    .with_detail("field", format!("{field}.revision")),
            )
        };
        let id: Id = text.trim().parse().map_err(|_| missing())?;
        let row = revision(tx, id)?.ok_or_else(missing)?;
        Source {
            text: row.body,
            document_id: row.id.to_string(),
            revision: Some(row.id.to_string()),
            version: None,
        }
    };
    if let Some(id) = &s.document_id {
        out.document_id.clone_from(id);
    }
    Ok(out)
}

/// The live page a render names by ID.
fn page_of(tx: &Transaction<'_>, field: &str, page: &str) -> Result<super::wiki::PageRow, Failure> {
    let missing = || {
        Failure::Op(
            messages::not_found("wiki-page-id-not-found", &[("page", page.to_owned())])
                .with_detail("field", format!("{field}.page")),
        )
    };
    let id: Id = page.trim().parse().map_err(|_| missing())?;
    super::wiki::by_id(tx, id)?.ok_or_else(missing)
}

fn render(q: &QueryCtx<'_>, input: RenderInput) -> Result<RenderOutput, Failure> {
    if input.documents.is_empty() || input.documents.len() > MAX_RENDER {
        return Err(messages::invalid(
            "documents",
            "render-count",
            &[("max", MAX_RENDER.to_string())],
        ));
    }
    // Task keys are looked up once per call.
    let known: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    let exists = |key: &str| -> bool {
        if let Some(found) = known.borrow().get(key) {
            return *found;
        }
        let found = lookup::task(q.tx, "key", key).is_ok();
        known.borrow_mut().insert(key.to_owned(), found);
        found
    };
    let mut items = Vec::with_capacity(input.documents.len());
    for (i, s) in input.documents.iter().enumerate() {
        let source = source_of(q.tx, i, s)?;
        let page = match &s.page {
            Some(page) => Some(page_of(q.tx, &format!("documents[{i}]"), page)?),
            None => None,
        };
        let macros = page.map(|p| super::macros::WikiMacros::new(q.tx, &q.env.zone, q.now, p));
        let mut options =
            gk_markdown::RenderOptions::new(&source.document_id).with_task_keys(&exists);
        if let Some(macros) = &macros {
            options = options.with_macros(macros);
        }
        items.push(Rendered {
            html: gk_markdown::render(&source.text, &options),
            checklist: checklist_items(&source.text),
            document_id: source.document_id,
            revision: source.revision,
            version: source.version,
        });
    }
    Ok(RenderOutput { items })
}

// ---------------------------------------------------------------------
// Checklists

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The line ending a text mostly uses.
fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

fn stale(
    field: &str,
    number: u32,
    expected: &str,
    items: &[gk_markdown::ChecklistItem],
) -> Failure {
    let actual = usize::try_from(number)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| items.get(n))
        .map_or_else(String::new, |i| i.text.clone());
    Failure::Op(
        messages::hint(
            messages::conflict(
                "checklist-changed",
                &[
                    ("number", number.to_string()),
                    ("actual", actual),
                    ("expected", expected.to_owned()),
                ],
            ),
            "checklist-changed-hint",
            &[],
        )
        .with_detail("field", field)
        .with_detail("number", number)
        .with_detail("items", items.len()),
    )
}

/// The item that `number` plus `expected` text names: the numbered one when
/// its text matches, else the only item with that text.
fn find_item<'a>(
    items: &'a [gk_markdown::ChecklistItem],
    field: &str,
    number: u32,
    expected: &str,
) -> Result<&'a gk_markdown::ChecklistItem, Failure> {
    let want = collapse(expected);
    let numbered = usize::try_from(number)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| items.get(n));
    if let Some(item) = numbered.filter(|i| i.text == want) {
        return Ok(item);
    }
    let mut same = items.iter().filter(|i| i.text == want);
    match (same.next(), same.next()) {
        (Some(item), None) => Ok(item),
        _ => Err(stale(field, number, &want, items)),
    }
}

/// `text` with the checklist changes of `input` made.
pub(crate) fn edit_checklist(text: &str, input: &ChecklistUpdateInput) -> Result<String, Failure> {
    let items = gk_markdown::checklist(text);
    let mut out = text.to_owned();
    // Ticks change one byte in place, so the items' places stay valid.
    for (i, t) in input.toggle.iter().enumerate() {
        let field = format!("toggle[{i}]");
        let item = find_item(&items, &field, t.number, &t.text)?;
        let checked = t.checked.unwrap_or(!item.checked);
        let current = gk_markdown::checklist(&out);
        let now = current
            .get(item.number - 1)
            .map_or(item.checked, |c| c.checked);
        if now == checked {
            continue;
        }
        match gk_markdown::set_checked(&out, item.number, &item.text, checked) {
            Ok(Some(next)) => out = next,
            Ok(None) => {}
            Err(_) => return Err(stale(&field, t.number, &t.text, &items)),
        }
    }
    let mut removed: Vec<Range<usize>> = Vec::new();
    for (i, r) in input.remove.iter().enumerate() {
        let item = find_item(&items, &format!("remove[{i}]"), r.number, &r.text)?;
        removed.push(item.lines.clone());
    }
    removed.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in removed {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    let eol = line_ending(text);
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for (i, a) in input.add.iter().enumerate() {
        let field = format!("add[{i}]");
        let line = gk_core::clean_line(&a.text);
        if line.is_empty() {
            return Err(messages::invalid(
                &format!("{field}.text"),
                "checklist-item-needs-text",
                &[],
            ));
        }
        let anchor = match a.after {
            Some(n) => Some(
                usize::try_from(n)
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| items.get(n))
                    .ok_or_else(|| {
                        messages::invalid(
                            &format!("{field}.after"),
                            "checklist-no-item",
                            &[
                                ("number", n.to_string()),
                                ("count", items.len().to_string()),
                            ],
                        )
                    })?,
            ),
            None => items.iter().rev().find(|i| i.depth == 0),
        };
        let symbol = if a.checked { 'x' } else { ' ' };
        let (at, entry) = match anchor {
            Some(item) => {
                // The new item copies the anchor's indentation and marker.
                let prefix = &out[item.lines.start..item.symbol_offset];
                let mut entry = String::new();
                if !out[..item.lines.end].ends_with('\n') {
                    entry.push_str(eol);
                }
                entry.push_str(prefix);
                entry.push(symbol);
                entry.push_str("] ");
                entry.push_str(&line);
                entry.push_str(eol);
                (item.lines.end, entry)
            }
            None => {
                let mut entry = String::new();
                let blank = format!("{eol}{eol}");
                if !(out.is_empty() || out.ends_with(&blank) || out.ends_with("\n\n")) {
                    if !out.ends_with('\n') {
                        entry.push_str(eol);
                    }
                    entry.push_str(eol);
                }
                entry.push_str("- [");
                entry.push(symbol);
                entry.push_str("] ");
                entry.push_str(&line);
                entry.push_str(eol);
                (out.len(), entry)
            }
        };
        let anchor_removed = anchor.is_some_and(|item| {
            merged
                .iter()
                .any(|r| r.start <= item.lines.start && item.lines.end <= r.end)
        });
        if anchor_removed || merged.iter().any(|r| r.start < at && at < r.end) {
            return Err(messages::invalid(
                &format!("{field}.after"),
                "checklist-inside-removed",
                &[],
            ));
        }
        inserts.push((at, entry));
    }
    if merged.is_empty() && inserts.is_empty() {
        return Ok(out);
    }
    inserts.sort_by_key(|(at, _)| *at);
    let mut result = String::with_capacity(out.len() + 64);
    let mut cursor = 0;
    let mut r = merged.iter().peekable();
    let mut a = inserts.iter().peekable();
    loop {
        let next_remove = r.peek().map(|x| x.start);
        let next_add = a.peek().map(|(at, _)| *at);
        match (next_remove, next_add) {
            (None, None) => break,
            (Some(rs), Some(at)) if at <= rs => {
                let (at, entry) = a.next().ok_or_else(|| Failure::Op(OpError::storage("")))?;
                result.push_str(&out[cursor..*at]);
                result.push_str(entry);
                cursor = *at;
            }
            (None, Some(_)) => {
                let (at, entry) = a.next().ok_or_else(|| Failure::Op(OpError::storage("")))?;
                result.push_str(&out[cursor..*at]);
                result.push_str(entry);
                cursor = *at;
            }
            (Some(_), _) => {
                let range = r.next().ok_or_else(|| Failure::Op(OpError::storage("")))?;
                result.push_str(&out[cursor..range.start]);
                cursor = range.end;
            }
        }
    }
    result.push_str(&out[cursor..]);
    Ok(result)
}

fn checklist_update(
    w: &mut WriteCtx<'_>,
    input: ChecklistUpdateInput,
) -> Result<ChecklistChange, Failure> {
    if input.toggle.is_empty() && input.add.is_empty() && input.remove.is_empty() {
        return Err(messages::with("checklist-nothing-to-change", &[], OpError::invalid).into());
    }
    if input.page.is_some() && (input.key.is_some() || input.comment.is_some()) {
        return Err(messages::with("checklist-one-owner", &[], OpError::invalid).into());
    }
    match (&input.key, &input.comment) {
        (Some(key), None) => {
            let task = lookup::task(w.tx(), "key", key)?;
            let project = lookup::project_by_id(w.tx(), task.project)?
                .ok_or_else(|| Failure::Op(messages::not_found("task-project-gone", &[])))?;
            lookup::ensure_active(&project)?;
            super::claim::guard(w, task.id, key, false)?;
            let owner = Owner::Task(task.id);
            let current = with_revision(w, owner)?;
            let text = edit_checklist(&current.text, &input)?;
            let rev = if text == current.text {
                current.rev
            } else {
                gk_core::check_markdown("description", &text)
                    .map_err(|e| super::task::invalid("description", e))?;
                let saved = save(w, owner, Some(project.id), &text, None, "normal")?;
                saved.revision.and_then(|r| r.parse().ok())
            };
            let (version, done, total): (i64, i64, i64) = w.tx().query_row(
                "SELECT version, checklist_done, checklist_total FROM tasks WHERE id = ?1",
                [b(task.id)],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            Ok(ChecklistChange {
                key: lookup::task_key(w.tx(), task.project, task.number)?,
                comment: None,
                page: None,
                items: checklist_items(&text),
                checklist: Checklist {
                    done: u32::try_from(done).unwrap_or(0),
                    total: u32::try_from(total).unwrap_or(0),
                },
                version,
                revision: rev.map(|r| r.to_string()),
                group: w.group_if_changed(),
            })
        }
        (None, Some(comment)) => {
            let c = super::comment::find(w.tx(), comment)?;
            if let Some(task) = lookup::task_by_id(w.tx(), c.task)?
                && let Some(p) = lookup::project_by_id(w.tx(), task.project)?
            {
                lookup::ensure_active(&p)?;
            }
            let body: String =
                w.tx()
                    .query_row("SELECT body FROM comments WHERE id = ?1", [b(c.id)], |r| {
                        r.get(0)
                    })?;
            let text = edit_checklist(&body, &input)?;
            if text != body {
                gk_core::check_comment(&text).map_err(|e| super::task::invalid("comment", e))?;
                super::comment::set_body(w, c.id, Some(c.project), &text)?;
            }
            let (version, key): (i64, String) = w.tx().query_row(
                "SELECT c.version, p.key || '-' || t.number FROM comments c
                   JOIN tasks t ON t.id = c.task_id JOIN projects p ON p.id = t.project_id
                  WHERE c.id = ?1",
                [b(c.id)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let items = checklist_items(&text);
            let done = items.iter().filter(|i| i.checked).count();
            Ok(ChecklistChange {
                key,
                comment: Some(c.id.to_string()),
                page: None,
                checklist: Checklist {
                    done: u32::try_from(done).unwrap_or(0),
                    total: u32::try_from(items.len()).unwrap_or(0),
                },
                items,
                version,
                revision: None,
                group: w.group_if_changed(),
            })
        }
        (None, None) if input.page.is_some() => {
            let page = super::wiki::find_page(
                w.tx(),
                input.project.as_deref(),
                "page",
                input.page.as_deref().unwrap_or_default(),
            )?;
            let project = lookup::project_by_id(w.tx(), page.project)?
                .ok_or_else(|| Failure::Op(messages::not_found("wiki-page-gone", &[])))?;
            lookup::ensure_active(&project)?;
            lookup::ensure_wiki_on(w.tx(), &project)?;
            let owner = Owner::Wiki(page.id);
            let current = stored(w.tx(), owner)?;
            let text = edit_checklist(&current.text, &input)?;
            let rev = if text == current.text {
                current.rev
            } else {
                gk_core::check_markdown("body", &text)
                    .map_err(|e| super::task::invalid("body", e))?;
                let saved = save(w, owner, Some(project.id), &text, None, "normal")?;
                saved.revision.and_then(|r| r.parse().ok())
            };
            let items = checklist_items(&text);
            let done = items.iter().filter(|i| i.checked).count();
            Ok(ChecklistChange {
                key: project.key.clone(),
                comment: None,
                page: Some(page.id.to_string()),
                checklist: Checklist {
                    done: u32::try_from(done).unwrap_or(0),
                    total: u32::try_from(items.len()).unwrap_or(0),
                },
                items,
                version: 0,
                revision: rev.map(|r| r.to_string()),
                group: w.group_if_changed(),
            })
        }
        _ => Err(messages::with("checklist-one-owner", &[], OpError::invalid).into()),
    }
}

#[cfg(test)]
mod tests {
    use gk_api::ops::description::{ChecklistAdd, ChecklistRemove, ChecklistToggle};

    use super::*;

    fn toggle(number: u32, text: &str, checked: Option<bool>) -> ChecklistUpdateInput {
        ChecklistUpdateInput {
            key: Some("TM-1".into()),
            toggle: vec![ChecklistToggle {
                number,
                text: text.into(),
                checked,
            }],
            ..ChecklistUpdateInput::default()
        }
    }

    fn code(f: Failure) -> gk_api::ErrorCode {
        OpError::from(f).code
    }

    #[test]
    fn a_tick_changes_one_byte() {
        let text = "Intro\r\n\r\n- [ ] one\r\n- [ ] two\r\n";
        let out = edit_checklist(text, &toggle(2, "two", None)).unwrap_or_default();
        assert_eq!(out, "Intro\r\n\r\n- [ ] one\r\n- [x] two\r\n");
        let diff = text
            .bytes()
            .zip(out.bytes())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!((diff, text.len()), (1, out.len()));
        // Setting the state it has changes nothing.
        let same = edit_checklist(text, &toggle(1, "one", Some(false))).unwrap_or_default();
        assert_eq!(same, text);
    }

    #[test]
    fn a_stale_number_finds_the_unique_text_or_is_refused() {
        let text = "- [ ] new first\n- [ ] one\n- [ ] two\n- [ ] two\n";
        let out = edit_checklist(text, &toggle(1, "one", Some(true))).unwrap_or_default();
        assert_eq!(out, "- [ ] new first\n- [x] one\n- [ ] two\n- [ ] two\n");
        let err = edit_checklist(text, &toggle(1, "two", Some(true))).map(drop);
        assert_eq!(err.map_err(code), Err(gk_api::ErrorCode::Conflict));
        let err = edit_checklist(text, &toggle(9, "gone", Some(true))).map(drop);
        assert_eq!(err.map_err(code), Err(gk_api::ErrorCode::Conflict));
    }

    #[test]
    fn items_are_added_and_removed_by_line() {
        let text = "Plan\n\n- [ ] one\n  - [ ] nested\n- [x] two\n\nAfter.\n";
        let input = ChecklistUpdateInput {
            key: Some("TM-1".into()),
            add: vec![
                ChecklistAdd {
                    text: "three".into(),
                    checked: false,
                    after: None,
                },
                ChecklistAdd {
                    text: "inner".into(),
                    checked: true,
                    after: Some(2),
                },
            ],
            remove: vec![ChecklistRemove {
                number: 1,
                text: "one".into(),
            }],
            ..ChecklistUpdateInput::default()
        };
        let err = edit_checklist(text, &input).map(drop).map_err(code);
        assert_eq!(err, Err(gk_api::ErrorCode::Invalid));
        let input = ChecklistUpdateInput {
            remove: vec![ChecklistRemove {
                number: 2,
                text: "nested".into(),
            }],
            add: vec![
                ChecklistAdd {
                    text: "three".into(),
                    checked: false,
                    after: None,
                },
                ChecklistAdd {
                    text: "inner\nline".into(),
                    checked: true,
                    after: Some(1),
                },
            ],
            ..input
        };
        let out = edit_checklist(text, &input).unwrap_or_default();
        assert_eq!(
            out,
            "Plan\n\n- [ ] one\n- [x] inner line\n- [x] two\n- [ ] three\n\nAfter.\n"
        );
    }

    #[test]
    fn a_new_checklist_starts_after_a_blank_line() {
        let input = ChecklistUpdateInput {
            add: vec![ChecklistAdd {
                text: "first".into(),
                checked: false,
                after: None,
            }],
            ..ChecklistUpdateInput::default()
        };
        assert_eq!(
            edit_checklist("Text", &input).unwrap_or_default(),
            "Text\n\n- [ ] first\n"
        );
        assert_eq!(
            edit_checklist("", &input).unwrap_or_default(),
            "- [ ] first\n"
        );
        assert_eq!(
            edit_checklist("A\r\n", &input).unwrap_or_default(),
            "A\r\n\r\n- [ ] first\r\n"
        );
    }
}
