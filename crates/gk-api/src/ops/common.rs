// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Types shared by several operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The program a change came through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// The desktop app.
    App,
    /// The `glyphkeel` command.
    Cli,
    /// An agent through `glyphkeel mcp`.
    Mcp,
}

impl Via {
    /// The name as stored in history.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Cli => "cli",
            Self::Mcp => "mcp",
        }
    }
}

/// A person or an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// The person who owns the workspace.
    User,
    /// An agent working through MCP.
    Agent,
}

impl ActorKind {
    /// The name as stored in history.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

/// Who did something.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    /// Person or agent.
    pub kind: ActorKind,
    /// The display name ("You", "Claude Code").
    pub name: String,
}

/// The default number of items in a list.
pub const DEFAULT_LIMIT: u32 = 50;
/// The most items a list returns at once.
pub const MAX_LIMIT: u32 = 200;

/// The design's nine colours, for projects, statuses, types and labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Hue {
    /// Red.
    Red,
    /// Orange.
    Orange,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Teal.
    Teal,
    /// Blue.
    Blue,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Grey.
    Grey,
}

/// The shape that shows a task's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Glyph {
    /// A plain task.
    Task,
    /// A bug.
    Bug,
    /// A feature.
    Feature,
    /// A milestone.
    Milestone,
}

/// What a status means for the work in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Not started.
    Todo,
    /// Being worked on.
    InProgress,
    /// Finished; closes the task.
    Done,
    /// Dropped; closes the task.
    Cancelled,
}

/// A task's priority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// No priority (the default).
    #[default]
    None,
    /// Low.
    Low,
    /// Normal.
    Normal,
    /// High.
    High,
    /// Urgent.
    Urgent,
}

/// The two ends of an ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// First.
    Top,
    /// Last.
    Bottom,
}
