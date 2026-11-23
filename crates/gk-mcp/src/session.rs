// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Who the agent is, and its session: one run of `glyphkeel mcp` is one
//! agent session, recorded in the workspace with the client's name and
//! version, the profile, the start and end time and the counts of reads and
//! writes.

use std::sync::Mutex;

use gk_api::{OpError, Profile};
use gk_core::Id;
use gk_engine::{Context, Engine, SessionStart};

/// The name used when neither the client nor `--agent-name` gives one.
pub(crate) const FALLBACK_NAME: &str = "Agent";

/// Display names of well-known MCP clients, by the name they send in
/// `clientInfo`.
const KNOWN_CLIENTS: &[(&str, &str)] = &[
    ("claude-code", "Claude Code"),
    ("claude-ai", "Claude Desktop"),
    ("codex-mcp-client", "Codex"),
    ("codex", "Codex"),
    ("cursor-vscode", "Cursor"),
    ("cursor", "Cursor"),
    ("gemini-cli-mcp-client", "Gemini CLI"),
    ("gemini-cli", "Gemini CLI"),
    ("visual studio code", "VS Code"),
    ("visual studio code - insiders", "VS Code"),
    ("inspector-client", "MCP Inspector"),
    ("mcp-inspector", "MCP Inspector"),
    ("jetbrains-ai-assistant-client", "JetBrains AI Assistant"),
];

/// What the client says about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo {
    /// Its own ID (`claude-code`).
    pub name: String,
    /// Its version.
    pub version: String,
    /// A name for people, when it sends one.
    pub title: Option<String>,
}

/// The display name for a client: a known client's name, else its title,
/// else the ID it sent.
#[must_use]
pub fn display_name(client: &ClientInfo) -> String {
    let id = client.name.trim();
    let lower = id.to_lowercase();
    if let Some((_, name)) = KNOWN_CLIENTS.iter().find(|(known, _)| *known == lower) {
        return (*name).to_owned();
    }
    client
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(id)
        .to_owned()
}

/// The running agent session. It starts on the first request, once the
/// client has said who it is, and ends when the input closes.
#[derive(Debug)]
pub struct Session {
    profile: Profile,
    agent_name: Option<String>,
    state: Mutex<Option<Started>>,
}

#[derive(Debug, Clone)]
struct Started {
    id: Id,
    name: String,
}

impl Session {
    /// A session not yet started, for an agent asking for `profile`, named
    /// `agent_name` when its client sends no name.
    #[must_use]
    pub fn new(profile: Profile, agent_name: Option<String>) -> Self {
        Self {
            profile,
            agent_name: agent_name.filter(|n| !n.trim().is_empty()),
            state: Mutex::new(None),
        }
    }

    /// The profile the agent asked for.
    #[must_use]
    pub fn requested_profile(&self) -> Profile {
        self.profile
    }

    /// The agent's context, starting the session on first use. The name
    /// comes from `client` (the first one seen wins), else from
    /// `--agent-name`.
    ///
    /// # Errors
    /// When the session can't be recorded.
    pub fn context(
        &self,
        engine: &Engine,
        client: Option<&ClientInfo>,
    ) -> Result<Context, OpError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let started = match &*state {
            Some(s) => s.clone(),
            None => {
                let name = client
                    .map(display_name)
                    .filter(|n| !n.is_empty())
                    .or_else(|| self.agent_name.clone())
                    .unwrap_or_else(|| FALLBACK_NAME.to_owned());
                let id = engine.start_session(&SessionStart {
                    name: name.clone(),
                    client_id: client.map(|c| c.name.clone()).filter(|n| !n.is_empty()),
                    client_version: client.map(|c| c.version.clone()).filter(|v| !v.is_empty()),
                    profile: self.profile,
                })?;
                tracing::info!(session = %id, agent = %name, profile = %self.profile, "agent session started");
                let started = Started { id, name };
                *state = Some(started.clone());
                started
            }
        };
        Ok(Context::agent(started.name, started.id, self.profile))
    }

    /// A context for the server's own reads (the profile cap for the tool
    /// list): the same agent, outside the session, so its reads aren't
    /// counted as the agent's.
    #[must_use]
    pub fn housekeeping_context(&self) -> Context {
        let mut ctx = Context::agent(FALLBACK_NAME, Id::generate(), self.profile);
        ctx.session = None;
        ctx
    }

    /// The session's ID, once started.
    #[must_use]
    pub fn id(&self) -> Option<Id> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|s| s.id)
    }

    /// Ends the session, if it started.
    pub fn end(&self, engine: &Engine) {
        if let Some(id) = self.id() {
            match engine.end_session(id) {
                Ok(()) => tracing::info!(session = %id, "agent session ended"),
                Err(e) => {
                    tracing::warn!(session = %id, error = %e, "ending the agent session failed")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(name: &str, title: Option<&str>) -> ClientInfo {
        ClientInfo {
            name: name.to_owned(),
            version: "1.0".to_owned(),
            title: title.map(str::to_owned),
        }
    }

    #[test]
    fn known_clients_get_their_display_name() {
        assert_eq!(display_name(&client("claude-code", None)), "Claude Code");
        assert_eq!(display_name(&client("Visual Studio Code", None)), "VS Code");
    }

    #[test]
    fn unknown_clients_use_their_title_then_their_id() {
        assert_eq!(display_name(&client("my-bot", Some("My Bot"))), "My Bot");
        assert_eq!(display_name(&client("my-bot", Some("  "))), "my-bot");
        assert_eq!(display_name(&client("my-bot", None)), "my-bot");
    }
}
