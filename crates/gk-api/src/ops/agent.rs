// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Agent sessions and the agent policy.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::settings::SettingsChange;
use crate::profile::Profile;

/// Calls of changing tools an agent session may make in any minute, unless
/// the person set another limit.
pub const AGENT_WRITES_PER_MINUTE: u32 = 120;
/// Items an agent with the standard profile may move to the trash in one
/// call (subtasks count), unless the person set another limit.
pub const AGENT_TRASH_ITEMS_PER_CALL: u32 = 20;
/// Items an agent session with the standard profile may move to the trash
/// in an hour, unless the person set another limit.
pub const AGENT_TRASH_ITEMS_PER_HOUR: u32 = 50;
/// Reads an agent session may make at once, before the sustained rate
/// applies, unless the person set another limit.
pub const AGENT_READ_BURST: u32 = 60;
/// Reads an agent session may make in a minute, sustained (ten a second),
/// unless the person set another limit.
pub const AGENT_READS_PER_MINUTE: u32 = 600;
/// Megabytes an agent session may read in a minute, unless the person set
/// another limit.
pub const AGENT_READ_MB_PER_MINUTE: u32 = 8;
/// The largest number a limit on agents can be set to.
pub const AGENT_LIMIT_MAX: u32 = 1_000_000;

/// The limits on every agent session, so a runaway loop can't flood the
/// workspace. A limit that is left out is lifted: there is no limit. Only
/// the person changes them, in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentLimits {
    /// Calls of changing tools allowed in any minute, per session; left
    /// out: no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writes_per_minute: Option<u32>,
    /// Tasks a session may create in any 24 hours; left out: no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_tasks_per_day: Option<u32>,
    /// Items a standard agent may move to the trash in one call; left out:
    /// no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_items_per_call: Option<u32>,
    /// Items a standard agent session may move to the trash in an hour;
    /// left out: no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_items_per_hour: Option<u32>,
    /// Reads a session may make at once before `reads_per_minute` applies;
    /// left out: one minute's worth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_burst: Option<u32>,
    /// Reads (tool calls that read, resources read, prompts) a session may
    /// make in a minute, sustained; left out: no limit on reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reads_per_minute: Option<u32>,
    /// Megabytes of results a session may read in a minute; left out: no
    /// limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_mb_per_minute: Option<u32>,
}

impl AgentLimits {
    /// The limits of a workspace where the person changed none: no limit on
    /// new tasks.
    pub const DEFAULT: Self = Self {
        writes_per_minute: Some(AGENT_WRITES_PER_MINUTE),
        new_tasks_per_day: None,
        trash_items_per_call: Some(AGENT_TRASH_ITEMS_PER_CALL),
        trash_items_per_hour: Some(AGENT_TRASH_ITEMS_PER_HOUR),
        read_burst: Some(AGENT_READ_BURST),
        reads_per_minute: Some(AGENT_READS_PER_MINUTE),
        read_mb_per_minute: Some(AGENT_READ_MB_PER_MINUTE),
    };

    /// The value of one limit.
    #[must_use]
    pub const fn get(&self, limit: AgentLimit) -> Option<u32> {
        match limit {
            AgentLimit::WritesPerMinute => self.writes_per_minute,
            AgentLimit::NewTasksPerDay => self.new_tasks_per_day,
            AgentLimit::TrashItemsPerCall => self.trash_items_per_call,
            AgentLimit::TrashItemsPerHour => self.trash_items_per_hour,
            AgentLimit::ReadBurst => self.read_burst,
            AgentLimit::ReadsPerMinute => self.reads_per_minute,
            AgentLimit::ReadMbPerMinute => self.read_mb_per_minute,
        }
    }

    /// Sets one limit; `None` lifts it.
    pub const fn set(&mut self, limit: AgentLimit, value: Option<u32>) {
        match limit {
            AgentLimit::WritesPerMinute => self.writes_per_minute = value,
            AgentLimit::NewTasksPerDay => self.new_tasks_per_day = value,
            AgentLimit::TrashItemsPerCall => self.trash_items_per_call = value,
            AgentLimit::TrashItemsPerHour => self.trash_items_per_hour = value,
            AgentLimit::ReadBurst => self.read_burst = value,
            AgentLimit::ReadsPerMinute => self.reads_per_minute = value,
            AgentLimit::ReadMbPerMinute => self.read_mb_per_minute = value,
        }
    }

    /// Whether these limits let agents do more than `before` in any way: a
    /// higher number, or no limit where there was one.
    #[must_use]
    pub fn allow_more_than(&self, before: &Self) -> bool {
        AgentLimit::ALL
            .iter()
            .any(|limit| match (before.get(*limit), self.get(*limit)) {
                (Some(_), None) => true,
                (Some(old), Some(new)) => new > old,
                (None, _) => false,
            })
    }
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One of the limits on agent sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentLimit {
    /// Calls of changing tools in any minute (default 120).
    WritesPerMinute,
    /// Tasks created in any 24 hours (default: no limit).
    NewTasksPerDay,
    /// Items a standard agent moves to the trash in one call (default 20).
    TrashItemsPerCall,
    /// Items a standard agent session moves to the trash in an hour
    /// (default 50).
    TrashItemsPerHour,
    /// Reads at once before the sustained rate applies (default 60).
    ReadBurst,
    /// Reads in a minute, sustained (default 600).
    ReadsPerMinute,
    /// Megabytes read in a minute (default 8).
    ReadMbPerMinute,
}

impl AgentLimit {
    /// Every limit.
    pub const ALL: [Self; 7] = [
        Self::WritesPerMinute,
        Self::NewTasksPerDay,
        Self::TrashItemsPerCall,
        Self::TrashItemsPerHour,
        Self::ReadBurst,
        Self::ReadsPerMinute,
        Self::ReadMbPerMinute,
    ];

    /// The limit's name, as in [`AgentLimits`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WritesPerMinute => "writes_per_minute",
            Self::NewTasksPerDay => "new_tasks_per_day",
            Self::TrashItemsPerCall => "trash_items_per_call",
            Self::TrashItemsPerHour => "trash_items_per_hour",
            Self::ReadBurst => "read_burst",
            Self::ReadsPerMinute => "reads_per_minute",
            Self::ReadMbPerMinute => "read_mb_per_minute",
        }
    }
}

/// The limits on agent sessions to change; limits left out keep their
/// value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentLimitsPatch {
    /// Calls of changing tools allowed in any minute, per session, 1 to
    /// 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writes_per_minute: Option<u32>,
    /// Tasks a session may create in any 24 hours, 1 to 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_tasks_per_day: Option<u32>,
    /// Items a standard agent may move to the trash in one call, 1 to
    /// 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_items_per_call: Option<u32>,
    /// Items a standard agent session may move to the trash in an hour, 1
    /// to 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash_items_per_hour: Option<u32>,
    /// Reads a session may make at once, 1 to 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_burst: Option<u32>,
    /// Reads a session may make in a minute, sustained, 1 to 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reads_per_minute: Option<u32>,
    /// Megabytes a session may read in a minute, 1 to 1000000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_mb_per_minute: Option<u32>,
    /// Limits to lift: agents then have no limit there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_limit: Vec<AgentLimit>,
    /// Limits to set back to their default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reset: Vec<AgentLimit>,
}

impl AgentLimitsPatch {
    /// The limits after this patch, unchecked: numbers given, then limits
    /// lifted, then limits set back to their default.
    #[must_use]
    pub fn apply(&self, mut limits: AgentLimits) -> AgentLimits {
        for limit in AgentLimit::ALL {
            if let Some(value) = self.given(limit) {
                limits.set(limit, Some(value));
            }
        }
        for limit in &self.no_limit {
            limits.set(*limit, None);
        }
        for limit in &self.reset {
            limits.set(*limit, AgentLimits::DEFAULT.get(*limit));
        }
        limits
    }

    /// The number given for one limit, if any.
    #[must_use]
    pub const fn given(&self, limit: AgentLimit) -> Option<u32> {
        match limit {
            AgentLimit::WritesPerMinute => self.writes_per_minute,
            AgentLimit::NewTasksPerDay => self.new_tasks_per_day,
            AgentLimit::TrashItemsPerCall => self.trash_items_per_call,
            AgentLimit::TrashItemsPerHour => self.trash_items_per_hour,
            AgentLimit::ReadBurst => self.read_burst,
            AgentLimit::ReadsPerMinute => self.reads_per_minute,
            AgentLimit::ReadMbPerMinute => self.read_mb_per_minute,
        }
    }
}

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
    /// Reads refused because the session read faster than its limits.
    #[serde(default)]
    pub rate_limited: u64,
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
    /// Whether agents whose client can ask the person may ask to make one
    /// change beyond their profile, or one that deletes for good or
    /// deletes many items; left out: unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<bool>,
    /// Which changes of agents wait for the person's approval, per
    /// profile, and how long a proposal waits; left out: unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals: Option<crate::ops::proposal::AgentApprovals>,
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
op!(
    /// Change the limits on agent sessions.
    AgentLimitsUpdate,
    "agent.limits.update",
    AgentLimitsPatch,
    SettingsChange
);

pub(crate) const AGENT_SESSIONS: OpDescriptor = OpDescriptor::of::<AgentSessions>(Meta {
    summary: "List agent sessions, newest first, with their client, profile, times and counts of reads, writes and reads refused by the read limits.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("agents sessions"),
    input_name: "AgentSessionsInput",
    output_name: "AgentSessionList",
});

pub(crate) const AGENT_POLICY_UPDATE: OpDescriptor = OpDescriptor::of::<AgentPolicyUpdate>(Meta {
    summary: "Set the most an agent may do, whatever profile it asks for, whether agents may ask the person to allow one change beyond it, and which of their changes wait for the person's approval.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "It sets the rights of agents; an agent that could call it would raise its own rights beyond what the person allowed.",
    },
    cli: Some("agents policy"),
    input_name: "AgentPolicyPatch",
    output_name: "SettingsChange",
});

pub(crate) const AGENT_LIMITS_UPDATE: OpDescriptor = OpDescriptor::of::<AgentLimitsUpdate>(Meta {
    summary: "Change the limits on agent sessions: changes a minute, new tasks a day, items moved to the trash per call and per hour, and reads (at once, a minute, megabytes a minute). Each is a number or no limit.",
    kind: OpKind::Command,
    safety: Safety::App,
    exposure: Exposure::App {
        reason: "It sets how much agents may do; an agent that could call it would raise its own limits beyond what the person allowed.",
    },
    cli: None,
    input_name: "AgentLimitsPatch",
    output_name: "SettingsChange",
});
