// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What a command handler works with inside its write transaction, and how
//! its changes become history rows.

use std::collections::{BTreeMap, HashMap};

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
    pub(crate) layout: gk_store::WorkspaceLayout,
    pub(crate) zone: crate::time::Zone,
    /// Whether the command is held to the rules for agents although its
    /// caller is no agent: an automation rule that an agent made, or that
    /// an agent's change set off.
    pub(crate) as_agent: bool,
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
    /// Where each item's entry is in `changes`, by item ID (a few
    /// entities may share an ID). Commands that touch thousands of items,
    /// such as imports, would otherwise search the record on every change.
    positions: HashMap<Id, Vec<usize>>,
    /// While [`WriteCtx::attempt`] runs: how to take back each change to
    /// the record, newest last.
    journal: Option<Vec<Undo>>,
    /// Tasks the engine made on its own in this command (occurrences of
    /// recurring tasks), which status rules don't refuse.
    automatic: Vec<Id>,
    /// Runs of automation rules the command asks for once it is done
    /// (the scheduled and due date rules).
    queued: Vec<crate::ops::automation::Queued>,
}

/// One step back in the record of changes.
#[derive(Debug)]
enum Undo {
    /// An item was added at the end.
    Pushed,
    /// A field of the item at `index` was set; `old` is what it held.
    Field {
        index: usize,
        field: String,
        old: Option<FieldChange>,
    },
    /// The item at `index` was taken out.
    Removed { index: usize, change: PendingChange },
}

#[derive(Debug, Clone)]
struct PendingChange {
    entity: String,
    id: Id,
    project: Option<Id>,
    fields: BTreeMap<String, FieldChange>,
}

/// One change recorded so far: entity, item, project and the changed
/// fields.
pub(crate) type Pending = (String, Id, Option<Id>, BTreeMap<String, FieldChange>);

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
            positions: HashMap::new(),
            journal: None,
            automatic: Vec::new(),
            queued: Vec::new(),
        }
    }

    /// Asks for a run of an automation rule once the command is done.
    pub(crate) fn queue_automation(&mut self, run: crate::ops::automation::Queued) {
        self.queued.push(run);
    }

    /// The runs of automation rules asked for so far, taken out.
    pub(crate) fn take_queued(&mut self) -> Vec<crate::ops::automation::Queued> {
        std::mem::take(&mut self.queued)
    }

    /// Whether the rules for agents hold for this command: its caller is an
    /// agent, or it is an automation rule held to them.
    pub(crate) fn acts_as_agent(&self) -> bool {
        self.meta.ctx.is_agent() || self.meta.as_agent
    }

    /// The command's shared facts, for commands run alongside it.
    pub(crate) fn meta(&self) -> &CommandMeta {
        self.meta
    }

    /// Every change recorded so far, in order: entity, item, project and
    /// the changed fields.
    pub(crate) fn pending(&self) -> Vec<Pending> {
        self.changes
            .iter()
            .map(|c| (c.entity.clone(), c.id, c.project, c.fields.clone()))
            .collect()
    }

    /// Notes that the engine made task `task` on its own, not the caller.
    pub(crate) fn made_automatically(&mut self, task: Id) {
        if !self.automatic.contains(&task) {
            self.automatic.push(task);
        }
    }

    /// Whether the engine made task `task` on its own in this command.
    pub(crate) fn is_automatic(&self, task: Id) -> bool {
        self.automatic.contains(&task)
    }

    /// The items of `entity` this command has changed so far, in order.
    pub(crate) fn changed_of(&self, entity: &str) -> Vec<Id> {
        self.changes
            .iter()
            .filter(|c| c.entity == entity)
            .map(|c| c.id)
            .collect()
    }

    /// The tasks this command closed so far (their `closed_at` went from
    /// none to a moment), in order.
    pub(crate) fn newly_closed(&self) -> Vec<Id> {
        self.changes
            .iter()
            .filter(|c| c.entity == crate::ops::fields::TASK)
            .filter(|c| {
                c.fields.get("closed_at").is_some_and(|f| {
                    f.before.as_ref().is_none_or(Value::is_null)
                        && f.after.as_ref().is_some_and(|v| !v.is_null())
                })
            })
            .map(|c| c.id)
            .collect()
    }

    /// The items of `entity` this command has changed so far, in order,
    /// each with its project and its changed fields.
    pub(crate) fn changes_of(
        &self,
        entity: &str,
    ) -> Vec<(Id, Option<Id>, BTreeMap<String, FieldChange>)> {
        self.changes
            .iter()
            .filter(|c| c.entity == entity)
            .map(|c| (c.id, c.project, c.fields.clone()))
            .collect()
    }

    /// How many items of `entity` this command has created so far.
    pub(crate) fn created_so_far(&self, entity: &str) -> usize {
        self.changes
            .iter()
            .filter(|c| c.entity == entity && c.fields.contains_key("created"))
            .count()
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

    /// The workspace's folder, for its stored files.
    #[must_use]
    pub fn layout(&self) -> &gk_store::WorkspaceLayout {
        &self.meta.layout
    }

    /// The time zone of local days and times of day.
    pub(crate) fn zone(&self) -> &crate::time::Zone {
        &self.meta.zone
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
        let index = match self.position(entity, id) {
            Some(i) => i,
            None => {
                if before == after {
                    return;
                }
                self.positions
                    .entry(id)
                    .or_default()
                    .push(self.changes.len());
                self.changes.push(PendingChange {
                    entity: entity.to_owned(),
                    id,
                    project,
                    fields: BTreeMap::new(),
                });
                if let Some(j) = &mut self.journal {
                    j.push(Undo::Pushed);
                }
                self.changes.len() - 1
            }
        };
        let change = &mut self.changes[index];
        if let Some(j) = &mut self.journal {
            j.push(Undo::Field {
                index,
                field: field.to_owned(),
                old: change.fields.get(field).cloned(),
            });
        }
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
                    truncated: false,
                },
            );
        }
        if change.fields.is_empty() {
            let removed = self.changes.remove(index);
            self.reindex();
            if let Some(j) = &mut self.journal {
                j.push(Undo::Removed {
                    index,
                    change: removed,
                });
            }
        }
    }

    /// Where the entry of `entity`/`id` is in the record of changes.
    fn position(&self, entity: &str, id: Id) -> Option<usize> {
        self.positions
            .get(&id)?
            .iter()
            .copied()
            .find(|&i| self.changes[i].entity == entity)
    }

    /// Forgets the position of the last entry, which was just taken out.
    fn forget_last(&mut self, id: Id) {
        let last = self.changes.len();
        if let Some(at) = self.positions.get_mut(&id) {
            at.retain(|&i| i != last);
            if at.is_empty() {
                self.positions.remove(&id);
            }
        }
    }

    /// Rebuilds the positions after entries moved (rare: a field changed
    /// back to where it started, or a step was taken back).
    fn reindex(&mut self) {
        self.positions.clear();
        for (i, c) in self.changes.iter().enumerate() {
            self.positions.entry(c.id).or_default().push(i);
        }
    }

    /// Runs `f`, then takes back everything it wrote: the transaction
    /// returns to a savepoint and the changes `f` recorded are forgotten.
    /// For previews that run the real code.
    pub(crate) fn preview<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let recorded = self.changes.clone();
        self.tx.execute_batch("SAVEPOINT preview")?;
        let result = f(self);
        let undone = self
            .tx
            .execute_batch("ROLLBACK TO preview; RELEASE preview");
        self.changes = recorded;
        self.reindex();
        undone?;
        result
    }

    /// Runs `f` and keeps what it wrote when it succeeds; when it fails,
    /// the transaction returns to where it was before `f` and the changes
    /// `f` recorded are forgotten, so the command can go on without them.
    pub(crate) fn attempt<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let outer = self.journal.replace(Vec::new());
        self.tx.execute_batch("SAVEPOINT attempt")?;
        let result = f(self);
        let journal = std::mem::replace(&mut self.journal, outer).unwrap_or_default();
        match &result {
            Ok(_) => {
                self.tx.execute_batch("RELEASE attempt")?;
                if let Some(o) = &mut self.journal {
                    o.extend(journal);
                }
            }
            Err(_) => {
                self.tx
                    .execute_batch("ROLLBACK TO attempt; RELEASE attempt")?;
                for step in journal.into_iter().rev() {
                    match step {
                        Undo::Pushed => {
                            if let Some(c) = self.changes.pop() {
                                self.forget_last(c.id);
                            }
                        }
                        Undo::Field { index, field, old } => {
                            if let Some(c) = self.changes.get_mut(index) {
                                match old {
                                    Some(v) => {
                                        c.fields.insert(field, v);
                                    }
                                    None => {
                                        c.fields.remove(&field);
                                    }
                                }
                            }
                        }
                        Undo::Removed { index, change } => {
                            self.changes.insert(index, change);
                            self.reindex();
                        }
                    }
                }
            }
        }
        result
    }

    /// Writes one history row per changed item, all in this command's
    /// group, with the inverse change; counts the write for an agent
    /// session.
    pub(crate) fn finish(self) -> Result<Finished, Failure> {
        let meta = self.meta;
        let ctx = &meta.ctx;
        if crate::scope::active() {
            for change in &self.changes {
                if let Some(project) = change.project
                    && !crate::scope::allows(self.tx, project)?
                {
                    return Err(crate::scope::outside());
                }
            }
        }
        if ctx.is_agent() {
            let trashed = self
                .changes
                .iter()
                .filter(|c| {
                    c.fields
                        .get(crate::ops::trash::TRASHED)
                        .is_some_and(|f| f.after == Some(Value::Bool(true)))
                })
                .count() as u64;
            crate::ops::trash::check_agent_caps(self.tx, ctx, meta.at, trashed)?;
        }
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
                    changes_json(fields, ctx.person_confirmed),
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
    /// Adds what a command run alongside this one (an automation rule's
    /// run, in the same history group) changed.
    pub(crate) fn absorb(&mut self, other: Self) {
        if other.seq.is_some() {
            self.seq = other.seq;
        }
        self.changes.extend(other.changes);
    }

    /// How many distinct items the change touched.
    pub(crate) fn items(&self) -> usize {
        self.changes
            .iter()
            .map(|c| (c.entity.as_str(), c.id.as_str()))
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

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

/// The `changes` column of one history row: the fields, and for a call the
/// person allowed when an agent asked, `confirmed`.
fn changes_json(fields: Map<String, Value>, confirmed: bool) -> String {
    if confirmed {
        json!({ "fields": fields, "confirmed": true }).to_string()
    } else {
        json!({ "fields": fields }).to_string()
    }
}
