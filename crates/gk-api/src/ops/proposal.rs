// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Changes agents propose and the person approves or rejects later.
//!
//! When the workspace says an agent's changes need approval, a changing
//! call of that agent is not run: it is kept as a proposal, with the
//! operation, its input, the agent session and a preview of what it would
//! change. The person approves it (it then runs as the agent, and history
//! says the person approved it) or rejects it with a reason the agent can
//! read.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;
use crate::ops::history::{ActivityTask, FieldChange};
use crate::profile::Profile;

/// Days a proposal waits for the person before it expires, unless the
/// person set another number.
pub const PROPOSAL_EXPIRE_DAYS: u32 = 14;
/// The most days a proposal can be set to wait.
pub const PROPOSAL_EXPIRE_DAYS_MAX: u32 = 365;
/// The most proposals one agent session may have waiting at once.
pub const PROPOSALS_PENDING_MAX: u32 = 500;
/// The longest reason for a rejection, in characters.
pub const REJECT_REASON_MAX: usize = 2000;

/// Which of an agent's changes wait for the person's approval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// None: the agent's profile decides alone.
    #[default]
    Off,
    /// Changes that move items to the trash or delete for good.
    Destructive,
    /// Every change.
    AllWrites,
}

impl ApprovalMode {
    /// Whether a change of class `safety` waits for approval in this mode.
    #[must_use]
    pub const fn covers(self, safety: Safety) -> bool {
        match self {
            Self::Off => false,
            Self::Destructive => matches!(safety, Safety::Trash | Safety::Permanent),
            Self::AllWrites => matches!(safety, Safety::Write | Safety::Trash | Safety::Permanent),
        }
    }

    /// The name as written in settings and on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Destructive => "destructive",
            Self::AllWrites => "all_writes",
        }
    }
}

/// Which changes of agents wait for the person's approval, per profile,
/// and how long a proposal waits. Only the person sets it, in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentApprovals {
    /// For read-only agents: `off` (their changes are refused) or
    /// `all_writes` (they may propose the changes a standard agent makes).
    pub read_only: ApprovalMode,
    /// For standard agents.
    pub standard: ApprovalMode,
    /// For full agents.
    pub full: ApprovalMode,
    /// Days a proposal waits before it expires, 1 to 365.
    pub expire_days: u32,
}

impl AgentApprovals {
    /// No approvals: what a workspace has until the person turns them on.
    pub const DEFAULT: Self = Self {
        read_only: ApprovalMode::Off,
        standard: ApprovalMode::Off,
        full: ApprovalMode::Off,
        expire_days: PROPOSAL_EXPIRE_DAYS,
    };

    /// The mode for agents working with `profile`.
    #[must_use]
    pub const fn mode(&self, profile: Profile) -> ApprovalMode {
        match profile {
            Profile::ReadOnly => self.read_only,
            Profile::Standard => self.standard,
            Profile::Full => self.full,
        }
    }
}

impl Default for AgentApprovals {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Where a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Waiting for the person.
    Pending,
    /// The person approved it and it is running now. One that stays so
    /// (`interrupted`) was stopped while it ran: whether it changed
    /// anything shows in the activity log.
    Approving,
    /// The person approved it and it ran.
    Approved,
    /// The person rejected it.
    Rejected,
    /// Nobody decided in time.
    Expired,
}

impl ProposalStatus {
    /// The name as stored and written in JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approving => "approving",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

/// One item a proposal would change, with each field before and after, as
/// the preview found it when the agent proposed it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposedChange {
    /// What kind of item.
    pub entity: String,
    /// The item's ID; for an item the change creates, the ID the preview
    /// gave it (the real one differs).
    pub id: String,
    /// The item's project, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The changed fields.
    pub fields: BTreeMap<String, FieldChange>,
}

/// A field that changed since the agent proposed the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalConflict {
    /// What kind of item.
    pub entity: String,
    /// The item's ID.
    pub id: String,
    /// The field.
    pub field: String,
    /// Its value when the agent proposed the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<Value>,
    /// Its value now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<Value>,
}

/// A change an agent proposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// The proposal's ID.
    pub id: String,
    /// The operation it runs (`task.update`).
    pub op: String,
    /// What the operation does, in one sentence.
    pub summary: String,
    /// The operation's input, as the agent sent it.
    pub input: Value,
    /// Where it stands.
    pub status: ProposalStatus,
    /// When the agent proposed it (RFC 3339, UTC).
    pub created_at: String,
    /// When it expires while nobody decides.
    pub expires_at: String,
    /// The agent.
    pub agent: Actor,
    /// The agent session it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The profile the agent asked for.
    pub profile: Profile,
    /// The project of what it changes, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The task it changes, as it is now; left out when it changes no
    /// task or the task is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActivityTask>,
    /// What it would change, as the preview found it.
    pub changes: Vec<ProposedChange>,
    /// Whether the change could be previewed: changes that read or write
    /// files outside the workspace are kept without a preview.
    pub previewed: bool,
    /// For a waiting proposal: the fields that changed since, which
    /// approving would overwrite. Empty when nothing changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ProposalConflict>,
    /// When the person decided, or when it expired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
    /// Who decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<Actor>,
    /// Why the person rejected it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The history group of the approved change, for revert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// For one being approved: when the approval started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    /// For one being approved: whether the program running it stopped
    /// before it ended (it has run for more than ten minutes). It is never
    /// run again by itself; the person can only reject it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interrupted: bool,
    /// Why the last approval failed, when it went back to waiting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Which proposals to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalListInput {
    /// Only proposals with this status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ProposalStatus>,
    /// Only proposals of this agent session (an ID, or `current` for an
    /// agent's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Only these proposals, by ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// At most this many, newest first (default 50, at most 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Proposals, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalList {
    /// The proposals.
    pub items: Vec<Proposal>,
    /// How many match in all.
    pub total: u64,
    /// How many of the caller's proposals wait for the person: an agent's
    /// own, or every agent's for the person.
    pub pending: u64,
}

/// One proposal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalGetInput {
    /// The proposal's ID.
    pub id: String,
}

/// The proposals to approve: by ID, or every waiting one of a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalApproveInput {
    /// The proposals, by ID, run in the order the agent proposed them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// Every waiting proposal of this agent session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Approve although what it changes changed since it was proposed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

/// What became of one proposal the person approved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalOutcome {
    /// The proposal, after the approval.
    pub proposal: Proposal,
    /// Whether it ran.
    pub approved: bool,
    /// Why it didn't run: it changed since (`conflict`, the proposal's
    /// `conflicts` say what) or the change failed now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::error::OpError>,
}

/// The proposals approved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalApproveResult {
    /// One per proposal, in the order they ran.
    pub items: Vec<ProposalOutcome>,
    /// How many ran.
    pub approved: u64,
}

/// The proposals to reject: by ID, or every waiting one of a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalRejectInput {
    /// The proposals, by ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// Every waiting proposal of this agent session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Why, for the agent to read; at most 2000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The proposals rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalRejectResult {
    /// The proposals, after the rejection.
    pub items: Vec<Proposal>,
}

op!(
    /// List proposed changes.
    ProposalListOp,
    "proposal.list",
    ProposalListInput,
    ProposalList
);
op!(
    /// One proposed change.
    ProposalGet,
    "proposal.get",
    ProposalGetInput,
    Proposal
);
op!(
    /// Approve proposed changes.
    ProposalApprove,
    "proposal.approve",
    ProposalApproveInput,
    ProposalApproveResult
);
op!(
    /// Reject proposed changes.
    ProposalReject,
    "proposal.reject",
    ProposalRejectInput,
    ProposalRejectResult
);

pub(crate) const PROPOSAL_LIST: OpDescriptor = OpDescriptor::of::<ProposalListOp>(Meta {
    summary: "List changes agents proposed, newest first, with their status, a preview of what each changes, the fields changed since and the person's decision. An agent sees its own.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("proposals list"),
    input_name: "ProposalListInput",
    output_name: "ProposalList",
});

pub(crate) const PROPOSAL_GET: OpDescriptor = OpDescriptor::of::<ProposalGet>(Meta {
    summary: "One proposed change: what it runs, its preview, the fields changed since and the decision.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("proposals show <id>"),
    input_name: "ProposalGetInput",
    output_name: "Proposal",
});

pub(crate) const PROPOSAL_APPROVE: OpDescriptor = OpDescriptor::of::<ProposalApprove>(Meta {
    summary: "Approve proposed changes, by ID or every waiting one of an agent session: each runs as the agent, and history records that the person approved it.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "The person decides on what agents propose; an agent that could call it would approve its own changes.",
    },
    cli: Some("proposals approve <ids>"),
    input_name: "ProposalApproveInput",
    output_name: "ProposalApproveResult",
});

pub(crate) const PROPOSAL_REJECT: OpDescriptor = OpDescriptor::of::<ProposalReject>(Meta {
    summary: "Reject proposed changes, by ID or every waiting one of an agent session, with a reason the agent can read.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::App {
        reason: "The person decides on what agents propose; an agent that could call it would decide on proposals instead of the person.",
    },
    cli: Some("proposals reject <ids>"),
    input_name: "ProposalRejectInput",
    output_name: "ProposalRejectResult",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_cover_classes() {
        use Safety::*;
        let table = [
            (ApprovalMode::Off, [false, false, false, false]),
            (ApprovalMode::Destructive, [false, false, true, true]),
            (ApprovalMode::AllWrites, [false, true, true, true]),
        ];
        for (mode, want) in table {
            let got = [Read, Write, Trash, Permanent].map(|s| mode.covers(s));
            assert_eq!(got, want, "{}", mode.as_str());
            assert!(!mode.covers(App));
        }
    }
}
