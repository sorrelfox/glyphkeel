// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel://open/<KEY>` links for the running app.
//!
//! The first link the app is started with opens with the main window (see
//! `StartData::open`). Links handed over later, by another start of the app,
//! go to the main window's page through a channel the page opens once: each
//! one names a task key, and the page opens that task. Links that arrive
//! before the page listens wait here, and are sent as soon as it does.

use std::sync::{Mutex, MutexGuard, PoisonError};

use gk_api::OpError;
use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};

use crate::start::task_in_link;

/// Task keys waiting for the main window's page, and its channel.
#[derive(Default)]
pub(crate) struct OpenLinks(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    channel: Option<Channel<String>>,
    pending: Vec<String>,
}

impl std::fmt::Debug for OpenLinks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenLinks").finish_non_exhaustive()
    }
}

/// The most links kept while the page isn't listening.
const KEPT: usize = 32;

impl OpenLinks {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hands the tasks of `links` to the page, or keeps them until it
    /// listens. Links that name no task are left out.
    pub(crate) fn push(&self, links: &[String]) {
        let mut inner = self.lock();
        for key in links.iter().filter_map(|link| task_in_link(link)) {
            inner.pending.push(key);
        }
        let excess = inner.pending.len().saturating_sub(KEPT);
        inner.pending.drain(..excess);
        flush(&mut inner);
    }

    /// Takes the page's channel (a reload replaces it) and sends what waits.
    fn listen(&self, channel: Channel<String>) {
        let mut inner = self.lock();
        inner.channel = Some(channel);
        flush(&mut inner);
    }
}

/// Sends the waiting keys in order; a key the channel refuses stays waiting,
/// with those after it.
fn flush(inner: &mut Inner) {
    let Some(channel) = &inner.channel else {
        return;
    };
    let mut sent = 0;
    for key in &inner.pending {
        if channel.send(key.clone()).is_err() {
            break;
        }
        sent += 1;
    }
    inner.pending.drain(..sent);
}

/// Hands over the main window's channel for the tasks of links that reach
/// the running app. Only the main window opens them.
#[tauri::command]
pub(crate) fn watch_open_links<R: Runtime>(
    window: WebviewWindow<R>,
    links: State<'_, OpenLinks>,
    channel: Channel<String>,
) -> Result<(), OpError> {
    if window.label() != crate::main_window::LABEL {
        return Err(crate::messages::error(
            "links-main-window-only",
            &[],
            OpError::forbidden,
        ));
    }
    links.listen(channel);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tauri::ipc::InvokeResponseBody;

    use super::*;

    fn channel() -> (Channel<String>, Arc<Mutex<Vec<String>>>) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&got);
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(json) = body else {
                panic!("keys travel as JSON");
            };
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(&json).unwrap());
            Ok(())
        });
        (channel, got)
    }

    fn links(list: &[&str]) -> Vec<String> {
        list.iter().map(|link| (*link).to_owned()).collect()
    }

    #[test]
    fn links_before_the_page_listens_wait_and_later_ones_go_straight_through() {
        let open = OpenLinks::default();
        open.push(&links(&[
            "glyphkeel://open/TM-41",
            "glyphkeel://view/TM-2",
            "glyphkeel://open/tm-3",
        ]));
        let (channel, got) = channel();
        open.listen(channel);
        assert_eq!(*got.lock().unwrap(), ["TM-41"]);
        open.push(&links(&["glyphkeel://open/GD-7/"]));
        assert_eq!(*got.lock().unwrap(), ["TM-41", "GD-7"]);
        assert!(open.lock().pending.is_empty());
    }

    #[test]
    fn only_the_latest_links_are_kept_while_nobody_listens() {
        let open = OpenLinks::default();
        let many: Vec<String> = (1..=40)
            .map(|n| format!("glyphkeel://open/TM-{n}"))
            .collect();
        open.push(&many);
        let pending = open.lock().pending.clone();
        assert_eq!(pending.len(), KEPT);
        assert_eq!(pending.first().map(String::as_str), Some("TM-9"));
        assert_eq!(pending.last().map(String::as_str), Some("TM-40"));
    }
}
