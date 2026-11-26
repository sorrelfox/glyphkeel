// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The quick-add window: one line to capture a task from anywhere.
//!
//! It is made the first time it is asked for, and shown once its page has
//! drawn: a window waiting hidden from the start would keep one more web
//! process running for someone who never uses it. The shortcut, the tray's
//! Quick add, the page's Quick add action and a start with `--quick-add`
//! show it; Enter (after creating the task) and Escape hide it again, and
//! the desktop gives the focus back to the window that had it. From then on
//! it is hidden, never closed, while the app runs, so later shows are
//! instant.
//!
//! Each time it is shown it is mapped anew: on Wayland a window that is
//! shown again may take the focus, where raising an open one may not.
//! With an activation token (from the portal's shortcut) it uses that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use gk_api::OpError;
use serde_json::{Value, json};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};

use crate::app_settings::AppSettings;
use crate::background::BackgroundMode;
use crate::page;
use crate::start::{self, StartData};
use crate::title_bar::TitleBar;

/// The window's label. Its own capability file names it.
pub(crate) const LABEL: &str = "quick-add";

/// The window's size: the field, the project and the recognised date and
/// priority below it, with room for longer translations.
const SIZE: (f64, f64) = (600.0, 136.0);
/// How long the page may take to draw before a show happens anyway.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// The window's state while the app runs.
#[derive(Default)]
pub(crate) struct QuickAdd {
    /// The page has drawn its first screen.
    ready: AtomicBool,
    /// Asked to show before the page was ready.
    pending: Mutex<Option<Option<String>>>,
    /// The page's channel: told each time the window shows.
    channel: Mutex<Option<Channel<Value>>>,
}

impl std::fmt::Debug for QuickAdd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickAdd")
            .field("ready", &self.ready.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// Makes the window, hidden; [`show`] shows it once its page has drawn.
/// Call on the main thread.
fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let start = StartData {
        title_bar: TitleBar::System,
        locale: start::locale_from_env(),
        task: None,
        wiki: None,
        open: None,
        prefs: app.try_state::<AppSettings>().map(|settings| settings.ui()),
        quick_add: true,
        first_run: false,
    };
    if let Some(state) = app.try_state::<QuickAdd>() {
        state.ready.store(false, Ordering::SeqCst);
    }
    let window = page::window(app, LABEL, &start)
        .title("Glyphkeel")
        .inner_size(SIZE.0, SIZE.1)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .visible(false)
        .build()?;
    #[cfg(target_os = "linux")]
    window.with_webview(|webview| crate::webkit::restrict(&webview.inner()))?;
    Ok(window)
}

/// Shows the window and gives it the focus, making it on first use. `token` is an activation token from the desktop. Call on the main
/// thread.
pub(crate) fn show(app: &AppHandle, token: Option<String>) {
    let state = app.state::<QuickAdd>();
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => match create(app) {
            Ok(window) => window,
            Err(err) => {
                eprintln!("glyphkeel-app: cannot make the quick-add window: {err}");
                return;
            }
        },
    };
    if !state.ready.load(Ordering::SeqCst) {
        // Shown as soon as its page has drawn, or after a while anyway, so
        // that a broken page can't swallow the shortcut.
        let waiting = state
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(token)
            .is_some();
        if !waiting {
            show_if_page_stays_silent(app);
        }
        return;
    }
    present(&window, token.as_deref());
    // The page empties its field and takes the current appearance.
    let appearance = app
        .try_state::<AppSettings>()
        .and_then(|settings| settings.ui().get("appearance").cloned())
        .unwrap_or(Value::Null);
    if let Some(channel) = state
        .channel
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        let _ = channel.send(json!({ "appearance": appearance }));
    }
}

/// Shows the window if its page hasn't signalled within the timeout.
fn show_if_page_stays_silent(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_TIMEOUT);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let pending = handle
                .state::<QuickAdd>()
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let (Some(token), Some(window)) = (pending, handle.get_webview_window(LABEL)) {
                eprintln!("glyphkeel-app: the quick-add page did not signal; showing it anyway");
                present(&window, token.as_deref());
            }
        });
    });
}

/// Maps the window anew and focuses it.
fn present<R: Runtime>(window: &WebviewWindow<R>, token: Option<&str>) {
    #[cfg(target_os = "linux")]
    if let Ok(gtk_window) = window.gtk_window() {
        use gtk::prelude::*;

        if let Some(token) = token {
            gtk_window.set_startup_id(token);
        }
        if gtk_window.is_visible() && !gtk_window.is_active() {
            gtk_window.hide();
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = token;
    for result in [window.show(), window.set_focus()] {
        if let Err(err) = result {
            eprintln!("glyphkeel-app: cannot show the quick-add window: {err}");
        }
    }
}

/// The page has drawn its first screen; a show that waited happens now.
pub(crate) fn on_ready<R: Runtime>(window: &WebviewWindow<R>) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<QuickAdd>() else {
        return;
    };
    state.ready.store(true, Ordering::SeqCst);
    let pending = state
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(token) = pending {
        present(window, token.as_deref());
    }
}

/// Whether the app keeps running with the windows `open` (label, visible):
/// in background mode always; otherwise while a window is open other than
/// the quick-add one waiting hidden.
pub(crate) fn keeps_running<'a>(
    open: impl IntoIterator<Item = (&'a str, bool)>,
    background: bool,
) -> bool {
    background
        || open
            .into_iter()
            .any(|(label, visible)| label != LABEL || visible)
}

/// Ends the app when only the hidden quick-add window is left and
/// background mode is off: closing the main window would otherwise leave a
/// process without a window to see. `gone` is a window being destroyed.
pub(crate) fn exit_if_idle(app: &AppHandle, gone: Option<&str>) {
    let background = app.state::<BackgroundMode>().enabled();
    let windows = app.webview_windows();
    let open = windows
        .iter()
        .filter(|(label, _)| Some(label.as_str()) != gone)
        .map(|(label, window)| (label.as_str(), window.is_visible().unwrap_or(false)));
    if !keeps_running(open, background) {
        app.exit(0);
    }
}

/// Shows the quick-add window: the page's Quick add action.
#[tauri::command]
pub(crate) fn show_quick_add(app: AppHandle) {
    show(&app, None);
}

/// Hides the quick-add window: Enter after the task is made, or Escape.
#[tauri::command]
pub(crate) fn hide_quick_add(app: AppHandle, window: WebviewWindow) -> Result<(), OpError> {
    if window.label() != LABEL {
        return Err(OpError::forbidden(
            "Only the quick-add window hides itself.",
        ));
    }
    if let Err(err) = window.hide() {
        eprintln!("glyphkeel-app: cannot hide the quick-add window: {err}");
    }
    exit_if_idle(&app, None);
    Ok(())
}

/// Takes the quick-add page's channel: it hears each time the window shows.
#[tauri::command]
pub(crate) fn watch_quick_add(
    window: WebviewWindow,
    state: State<'_, QuickAdd>,
    channel: Channel<Value>,
) -> Result<(), OpError> {
    if window.label() != LABEL {
        return Err(OpError::forbidden(
            "Only the quick-add window listens here.",
        ));
    }
    *state.channel.lock().unwrap_or_else(PoisonError::into_inner) = Some(channel);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LABEL, keeps_running};

    #[test]
    fn the_hidden_quick_add_window_alone_doesnt_keep_the_app_running() {
        assert!(!keeps_running([(LABEL, false)], false));
        assert!(!keeps_running([], false));
        assert!(keeps_running([(LABEL, false), ("main", true)], false));
        // A hidden main window (before its first screen) still counts.
        assert!(keeps_running([("main", false)], false));
        assert!(keeps_running([("task-TM-41", true), (LABEL, false)], false));
        // While someone types in it, it keeps the app too.
        assert!(keeps_running([(LABEL, true)], false));
        // Background mode keeps it running for reminders and the shortcut.
        assert!(keeps_running([(LABEL, false)], true));
        assert!(keeps_running([], true));
    }

    #[test]
    fn the_quick_add_window_has_a_capability_of_its_own() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/quick-add.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!([LABEL]));
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        assert!(
            !main["windows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w == LABEL)
        );
    }
}
