// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The engine: opens, locks and migrates workspaces, executes operations,
//! keeps the activity log, undo, redo and revert, publishes change events, and
//! owns files, backups, export, import and background jobs.
//!
//! Every adapter (the app, `glyphkeel`, `glyphkeel mcp`) opens an [`Engine`]
//! and runs operations through [`Engine::call`] with a [`Context`] saying
//! who is calling. Nothing else writes to the workspace.

pub mod backup;
mod context;
mod engine;
mod events;
mod failure;
mod jobs;
mod messages;
mod ops;
mod registry;
mod thumbs;
mod time;
pub mod transfer;
mod write;

pub use context::{ConfirmRequest, Confirmation, Context};
pub use engine::{Engine, EngineConfig, Role, SessionStart};
pub use failure::{Failure, store_error};
pub use jobs::JobsReport;
pub use messages::{ENGINE as ENGINE_MESSAGES, MESSAGES_FTL, Messages};
pub use registry::{Env, QueryCtx, Registry};
pub use time::{
    Clock, ZoneChoice, check_time_zone, personal_time_zone, rfc3339, system_clock, time_zones,
};
pub use write::WriteCtx;
