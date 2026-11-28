// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The engine: opens, locks and migrates workspaces, executes operations,
//! keeps the activity log, undo, redo and revert, publishes change events, and
//! owns files, backups, export, import and background jobs.
//!
//! Every adapter (the app, `glyphkeel`, `glyphkeel mcp`) opens an [`Engine`]
//! and runs operations through [`Engine::call`] with a [`Context`] saying
//! who is calling. Nothing else writes to the workspace.

mod agent_files;
pub mod backup;
mod context;
pub mod encryption;
mod engine;
mod events;
mod failure;
mod file_text;
mod flow;
mod jobs;
pub mod language;
mod messages;
mod ops;
mod registry;
mod scope;
mod thumbs;
mod time;
pub mod transfer;
pub mod workspaces;
mod write;

pub use agent_files::AgentFolders;
pub use context::{Approval, ConfirmRequest, Confirmation, Context};
pub use engine::{Engine, EngineConfig, Role, SessionStart};
pub use failure::{Failure, store_error};
pub use file_text::FileIndexTurn;
pub use jobs::JobsReport;
pub use messages::{Arg as MessageArg, ENGINE as ENGINE_MESSAGES, MESSAGES_FTL, Messages};
pub use registry::{Env, QueryCtx, Registry};
pub use time::{
    Clock, ZoneChoice, check_time_zone, personal_time_zone, rfc3339, system_clock, time_zones,
};
pub use write::WriteCtx;
