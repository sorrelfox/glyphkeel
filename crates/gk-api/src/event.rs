// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Change events: what the engine publishes after every commit, its own or
//! another program's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ops::common::{Actor, Via};

/// One committed command. Events carry no content: a view re-reads what it
/// shows, so there is one source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Changed {
    /// The highest history sequence number of the command.
    pub seq: i64,
    /// The history group: what a revert or Undo names.
    pub group: String,
    /// Who made the change, and through which program.
    pub origin: Origin,
    /// One entry per changed item.
    pub changes: Vec<EntityChange>,
}

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    /// The program: the app, the command line or MCP.
    pub via: Via,
    /// The person or the agent.
    pub actor: Actor,
    /// The agent session, for changes made through MCP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// One changed item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityChange {
    /// What kind of item (`task`, `project`, `settings`, …).
    pub entity: String,
    /// Its ID.
    pub id: String,
    /// The project it belongs to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The fields that changed.
    pub fields: Vec<String>,
}
