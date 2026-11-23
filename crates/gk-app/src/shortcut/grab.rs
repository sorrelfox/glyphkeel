// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The quick-add shortcut as a key grab on the X server, through Tauri's
//! global-shortcut plugin: on X11, and on KDE's Wayland session, where the
//! compositor passes key combinations on to XWayland even while a Wayland
//! window has the focus.
//!
//! The plugin is added only when this route is taken: it connects to the X
//! server as it starts, which a Wayland session without XWayland doesn't
//! have.

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcut, GlobalShortcutExt, ShortcutState};

/// Grabs `keys` (`CTRL+ALT+SPACE`); pressing them opens the quick-add window.
pub(crate) fn register(app: &AppHandle, keys: &str) -> Result<(), String> {
    if app.try_state::<GlobalShortcut<Wry>>().is_none() {
        app.plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .map_err(|err| err.to_string())?;
    }
    app.global_shortcut()
        .on_shortcut(keys, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || crate::quick_add::show(&handle, None));
            }
        })
        .map_err(|err| err.to_string())
}

/// Lets go of `keys`.
pub(crate) fn release(app: &AppHandle, keys: &str) {
    if app.try_state::<GlobalShortcut<Wry>>().is_none() {
        return;
    }
    if let Err(err) = app.global_shortcut().unregister(keys) {
        eprintln!("glyphkeel-app: cannot let go of the quick-add key: {err}");
    }
}
