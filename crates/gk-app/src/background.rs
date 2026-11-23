// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Background mode and the tray icon.
//!
//! With background mode on, closing the last window keeps the process (and
//! with it the reminders, the timer's reminder and the background jobs)
//! running. The windows are destroyed, not hidden, so their web content
//! processes and their memory are freed; the tray's "Open" or a new start
//! of the app creates the main window again, with current data.
//!
//! The tray icon is a StatusNotifierItem through libappindicator, which
//! gives a menu but no click events. Without a tray, as on GNOME without
//! the extension, background mode still works: starting the app again
//! brings its window back.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};

use crate::main_window;
use crate::open_links::OpenLinks;
use crate::start::Shown;
use crate::title_bar::TitleBar;

/// Whether closing the last window keeps the app running.
#[derive(Debug)]
pub(crate) struct BackgroundMode(AtomicBool);

impl BackgroundMode {
    /// The switch in Preferences, once it has been set; until then on
    /// unless `GLYPHKEEL_BACKGROUND` is `0` or `off`.
    pub(crate) fn new(setting: Option<bool>) -> Self {
        Self(AtomicBool::new(setting.unwrap_or_else(|| {
            parse_switch(std::env::var("GLYPHKEEL_BACKGROUND").ok().as_deref())
        })))
    }

    pub(crate) fn enabled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn set(&self, on: bool) {
        self.0.store(on, Ordering::SeqCst);
    }
}

/// The tray icon, while background mode is on and the desktop has a tray.
#[derive(Default)]
pub(crate) struct Tray(Mutex<Option<TrayIcon>>);

/// Puts the tray icon up when background mode is on. Call on the main thread.
pub(crate) fn start(app: &AppHandle) {
    if app.state::<BackgroundMode>().enabled() {
        show_tray(app);
    }
}

/// Switches background mode on or off while the app runs, with its tray
/// icon. Call on the main thread.
pub(crate) fn switch(app: &AppHandle, on: bool) {
    app.state::<BackgroundMode>().set(on);
    if on {
        show_tray(app);
    } else {
        let tray = app.state::<Tray>();
        let mut icon = tray.0.lock().unwrap_or_else(PoisonError::into_inner);
        if icon.take().is_some() {
            let _ = app.remove_tray_by_id(TRAY_ID);
        }
    }
}

fn show_tray(app: &AppHandle) {
    let tray = app.state::<Tray>();
    let mut icon = tray.0.lock().unwrap_or_else(PoisonError::into_inner);
    if icon.is_some() {
        return;
    }
    match build_tray(app) {
        // The icon stays as long as the app keeps it.
        Ok(built) => *icon = Some(built),
        Err(err) => eprintln!("glyphkeel-app: no tray icon: {err}"),
    }
}

/// The variable's value: anything but `0`, `off`, `false` or `no` is on.
pub(crate) fn parse_switch(value: Option<&str>) -> bool {
    !matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("0" | "off" | "false" | "no")
    )
}

/// The title bar the main window opens with, kept for opening it again.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MainTitleBar(pub(crate) TitleBar);

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";
// English until the Rust side reads the message files the page uses.
const OPEN_LABEL: &str = "Open Glyphkeel";
const QUIT_LABEL: &str = "Quit";
const TOOLTIP: &str = "Glyphkeel";

/// Puts the app's icon in the tray, with Open and Quit.
fn build_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let open = MenuItem::with_id(app, OPEN_ID, OPEN_LABEL, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_ID, QUIT_LABEL, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(TOOLTIP)
        .title(TOOLTIP)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_ID => show_main(app, None, None),
            QUIT_ID => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

/// Brings the main window forward, creating it again if it was closed;
/// opens task `task` in it when given. `token` is an activation token from
/// the launcher or the notification server. Call on the main thread.
pub(crate) fn show_main(app: &AppHandle, task: Option<String>, token: Option<String>) {
    if let Some(window) = app.get_webview_window(main_window::LABEL) {
        if let Some(task) = task {
            app.state::<OpenLinks>()
                .push(&[format!("glyphkeel://open/{task}")]);
        }
        // Before the page's first screen the window isn't shown yet; it
        // comes forward by itself when it is.
        if app.state::<Shown>().0.load(Ordering::SeqCst) {
            main_window::activate(&window, token.as_deref());
        }
        return;
    }
    let title_bar = app
        .try_state::<MainTitleBar>()
        .map_or_else(TitleBar::from_env, |kept| kept.0);
    app.state::<Shown>().0.store(false, Ordering::SeqCst);
    if let Err(err) = main_window::open(app, title_bar, task) {
        eprintln!("glyphkeel-app: cannot open the main window: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::parse_switch;

    #[test]
    fn background_mode_is_on_unless_switched_off() {
        assert!(parse_switch(None));
        assert!(parse_switch(Some("1")));
        assert!(parse_switch(Some("")));
        for off in ["0", "off", "OFF", "false", "no", " no "] {
            assert!(!parse_switch(Some(off)), "{off}");
        }
    }
}
