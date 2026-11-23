// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Claims: an agent session marks a task as taken for a while, so several
//! agents can work from one queue without colliding.
//!
//! Rules: only agent sessions claim; a claim lasts its lease (60 minutes
//! unless asked otherwise) and every change the claiming session makes to
//! the task renews it; other agents' changes to the task's fields, moves
//! and claims are refused while it lasts, unless they pass
//! `override_claim` with the full profile; the person is never blocked; an
//! expired claim simply stops counting.
//!
//! History records the claim as the task's `claim` field, holding the
//! session and the agent's name; the lease end changes with every renewal
//! and is not recorded.

use gk_api::OpError;
use gk_api::ops::common::Category;
use gk_api::ops::task::{
    TaskChange, TaskClaim, TaskClaimInput, TaskMoveInput, TaskRelease, TaskReleaseInput,
};
use gk_api::{Profile, ops::comment::CommentAddInput};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::desc;
use super::fields::TASK;
use super::lookup::{self, List, b, opt_id_at};
use crate::failure::Failure;
use crate::registry::Registry;
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// The lease when none is asked for, and the least a renewal gives.
const DEFAULT_LEASE_MIN: i64 = 60;
/// The longest lease.
const MAX_LEASE_MIN: u32 = 480;
const MINUTE_MS: i64 = 60_000;

pub(super) fn register(r: &mut Registry) {
    r.command::<TaskClaim, _>(desc::<TaskClaim>(), claim);
    r.command::<TaskRelease, _>(desc::<TaskRelease>(), release);
}

/// A task's stored claim: session, agent name, lease end.
struct Stored {
    session: Id,
    name: String,
    until: i64,
}

fn stored(tx: &Transaction<'_>, task: Id) -> Result<Option<Stored>, Failure> {
    let row = tx
        .query_row(
            "SELECT claim_session, claim_name, claim_until FROM tasks WHERE id = ?1",
            [b(task)],
            |r| {
                Ok((
                    opt_id_at(r, 0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(match row {
        Some((Some(session), name, Some(until))) => Some(Stored {
            session,
            name: name.unwrap_or_default(),
            until,
        }),
        _ => None,
    })
}

/// The claim as history records it.
fn value(session: Id, name: &str) -> Value {
    json!({ "session": session.to_string(), "agent": name })
}

/// The task's recorded claim, whether or not its lease has ended.
pub(crate) fn raw(tx: &Transaction<'_>, task: Id) -> Result<Option<Value>, StoreError> {
    let row: Option<(Option<Vec<u8>>, Option<String>)> = tx
        .query_row(
            "SELECT claim_session, claim_name FROM tasks WHERE id = ?1",
            [b(task)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((Some(session), name)) => Id::from_slice(&session)
            .ok()
            .map(|s| value(s, &name.unwrap_or_default())),
        _ => None,
    })
}

/// Writes a claim (`None`: no claim) without recording it.
fn write(w: &WriteCtx<'_>, task: Id, claim: Option<(Id, &str, i64)>) -> Result<(), Failure> {
    let (session, name, until) = match claim {
        Some((s, n, u)) => (Some(b(s)), Some(n.to_owned()), Some(u)),
        None => (None, None, None),
    };
    w.tx().execute(
        "UPDATE tasks SET claim_session = ?2, claim_name = ?3, claim_until = ?4, updated_at = ?5
          WHERE id = ?1",
        params![b(task), session, name, until, w.now()],
    )?;
    Ok(())
}

/// Sets the recorded claim (revert's path): a claim brought back gets a
/// fresh default lease.
pub(crate) fn apply(
    w: &mut WriteCtx<'_>,
    task: Id,
    project: Option<Id>,
    value: Option<Value>,
) -> Result<(), Failure> {
    let before = raw(w.tx(), task)?;
    let parsed = value.as_ref().and_then(|v| {
        let session = v.get("session")?.as_str()?.parse::<Id>().ok()?;
        let name = v.get("agent")?.as_str()?.to_owned();
        Some((session, name))
    });
    match &parsed {
        Some((session, name)) => write(
            w,
            task,
            Some((*session, name, w.now() + DEFAULT_LEASE_MIN * MINUTE_MS)),
        )?,
        None => write(w, task, None)?,
    }
    w.record_field(TASK, task, project, "claim", before, value);
    Ok(())
}

/// The profile an agent works with: the one it asked for, capped by the
/// workspace setting.
fn effective_profile(w: &WriteCtx<'_>) -> Result<Profile, Failure> {
    let cap = super::settings::load(w.tx())?.agent_max_profile;
    Ok(w.caller()
        .profile
        .unwrap_or(Profile::Standard)
        .capped_by(cap))
}

/// Checks that the caller may change task `task` (named `key` in errors):
/// the person always may; an agent may unless another session's claim
/// lasts, or it overrides the claim with the full profile. The claiming
/// session's own changes renew its claim.
pub(crate) fn guard(
    w: &WriteCtx<'_>,
    task: Id,
    key: &str,
    override_claim: bool,
) -> Result<(), Failure> {
    if !w.caller().is_agent() {
        return Ok(());
    }
    let Some(claim) = stored(w.tx(), task)? else {
        return Ok(());
    };
    if claim.until <= w.now() {
        return Ok(());
    }
    if Some(claim.session) == w.caller().session {
        let renewed = claim.until.max(w.now() + DEFAULT_LEASE_MIN * MINUTE_MS);
        w.tx().execute(
            "UPDATE tasks SET claim_until = ?2 WHERE id = ?1",
            params![b(task), renewed],
        )?;
        return Ok(());
    }
    if override_claim {
        if effective_profile(w)? == Profile::Full {
            return Ok(());
        }
        return Err(OpError::forbidden(format!(
            "Overriding {}'s claim on {key} needs the full profile.",
            claim.name
        ))
        .into());
    }
    Err(OpError::conflict(format!(
        "{key} is claimed by {} until {}.",
        claim.name,
        rfc3339(claim.until)
    ))
    .with_detail("claimed_by", claim.name)
    .with_detail("until", rfc3339(claim.until))
    .with_hint("Pick another task, wait for the claim to end, or ask the person.")
    .into())
}

fn claim(w: &mut WriteCtx<'_>, input: TaskClaimInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(OpError::not_found(
            format!("There is no task \"{}\".", input.key),
            &[],
        ))
    })?;
    lookup::ensure_active(&project)?;
    let Some(session) = w.caller().session else {
        return Err(OpError::invalid(
            "Only agent sessions claim tasks; the person is never blocked by claims.",
        )
        .into());
    };
    if input.keep_status && input.status.is_some() {
        return Err(OpError::invalid("Give either `status` or `keep_status`.").into());
    }
    let lease = input.lease_minutes.unwrap_or(60);
    if !(1..=MAX_LEASE_MIN).contains(&lease) {
        return Err(OpError::invalid_at(
            "lease_minutes",
            format!("A claim lasts 1 to {MAX_LEASE_MIN} minutes."),
        )
        .into());
    }
    guard(w, task.id, &input.key, input.override_claim)?;
    let name = w.caller().actor.name.clone();
    let until = w.now() + i64::from(lease) * MINUTE_MS;
    let before = raw(w.tx(), task.id)?;
    write(w, task.id, Some((session, &name, until)))?;
    w.record_field(
        TASK,
        task.id,
        Some(project.id),
        "claim",
        before,
        Some(value(session, &name)),
    );
    let status = if input.keep_status {
        None
    } else if let Some(s) = &input.status {
        Some(lookup::named(w.tx(), List::Statuses, &project, "status", s)?.id)
    } else {
        let current: String = w.tx().query_row(
            "SELECT category FROM statuses WHERE id = ?1",
            [b(task.status)],
            |r| r.get(0),
        )?;
        if current == super::project::enum_text(Category::InProgress) {
            None
        } else {
            lookup::all(w.tx(), List::Statuses, project.id)?
                .into_iter()
                .find(|s| s.kind == "in_progress")
                .map(|s| s.id)
        }
    };
    let mut warnings = Vec::new();
    if let Some(status) = status.filter(|s| *s != task.status) {
        let moved = super::task::move_task(
            w,
            TaskMoveInput {
                key: task.id.to_string(),
                status: Some(status.to_string()),
                ..TaskMoveInput::default()
            },
        )?;
        warnings = moved.warnings;
    }
    super::task::changed(w, task.id, warnings)
}

fn release(w: &mut WriteCtx<'_>, input: TaskReleaseInput) -> Result<TaskChange, Failure> {
    let task = lookup::task(w.tx(), "key", &input.key)?;
    let project = lookup::project_by_id(w.tx(), task.project)?.ok_or_else(|| {
        Failure::Op(OpError::not_found(
            format!("There is no task \"{}\".", input.key),
            &[],
        ))
    })?;
    lookup::ensure_active(&project)?;
    guard(w, task.id, &input.key, input.override_claim)?;
    let mut warnings = Vec::new();
    if let Some(status) = &input.status {
        let moved = super::task::move_task(
            w,
            TaskMoveInput {
                key: task.id.to_string(),
                status: Some(status.clone()),
                override_claim: input.override_claim,
                ..TaskMoveInput::default()
            },
        )?;
        warnings = moved.warnings;
    }
    if let Some(body) = &input.comment {
        super::comment::add_comment(
            w,
            &CommentAddInput {
                key: task.id.to_string(),
                body: body.clone(),
                ..CommentAddInput::default()
            },
            "comment",
        )?;
    }
    if stored(w.tx(), task.id)?.is_some() {
        apply(w, task.id, Some(project.id), None)?;
    }
    super::task::changed(w, task.id, warnings)
}
