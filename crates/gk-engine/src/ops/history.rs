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

use super::{agent, desc, settings};
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

/// Reads and writes the fields of one kind of item for revert.
struct Applier {
    current: fn(&Transaction<'_>, Id, &str) -> Result<Option<Value>, StoreError>,
    set: fn(&mut WriteCtx<'_>, Id, &str, Option<Value>) -> Result<(), Failure>,
}

fn applier(entity: &str) -> Option<Applier> {
    match entity {
        settings::ENTITY => Some(Applier {
            current: |tx, _, field| settings::raw(tx, field),
            set: |w, _, field, value| settings::write_raw(w, field, value),
        }),
        _ => None,
    }
}

struct GroupRow {
    entity: String,
    id: Id,
    fields: Map<String, Value>,
    inverse: Map<String, Value>,
    reverted: bool,
}

fn group_rows(tx: &Transaction<'_>, group: Id) -> Result<Vec<GroupRow>, StoreError> {
    let mut stmt = tx.prepare_cached(
        "SELECT entity_kind, entity_id, changes, inverse, reverted_by IS NOT NULL
           FROM activity WHERE group_id = ?1 ORDER BY seq DESC",
    )?;
    let rows = stmt.query_map([group.as_bytes().as_slice()], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, bool>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (entity, id, changes, inverse, reverted) = row?;
        let object = |text: &str, key: &str| {
            serde_json::from_str::<Value>(text)
                .ok()
                .and_then(|v| v.get(key).and_then(Value::as_object).cloned())
                .unwrap_or_default()
        };
        out.push(GroupRow {
            entity,
            id: Id::from_slice(&id).map_err(|e| StoreError::Corrupt(e.to_string()))?,
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
        let Some(applier) = applier(&row.entity) else {
            return Ok(Some(format!(
                "changes to {} can't be reverted yet",
                row.entity
            )));
        };
        for (field, change) in &row.fields {
            let after = change.get("after").cloned();
            if (applier.current)(tx, row.id, field)? != after {
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
                let Some(applier) = applier(&row.entity) else {
                    continue;
                };
                for (field, before) in &row.inverse {
                    let value = (!before.is_null()).then(|| before.clone());
                    (applier.set)(w, row.id, field, value)?;
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
