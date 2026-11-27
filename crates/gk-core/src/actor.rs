// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Who made a change and through which program.
//!
//! There are no user accounts: a change is made by the person at the
//! computer ("Me"), by a named agent, or by one of the workspace's
//! automation rules, through the app, the command line or the agent server.

use std::fmt;
use std::str::FromStr;

use crate::kinds::KindError;

/// Who made a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    /// The person at this computer.
    User,
    /// An agent, identified by its name and session.
    Agent,
    /// An automation rule, identified by its name.
    Automation,
}

/// The program a change came through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Via {
    /// The desktop app.
    App,
    /// The `glyphkeel` command line.
    Cli,
    /// The agent server (`glyphkeel mcp`).
    Mcp,
}

impl ActorKind {
    /// The stored name: `user` or `agent`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::Automation => "automation",
        }
    }
}

impl Via {
    /// The stored name: `app`, `cli` or `mcp`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Cli => "cli",
            Self::Mcp => "mcp",
        }
    }
}

impl FromStr for ActorKind {
    type Err = KindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            "automation" => Ok(Self::Automation),
            _ => Err(KindError::new(
                "kind of actor",
                s,
                &["user", "agent", "automation"],
            )),
        }
    }
}

impl FromStr for Via {
    type Err = KindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "app" => Ok(Self::App),
            "cli" => Ok(Self::Cli),
            "mcp" => Ok(Self::Mcp),
            _ => Err(KindError::new("program", s, &["app", "cli", "mcp"])),
        }
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for Via {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for a in [ActorKind::User, ActorKind::Agent, ActorKind::Automation] {
            assert_eq!(a.as_str().parse::<ActorKind>(), Ok(a));
        }
        for v in [Via::App, Via::Cli, Via::Mcp] {
            assert_eq!(v.as_str().parse::<Via>(), Ok(v));
        }
        assert!("me".parse::<ActorKind>().is_err());
        assert!("web".parse::<Via>().is_err());
    }
}
