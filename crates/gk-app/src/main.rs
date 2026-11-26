// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `glyphkeel-app`: the desktop app. A Tauri program that shows the web UI in
//! the system web view and hosts the engine in its own process.

// Release builds on Windows have no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_settings;
mod autostart;
mod background;
mod cli_tool;
mod files;
mod graphics;
mod instance;
mod ipc;
mod legal;
mod links;
mod main_window;
#[cfg(target_os = "linux")]
mod memory_env;
mod messages;
mod navigation;
#[cfg(target_os = "linux")]
mod notifications;
mod open_links;
mod page;
mod platform;
mod quick_add;
mod reminders;
mod shortcut;
mod spell;
mod start;
mod task_window;
mod title_bar;
#[cfg(any(windows, test))]
mod toasts;
mod transfer;
#[cfg(target_os = "linux")]
mod webkit;
mod wiki_window;
mod window_state;
mod workspaces;

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
    // Safe rendering, as kept in the settings or asked for on the command
    // line, and the graphics driver's workarounds, before any web view.
    #[cfg(target_os = "linux")]
    {
        let safe = graphics::asked_on_command_line()
            || app_settings::file().is_some_and(|file| app_settings::load(&file).safe_rendering);
        memory_env::restart_with_settings(&graphics::settings(graphics::Session::detect(), safe));
    }
    // Started with the system: in background mode only the tray shows.
    let background_start = std::env::args().any(|arg| arg == autostart::BACKGROUND_ARG);
    #[cfg(target_os = "linux")]
    let (activations, _bus, first_links, first_quick_add) = {
        let this_start = instance::this_start();
        match instance::claim(&this_start) {
            instance::Claim::Primary { bus, activations } => (
                Some(activations),
                Some(bus),
                this_start.links,
                this_start.quick_add,
            ),
            // The running app has the request; this process has nothing to do.
            instance::Claim::Forwarded => return ExitCode::SUCCESS,
            instance::Claim::Unavailable(err) => {
                eprintln!(
                    "glyphkeel-app: no session bus, so a second start opens a second window: {err}"
                );
                (None, None, this_start.links, this_start.quick_add)
            }
        }
    };
    // On Windows a later start is handed over by the single-instance plugin,
    // registered first below.
    #[cfg(not(target_os = "linux"))]
    let (first_links, first_quick_add) = {
        let this_start = instance::this_start();
        (this_start.links, this_start.quick_add)
    };

    let title_bar = TitleBar::from_env();
    let app_settings = AppSettings::load();
    let background_mode = BackgroundMode::new(app_settings.stored().background);
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.plugin(platform::windows::single_instance());
    let built = builder
        .manage(Shown::default())
        .manage(open_links::OpenLinks::default())
        .manage(files::Drops::default())
        .manage(background_mode)
        .manage(background::Tray::default())
        .manage(app_settings)
        .manage(MainTitleBar(title_bar))
        .manage(quick_add::QuickAdd::default())
        .manage(app_settings::Restart::default())
        .manage(Arc::new(transfer::ImportChoices::default()))
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
            transfer::export_archive,
            transfer::export_wiki_page,
            transfer::choose_import,
            transfer::choose_wiki_folder,
            transfer::import_wiki_folder,
            transfer::import_archive,
            transfer::choose_csv,
            transfer::import_csv,
            transfer::choose_backup_folder,
            links::open_link,
            open_links::watch_open_links,
            task_window::open_task_window,
            wiki_window::open_wiki_window,
            app_settings::app_settings,
            app_settings::update_app_settings,
            app_settings::add_agent_folder,
            app_settings::remove_agent_folder,
            app_settings::set_ui_pref,
            app_settings::time_zones,
            app_settings::set_autostart,
            app_settings::restart_app,
            quick_add::show_quick_add,
            quick_add::hide_quick_add,
            quick_add::watch_quick_add,
            shortcut::quick_add_shortcut,
            shortcut::set_quick_add_shortcut,
            cli_tool::cli_status,
            cli_tool::install_cli,
            workspaces::workspaces,
            workspaces::create_workspace,
            workspaces::open_workspace,
            workspaces::rename_workspace,
            quit_app,
            legal::legal_text
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
            // The quick-add window is made when it is first asked for, so
            // that its web process isn't running before then.
            // Started for quick add, the app shows only that window.
            if first_quick_add && first_task.is_none() {
                quick_add::show(app.handle(), None);
            } else if background_start
                && first_task.is_none()
                && app.state::<BackgroundMode>().enabled()
            {
                // Started with the system: the tray and the reminders only,
                // until the person opens the window.
            } else {
                main_window::open(app.handle(), title_bar, first_task)?;
            }
            shortcut::start(app.handle());
            #[cfg(target_os = "linux")]
            if let Some(activations) = activations {
                forward_activations(app.handle().clone(), activations);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // The quick-add window is hidden, never closed (Alt+F4, say).
            WindowEvent::CloseRequested { api, .. } if window.label() == quick_add::LABEL => {
                api.prevent_close();
                let _ = window.hide();
                quick_add::exit_if_idle(window.app_handle(), None);
            }
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
                // The hidden quick-add window alone doesn't keep the app.
                if window.label() != quick_add::LABEL {
                    quick_add::exit_if_idle(window.app_handle(), Some(window.label()));
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
                    #[cfg(target_os = "linux")]
                    if app
                        .state::<app_settings::Restart>()
                        .0
                        .load(Ordering::SeqCst)
                    {
                        let err = memory_env::start_again();
                        eprintln!("glyphkeel-app: cannot start again: {err}");
                    }
                    // A restart from Preferences: let go of the single
                    // instance first, or the new run would hand itself over
                    // to this one and exit.
                    #[cfg(not(target_os = "linux"))]
                    if app
                        .state::<app_settings::Restart>()
                        .0
                        .load(Ordering::SeqCst)
                    {
                        #[cfg(windows)]
                        tauri_plugin_single_instance::destroy(app);
                        if let Err(err) = graphics::start_again() {
                            eprintln!("glyphkeel-app: cannot start again: {err}");
                        }
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
/// `GLYPHKEEL_WORKSPACE` names another or the person last opened another). When that fails, the app still
/// starts, and every call of the page answers with the reason.
fn open_workspace(app: &AppHandle) -> Workspace {
    let mut config = EngineConfig::new(Role::App);
    config.program_version = env!("CARGO_PKG_VERSION").to_owned();
    // The person's own zone, shared with the command line and the agent
    // server; an unknown one falls back to the system's.
    config.time_zone = gk_engine::personal_time_zone(None, &gk_store::UserSettings::load_default())
        .unwrap_or_default();
    let layout = match ipc::location_from_env().layout(workspaces::data_dir(app).as_deref()) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("glyphkeel-app: {}", err.message);
            return Workspace::failed_with(
                err,
                Some(config),
                ipc::native_confirmation(app.clone()),
            );
        }
    };
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
/// arrive (on Linux; the single-instance plugin does it on Windows).
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

/// Does what a later start of the app asks for. Call on the main thread.
#[cfg(any(target_os = "linux", windows))]
fn on_activation(app: &AppHandle, activation: instance::Activation) {
    if activation.quick_add {
        quick_add::show(app, activation.token);
        return;
    }
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

/// Where reminders show on Windows: toasts, whose click brings the task
/// forward in the main window.
#[cfg(windows)]
fn notifier(app: &AppHandle) -> Arc<dyn reminders::Notifier> {
    let handle = app.clone();
    let on_open: toasts::OnOpen = Box::new(move |task| {
        let app = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            background::show_main(&app, Some(task), None);
        });
    });
    let system = platform::windows::Toasts::new(app.config().identifier.clone());
    Arc::new(toasts::ToastNotifier::new(system, on_open))
}

#[cfg(not(any(target_os = "linux", windows)))]
fn notifier(_app: &AppHandle) -> Arc<dyn reminders::Notifier> {
    Arc::new(LogNotifier)
}

/// Reminders without a notification server: written to the log.
#[cfg(not(windows))]
struct LogNotifier;

#[cfg(not(windows))]
impl reminders::Notifier for LogNotifier {
    fn show(&self, notice: &reminders::Notice) {
        eprintln!("glyphkeel-app: reminder: {}: {}", notice.title, notice.body);
    }
}
