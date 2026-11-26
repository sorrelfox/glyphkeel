// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The main window: created hidden with the remembered geometry and the
//! chosen title bar, shown by the page's first commit, and brought forward
//! when the app is started again.

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::page;
use crate::start::{self, Shown, StartData};
use crate::title_bar::TitleBar;
use crate::window_state::{self, MIN_SIZE};

/// The label of the main window. The capability file names it too.
pub(crate) const LABEL: &str = "main";

/// How long the page may take to signal its first commit before the window
/// is shown anyway, so that a broken page can't leave an invisible process.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the main window, hidden until the page has committed its first
/// screen.
/// `first_task` is a task to open first, from a link the app was started with.
pub(crate) fn open(
    app: &AppHandle,
    title_bar: TitleBar,
    first_task: Option<String>,
) -> tauri::Result<WebviewWindow> {
    let state_file = window_state::file();
    let geometry = state_file
        .as_deref()
        .map(window_state::load)
        .unwrap_or_default();
    let start = StartData {
        title_bar,
        locale: start::locale_from_env(),
        task: None,
        wiki: None,
        open: first_task,
        prefs: app
            .try_state::<crate::app_settings::AppSettings>()
            .map(|settings| settings.ui()),
        quick_add: false,
        first_run: app
            .try_state::<std::sync::Arc<crate::ipc::Workspace>>()
            .is_some_and(|workspace| workspace.created()),
    };

    let window = page::window(app, LABEL, &start)
        .title("Glyphkeel")
        .inner_size(geometry.width, geometry.height)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .maximized(geometry.maximized)
        // With the app's header bar, GTK keeps its client-side frame and only
        // its title bar is replaced (below); with the system title bar, the
        // window manager draws the frame.
        .decorations(true)
        .visible(false)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                show_if_page_stays_silent(&window);
            }
        })
        .build()?;

    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::*;

        window.with_webview(|webview| crate::webkit::restrict(&webview.inner()))?;
        let gtk_window = window.gtk_window()?;
        if title_bar == TitleBar::App {
            crate::title_bar::hide_gtk_title_bar(&gtk_window);
        }
        if let Some((x, y)) = geometry.position {
            gtk_window.move_(x, y);
        }
        if let Some(path) = state_file {
            let store = window_state::track::follow(&gtk_window, geometry, path);
            // However the window goes (its close button, the window manager,
            // the app quitting), GTK destroys it.
            gtk_window.connect_destroy(move |_| store());
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = state_file;

    Ok(window)
}

/// Shows the window if the page hasn't signalled within the timeout.
fn show_if_page_stays_silent(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_TIMEOUT);
        let shown = app.state::<Shown>();
        if shown.0.load(Ordering::SeqCst) {
            return;
        }
        eprintln!(
            "glyphkeel-app: the page did not signal its first screen; showing the window anyway"
        );
        let handle = app.clone();
        let result = app.run_on_main_thread(move || {
            if let Some(window) = handle.get_webview_window(LABEL) {
                start::show(&window, &handle.state::<Shown>());
            }
        });
        if let Err(err) = result {
            eprintln!("glyphkeel-app: cannot show the main window: {err}");
        }
    });
}

/// Brings the window forward for a later start of the app.
///
/// With an activation token from the launcher, the compositor lets the
/// window take focus. Without one, Wayland compositors may refuse to raise a
/// minimized window; unmapping and mapping it again works there, as a new
/// window is allowed to take focus.
pub(crate) fn activate(window: &WebviewWindow, token: Option<&str>) {
    #[cfg(target_os = "linux")]
    if let Ok(gtk_window) = window.gtk_window() {
        use gtk::prelude::*;

        match token {
            Some(token) => gtk_window.set_startup_id(token),
            None if !gtk_window.is_active() => {
                gtk_window.hide();
                gtk_window.show();
            }
            None => {}
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = token;

    for result in [window.unminimize(), window.show(), window.set_focus()] {
        if let Err(err) = result {
            eprintln!("glyphkeel-app: cannot bring the main window forward: {err}");
        }
    }
}
