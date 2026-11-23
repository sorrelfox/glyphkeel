// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel-app`: the desktop app. A Tauri program that shows the web UI in
//! the system web view and hosts the engine in its own process.

// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod files;
#[cfg(target_os = "linux")]
mod instance;
mod ipc;
mod links;
mod main_window;
mod navigation;
mod spell;
mod start;
mod task_window;
mod title_bar;
#[cfg(target_os = "linux")]
mod webkit;
mod window_state;

use std::process::ExitCode;
#[cfg(target_os = "linux")]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use gk_engine::{EngineConfig, Role};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

use crate::ipc::Workspace;
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
    let built = tauri::Builder::default()
        .manage(Shown::default())
        .manage(PendingLinks(Mutex::new(first_links)))
        .manage(files::Drops::default())
        .invoke_handler(tauri::generate_handler![
            start::window_ready,
            ipc::op,
            ipc::subscribe,
            spell::spell_check,
            spell::spell_suggest,
            files::attach_files,
            files::watch_drops,
            files::attach_drop,
            files::open_file,
            files::save_file,
            links::open_link,
            task_window::open_task_window
        ])
        // Attached images, by attachment ID, for the page's <img> elements.
        .register_asynchronous_uri_scheme_protocol(files::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let method = request.method().as_str().to_owned();
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                let response = match app.try_state::<Arc<Workspace>>() {
                    Some(workspace) => files::respond(&workspace, &method, &path),
                    None => tauri::http::Response::builder()
                        .status(tauri::http::StatusCode::SERVICE_UNAVAILABLE)
                        .body(Vec::new())
                        .unwrap_or_default(),
                };
                responder.respond(response);
            });
        })
        .setup(move |app| {
            // The workspace opens before the window, so the page's first
            // calls find it.
            app.manage(Arc::new(open_workspace(app.handle())));
            // A task named by the first link opens with the window; the
            // links that arrive later wait for the part of the app that
            // handles them.
            let first_task = app
                .state::<PendingLinks>()
                .0
                .lock()
                .ok()
                .and_then(|mut links| {
                    let at = links.iter().position(|link| start::task_in_link(link).is_some())?;
                    start::task_in_link(&links.remove(at))
                });
            main_window::open(app.handle(), title_bar, first_task)?;
            #[cfg(target_os = "linux")]
            if let Some(activations) = activations {
                forward_activations(app.handle().clone(), activations);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Destroyed => {
                if let Some(workspace) = window.try_state::<Arc<Workspace>>() {
                    workspace.unsubscribe(window.label());
                }
                if let Some(drops) = window.try_state::<files::Drops>() {
                    drops.forget(window.label());
                }
            }
            WindowEvent::DragDrop(drop) => files::on_drag_drop(window, drop),
            _ => {}
        })
        .build(tauri::generate_context!());
    match built {
        Ok(app) => {
            app.run(|app, event| {
                if matches!(event, RunEvent::Exit)
                    && let Some(workspace) = app.try_state::<Arc<Workspace>>()
                {
                    workspace.close();
                }
            });
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("glyphkeel-app: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the workspace (the default one, created on the first run, unless
/// `GLYPHKEEL_WORKSPACE` names another). When that fails, the app still
/// starts, and every call of the page answers with the reason.
fn open_workspace(app: &AppHandle) -> Workspace {
    let data_dir = app.path().data_dir().ok().map(|dir| dir.join("glyphkeel"));
    let layout = match ipc::location_from_env().layout(data_dir.as_deref()) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("glyphkeel-app: {}", err.message);
            return Workspace::failed(err);
        }
    };
    let mut config = EngineConfig::new(Role::App);
    config.program_version = env!("CARGO_PKG_VERSION").to_owned();
    let workspace = Workspace::open(&layout, config, ipc::native_confirmation(app.clone()));
    if let Some(err) = workspace.failure() {
        eprintln!(
            "glyphkeel-app: cannot open the workspace at {}: {}",
            layout.root().display(),
            err.message
        );
    }
    workspace
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
