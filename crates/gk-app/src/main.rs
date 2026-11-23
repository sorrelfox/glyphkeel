// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel-app`: the desktop app. A Tauri program that shows the web UI in
//! the system web view and hosts the engine in its own process.

// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod instance;
mod main_window;
mod navigation;
mod start;
mod title_bar;
#[cfg(target_os = "linux")]
mod webkit;
mod window_state;

use std::process::ExitCode;
use std::sync::Mutex;
#[cfg(target_os = "linux")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "linux")]
use tauri::{AppHandle, Manager};

use crate::start::Shown;
use crate::title_bar::TitleBar;

/// Links the app was asked to open, waiting for the part of the app that
/// opens them.
#[derive(Debug, Default)]
struct PendingLinks(Mutex<Vec<String>>);

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    let (activations, _bus, first_links) = {
        let this_start = instance::this_start();
        match instance::claim(&this_start) {
            instance::Claim::Primary { bus, activations } => {
                (Some(activations), Some(bus), this_start.links)
            }
            // The running app has the request; this process has nothing to do.
            instance::Claim::Forwarded => return ExitCode::SUCCESS,
            instance::Claim::Unavailable(err) => {
                eprintln!(
                    "glyphkeel-app: no session bus, so a second start opens a second window: {err}"
                );
                (None, None, this_start.links)
            }
        }
    };
    #[cfg(not(target_os = "linux"))]
    let first_links = Vec::new();

    let title_bar = TitleBar::from_env();
    let result = tauri::Builder::default()
        .manage(Shown::default())
        .manage(PendingLinks(Mutex::new(first_links)))
        .invoke_handler(tauri::generate_handler![start::window_ready])
        .setup(move |app| {
            main_window::open(app.handle(), title_bar)?;
            #[cfg(target_os = "linux")]
            if let Some(activations) = activations {
                forward_activations(app.handle().clone(), activations);
            }
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

/// Handles later starts of the app on the main thread, in the order they
/// arrive.
#[cfg(target_os = "linux")]
fn forward_activations(
    app: AppHandle,
    activations: std::sync::mpsc::Receiver<instance::Activation>,
) {
    std::thread::spawn(move || {
        for activation in activations {
            let handle = app.clone();
            let result = app.run_on_main_thread(move || on_activation(&handle, activation));
            if result.is_err() {
                // The event loop has ended: the app is quitting.
                break;
            }
        }
    });
}

#[cfg(target_os = "linux")]
fn on_activation(app: &AppHandle, activation: instance::Activation) {
    if !activation.links.is_empty()
        && let Ok(mut pending) = app.state::<PendingLinks>().0.lock()
    {
        pending.extend(activation.links);
    }
    // Before the page's first screen the window isn't shown yet; it comes
    // forward by itself when it is.
    if !app.state::<Shown>().0.load(Ordering::SeqCst) {
        return;
    }
    if let Some(window) = app.get_webview_window(main_window::LABEL) {
        main_window::activate(&window, activation.token.as_deref());
    }
}
