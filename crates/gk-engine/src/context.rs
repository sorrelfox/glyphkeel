// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Who calls an operation, through which program, with which rights.

use std::fmt;
use std::sync::Arc;

use gk_api::ops::common::{Actor, ActorKind, Via};
use gk_api::{OpDescriptor, Profile};
use gk_core::Id;

/// The caller of an operation. Adapters build one per call (the app and
/// the command line) or per session (MCP).
#[derive(Debug, Clone)]
pub struct Context {
    /// The person or the agent.
    pub actor: Actor,
    /// The program the call came through.
    pub via: Via,
    /// The agent session, for agents.
    pub session: Option<Id>,
    /// The profile an agent asked for; the workspace setting caps it. `None`
    /// for the person.
    pub profile: Option<Profile>,
    /// The adapter's request ID, for logs only.
    pub request_id: Option<String>,
    /// How a `permanent` operation is confirmed by the person.
    pub confirm: Confirmation,
    /// The project (key or ID) the caller is limited to, with its
    /// sub-projects: other projects don't appear in lists or searches and
    /// can't be named or changed. `None`: the whole workspace.
    pub project_scope: Option<String>,
}

impl Context {
    /// The person, through `via`. Permanent operations are refused until
    /// [`Context::confirm`] says how to confirm them.
    #[must_use]
    pub fn user(via: Via) -> Self {
        Self {
            actor: Actor {
                kind: ActorKind::User,
                name: "You".to_owned(),
            },
            via,
            session: None,
            profile: None,
            request_id: None,
            confirm: Confirmation::Refuse,
            project_scope: None,
        }
    }

    /// An agent through MCP, in `session`, asking for `profile`.
    #[must_use]
    pub fn agent(name: impl Into<String>, session: Id, profile: Profile) -> Self {
        Self {
            actor: Actor {
                kind: ActorKind::Agent,
                name: name.into(),
            },
            via: Via::Mcp,
            session: Some(session),
            profile: Some(profile),
            request_id: None,
            confirm: Confirmation::Refuse,
            project_scope: None,
        }
    }

    /// The same caller with another way to confirm permanent operations.
    #[must_use]
    pub fn with_confirmation(mut self, confirm: Confirmation) -> Self {
        self.confirm = confirm;
        self
    }

    /// The same caller limited to `project` (a key or ID) and its
    /// sub-projects.
    #[must_use]
    pub fn within_project(mut self, project: impl Into<String>) -> Self {
        self.project_scope = Some(project.into());
        self
    }

    /// Whether the caller is an agent.
    #[must_use]
    pub fn is_agent(&self) -> bool {
        self.actor.kind == ActorKind::Agent
    }
}

/// What a confirmation dialog shows.
#[derive(Debug, Clone)]
pub struct ConfirmRequest {
    /// The operation.
    pub op: &'static OpDescriptor,
    /// Its input, as given.
    pub input: serde_json::Value,
}

/// How the person confirms a `permanent` operation. Agents don't confirm:
/// their profile decides.
#[derive(Clone, Default)]
pub enum Confirmation {
    /// Permanent operations are refused (`forbidden`).
    #[default]
    Refuse,
    /// The person confirmed before the call, for example with `--yes` on
    /// the command line.
    Given,
    /// Ask now. The app shows a native dialog from Rust, so the page can't
    /// answer it; the call waits for the answer.
    Ask(Arc<dyn Fn(&ConfirmRequest) -> bool + Send + Sync>),
}

impl fmt::Debug for Confirmation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Refuse => "Refuse",
            Self::Given => "Given",
            Self::Ask(_) => "Ask",
        })
    }
}
