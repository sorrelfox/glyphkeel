// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `agent.sessions` and `agent.policy.update`.

use gk_api::Profile;
use gk_api::ops::agent::{
    AgentPolicyUpdate, AgentSession, AgentSessionList, AgentSessions, AgentSessionsInput,
};
use gk_api::ops::common::{DEFAULT_LIMIT, MAX_LIMIT};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{Row, Transaction, params};
use serde_json::Value;

use super::{desc, settings};
use crate::failure::Failure;
use crate::registry::Registry;
use crate::time::rfc3339;

pub(super) fn register(r: &mut Registry) {
    r.query::<AgentSessions, _>(desc::<AgentSessions>(), |q, input| {
        list(q.tx, &input).map_err(Failure::from)
    });
    r.command::<AgentPolicyUpdate, _>(desc::<AgentPolicyUpdate>(), |w, input| {
        let value = Value::String(input.max_profile.as_str().to_owned());
        settings::set_all(w, &[("agent_max_profile", value)])
    });
}

/// Agent sessions, newest first.
pub(crate) fn list(
    tx: &Transaction<'_>,
    input: &AgentSessionsInput,
) -> Result<AgentSessionList, StoreError> {
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let filter = if input.running_only {
        "WHERE ended_at IS NULL"
    } else {
        ""
    };
    let total: i64 = tx.query_row(
        &format!("SELECT count(*) FROM agent_sessions {filter}"),
        [],
        |r| r.get(0),
    )?;
    let mut stmt = tx.prepare(&format!(
        "SELECT id, display_name, client_id, client_version, profile, started_at,
                last_seen_at, ended_at, reads, writes
           FROM agent_sessions {filter}
          ORDER BY started_at DESC, id DESC LIMIT ?1"
    ))?;
    let items = stmt
        .query_map(params![limit], session)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AgentSessionList {
        items,
        total: u64::try_from(total).unwrap_or(0),
    })
}

fn session(r: &Row<'_>) -> gk_store::rusqlite::Result<AgentSession> {
    let id: Vec<u8> = r.get(0)?;
    let profile: String = r.get(4)?;
    Ok(AgentSession {
        id: Id::from_slice(&id).map_or_else(|_| String::new(), |i| i.to_string()),
        name: r.get(1)?,
        client: r.get(2)?,
        client_version: r.get(3)?,
        profile: profile.parse().unwrap_or(Profile::ReadOnly),
        started_at: rfc3339(r.get(5)?),
        last_seen_at: rfc3339(r.get(6)?),
        ended_at: r.get::<_, Option<i64>>(7)?.map(rfc3339),
        reads: u64::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
        writes: u64::try_from(r.get::<_, i64>(9)?).unwrap_or(0),
    })
}
