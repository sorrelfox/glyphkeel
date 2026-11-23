// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel-app`: the desktop app. A Tauri program that shows the web UI in
//! the system web view and hosts the engine in its own process.

// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod navigation;
#[cfg(target_os = "linux")]
mod webkit;

use std::process::ExitCode;

use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Runtime, WebviewUrl, WebviewWindowBuilder};

/// The label of the main window. The capability file names it too.
const MAIN_WINDOW: &str = "main";

fn main() -> ExitCode {
    let result = tauri::Builder::default()
        .setup(|app| {
            open_main_window(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("glyphkeel-app: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the main window. It starts hidden and is shown once the page has
/// loaded, so the user never sees an empty white frame.
fn open_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Glyphkeel")
        .inner_size(1280.0, 800.0)
        .min_inner_size(760.0, 520.0)
        // The UI draws its own header bar.
        .decorations(false)
        .visible(false)
        // Nothing the page stores outlives the process.
        .incognito(true)
        .zoom_hotkeys_enabled(false)
        // The page never leaves the app and never opens windows of its own.
        .on_navigation(navigation::is_app_url)
        .on_new_window(|_url, _features| NewWindowResponse::Deny)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished
                && let Err(err) = window.show()
            {
                eprintln!("glyphkeel-app: cannot show the main window: {err}");
            }
        })
        .build()?;

    #[cfg(target_os = "linux")]
    window.with_webview(|webview| webkit::restrict(&webview.inner()))?;
    #[cfg(not(target_os = "linux"))]
    let _ = window;

    Ok(())
}
