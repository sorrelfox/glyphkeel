// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The page's way to the engine: the `op` command, which runs any operation
//! of the catalog, and `subscribe`, which hands the app a channel for the
//! change events of one window.
//!
//! Both are declared in `build.rs` and granted in the capability file;
//! nothing else of the workspace is reachable from the page.

mod confirm;
mod location;
mod windows;
mod workspace;

use std::sync::Arc;

use gk_api::{Changed, OpError};
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};

pub(crate) use confirm::native as native_confirmation;
pub(crate) use location::from_env as location_from_env;
pub(crate) use workspace::Workspace;

/// Runs operation `op` with its JSON `input` for the person, through the
/// app. A `permanent` operation first shows the native confirmation, here
/// in Rust, so script in the page can't answer it. `request_id` only goes
/// into the logs.
#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn op(
    workspace: State<'_, Arc<Workspace>>,
    op: String,
    input: Option<Value>,
    request_id: Option<String>,
) -> Result<Value, OpError> {
    let input = input.unwrap_or(Value::Null);
    crate::transfer::page_may_run(&op, &input)?;
    let workspace = Arc::clone(&workspace);
    // The engine blocks (transactions, and the confirmation dialog), so it
    // runs on a blocking thread, never on the main thread.
    let answer =
        tauri::async_runtime::spawn_blocking(move || workspace.call(&op, input, request_id))
            .await
            .map_err(|e| OpError::storage(format!("The operation stopped unexpectedly: {e}.")))?;
    crate::heap::worked();
    answer
}

/// Hands over the channel for this window's change events. A reload of the
/// page subscribes again and replaces the old channel; the answer says
/// whether it did, and the page refetches what it shows either way.
#[tauri::command]
pub(crate) fn subscribe<R: Runtime>(
    window: WebviewWindow<R>,
    workspace: State<'_, Arc<Workspace>>,
    channel: Channel<Changed>,
) -> bool {
    workspace.subscribe(window.label(), channel)
}

#[cfg(test)]
mod tests;
