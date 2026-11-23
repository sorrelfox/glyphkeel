// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Agent sessions and the agent policy.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::settings::SettingsChange;
use crate::profile::Profile;

/// One run of `glyphkeel mcp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentSession {
    /// The session's ID.
    pub id: String,
    /// The agent's display name.
    pub name: String,
    /// The client's own ID (`claude-code`), when it sent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// The client's version, when it sent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    /// The profile the session ran with.
    pub profile: Profile,
    /// When it started (RFC 3339, UTC).
    pub started_at: String,
    /// When it was last seen working.
    pub last_seen_at: String,
    /// When it ended; left out while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Operations that read.
    pub reads: u64,
    /// Operations that changed something.
    pub writes: u64,
}

/// Which sessions to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentSessionsInput {
    /// Only sessions that haven't ended.
    #[serde(default)]
    pub running_only: bool,
    /// At most this many, newest first (default 50, at most 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Agent sessions, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentSessionList {
    /// The sessions.
    pub items: Vec<AgentSession>,
    /// How many sessions match in all.
    pub total: u64,
}

/// The agent policy to set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentPolicyPatch {
    /// The most an agent may do, whatever profile it asks for.
    pub max_profile: Profile,
}

op!(
    /// List agent sessions.
    AgentSessions,
    "agent.sessions",
    AgentSessionsInput,
    AgentSessionList
);
op!(
    /// Set the most an agent may do.
    AgentPolicyUpdate,
    "agent.policy.update",
    AgentPolicyPatch,
    SettingsChange
);

pub(crate) const AGENT_SESSIONS: OpDescriptor = OpDescriptor::of::<AgentSessions>(Meta {
    summary: "List agent sessions, newest first, with their client, profile, times and counts of reads and writes.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("agents sessions"),
    input_name: "AgentSessionsInput",
    output_name: "AgentSessionList",
});

pub(crate) const AGENT_POLICY_UPDATE: OpDescriptor = OpDescriptor::of::<AgentPolicyUpdate>(Meta {
    summary: "Set the most an agent may do, whatever profile it asks for.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "It sets the rights of agents; an agent that could call it would raise its own rights beyond what the person allowed.",
    },
    cli: Some("agents policy"),
    input_name: "AgentPolicyPatch",
    output_name: "SettingsChange",
});
