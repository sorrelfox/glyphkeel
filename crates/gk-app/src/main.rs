// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel-app`: the desktop app. A Tauri program that shows the web UI in
//! the system web view and hosts the engine in its own process.

// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_settings;
mod background;
mod files;
#[cfg(target_os = "linux")]
mod instance;
mod ipc;
mod links;
mod main_window;
mod navigation;
#[cfg(target_os = "linux")]
mod notifications;
mod open_links;
mod page;
mod reminders;
mod spell;
mod start;
mod task_window;
mod title_bar;
#[cfg(target_os = "linux")]
mod webkit;
mod window_state;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use gk_engine::{EngineConfig, Role};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

use crate::app_settings::AppSettings;
use crate::background::{BackgroundMode, MainTitleBar};
use crate::ipc::Workspace;
use crate::start::Shown;
use crate::title_bar::TitleBar;

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
    let app_settings = AppSettings::load();
    let background_mode = BackgroundMode::new(app_settings.stored().background);
    let built = tauri::Builder::default()
        .manage(Shown::default())
        .manage(open_links::OpenLinks::default())
        .manage(files::Drops::default())
        .manage(background_mode)
        .manage(background::Tray::default())
        .manage(app_settings)
        .manage(MainTitleBar(title_bar))
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
            files::save_export,
            links::open_link,
            open_links::watch_open_links,
            task_window::open_task_window,
            app_settings::app_settings,
            app_settings::update_app_settings,
            app_settings::set_ui_pref,
            app_settings::time_zones,
            quit_app
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
            // Reminders show whether or not a window is open, as
            // notifications while Preferences has them on.
            let notifier = app.state::<AppSettings>().switched(notifier(app.handle()));
            app.state::<Arc<Workspace>>().start_reminders(notifier);
            background::start(app.handle());
            // A task named by the first link opens with the window; the
            // others go to the page once it listens.
            let mut first_links = first_links;
            let first_task = first_links
                .iter()
                .position(|link| start::task_in_link(link).is_some())
                .and_then(|at| start::task_in_link(&first_links.remove(at)));
            app.state::<open_links::OpenLinks>().push(&first_links);
            main_window::open(app.handle(), title_bar, first_task)?;
            #[cfg(target_os = "linux")]
            if let Some(activations) = activations {
                forward_activations(app.handle().clone(), activations);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Destroyed => {
                // A main window opened again shows on its page's first screen.
                if window.label() == main_window::LABEL {
                    window.state::<Shown>().0.store(false, Ordering::SeqCst);
                }
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
            app.run(|app, event| match event {
                // The last window closed: in background mode the app keeps
                // running for reminders. Quitting asks with an exit code.
                RunEvent::ExitRequested {
                    code: None, api, ..
                } if app.state::<BackgroundMode>().enabled() => api.prevent_exit(),
                RunEvent::Exit => {
                    if let Some(workspace) = app.try_state::<Arc<Workspace>>() {
                        workspace.close();
                    }
                }
                _ => {}
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
    // The person's own zone, shared with the command line and the agent
    // server; an unknown one falls back to the system's.
    config.time_zone = gk_engine::personal_time_zone(None, &gk_store::UserSettings::load_default())
        .unwrap_or_default();
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
    // The page opens the tasks the links name, once it listens.
    app.state::<open_links::OpenLinks>().push(&activation.links);
    // The window comes forward, or back after background mode closed it.
    background::show_main(app, None, activation.token);
}

/// Ends the app, also in background mode: the page's Quit.
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Where reminders show: the desktop's notification server, whose "Open"
/// brings the task forward in the main window.
#[cfg(target_os = "linux")]
fn notifier(app: &AppHandle) -> Arc<dyn reminders::Notifier> {
    let handle = app.clone();
    let on_open: notifications::OnOpen = Box::new(move |task, token| {
        let app = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            background::show_main(&app, Some(task), token);
        });
    });
    match notifications::DesktopNotifier::connect(on_open) {
        Ok(notifier) => Arc::new(notifier),
        Err(err) => {
            eprintln!("glyphkeel-app: no desktop notifications, reminders go to the log: {err}");
            Arc::new(LogNotifier)
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn notifier(_app: &AppHandle) -> Arc<dyn reminders::Notifier> {
    Arc::new(LogNotifier)
}

/// Reminders without a notification server: written to the log.
struct LogNotifier;

impl reminders::Notifier for LogNotifier {
    fn show(&self, notice: &reminders::Notice) {
        eprintln!("glyphkeel-app: reminder: {}: {}", notice.title, notice.body);
    }
}
