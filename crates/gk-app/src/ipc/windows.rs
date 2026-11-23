// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The change-event channels of the open windows, one per window.
//!
//! A channel needs no Tauri permission, unlike `emit`, and Tauri delivers
//! its messages in the order they are sent. One thread forwards the
//! engine's events to every channel, so every window sees them in commit
//! order.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gk_api::Changed;
use tauri::ipc::Channel;

/// The channels, by window label.
#[derive(Default)]
pub(crate) struct Windows {
    channels: Mutex<HashMap<String, Channel<Changed>>>,
}

impl std::fmt::Debug for Windows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Windows")
            .field("windows", &self.lock().keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Windows {
    /// Sets the channel of window `label`. Returns whether it replaced one:
    /// the page was reloaded.
    pub(crate) fn subscribe(&self, label: &str, channel: Channel<Changed>) -> bool {
        self.lock().insert(label.to_owned(), channel).is_some()
    }

    /// Forgets the channel of a window that went away.
    pub(crate) fn unsubscribe(&self, label: &str) {
        self.lock().remove(label);
    }

    /// Sends `event` to every window. A channel that fails (its page is
    /// gone) is dropped; the page subscribes again when it loads.
    pub(crate) fn send(&self, event: &Changed) {
        self.lock()
            .retain(|label, channel| match channel.send(event.clone()) {
                Ok(()) => true,
                Err(err) => {
                    eprintln!("glyphkeel-app: change events to window {label} stopped: {err}");
                    false
                }
            });
    }

    /// Forwards every event from `events` until the engine closes.
    pub(crate) fn forward(self: &Arc<Self>, events: Receiver<Changed>) {
        let windows = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("gk-app-events".into())
            .spawn(move || {
                for event in events {
                    windows.send(&event);
                }
            });
        if let Err(err) = spawned {
            eprintln!("glyphkeel-app: cannot start the change-event thread: {err}");
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Channel<Changed>>> {
        self.channels.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
