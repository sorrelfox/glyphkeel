// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `proposal.list`, `proposal.get`, `proposal.approve`, `proposal.reject`,
//! and the proposals themselves.
//!
//! When the workspace says an agent's change needs the person's approval,
//! the pipeline doesn't run it: it runs the operation's handler once inside
//! a savepoint to see what it would change, takes that back, and keeps the
//! operation, its input and that preview as a proposal. Approving runs the
//! operation again through the pipeline, as the agent, and marks the
//! proposal approved in the same transaction, so it runs once. Each step is
//! a history row of kind `proposal`, which revert leaves alone: reverting an
//! approved change takes back the change only.

use std::sync::Arc;

use gk_api::ops::common::{Actor, ActorKind, DEFAULT_LIMIT, MAX_LIMIT};
use gk_api::ops::proposal::{
    PROPOSALS_PENDING_MAX, Proposal, ProposalApprove, ProposalApproveInput, ProposalApproveResult,
    ProposalConflict, ProposalGet, ProposalList, ProposalListInput, ProposalListOp,
    ProposalOutcome, ProposalReject, ProposalRejectInput, ProposalRejectResult, ProposalStatus,
    ProposedChange, REJECT_REASON_MAX,
};
use gk_api::{ErrorCode, OpDescriptor, OpError, Profile};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::{Value, json};

use super::{desc, fields, history, settings};
use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{CommandFn, QueryCtx, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// The kind of item proposals are recorded under in history.
pub(crate) const ENTITY: &str = "proposal";

/// How long an approval may run before it counts as interrupted.
const INTERRUPTED_MS: i64 = 10 * 60 * 1000;

/// A day, in milliseconds.
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub(super) fn register(r: &mut Registry) {
    r.query::<ProposalListOp, _>(desc::<ProposalListOp>(), list);
    r.query::<ProposalGet, _>(desc::<ProposalGet>(), |q, input| {
        let id = parse_id("id", &input.id)?;
        let days = settings::load(q.tx)?.agent_approvals.expire_days;
        let row = load(q.tx, id)?
            .filter(|row| visible_to(q.caller, row))
            .ok_or_else(|| unknown(&input.id))?;
        view(q.tx, row, q.now, days)
    });
    r.command::<ProposalReject, _>(desc::<ProposalReject>(), reject);
    r.engine_op::<ProposalApprove, _>(desc::<ProposalApprove>(), approve);
}

/// One stored proposal.
#[derive(Debug, Clone)]
pub(crate) struct Stored {
    id: Id,
    created_at: i64,
    op: String,
    input: Value,
    session: Option<Id>,
    agent: String,
    profile: Profile,
    scope: Option<String>,
    project: Option<Id>,
    task: Option<Id>,
    summary: String,
    previewed: bool,
    changes: Vec<ProposedChange>,
    status: String,
    decided_at: Option<i64>,
    decided_by: Option<Actor>,
    reason: Option<String>,
    group: Option<Id>,
    claimed_at: Option<i64>,
    last_error: Option<String>,
}

impl Stored {
    /// Where it stands at `now`, when proposals wait `days`.
    fn status(&self, now: i64, days: u32) -> ProposalStatus {
        match self.status.as_str() {
            "approving" => ProposalStatus::Approving,
            "approved" => ProposalStatus::Approved,
            "rejected" => ProposalStatus::Rejected,
            _ if now >= self.expires_at(days) => ProposalStatus::Expired,
            _ => ProposalStatus::Pending,
        }
    }

    /// Whether it is being approved by a program that stopped meanwhile.
    fn interrupted(&self, now: i64) -> bool {
        self.status == "approving" && self.claimed_at.is_some_and(|at| now - at > INTERRUPTED_MS)
    }

    fn expires_at(&self, days: u32) -> i64 {
        self.created_at + i64::from(days) * DAY_MS
    }
}

const COLUMNS: &str = "id, created_at, op, input, session_id, agent_name, profile, project_scope,
                       project_id, task_id, summary, previewed, preview, status, decided_at,
                       decided_by_kind, decided_by_name, reason, group_id, claimed_at,
                       last_error";

fn stored(r: &Row<'_>) -> gk_store::rusqlite::Result<Stored> {
    let id_at = |i: usize| -> gk_store::rusqlite::Result<Option<Id>> {
        Ok(r.get::<_, Option<Vec<u8>>>(i)?
            .and_then(|b| Id::from_slice(&b).ok()))
    };
    let input: String = r.get(3)?;
    let preview: String = r.get(12)?;
    let profile: String = r.get(6)?;
    let decided_kind: Option<String> = r.get(15)?;
    let decided_name: Option<String> = r.get(16)?;
    Ok(Stored {
        id: id_at(0)?.unwrap_or_else(|| Id::from_bytes([0; 16])),
        created_at: r.get(1)?,
        op: r.get(2)?,
        input: serde_json::from_str(&input).unwrap_or(Value::Null),
        session: id_at(4)?,
        agent: r.get(5)?,
        profile: profile.parse().unwrap_or(Profile::ReadOnly),
        scope: r.get(7)?,
        project: id_at(8)?,
        task: id_at(9)?,
        summary: r.get(10)?,
        previewed: r.get(11)?,
        changes: serde_json::from_str(&preview).unwrap_or_default(),
        status: r.get(13)?,
        decided_at: r.get(14)?,
        decided_by: decided_kind.zip(decided_name).map(|(kind, name)| Actor {
            kind: ActorKind::from_stored(&kind),
            name,
        }),
        reason: r.get(17)?,
        group: id_at(18)?,
        claimed_at: r.get(19)?,
        last_error: r.get(20)?,
    })
}

/// The proposal `id`, if there is one.
pub(crate) fn load(tx: &Transaction<'_>, id: Id) -> Result<Option<Stored>, StoreError> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM proposals WHERE id = ?1"),
            [id.as_bytes().as_slice()],
            stored,
        )
        .optional()?)
}

/// Whether `caller` sees `row`: the person sees every proposal, an agent
/// those of agents with its name (its sessions before this one too).
fn visible_to(caller: &Context, row: &Stored) -> bool {
    !caller.is_agent() || row.agent == caller.actor.name
}

fn parse_id(field: &str, text: &str) -> Result<Id, Failure> {
    text.trim().parse().map_err(|_| {
        Failure::Op(messages::hint(
            messages::invalid_at(field, "input-not-an-id", &[("text", text.to_owned())]),
            "input-id-hint",
            &[],
        ))
    })
}

fn unknown(id: &str) -> Failure {
    Failure::Op(messages::not_found(
        "proposal-not-found",
        &[("id", id.to_owned())],
    ))
}

/// The session a filter names: an ID, or `current` for an agent's own.
fn session_filter(caller: &Context, text: &str) -> Result<Id, Failure> {
    if text == "current" {
        return caller
            .session
            .ok_or_else(|| messages::invalid("session", "proposal-no-own-session", &[]));
    }
    parse_id("session", text)
}

/// The proposal as callers see it at `now`: long text cut as in every
/// list, the task as it is now, and for a waiting one what changed since.
fn view(tx: &Transaction<'_>, row: Stored, now: i64, days: u32) -> Result<Proposal, Failure> {
    let status = row.status(now, days);
    let interrupted = row.interrupted(now);
    let conflicts = if status == ProposalStatus::Pending {
        conflicts(tx, &row.changes)?
    } else {
        Vec::new()
    };
    let task = match row.task {
        Some(task) => history::task_of(tx, fields::TASK, &task.to_string())?,
        None => None,
    };
    let mut changes = row.changes;
    for change in &mut changes {
        for field in change.fields.values_mut() {
            field.cut_long_text();
        }
    }
    let expires_at = row.created_at + i64::from(days) * DAY_MS;
    let decided_at = match status {
        ProposalStatus::Expired => Some(expires_at),
        _ => row.decided_at,
    };
    Ok(Proposal {
        id: row.id.to_string(),
        op: row.op,
        summary: row.summary,
        input: row.input,
        status,
        created_at: rfc3339(row.created_at),
        expires_at: rfc3339(expires_at),
        agent: Actor {
            kind: ActorKind::Agent,
            name: row.agent,
        },
        session: row.session.map(|s| s.to_string()),
        profile: row.profile,
        project: row.project.map(|p| p.to_string()),
        task,
        changes,
        previewed: row.previewed,
        conflicts,
        decided_at: decided_at.map(rfc3339),
        decided_by: row.decided_by,
        reason: row.reason,
        group: row.group.map(|g| g.to_string()),
        claimed_at: row
            .claimed_at
            .filter(|_| status == ProposalStatus::Approving)
            .map(rfc3339),
        interrupted,
        last_error: row.last_error.filter(|_| status == ProposalStatus::Pending),
    })
}

/// The fields of `changes` whose value differs now from the one the
/// preview started from. Items the change creates are left out.
fn conflicts(
    tx: &Transaction<'_>,
    changes: &[ProposedChange],
) -> Result<Vec<ProposalConflict>, StoreError> {
    let mut out = Vec::new();
    for change in changes {
        if change.fields.contains_key("created") {
            continue;
        }
        let Ok(id) = change.id.parse::<Id>() else {
            continue;
        };
        for (field, recorded) in &change.fields {
            let Some(now) = history::current(tx, &change.entity, id, field)? else {
                continue;
            };
            let then = recorded.before.clone().filter(|v| !v.is_null());
            let now = now.filter(|v| !v.is_null());
            if now != then {
                out.push(ProposalConflict {
                    entity: change.entity.clone(),
                    id: change.id.clone(),
                    field: field.clone(),
                    then,
                    now,
                });
            }
        }
    }
    Ok(out)
}

fn list(q: &QueryCtx<'_>, input: ProposalListInput) -> Result<ProposalList, Failure> {
    let days = settings::load(q.tx)?.agent_approvals.expire_days;
    let cutoff = q.now - i64::from(days) * DAY_MS;
    let agent = q.caller.is_agent().then(|| q.caller.actor.name.clone());
    let pending: i64 = q.tx.query_row(
        "SELECT count(*) FROM proposals
          WHERE status = 'pending' AND created_at > ?1 AND (?2 IS NULL OR agent_name = ?2)",
        params![cutoff, agent],
        |r| r.get(0),
    )?;
    let pending = u64::try_from(pending).unwrap_or(0);
    if !input.ids.is_empty() {
        let mut items = Vec::new();
        for text in &input.ids {
            let id = parse_id("ids", text)?;
            let row = load(q.tx, id)?
                .filter(|row| visible_to(q.caller, row))
                .ok_or_else(|| unknown(text))?;
            items.push(view(q.tx, row, q.now, days)?);
        }
        let total = items.len() as u64;
        return Ok(ProposalList {
            items,
            total,
            pending,
        });
    }
    let session = input
        .session
        .as_deref()
        .map(|s| session_filter(q.caller, s))
        .transpose()?
        .map(|s| s.as_bytes().to_vec());
    let status = input.status.map(ProposalStatus::as_str);
    let filter = "WHERE (?1 IS NULL OR agent_name = ?1)
                    AND (?2 IS NULL OR session_id = ?2)
                    AND (?3 IS NULL OR (CASE WHEN status = 'pending' AND created_at <= ?4
                                             THEN 'expired' ELSE status END) = ?3)";
    let total: i64 = q.tx.query_row(
        &format!("SELECT count(*) FROM proposals {filter}"),
        params![agent, session, status, cutoff],
        |r| r.get(0),
    )?;
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let rows = {
        let mut stmt = q.tx.prepare(&format!(
            "SELECT {COLUMNS} FROM proposals {filter}
              ORDER BY created_at DESC, id DESC LIMIT ?5"
        ))?;
        stmt.query_map(params![agent, session, status, cutoff, limit], stored)?
            .collect::<Result<Vec<_>, _>>()?
    };
    let items = rows
        .into_iter()
        .map(|row| view(q.tx, row, q.now, days))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProposalList {
        items,
        total: u64::try_from(total).unwrap_or(0),
        pending,
    })
}

/// The proposals that change task `task` and wait for the person, as the
/// caller of `q` sees them (an agent, its own), newest first, at most
/// `limit`.
pub(crate) fn pending_on_task(
    q: &QueryCtx<'_>,
    task: Id,
    limit: u32,
) -> Result<Vec<Proposal>, Failure> {
    let days = settings::load(q.tx)?.agent_approvals.expire_days;
    let cutoff = q.now - i64::from(days) * DAY_MS;
    let agent = q.caller.is_agent().then(|| q.caller.actor.name.clone());
    let rows = {
        let mut stmt = q.tx.prepare(&format!(
            "SELECT {COLUMNS} FROM proposals
              WHERE task_id = ?1 AND status = 'pending' AND created_at > ?2
                AND (?3 IS NULL OR agent_name = ?3)
              ORDER BY created_at DESC, id DESC LIMIT ?4"
        ))?;
        stmt.query_map(
            params![task.as_bytes().as_slice(), cutoff, agent, limit],
            stored,
        )?
        .collect::<Result<Vec<_>, _>>()?
    };
    rows.into_iter()
        .map(|row| view(q.tx, row, q.now, days))
        .collect()
}

/// Keeps the agent's call of `desc` with `input` as a proposal, inside the
/// write transaction of `w` (whose caller is the agent). `handler` is the
/// operation's own handler, run once in a savepoint for the preview;
/// operations that work outside the transaction (files, backups) have
/// none and are kept without a preview. `request` is the agent's request
/// key: a repeat finds the first proposal. Returns the proposal's ID.
pub(crate) fn create(
    w: &mut WriteCtx<'_>,
    desc: &'static OpDescriptor,
    handler: Option<&CommandFn>,
    input: &Value,
    request: Option<&str>,
) -> Result<Value, Failure> {
    let ctx = w.caller().clone();
    if let Some(key) = request {
        let found: Option<Vec<u8>> = w
            .tx()
            .query_row(
                "SELECT id FROM proposals WHERE request_key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = found.and_then(|b| Id::from_slice(&b).ok()) {
            return Ok(json!({ "id": id.to_string() }));
        }
    }
    if let Some(session) = ctx.session {
        let waiting: i64 = w.tx().query_row(
            "SELECT count(*) FROM proposals WHERE session_id = ?1 AND status = 'pending'",
            [session.as_bytes().as_slice()],
            |r| r.get(0),
        )?;
        let max = i64::from(PROPOSALS_PENDING_MAX);
        if waiting >= max {
            let args = [("max", max.to_string())];
            return Err(Failure::Op(messages::hint(
                messages::tag(
                    OpError::limit(
                        messages::text("proposal-too-many", &args),
                        u64::from(PROPOSALS_PENDING_MAX),
                    ),
                    "proposal-too-many",
                    &args,
                ),
                "proposal-too-many-hint",
                &[],
            )));
        }
    }
    let (changes, task) = match handler {
        Some(f) => preview(w, desc, f, input)?,
        None => (Vec::new(), None),
    };
    let project = changes
        .iter()
        .find_map(|c| c.project.as_deref().and_then(|p| p.parse::<Id>().ok()));
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let preview_json = serde_json::to_string(&changes).map_err(|e| {
        Failure::Op(messages::storage(
            "storage-json",
            &[("error", e.to_string())],
        ))
    })?;
    w.tx().execute(
        "INSERT INTO proposals (id, created_at, op, input, session_id, agent_name, profile,
                                project_scope, project_id, task_id, summary, previewed, preview,
                                request_key, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 'pending')",
        params![
            id.as_bytes().as_slice(),
            w.now(),
            desc.id,
            input.to_string(),
            ctx.session.map(|s| s.as_bytes().to_vec()),
            ctx.actor.name,
            ctx.profile.unwrap_or(Profile::Standard).as_str(),
            ctx.project_scope,
            project.map(|p| p.as_bytes().to_vec()),
            task.map(|t| t.as_bytes().to_vec()),
            desc.summary,
            handler.is_some(),
            preview_json,
            request,
        ],
    )?;
    w.record_field(
        ENTITY,
        id,
        project,
        "status",
        None,
        Some(Value::from(ProposalStatus::Pending.as_str())),
    );
    w.record_field(ENTITY, id, project, "op", None, Some(Value::from(desc.id)));
    Ok(json!({ "id": id.to_string() }))
}

/// Runs `f` for `desc` with `input` as the agent would, with the status
/// rules and what follows a command, and takes it all back: what it would
/// change, and the task it changes.
fn preview(
    w: &WriteCtx<'_>,
    desc: &'static OpDescriptor,
    f: &CommandFn,
    input: &Value,
) -> Result<(Vec<ProposedChange>, Option<Id>), Failure> {
    let mut meta = w.meta().clone();
    meta.op = desc.id;
    let tx = w.tx();
    tx.execute_batch("SAVEPOINT proposal_preview")?;
    let result = (|| -> Result<_, Failure> {
        let mut inner = WriteCtx::new(tx, &meta);
        f(&mut inner, input.clone())?;
        crate::ops::recurrence::after_command(&mut inner)?;
        crate::ops::rules::enforce(&mut inner)?;
        let pending = inner.pending();
        // While the preview's items still exist: a new comment's task.
        let mut task = None;
        for (entity, id, _, fields) in &pending {
            if entity == fields::TASK && fields.contains_key("created") {
                continue;
            }
            if let Some(found) = history::task_of(tx, entity, &id.to_string())? {
                task = found.id.parse::<Id>().ok();
                break;
            }
        }
        Ok((pending, task))
    })();
    let undone = tx.execute_batch("ROLLBACK TO proposal_preview; RELEASE proposal_preview");
    let (pending, task) = result?;
    undone?;
    let changes = pending
        .into_iter()
        .map(|(entity, id, project, fields)| ProposedChange {
            entity,
            id: id.to_string(),
            project: project.map(|p| p.to_string()),
            fields,
        })
        .collect();
    Ok((changes, task))
}

/// Marks the proposal the caller carries out approved, in the write
/// transaction of its change: refused when it was decided meanwhile, so it
/// runs once. `group` is the change's history group when it is not this
/// command's own (operations that run outside one transaction).
pub(crate) fn settle_approved(
    w: &mut WriteCtx<'_>,
    group: Option<Id>,
    claimed: bool,
) -> Result<(), Failure> {
    let Some(approval) = w.caller().approved.clone() else {
        return Ok(());
    };
    let id = approval.proposal;
    let row = load(w.tx(), id)?.ok_or_else(|| unknown(&id.to_string()))?;
    let days = settings::load(w.tx())?.agent_approvals.expire_days;
    // A change in this transaction settles a waiting proposal; one that ran
    // outside a transaction settles the one it claimed.
    let from = if claimed {
        ProposalStatus::Approving
    } else {
        ProposalStatus::Pending
    };
    let status = row.status(w.now(), days);
    if status != from {
        return Err(decided(&row, status));
    }
    let group = group.unwrap_or_else(|| w.group());
    let changed = w.tx().execute(
        "UPDATE proposals
            SET status = 'approved', decided_at = ?2, decided_by_kind = ?3,
                decided_by_name = ?4, group_id = ?5, claimed_at = NULL, last_error = NULL
          WHERE id = ?1 AND status = ?6",
        params![
            id.as_bytes().as_slice(),
            w.now(),
            approval.by.kind.as_str(),
            approval.by.name,
            group.as_bytes().as_slice(),
            from.as_str(),
        ],
    )?;
    if changed != 1 {
        return Err(decided(&row, status));
    }
    w.record_field(
        ENTITY,
        id,
        row.project,
        "status",
        Some(Value::from(from.as_str())),
        Some(Value::from(ProposalStatus::Approved.as_str())),
    );
    Ok(())
}

/// Claims the proposal the caller carries out, for an operation that runs
/// outside one write transaction: one conditional update, in a transaction
/// of its own, takes it from waiting to being approved; any other program
/// (or a second click) finds it claimed and runs nothing.
pub(crate) fn claim(engine: &Engine, ctx: &Context) -> Result<(), OpError> {
    let f: CommandFn = Arc::new(move |w, _| {
        let Some(approval) = w.caller().approved.clone() else {
            return Ok(Value::Null);
        };
        let id = approval.proposal;
        let row = load(w.tx(), id)?.ok_or_else(|| unknown(&id.to_string()))?;
        let days = settings::load(w.tx())?.agent_approvals.expire_days;
        let status = row.status(w.now(), days);
        let changed = if status == ProposalStatus::Pending {
            w.tx().execute(
                "UPDATE proposals SET status = 'approving', claimed_at = ?2
                  WHERE id = ?1 AND status = 'pending'",
                params![id.as_bytes().as_slice(), w.now()],
            )?
        } else {
            0
        };
        if changed != 1 {
            let status = load(w.tx(), id)?.map_or(status, |r| r.status(w.now(), days));
            return Err(claimed_by_another(&row, status));
        }
        w.record_field(
            ENTITY,
            id,
            row.project,
            "status",
            Some(Value::from(ProposalStatus::Pending.as_str())),
            Some(Value::from(ProposalStatus::Approving.as_str())),
        );
        Ok(Value::Null)
    });
    engine
        .execute(ctx, "proposal.approve", f, Value::Null)
        .map(drop)
}

/// Gives a claimed proposal back to waiting after its operation failed,
/// with the error, so the person sees why and may try again or reject it.
pub(crate) fn release(engine: &Engine, ctx: &Context, error: &OpError) -> Result<(), OpError> {
    let message = error.message.clone();
    let f: CommandFn = Arc::new(move |w, _| {
        let Some(approval) = w.caller().approved.clone() else {
            return Ok(Value::Null);
        };
        let id = approval.proposal;
        let Some(row) = load(w.tx(), id)? else {
            return Ok(Value::Null);
        };
        let changed = w.tx().execute(
            "UPDATE proposals SET status = 'pending', claimed_at = NULL, last_error = ?2
              WHERE id = ?1 AND status = 'approving'",
            params![id.as_bytes().as_slice(), message],
        )?;
        if changed == 1 {
            w.record_field(
                ENTITY,
                id,
                row.project,
                "status",
                Some(Value::from(ProposalStatus::Approving.as_str())),
                Some(Value::from(ProposalStatus::Pending.as_str())),
            );
        }
        Ok(Value::Null)
    });
    engine
        .execute(ctx, "proposal.approve", f, Value::Null)
        .map(drop)
}

/// The error for a proposal another approval holds, or that was decided.
fn claimed_by_another(row: &Stored, status: ProposalStatus) -> Failure {
    if status != ProposalStatus::Approving {
        return decided(row, status);
    }
    let id = row.id.to_string();
    Failure::Op(
        messages::hint(
            messages::conflict("proposal-being-decided", &[("id", id.clone())]),
            "proposal-being-decided-hint",
            &[],
        )
        .with_detail("proposal", id)
        .with_detail("status", status.as_str()),
    )
}

/// The error for a proposal that is no longer waiting.
fn decided(row: &Stored, status: ProposalStatus) -> Failure {
    if status == ProposalStatus::Approving {
        return claimed_by_another(row, status);
    }
    let id = row.id.to_string();
    let message = if status == ProposalStatus::Expired {
        "proposal-expired"
    } else {
        "proposal-decided"
    };
    Failure::Op(
        messages::conflict(
            message,
            &[("id", id.clone()), ("status", status.as_str().to_owned())],
        )
        .with_detail("proposal", id)
        .with_detail("status", status.as_str()),
    )
}

/// The waiting proposals that `ids` or `session` name, oldest first.
fn chosen(
    tx: &Transaction<'_>,
    ids: &[String],
    session: Option<&str>,
    now: i64,
    days: u32,
) -> Result<Vec<Stored>, Failure> {
    if ids.is_empty() == session.is_none() {
        return Err(messages::invalid("ids", "proposal-ids-or-session", &[]));
    }
    let mut rows = Vec::new();
    for text in ids {
        let id = parse_id("ids", text)?;
        rows.push(load(tx, id)?.ok_or_else(|| unknown(text))?);
    }
    if let Some(session) = session {
        let session = parse_id("session", session)?;
        let cutoff = now - i64::from(days) * DAY_MS;
        let mut stmt = tx.prepare(&format!(
            "SELECT {COLUMNS} FROM proposals
              WHERE session_id = ?1 AND status = 'pending' AND created_at > ?2"
        ))?;
        rows.extend(
            stmt.query_map(params![session.as_bytes().as_slice(), cutoff], stored)?
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    rows.sort_by_key(|r| (r.created_at, *r.id.as_bytes()));
    rows.dedup_by_key(|r| r.id);
    Ok(rows)
}

fn reject(
    w: &mut WriteCtx<'_>,
    input: ProposalRejectInput,
) -> Result<ProposalRejectResult, Failure> {
    let reason = input
        .reason
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    if let Some(reason) = &reason
        && reason.chars().count() > REJECT_REASON_MAX
    {
        return Err(messages::invalid(
            "reason",
            "proposal-reason-too-long",
            &[("max", REJECT_REASON_MAX.to_string())],
        ));
    }
    let days = settings::load(w.tx())?.agent_approvals.expire_days;
    let rows = chosen(w.tx(), &input.ids, input.session.as_deref(), w.now(), days)?;
    if rows.is_empty() {
        return Err(Failure::Op(messages::not_found(
            "proposal-none-waiting",
            &[],
        )));
    }
    let by = w.caller().actor.clone();
    let mut done = Vec::new();
    for row in rows {
        let status = row.status(w.now(), days);
        // An approval that was interrupted is never run again by itself:
        // the person may only set it aside.
        let interrupted = row.interrupted(w.now());
        if status != ProposalStatus::Pending && !interrupted {
            return Err(decided(&row, status));
        }
        w.tx().execute(
            "UPDATE proposals
                SET status = 'rejected', decided_at = ?2, decided_by_kind = ?3,
                    decided_by_name = ?4, reason = ?5, claimed_at = NULL
              WHERE id = ?1 AND status = ?6",
            params![
                row.id.as_bytes().as_slice(),
                w.now(),
                by.kind.as_str(),
                by.name,
                reason,
                row.status.as_str(),
            ],
        )?;
        w.record_field(
            ENTITY,
            row.id,
            row.project,
            "status",
            Some(Value::from(row.status.clone())),
            Some(Value::from(ProposalStatus::Rejected.as_str())),
        );
        if let Some(reason) = &reason {
            w.record_field(
                ENTITY,
                row.id,
                row.project,
                "reason",
                None,
                Some(Value::from(reason.clone())),
            );
        }
        done.push(row.id);
    }
    let mut items = Vec::new();
    for id in done {
        if let Some(row) = load(w.tx(), id)? {
            items.push(view(w.tx(), row, w.now(), days)?);
        }
    }
    Ok(ProposalRejectResult { items })
}

/// `proposal.approve`: each chosen proposal runs through the pipeline as
/// its agent, oldest first. One that changed since, or fails now, stays
/// waiting and says why; the others still run.
fn approve(
    engine: &Engine,
    ctx: &Context,
    input: ProposalApproveInput,
) -> Result<ProposalApproveResult, OpError> {
    let _one_at_a_time = engine.lock_approvals();
    let now = (engine.clock)();
    let (days, rows) = engine
        .store
        .read(|tx| {
            let days = settings::load(tx)?.agent_approvals.expire_days;
            Ok::<_, Failure>((
                days,
                chosen(tx, &input.ids, input.session.as_deref(), now, days)?,
            ))
        })
        .map_err(OpError::from)?;
    if rows.is_empty() {
        return Err(messages::not_found("proposal-none-waiting", &[]));
    }
    let mut items = Vec::new();
    for row in rows {
        items.push(approve_one(engine, ctx, row, input.force, days)?);
    }
    let approved = items.iter().filter(|o| o.approved).count() as u64;
    Ok(ProposalApproveResult { items, approved })
}

fn approve_one(
    engine: &Engine,
    ctx: &Context,
    row: Stored,
    force: bool,
    days: u32,
) -> Result<ProposalOutcome, OpError> {
    let id = row.id;
    let now = (engine.clock)();
    let refusal = match row.status(now, days) {
        ProposalStatus::Pending => None,
        status => Some(OpError::from(decided(&row, status))),
    };
    let refusal = match refusal {
        Some(e) => Some(e),
        None if force => None,
        None => {
            let stale = engine
                .store
                .read(|tx| conflicts(tx, &row.changes).map_err(Failure::from))
                .map_err(OpError::from)?;
            (!stale.is_empty()).then(|| {
                messages::hint(
                    messages::conflict(
                        "proposal-stale",
                        &[("id", id.to_string()), ("count", stale.len().to_string())],
                    ),
                    "proposal-stale-hint",
                    &[],
                )
                .with_detail("proposal", id.to_string())
            })
        }
    };
    let refusal_was_none = refusal.is_none();
    let error = match refusal {
        Some(e) => Some(e),
        None => {
            let mut agent = Context::agent(
                row.agent.clone(),
                row.session.unwrap_or_else(|| Id::from_bytes([0; 16])),
                row.profile,
            );
            agent.session = row.session;
            if let Some(scope) = &row.scope {
                agent = agent.within_project(scope.clone());
            }
            let agent = agent.approved_by(id, ctx.actor.clone());
            engine.call(&agent, &row.op, row.input.clone()).err()
        }
    };
    let proposal = engine
        .store
        .read(|tx| {
            let row = load(tx, id)?.ok_or_else(|| unknown(&id.to_string()))?;
            view(tx, row, (engine.clock)(), days)
        })
        .map_err(OpError::from)?;
    let ran_and_failed = refusal_was_none && error.is_some();
    let proposal = if ran_and_failed && proposal.status == ProposalStatus::Pending {
        let message = error.as_ref().map(|e| e.message.clone());
        let stored = message.clone();
        let f: CommandFn = Arc::new(move |w, _| {
            w.tx().execute(
                "UPDATE proposals SET last_error = ?2 WHERE id = ?1 AND status = 'pending'",
                params![id.as_bytes().as_slice(), stored],
            )?;
            Ok(Value::Null)
        });
        engine.execute(ctx, "proposal.approve", f, Value::Null)?;
        Proposal {
            last_error: message,
            ..proposal
        }
    } else {
        proposal
    };
    let approved = error.is_none() && proposal.status == ProposalStatus::Approved;
    if approved
        && ctx.via == gk_api::ops::common::Via::App
        && !ctx.is_agent()
        && let Some(group) = proposal.group.as_deref().and_then(|g| g.parse().ok())
    {
        engine.push_undo(group);
    }
    Ok(ProposalOutcome {
        proposal,
        approved,
        error,
    })
}

/// Wraps the handler of an approved proposal's operation so the proposal
/// is marked approved in the same transaction as its change.
pub(crate) fn settling(f: &CommandFn) -> CommandFn {
    let f = Arc::clone(f);
    Arc::new(move |w, input| {
        let output = f(w, input)?;
        settle_approved(w, None, false)?;
        Ok(output)
    })
}

/// Marks an approved proposal whose operation ran outside one write
/// transaction approved, after it ran; `output` is what it returned.
pub(crate) fn settle_after(engine: &Engine, ctx: &Context, output: &Value) -> Result<(), OpError> {
    let group = output
        .get("group")
        .and_then(Value::as_str)
        .and_then(|g| g.parse::<Id>().ok());
    let f: CommandFn = Arc::new(move |w, _| {
        settle_approved(w, group, true)?;
        Ok(Value::Null)
    });
    engine
        .execute(ctx, "proposal.approve", f, Value::Null)
        .map(drop)
}

/// The `pending_approval` answer to an agent's change kept as proposal
/// `id`.
pub(crate) fn pending(id: &str) -> OpError {
    messages::hint(
        messages::error(
            ErrorCode::PendingApproval,
            "proposal-pending",
            &[("id", id.to_owned())],
        ),
        "proposal-pending-hint",
        &[],
    )
    .with_detail("proposal", id.to_owned())
    .with_detail("status", ProposalStatus::Pending.as_str())
}

/// A field change as a preview records it, for tests.
#[cfg(test)]
pub(crate) fn field(
    before: Option<Value>,
    after: Option<Value>,
) -> gk_api::ops::history::FieldChange {
    gk_api::ops::history::FieldChange {
        before,
        after,
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_expires_after_its_days() {
        let row = Stored {
            id: Id::from_bytes([1; 16]),
            created_at: 0,
            op: "task.update".to_owned(),
            input: Value::Null,
            session: None,
            agent: "Agent".to_owned(),
            profile: Profile::Standard,
            scope: None,
            project: None,
            task: None,
            summary: String::new(),
            previewed: true,
            changes: vec![ProposedChange {
                entity: "task".to_owned(),
                id: Id::from_bytes([2; 16]).to_string(),
                project: None,
                fields: [(
                    "title".to_owned(),
                    field(Some("a".into()), Some("b".into())),
                )]
                .into_iter()
                .collect(),
            }],
            status: "pending".to_owned(),
            decided_at: None,
            decided_by: None,
            reason: None,
            group: None,
            claimed_at: None,
            last_error: None,
        };
        assert_eq!(row.status(DAY_MS * 14 - 1, 14), ProposalStatus::Pending);
        assert_eq!(row.status(DAY_MS * 14, 14), ProposalStatus::Expired);
        assert_eq!(row.status(DAY_MS * 2, 1), ProposalStatus::Expired);
    }
}
