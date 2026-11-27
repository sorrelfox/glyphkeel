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

/// A person, an agent, or an automation rule acting on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// The person who owns the workspace.
    User,
    /// An agent working through MCP.
    Agent,
    /// An automation rule of the workspace; the actor's name is the rule's.
    Automation,
}

impl ActorKind {
    /// The name as stored in history.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::Automation => "automation",
        }
    }

    /// The kind stored as `text`; anything unknown reads as the person.
    #[must_use]
    pub fn from_stored(text: &str) -> Self {
        match text {
            "agent" => Self::Agent,
            "automation" => Self::Automation,
            _ => Self::User,
        }
    }
}

/// Who did something.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    /// Person, agent or automation rule.
    pub kind: ActorKind,
    /// The display name ("You", "Claude Code").
    pub name: String,
}

/// The default number of items in a list.
pub const DEFAULT_LIMIT: u32 = 50;
/// The most items a list returns at once.
pub const MAX_LIMIT: u32 = 200;

/// The design's nine colours, for projects, statuses, types and labels, in
/// palette order: a new label or project without a colour gets the first one
/// its neighbours don't use yet.
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

/// An icon that stands for a project instead of its coloured circle, drawn
/// in the project's colour. The names are those of the app's icon family
/// (Lucide), so each variant's name says what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectIcon {
    Folder,
    Star,
    Bug,
    Rocket,
    Book,
    House,
    Briefcase,
    Code,
    Terminal,
    Globe,
    Heart,
    Flag,
    Zap,
    Target,
    Lightbulb,
    Wrench,
    Hammer,
    Palette,
    Music,
    Camera,
    GraduationCap,
    ShoppingCart,
    Wallet,
    ChartLine,
    Calendar,
    Users,
    Building,
    Plane,
    Leaf,
    Coffee,
    Gamepad,
    Puzzle,
    Shield,
    FlaskConical,
    Layers,
    Package,
    Compass,
    Mountain,
    Sparkles,
    Database,
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
    /// An epic: a large piece of work that holds stories.
    Epic,
    /// A story: a piece of work told from the user's side.
    Story,
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
