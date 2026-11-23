// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What a command handler works with inside its write transaction, and how
//! its changes become history rows.

use std::collections::BTreeMap;

use gk_api::ops::history::FieldChange;
use gk_api::{Changed, EntityChange, Origin};
use gk_core::Id;
use gk_store::rusqlite::{Transaction, params};
use serde_json::{Map, Value, json};

use crate::context::Context;
use crate::failure::Failure;

/// The facts every history row of one command shares.
#[derive(Debug, Clone)]
pub(crate) struct CommandMeta {
    pub(crate) op: &'static str,
    pub(crate) group: Id,
    pub(crate) at: i64,
    pub(crate) ctx: Context,
    pub(crate) workspace_id: Id,
}

/// A command's view of the workspace: the open write transaction and a
/// record of what it changed. Every change a handler makes is recorded
/// with [`WriteCtx::record_field`]; the engine turns the record into history
/// rows, with the inverse change, before it commits.
#[derive(Debug)]
pub struct WriteCtx<'a> {
    tx: &'a Transaction<'a>,
    meta: &'a CommandMeta,
    changes: Vec<PendingChange>,
}

#[derive(Debug)]
struct PendingChange {
    entity: String,
    id: Id,
    project: Option<Id>,
    fields: BTreeMap<String, FieldChange>,
}

/// What a committed command changed.
#[derive(Debug, Clone)]
pub(crate) struct Finished {
    pub(crate) seq: Option<i64>,
    pub(crate) changes: Vec<EntityChange>,
}

impl<'a> WriteCtx<'a> {
    pub(crate) fn new(tx: &'a Transaction<'a>, meta: &'a CommandMeta) -> Self {
        Self {
            tx,
            meta,
            changes: Vec::new(),
        }
    }

    /// The write transaction.
    #[must_use]
    pub fn tx(&self) -> &Transaction<'a> {
        self.tx
    }

    /// The history group of this command.
    #[must_use]
    pub fn group(&self) -> Id {
        self.meta.group
    }

    /// The command's time, in milliseconds since 1970 UTC.
    #[must_use]
    pub fn now(&self) -> i64 {
        self.meta.at
    }

    /// The caller.
    #[must_use]
    pub fn caller(&self) -> &Context {
        &self.meta.ctx
    }

    /// The workspace's ID.
    #[must_use]
    pub fn workspace_id(&self) -> Id {
        self.meta.workspace_id
    }

    /// The operation running.
    #[must_use]
    pub fn op(&self) -> &'static str {
        self.meta.op
    }

    /// Whether anything has changed so far.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// The group as output carries it, when something changed.
    #[must_use]
    pub fn group_if_changed(&self) -> Option<String> {
        self.has_changes().then(|| self.group().to_string())
    }

    /// Records that `field` of the item `entity`/`id` went from `before` to
    /// `after` (`None`: no value). A field changed twice keeps its first
    /// `before`; a field changed back to where it started is dropped.
    pub fn record_field(
        &mut self,
        entity: &str,
        id: Id,
        project: Option<Id>,
        field: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) {
        let index = match self
            .changes
            .iter()
            .position(|c| c.entity == entity && c.id == id)
        {
            Some(i) => i,
            None => {
                if before == after {
                    return;
                }
                self.changes.push(PendingChange {
                    entity: entity.to_owned(),
                    id,
                    project,
                    fields: BTreeMap::new(),
                });
                self.changes.len() - 1
            }
        };
        let change = &mut self.changes[index];
        let first_before = change
            .fields
            .get(field)
            .map_or(before, |f| f.before.clone());
        if first_before == after {
            change.fields.remove(field);
        } else {
            change.fields.insert(
                field.to_owned(),
                FieldChange {
                    before: first_before,
                    after,
                },
            );
        }
        if change.fields.is_empty() {
            self.changes.remove(index);
        }
    }

    /// Writes one history row per changed item, all in this command's
    /// group, with the inverse change; counts the write for an agent
    /// session.
    pub(crate) fn finish(self) -> Result<Finished, Failure> {
        let meta = self.meta;
        let ctx = &meta.ctx;
        let mut seq = None;
        let mut out = Vec::with_capacity(self.changes.len());
        {
            let mut insert = self.tx.prepare_cached(
                "INSERT INTO activity (group_id, at, op, actor_kind, actor_name, via, session_id,
                                       project_id, entity_kind, entity_id, changes, inverse)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            for change in &self.changes {
                let fields: Map<String, Value> = change
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
                    .collect();
                let inverse: Map<String, Value> = change
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.before.clone().unwrap_or(Value::Null)))
                    .collect();
                insert.execute(params![
                    meta.group.as_bytes().as_slice(),
                    meta.at,
                    meta.op,
                    ctx.actor.kind.as_str(),
                    ctx.actor.name,
                    ctx.via.as_str(),
                    ctx.session.map(|s| s.as_bytes().to_vec()),
                    change.project.map(|p| p.as_bytes().to_vec()),
                    change.entity,
                    change.id.as_bytes().as_slice(),
                    json!({ "fields": fields }).to_string(),
                    json!({ "set": inverse }).to_string(),
                ])?;
                seq = Some(self.tx.last_insert_rowid());
                out.push(EntityChange {
                    entity: change.entity.clone(),
                    id: change.id.to_string(),
                    project: change.project.map(|p| p.to_string()),
                    fields: change.fields.keys().cloned().collect(),
                });
            }
        }
        if let (Some(session), false) = (ctx.session, out.is_empty()) {
            self.tx.execute(
                "UPDATE agent_sessions SET writes = writes + 1, last_seen_at = ?2 WHERE id = ?1",
                params![session.as_bytes().as_slice(), meta.at],
            )?;
        }
        Ok(Finished { seq, changes: out })
    }
}

impl Finished {
    /// A command that changed nothing.
    pub(crate) fn nothing() -> Self {
        Self {
            seq: None,
            changes: Vec::new(),
        }
    }

    /// The change event for this command, if it changed anything.
    pub(crate) fn event(&self, meta: &CommandMeta) -> Option<Changed> {
        let seq = self.seq?;
        Some(Changed {
            seq,
            group: meta.group.to_string(),
            origin: Origin {
                via: meta.ctx.via,
                actor: meta.ctx.actor.clone(),
                session: meta.ctx.session.map(|s| s.to_string()),
            },
            changes: self.changes.clone(),
        })
    }
}
