// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Links between tasks, and subtasks made through links.
//!
//! Rules: no link from a task to itself, none between a task and its
//! ancestor or descendant, no loop of `blocks`, one link per pair and kind
//! ("blocked by" is "blocks" read backwards; "relates" has no direction).
//! Tasks in other projects may be linked. A task is blocked while a task
//! that blocks it is open.
//!
//! History records a link as the `link` field of the `relation` item,
//! holding both ends, the kind and a `blocks` link's lag, so a removed link
//! comes back with the same ID when its removal is reverted, and a changed
//! lag goes back to what it was.
//!
//! The lag of a `blocks` link is 0 to 365 calendar days between the end of
//! the blocking task and the start of the blocked one; 0 is stored as none.

use gk_api::ops::task::{
    Link, LinkKind as ApiLinkKind, RelationAdd, RelationInput, RelationKind, RelationRemove,
    TaskChange, TaskPart,
};
use gk_core::{Id, LinkKind, Relation};
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::desc;
use super::lookup::{self, TaskRow, b, id_at};
use crate::failure::Failure;
use crate::messages;
use crate::registry::Registry;
use crate::write::WriteCtx;

/// History's name for links.
pub(crate) const ENTITY: &str = "relation";

pub(super) fn register(r: &mut Registry) {
    r.command::<RelationAdd, _>(desc::<RelationAdd>(), add);
    r.command::<RelationRemove, _>(desc::<RelationRemove>(), remove);
}

fn core_kind(kind: RelationKind) -> Option<LinkKind> {
    Some(match kind {
        RelationKind::Blocks => LinkKind::Blocks,
        RelationKind::BlockedBy => LinkKind::BlockedBy,
        RelationKind::Relates => LinkKind::Relates,
        RelationKind::Duplicates => LinkKind::Duplicates,
        RelationKind::DuplicatedBy => LinkKind::DuplicatedBy,
        RelationKind::Clones => LinkKind::Clones,
        RelationKind::ClonedBy => LinkKind::ClonedBy,
        RelationKind::SubtaskOf | RelationKind::ParentOf => return None,
    })
}

fn api_kind(kind: LinkKind) -> ApiLinkKind {
    match kind {
        LinkKind::Blocks => ApiLinkKind::Blocks,
        LinkKind::BlockedBy => ApiLinkKind::BlockedBy,
        LinkKind::Relates => ApiLinkKind::Relates,
        LinkKind::Duplicates => ApiLinkKind::Duplicates,
        LinkKind::DuplicatedBy => ApiLinkKind::DuplicatedBy,
        LinkKind::Clones => ApiLinkKind::Clones,
        LinkKind::ClonedBy => ApiLinkKind::ClonedBy,
    }
}

/// Every link of task `id`, in the order they were made. Links to tasks
/// in the trash are left out, and so are, for a caller limited to a
/// project, links to tasks outside it.
pub(crate) fn links(tx: &Transaction<'_>, id: Id) -> Result<Vec<Link>, Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT r.id, r.from_task, r.to_task, r.kind, f.project_id, t.project_id, r.lag_days
           FROM task_relations r
           JOIN tasks f ON f.id = r.from_task JOIN tasks t ON t.id = r.to_task
          WHERE (r.from_task = ?1 OR r.to_task = ?1)
            AND f.deleted_at IS NULL AND t.deleted_at IS NULL
          ORDER BY r.created_at, r.id",
    )?;
    let rows = stmt
        .query_map([b(id)], |r| {
            Ok((
                id_at(r, 0)?,
                id_at(r, 1)?,
                id_at(r, 2)?,
                r.get::<_, String>(3)?,
                id_at(r, 4)?,
                id_at(r, 5)?,
                r.get::<_, Option<i64>>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (link, from, to, kind, from_project, to_project, lag) in rows {
        let Ok(relation) = kind.parse::<Relation>() else {
            continue;
        };
        let other_project = if from == id { to_project } else { from_project };
        if !crate::scope::allows(tx, other_project)? {
            continue;
        }
        if let Some((kind, other)) = LinkKind::seen_from(relation, from, to, id) {
            out.push(Link {
                kind: api_kind(kind),
                task: super::task::task_ref(tx, other)?,
                id: link.to_string(),
                lag_days: lag.filter(|d| *d != 0),
            });
        }
    }
    Ok(out)
}

/// The open tasks that block task `id`: the keys of those the caller may
/// see, and how many others there are (outside the project a caller is
/// limited to).
pub(crate) fn open_blockers(tx: &Transaction<'_>, id: Id) -> Result<(Vec<String>, usize), Failure> {
    let mut stmt = tx.prepare_cached(
        "SELECT p.key || '-' || o.number, p.id FROM task_relations r
           JOIN tasks o ON o.id = r.from_task JOIN projects p ON p.id = o.project_id
          WHERE r.to_task = ?1 AND r.kind = 'blocks' AND o.deleted_at IS NULL
            AND o.closed_at IS NULL
          ORDER BY p.key, o.number",
    )?;
    let rows = stmt
        .query_map([b(id)], |r| Ok((r.get::<_, String>(0)?, id_at(r, 1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut keys = Vec::with_capacity(rows.len());
    let mut hidden = 0;
    for (key, project) in rows {
        if crate::scope::allows(tx, project)? {
            keys.push(key);
        } else {
            hidden += 1;
        }
    }
    Ok((keys, hidden))
}

/// Whether `ancestor` is above `id` in its parent chain.
fn is_ancestor(tx: &Transaction<'_>, ancestor: Id, id: Id) -> Result<bool, Failure> {
    let mut next = Some(id);
    let mut steps = 0;
    while let Some(current) = next {
        next = tx
            .query_row(
                "SELECT parent_id FROM tasks WHERE id = ?1",
                [b(current)],
                |r| lookup::opt_id_at(r, 0),
            )
            .optional()?
            .flatten();
        if next == Some(ancestor) {
            return Ok(true);
        }
        steps += 1;
        if steps > 64 {
            break;
        }
    }
    Ok(false)
}

/// Whether `from` reaches `to` by following `blocks` links.
fn blocks_path(tx: &Transaction<'_>, from: Id, to: Id) -> Result<bool, Failure> {
    let mut seen = vec![from];
    let mut i = 0;
    let mut stmt = tx.prepare_cached(
        "SELECT to_task FROM task_relations WHERE from_task = ?1 AND kind = 'blocks'",
    )?;
    while i < seen.len() {
        let next = stmt
            .query_map([b(seen[i])], |r| id_at(r, 0))?
            .collect::<Result<Vec<_>, _>>()?;
        for n in next {
            if n == to {
                return Ok(true);
            }
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
        i += 1;
        if seen.len() > 100_000 {
            break;
        }
    }
    Ok(false)
}

/// The longest lag of a `blocks` link, in days.
pub(crate) const MAX_LAG_DAYS: i64 = 365;

/// A link's recorded value; the lag only when there is one.
fn value(from: Id, to: Id, relation: Relation, lag: Option<i64>) -> Value {
    let mut v =
        json!({ "from": from.to_string(), "to": to.to_string(), "kind": relation.as_str() });
    if let Some(lag) = lag.filter(|d| *d != 0) {
        v["lag_days"] = lag.into();
    }
    v
}

/// A link's row: both ends, the kind and the lag.
type LinkRow = (Vec<u8>, Vec<u8>, String, Option<i64>);

/// The link `id` as history records it; `None` when it doesn't exist.
pub(crate) fn raw(tx: &Transaction<'_>, id: Id) -> Result<Option<Value>, StoreError> {
    let row: Option<LinkRow> = tx
        .query_row(
            "SELECT from_task, to_task, kind, lag_days FROM task_relations WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(row.and_then(|(from, to, kind, lag)| {
        let from = Id::from_slice(&from).ok()?;
        let to = Id::from_slice(&to).ok()?;
        Some(value(from, to, kind.parse().ok()?, lag))
    }))
}

/// Marks a task as having changed (its activity, not its own fields).
pub(crate) fn bump(w: &WriteCtx<'_>, task: Id) -> Result<(), Failure> {
    w.tx().execute(
        "UPDATE tasks SET updated_at = ?2 WHERE id = ?1",
        params![b(task), w.now()],
    )?;
    Ok(())
}

fn insert_row(
    w: &WriteCtx<'_>,
    id: Id,
    from: Id,
    to: Id,
    relation: Relation,
    lag: Option<i64>,
) -> Result<(), Failure> {
    let caller = w.caller();
    w.tx().execute(
        "INSERT INTO task_relations (id, from_task, to_task, kind, lag_days, created_at,
                                     created_by_kind, created_by_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            b(id),
            b(from),
            b(to),
            relation.as_str(),
            lag.filter(|d| *d != 0 && relation == Relation::Blocks),
            w.now(),
            caller.actor.kind.as_str(),
            caller.actor.name
        ],
    )?;
    bump(w, from)?;
    bump(w, to)
}

/// Writes the recorded link value back (revert's path): `None` removes the
/// link `id`, a value puts it back.
pub(crate) fn apply(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    value: Option<Value>,
) -> Result<(), Failure> {
    let before = raw(w.tx(), id)?;
    match &value {
        None => {
            if let Some(v) = &before {
                for end in ["from", "to"] {
                    if let Some(task) = v[end].as_str().and_then(|t| t.parse().ok()) {
                        bump(w, task)?;
                    }
                }
            }
            w.tx()
                .execute("DELETE FROM task_relations WHERE id = ?1", [b(id)])?;
        }
        Some(v) => {
            let end = |k: &str| v[k].as_str().and_then(|t| t.parse::<Id>().ok());
            let (Some(from), Some(to), Some(kind)) = (
                end("from"),
                end("to"),
                v["kind"].as_str().and_then(|k| k.parse::<Relation>().ok()),
            ) else {
                return Err(messages::storage("storage-link-broken", &[]).into());
            };
            let lag = v["lag_days"]
                .as_i64()
                .filter(|d| (0..=MAX_LAG_DAYS).contains(d));
            if before.is_some() {
                // The same link with another lag.
                w.tx().execute(
                    "UPDATE task_relations SET lag_days = ?2 WHERE id = ?1",
                    params![b(id), lag.filter(|d| *d != 0 && kind == Relation::Blocks)],
                )?;
                bump(w, from)?;
                bump(w, to)?;
            } else {
                for task in [from, to] {
                    if lookup::task_by_id(w.tx(), task)?.is_none() {
                        return Err(messages::conflict("link-task-gone", &[]).into());
                    }
                }
                insert_row(w, id, from, to, kind, lag)?;
            }
        }
    }
    w.record_field(ENTITY, id, project, "link", before, value);
    Ok(())
}

fn active_task(w: &WriteCtx<'_>, field: &str, text: &str) -> Result<TaskRow, Failure> {
    let t = lookup::task(w.tx(), field, text)?;
    let p = lookup::project_by_id(w.tx(), t.project)?.ok_or_else(|| {
        Failure::Op(messages::not_found(
            "task-not-found",
            &[("task", text.to_owned())],
        ))
    })?;
    lookup::ensure_active(&p)?;
    Ok(t)
}

/// Makes `child` a subtask of `parent` (or takes it out, with `None`).
fn set_parent(
    w: &mut WriteCtx<'_>,
    child: &TaskRow,
    child_key: &str,
    parent: Option<&TaskRow>,
) -> Result<(), Failure> {
    if let Some(parent) = parent {
        super::task::check_parent(
            w.tx(),
            Some(child.id),
            child.project,
            parent,
            Some(child_key),
        )?;
    }
    super::task::set_parent(w, child.id, child.project, parent.map(|p| p.id))
}

pub(crate) fn add(w: &mut WriteCtx<'_>, input: RelationInput) -> Result<TaskChange, Failure> {
    let task = active_task(w, "key", &input.key)?;
    let other = active_task(w, "other", &input.other)?;
    if task.id == other.id {
        return Err(messages::invalid("other", "link-to-itself", &[]));
    }
    let Some(kind) = core_kind(input.kind) else {
        if input.kind == RelationKind::SubtaskOf {
            set_parent(w, &task, &input.key, Some(&other))?;
        } else {
            set_parent(w, &other, &input.other, Some(&task))?;
        }
        return super::task::changed_with(w, task.id, Vec::new(), &[TaskPart::Subtasks]);
    };
    if is_ancestor(w.tx(), task.id, other.id)? || is_ancestor(w.tx(), other.id, task.id)? {
        return Err(messages::invalid(
            "other",
            "link-to-subtask",
            &[("task", input.key.clone()), ("other", input.other.clone())],
        ));
    }
    let (relation, from, to) = kind.stored(task.id, other.id);
    if let Some(lag) = input.lag_days {
        if relation != Relation::Blocks {
            return Err(messages::invalid("lag_days", "link-lag-blocks-only", &[]));
        }
        if !(0..=MAX_LAG_DAYS).contains(&lag) {
            return Err(messages::invalid(
                "lag_days",
                "link-lag-range",
                &[("max", MAX_LAG_DAYS.to_string())],
            ));
        }
    }
    if relation == Relation::Blocks && blocks_path(w.tx(), to, from)? {
        return Err(messages::invalid("kind", "link-loop", &[]));
    }
    let exists: Option<Id> = w
        .tx()
        .query_row(
            "SELECT id FROM task_relations WHERE from_task = ?1 AND to_task = ?2 AND kind = ?3",
            params![b(from), b(to), relation.as_str()],
            |r| id_at(r, 0),
        )
        .optional()?;
    match exists {
        None => {
            let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
            insert_row(w, id, from, to, relation, input.lag_days)?;
            w.record_field(
                ENTITY,
                id,
                Some(task.project),
                "link",
                None,
                Some(value(from, to, relation, input.lag_days)),
            );
        }
        Some(id) => {
            if let Some(lag) = input.lag_days {
                let after = Some(value(from, to, relation, Some(lag)));
                if raw(w.tx(), id)? != after {
                    apply(w, id, Some(task.project), after)?;
                }
            }
        }
    }
    super::task::changed_with(w, task.id, Vec::new(), &[TaskPart::Links])
}

fn remove(w: &mut WriteCtx<'_>, input: RelationInput) -> Result<TaskChange, Failure> {
    let task = active_task(w, "key", &input.key)?;
    let other = active_task(w, "other", &input.other)?;
    let Some(kind) = core_kind(input.kind) else {
        let (child, child_key, parent) = if input.kind == RelationKind::SubtaskOf {
            (&task, &input.key, &other)
        } else {
            (&other, &input.other, &task)
        };
        if child.parent != Some(parent.id) {
            return Err(
                messages::not_found("link-not-a-subtask", &[("task", child_key.clone())]).into(),
            );
        }
        set_parent(w, child, child_key, None)?;
        return super::task::changed_with(w, task.id, Vec::new(), &[TaskPart::Subtasks]);
    };
    let (relation, from, to) = kind.stored(task.id, other.id);
    let found: Option<Id> = w
        .tx()
        .query_row(
            "SELECT id FROM task_relations WHERE from_task = ?1 AND to_task = ?2 AND kind = ?3",
            params![b(from), b(to), relation.as_str()],
            |r| id_at(r, 0),
        )
        .optional()?;
    let Some(id) = found else {
        let error = messages::not_found(
            "link-not-found",
            &[("task", input.key.clone()), ("other", input.other.clone())],
        );
        return Err(messages::hint(error, "link-not-found-hint", &[]).into());
    };
    apply(w, id, Some(task.project), None)?;
    super::task::changed_with(w, task.id, Vec::new(), &[TaskPart::Links])
}
