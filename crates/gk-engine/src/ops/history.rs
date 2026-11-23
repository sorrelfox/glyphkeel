// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The activity log, revert, Undo and Redo.
//!
//! Every history row stores the fields a command changed, each with its
//! value before and after, and the inverse change (`{"set": {field:
//! before}}`). Reverting a group applies the inverse as a new command, after
//! checking that every field still has the value the group left there; a
//! group that something changed since is skipped, never overwritten.
//!
//! Each kind of item has an applier that reads and writes its fields. Kinds
//! without one can't be reverted yet and are reported as skipped.

use std::collections::BTreeMap;

use gk_api::OpError;
use gk_api::ops::agent::AgentSessionsInput;
use gk_api::ops::common::{Actor, ActorKind, DEFAULT_LIMIT, MAX_LIMIT, Via};
use gk_api::ops::history::{
    ActivityEntry, ActivityPage, ActivityQuery, FieldChange, HistoryFeed, HistoryRedo,
    HistoryRevert, HistoryUndo, RevertResult, SkippedGroup, UndoResult,
};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::types::Value as SqlValue;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params, params_from_iter};
use serde_json::{Map, Value};

use super::description::{self, Owner};
use super::{
    agent, board, claim, comment, desc, fields, file, lookup, project, relation, reminder,
    settings, task, time,
};
use crate::failure::Failure;
use crate::registry::{QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

pub(super) fn register(r: &mut Registry) {
    r.query::<HistoryFeed, _>(desc::<HistoryFeed>(), feed);
    r.engine_op::<HistoryRevert, _>(desc::<HistoryRevert>(), |engine, ctx, input| {
        let mut groups = Vec::new();
        for g in &input.groups {
            groups.push(parse_id("groups", g)?);
        }
        if let Some(session) = &input.session {
            let session = session_id(ctx.session, session)?;
            let of_session = engine
                .store()
                .read(|tx| session_groups(tx, session).map_err(Failure::from))
                .map_err(OpError::from)?;
            groups.extend(of_session);
        }
        if groups.is_empty() {
            return Err(
                OpError::invalid("Name the history groups or the session to revert.")
                    .with_hint("Pass `groups` (from get_activity) or `session`."),
            );
        }
        let newest_first = engine
            .store()
            .read(|tx| newest_first(tx, groups).map_err(Failure::from))
            .map_err(OpError::from)?;
        engine.revert(ctx, "history.revert", newest_first, input.dry_run)
    });
    r.engine_op::<HistoryUndo, _>(desc::<HistoryUndo>(), |engine, ctx, _| {
        let (target, group, op) = engine.undo_or_redo(ctx, false)?;
        Ok(UndoResult {
            target: target.to_string(),
            group: group.to_string(),
            op,
        })
    });
    r.engine_op::<HistoryRedo, _>(desc::<HistoryRedo>(), |engine, ctx, _| {
        let (target, group, op) = engine.undo_or_redo(ctx, true)?;
        Ok(UndoResult {
            target: target.to_string(),
            group: group.to_string(),
            op,
        })
    });
}

fn parse_id(field: &str, text: &str) -> Result<Id, OpError> {
    text.parse().map_err(|_| {
        OpError::invalid_at(field, format!("\"{text}\" is not an ID."))
            .with_hint("IDs look like 0193b2a4-5c6d-7e8f-9a0b-1c2d3e4f5a6b.")
    })
}

fn session_id(own: Option<Id>, text: &str) -> Result<Id, OpError> {
    if text == "current" {
        own.ok_or_else(|| OpError::invalid_at("session", "You have no agent session."))
    } else {
        parse_id("session", text)
    }
}

/// The activity rows of one task: its own (fields and description
/// revisions), its comments', its time entries', its files' (also those on
/// its comments) and its links', live or removed. Takes the task's ID seven
/// times as bytes, then four times as text.
const OF_TASK: &str = "(
    (entity_kind = 'task' AND entity_id = ?)
    OR (entity_kind = 'comment'
        AND entity_id IN (SELECT id FROM comments WHERE task_id = ?))
    OR (entity_kind = 'time_entry'
        AND entity_id IN (SELECT id FROM time_entries WHERE task_id = ?))
    OR (entity_kind = 'attachment'
        AND entity_id IN (SELECT id FROM attachments
                           WHERE (owner_kind = 'task' AND owner_id = ?)
                              OR (owner_kind = 'comment'
                                  AND owner_id IN (SELECT id FROM comments WHERE task_id = ?))))
    OR (entity_kind = 'relation'
        AND (entity_id IN (SELECT id FROM task_relations WHERE from_task = ? OR to_task = ?)
             OR ? = json_extract(changes, '$.fields.link.before.from')
             OR ? = json_extract(changes, '$.fields.link.before.to')
             OR ? = json_extract(changes, '$.fields.link.after.from')
             OR ? = json_extract(changes, '$.fields.link.after.to'))))";

fn feed(q: &QueryCtx<'_>, input: ActivityQuery) -> Result<ActivityPage, Failure> {
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(
            OpError::invalid_at("limit", format!("limit must be from 1 to {MAX_LIMIT}.")).into(),
        );
    }
    let mut filters: Vec<&str> = Vec::new();
    let mut args: Vec<SqlValue> = Vec::new();
    if let Some(session) = &input.session {
        filters.push("session_id = ?");
        args.push(SqlValue::Blob(
            session_id(q.caller.session, session)?.as_bytes().to_vec(),
        ));
    }
    if let Some(actor) = &input.actor {
        filters.push("lower(actor_name) = lower(?)");
        args.push(SqlValue::Text(actor.clone()));
    }
    if let Some(group) = &input.group {
        filters.push("group_id = ?");
        args.push(SqlValue::Blob(
            parse_id("group", group)?.as_bytes().to_vec(),
        ));
    }
    if let Some(task) = &input.task {
        let id = match task.trim().parse::<Id>() {
            Ok(id) => id,
            Err(_) => lookup::task(q.tx, "task", task)?.id,
        };
        filters.push(OF_TASK);
        let blob = SqlValue::Blob(id.as_bytes().to_vec());
        let text = SqlValue::Text(id.to_string());
        args.extend([blob.clone(), blob.clone(), blob.clone(), blob.clone()]);
        args.extend([blob.clone(), blob.clone(), blob]);
        args.extend(std::iter::repeat_n(text, 4));
    }
    let base = if filters.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", filters.join(" AND "))
    };
    let total: i64 = q.tx.query_row(
        &format!("SELECT count(*) FROM activity {base}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;

    let mut page_filters = filters.clone();
    let mut page_args = args.clone();
    if let Some(cursor) = &input.cursor {
        let seq: i64 = cursor
            .parse()
            .map_err(|_| OpError::invalid_at("cursor", "The cursor is not one this list gave."))?;
        page_filters.push("seq < ?");
        page_args.push(SqlValue::Integer(seq));
    }
    page_args.push(SqlValue::Integer(i64::from(limit) + 1));
    let page_where = if page_filters.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", page_filters.join(" AND "))
    };
    let mut stmt = q.tx.prepare(&format!(
        "SELECT seq, group_id, at, op, actor_kind, actor_name, via, session_id, project_id,
                entity_kind, entity_id, changes, reverted_by
           FROM activity {page_where} ORDER BY seq DESC LIMIT ?"
    ))?;
    let mut items = stmt
        .query_map(params_from_iter(page_args.iter()), entry)?
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        items.last().map(|e| e.seq.to_string())
    } else {
        None
    };
    let sessions = if input.sessions {
        Some(agent::list(q.tx, &AgentSessionsInput::default())?.items)
    } else {
        None
    };
    Ok(ActivityPage {
        items,
        total: u64::try_from(total).unwrap_or(0),
        next_cursor,
        sessions,
    })
}

fn id_at(r: &Row<'_>, i: usize) -> gk_store::rusqlite::Result<Option<String>> {
    let bytes: Option<Vec<u8>> = r.get(i)?;
    Ok(bytes
        .and_then(|b| Id::from_slice(&b).ok())
        .map(|id| id.to_string()))
}

fn entry(r: &Row<'_>) -> gk_store::rusqlite::Result<ActivityEntry> {
    let changes: String = r.get(11)?;
    let fields = serde_json::from_str::<Value>(&changes)
        .ok()
        .and_then(|v| v.get("fields").cloned())
        .and_then(|f| serde_json::from_value::<BTreeMap<String, FieldChange>>(f).ok())
        .unwrap_or_default();
    let kind: String = r.get(4)?;
    let via: String = r.get(6)?;
    Ok(ActivityEntry {
        seq: r.get(0)?,
        group: id_at(r, 1)?.unwrap_or_default(),
        at: rfc3339(r.get(2)?),
        op: r.get(3)?,
        actor: Actor {
            kind: if kind == "agent" {
                ActorKind::Agent
            } else {
                ActorKind::User
            },
            name: r.get(5)?,
        },
        via: match via.as_str() {
            "cli" => Via::Cli,
            "mcp" => Via::Mcp,
            _ => Via::App,
        },
        session: id_at(r, 7)?,
        project: id_at(r, 8)?,
        entity: r.get(9)?,
        id: id_at(r, 10)?.unwrap_or_default(),
        fields,
        reverted_by: id_at(r, 12)?,
    })
}

/// The operation that made `group`.
pub(crate) fn group_op(tx: &Transaction<'_>, group: Id) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT op FROM activity WHERE group_id = ?1 LIMIT 1",
            [group.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .optional()?)
}

/// The groups of `session` that are not reverted yet.
fn session_groups(tx: &Transaction<'_>, session: Id) -> Result<Vec<Id>, StoreError> {
    let mut stmt = tx.prepare(
        "SELECT DISTINCT group_id FROM activity WHERE session_id = ?1 AND reverted_by IS NULL",
    )?;
    let rows = stmt.query_map([session.as_bytes().as_slice()], |r| r.get::<_, Vec<u8>>(0))?;
    let mut out = Vec::new();
    for bytes in rows {
        if let Ok(id) = Id::from_slice(&bytes?) {
            out.push(id);
        }
    }
    Ok(out)
}

/// `groups` without repeats, the newest (highest sequence number) first;
/// unknown groups last, in the order given.
fn newest_first(tx: &Transaction<'_>, groups: Vec<Id>) -> Result<Vec<Id>, StoreError> {
    let mut keyed = Vec::new();
    for g in groups {
        if keyed.iter().any(|(_, k)| *k == g) {
            continue;
        }
        let seq: Option<i64> = tx.query_row(
            "SELECT max(seq) FROM activity WHERE group_id = ?1",
            [g.as_bytes().as_slice()],
            |r| r.get(0),
        )?;
        keyed.push((seq.unwrap_or(i64::MIN), g));
    }
    keyed.sort_by_key(|k| std::cmp::Reverse(k.0));
    Ok(keyed.into_iter().map(|(_, g)| g).collect())
}

/// The fields of a row that revert writes back: all of them, or for a row
/// that created an item, only its existence.
fn revertible_fields(row: &GroupRow) -> Vec<&String> {
    match row.fields.keys().find(|f| *f == "created") {
        Some(created) => vec![created],
        None => row.fields.keys().collect(),
    }
}

/// The current value of a recorded field, as history records it; `None`
/// (outer) when this kind of change can't be reverted.
fn current(
    tx: &Transaction<'_>,
    entity: &str,
    id: Id,
    field: &str,
) -> Result<Option<Option<Value>>, StoreError> {
    Ok(Some(match (entity, field) {
        (settings::ENTITY, _) => settings::raw(tx, field)?,
        (board::ENTITY, "settings") => board::raw(tx, id)?,
        (_, "deleted") => return Ok(None),
        (relation::ENTITY, "link") => relation::raw(tx, id)?,
        (comment::ENTITY, "created" | "trashed") => comment::raw_life(tx, id, field)?,
        (comment::ENTITY, "body") => comment::raw_body(tx, id)?,
        (file::ENTITY, "created" | "trashed") => file::raw_life(tx, id, field)?,
        (time::ENTRY, "created" | "trashed") => time::raw_life(tx, id, field)?,
        (time::ENTRY, "date" | "minutes" | "note") => time::raw_field(tx, id, field)?,
        (time::TIMER, "timer") => time::raw_timer(tx)?,
        (fields::TASK, "reminder") => reminder::raw(tx, id)?,
        (fields::TASK, "claim") => claim::raw(tx, id)?,
        (fields::PROJECT | fields::TASK, "description") => match Owner::of_entity(entity, id) {
            Some(owner) => description::raw(tx, owner)?,
            None => return Ok(None),
        },
        (fields::PROJECT | fields::TASK, "created") => {
            let table = if entity == fields::TASK {
                "tasks"
            } else {
                "projects"
            };
            let live: Option<bool> = tx
                .query_row(
                    &format!("SELECT deleted_at IS NULL FROM {table} WHERE id = ?1"),
                    [id.as_bytes().as_slice()],
                    |r| r.get(0),
                )
                .optional()?;
            live.filter(|l| *l).map(Value::Bool)
        }
        (fields::STATUS | fields::TYPE | fields::LABEL, "created") => {
            let table = match entity {
                fields::STATUS => "statuses",
                fields::TYPE => "task_types",
                _ => "labels",
            };
            let exists: Option<i64> = tx
                .query_row(
                    &format!("SELECT 1 FROM {table} WHERE id = ?1"),
                    [id.as_bytes().as_slice()],
                    |r| r.get(0),
                )
                .optional()?;
            exists.map(|_| Value::Bool(true))
        }
        (fields::TASK, "labels") => {
            let ids = task::label_ids(tx, id).map_err(|e| match e {
                Failure::Store(s) => s,
                Failure::Op(o) => StoreError::Corrupt(o.message),
            })?;
            task::labels_value(&ids)
        }
        (entity, field) if fields::is_column(entity, field) => fields::read(tx, entity, id, field)?,
        _ => return Ok(None),
    }))
}

/// Writes a recorded value back and records that change.
fn apply(
    w: &mut WriteCtx<'_>,
    entity: &str,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    match (entity, field) {
        (settings::ENTITY, _) => settings::write_raw(w, field, value),
        (board::ENTITY, "settings") => {
            let before = board::raw(w.tx(), id)?;
            board::write_raw(w, id, value.as_ref())?;
            w.record_field(entity, id, project, field, before, value);
            Ok(())
        }
        (relation::ENTITY, "link") => relation::apply(w, id, project, value),
        (comment::ENTITY, "created" | "trashed") => comment::set_live(w, id, project, field, value),
        (comment::ENTITY, "body") => {
            let body = value.as_ref().and_then(Value::as_str).unwrap_or_default();
            comment::set_body(w, id, project, body).map(drop)
        }
        (file::ENTITY, "created" | "trashed") => file::set_live(w, id, project, field, value),
        (time::ENTRY, "created" | "trashed") => time::set_live(w, id, project, field, value),
        (time::ENTRY, "date" | "minutes" | "note") => {
            time::apply_field(w, id, project, field, value)
        }
        (time::TIMER, "timer") => time::apply_timer(w, value.as_ref()),
        (fields::TASK, "reminder") => reminder::write_value(w, id, project, value),
        (fields::TASK, "claim") => claim::apply(w, id, project, value),
        (fields::PROJECT | fields::TASK, "description") => match Owner::of_entity(entity, id) {
            Some(owner) => description::restore(w, owner, project, value.as_ref()),
            None => Ok(()),
        },
        (fields::PROJECT | fields::TASK, "created") => {
            let is_task = entity == fields::TASK;
            let (table, child) = if is_task {
                (
                    "tasks",
                    "SELECT count(*) FROM tasks WHERE parent_id = ?1 AND deleted_at IS NULL",
                )
            } else {
                (
                    "projects",
                    "SELECT (SELECT count(*) FROM tasks WHERE project_id = ?1 AND deleted_at IS NULL)
                          + (SELECT count(*) FROM projects WHERE parent_id = ?1 AND deleted_at IS NULL)",
                )
            };
            if value.is_none() {
                let inside: i64 = w
                    .tx()
                    .query_row(child, [id.as_bytes().as_slice()], |r| r.get(0))?;
                if inside > 0 {
                    return Err(OpError::conflict(format!(
                        "The {} now holds {}, so its creation can't be reverted.",
                        if is_task { "task" } else { "project" },
                        if is_task {
                            "subtasks"
                        } else {
                            "tasks or sub-projects"
                        }
                    ))
                    .into());
                }
                w.tx().execute(
                    &format!(
                        "UPDATE {table} SET deleted_at = ?2, deleted_group = ?3 WHERE id = ?1"
                    ),
                    params![
                        id.as_bytes().as_slice(),
                        w.now(),
                        w.group().as_bytes().as_slice()
                    ],
                )?;
            } else {
                w.tx().execute(
                    &format!(
                        "UPDATE {table} SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1"
                    ),
                    [id.as_bytes().as_slice()],
                )?;
            }
            let before = (value.is_none()).then_some(Value::Bool(true));
            w.record_field(entity, id, project, field, before, value);
            Ok(())
        }
        (fields::STATUS | fields::TYPE | fields::LABEL, "created") => {
            if value.is_some() {
                return Err(OpError::conflict(format!(
                    "A deleted {entity} can't be created again by a revert."
                ))
                .into());
            }
            let (table, used) = match entity {
                fields::STATUS => (
                    "statuses",
                    "SELECT count(*) FROM tasks WHERE status_id = ?1",
                ),
                fields::TYPE => (
                    "task_types",
                    "SELECT count(*) FROM tasks WHERE type_id = ?1",
                ),
                _ => ("labels", "SELECT 0"),
            };
            let n: i64 = if table == "labels" {
                0
            } else {
                w.tx()
                    .query_row(used, [id.as_bytes().as_slice()], |r| r.get(0))?
            };
            if n > 0 {
                return Err(OpError::conflict(format!(
                    "The {entity} is in use by {n} tasks, so its creation can't be reverted."
                ))
                .into());
            }
            w.tx().execute(
                &format!("DELETE FROM {table} WHERE id = ?1"),
                [id.as_bytes().as_slice()],
            )?;
            w.record_field(entity, id, project, field, Some(Value::Bool(true)), None);
            Ok(())
        }
        (fields::TASK, "labels") => {
            let ids: Vec<Id> = value
                .as_ref()
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str()?.parse().ok()).collect())
                .unwrap_or_default();
            let project = project.ok_or_else(|| Failure::Op(OpError::storage("no project")))?;
            task::set_labels(w, id, project, &ids).map(drop)
        }
        (fields::PROJECT, "key") => {
            let old = fields::read(w.tx(), entity, id, field)?;
            if let (Some(Value::String(old)), Some(Value::String(new))) = (&old, &value) {
                project::rekey(w, id, old, new)?;
            }
            fields::set(w, entity, id, project, field, value).map(drop)
        }
        _ => {
            fields::set(w, entity, id, project, field, value)?;
            if entity == fields::TASK && field == "title" {
                task::refresh_text(w, id)?;
            }
            Ok(())
        }
    }
}

struct GroupRow {
    entity: String,
    id: Id,
    project: Option<Id>,
    fields: Map<String, Value>,
    inverse: Map<String, Value>,
    reverted: bool,
}

fn group_rows(tx: &Transaction<'_>, group: Id) -> Result<Vec<GroupRow>, StoreError> {
    let mut stmt = tx.prepare_cached(
        "SELECT entity_kind, entity_id, changes, inverse, reverted_by IS NOT NULL, project_id
           FROM activity WHERE group_id = ?1 ORDER BY seq DESC",
    )?;
    let rows = stmt.query_map([group.as_bytes().as_slice()], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, Option<Vec<u8>>>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (entity, id, changes, inverse, reverted, project) = row?;
        let object = |text: &str, key: &str| {
            serde_json::from_str::<Value>(text)
                .ok()
                .and_then(|v| v.get(key).and_then(Value::as_object).cloned())
                .unwrap_or_default()
        };
        out.push(GroupRow {
            entity,
            id: Id::from_slice(&id).map_err(|e| StoreError::Corrupt(e.to_string()))?,
            project: project.and_then(|p| Id::from_slice(&p).ok()),
            fields: object(&changes, "fields"),
            inverse: inverse.map(|i| object(&i, "set")).unwrap_or_default(),
            reverted,
        });
    }
    Ok(out)
}

/// Why `group` can't be reverted now, or `None` if it can.
fn blocker(tx: &Transaction<'_>, rows: &[GroupRow]) -> Result<Option<String>, StoreError> {
    if rows.is_empty() {
        return Ok(Some("there is no such history group".into()));
    }
    if rows.iter().any(|r| r.reverted) {
        return Ok(Some("it was already reverted".into()));
    }
    for row in rows {
        if row.fields.is_empty() {
            return Ok(Some(format!(
                "the {} it changed was deleted for good",
                row.entity
            )));
        }
        for field in revertible_fields(row) {
            let after = row.fields.get(field).and_then(|c| c.get("after")).cloned();
            let Some(now) = current(tx, &row.entity, row.id, field)? else {
                return Ok(Some(if field == "deleted" {
                    format!("a deleted {} can't be brought back by a revert", row.entity)
                } else {
                    format!("changes to {} {field} can't be reverted yet", row.entity)
                }));
            };
            if now != after {
                return Ok(Some(format!(
                    "{} {field} was changed again since",
                    row.entity
                )));
            }
        }
    }
    Ok(None)
}

/// Reverts `groups` in the given order inside `w`'s transaction.
pub(crate) fn revert_groups(
    w: &mut WriteCtx<'_>,
    groups: &[Id],
    dry_run: bool,
) -> Result<RevertResult, Failure> {
    let mut reverted = Vec::new();
    let mut skipped = Vec::new();
    for &group in groups {
        let rows = group_rows(w.tx(), group)?;
        if let Some(reason) = blocker(w.tx(), &rows)? {
            skipped.push(SkippedGroup {
                group: group.to_string(),
                reason,
            });
            continue;
        }
        if !dry_run {
            for row in &rows {
                let before = |field: &str| row.inverse.get(field).cloned().filter(|v| !v.is_null());
                let home = row.entity == fields::TASK
                    && row.fields.contains_key("project")
                    && row.fields.contains_key("number");
                if home {
                    task::set_home(w, row.id, row.project, before("project"), before("number"))?;
                }
                for field in revertible_fields(row) {
                    if home && matches!(field.as_str(), "project" | "number") {
                        continue;
                    }
                    apply(w, &row.entity, row.id, row.project, field, before(field))?;
                }
            }
            w.tx().execute(
                "UPDATE activity SET reverted_by = ?2 WHERE group_id = ?1",
                params![group.as_bytes().as_slice(), w.group().as_bytes().as_slice()],
            )?;
        }
        reverted.push(group.to_string());
    }
    Ok(RevertResult {
        reverted,
        skipped,
        group: if dry_run { None } else { w.group_if_changed() },
    })
}
