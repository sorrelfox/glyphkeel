// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The workspace as a whole.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::agent::AgentLimits;
use crate::ops::common::{Actor, Via};
use crate::ops::settings::Settings;
use crate::profile::Profile;

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGetInput {}

/// The open workspace and who is asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceInfo {
    /// The workspace's ID.
    pub id: String,
    /// The workspace's name (its folder name).
    pub name: String,
    /// The schema version of its database.
    pub schema_version: i64,
    /// The version of the program answering.
    pub program_version: String,
    /// Who is asking, through which program, with which rights.
    pub caller: Caller,
    /// The workspace settings.
    pub settings: Settings,
    /// The limits on agent sessions enforced now, as in the settings; a
    /// limit left out is lifted.
    pub agent_limits: AgentLimits,
    /// For an agent: the folders whose files it may name by path, to attach,
    /// import or export, as absolute paths. The first is the workspace's
    /// `exports` folder, where a plain file name goes; the others are the
    /// ones the person added. Paths elsewhere are refused. Left out for the
    /// person, whose paths aren't limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_folders: Option<Vec<String>>,
    /// The timer, while one runs or is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<crate::ops::time::Timer>,
    /// A short guide to keys, references and the query language.
    pub guide: String,
}

/// The caller of an operation as the engine sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Caller {
    /// The person or the agent.
    pub actor: Actor,
    /// The program.
    pub via: Via,
    /// The agent's profile, after the workspace's cap; left out for the
    /// person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Profile>,
    /// The agent session; left out for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// For an agent: which of its changes wait for the person's approval
    /// (`off`, `destructive` or `all_writes`); such a change returns
    /// `pending_approval` with the proposal's ID. Left out for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvals: Option<crate::ops::proposal::ApprovalMode>,
}

op!(
    /// The workspace, its settings and the caller.
    WorkspaceGet,
    "workspace.get",
    WorkspaceGetInput,
    WorkspaceInfo
);

pub(crate) const WORKSPACE_GET: OpDescriptor = OpDescriptor::of::<WorkspaceGet>(Meta {
    summary: "The open workspace: its name and ID, the schema and program versions, who is asking with which rights, the settings, the limits on agents and a short guide.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("workspace"),
    input_name: "WorkspaceGetInput",
    output_name: "WorkspaceInfo",
});
