// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The desktop shell on Windows, through Tauri's plugins and the system
//! dialogs of `rfd`:
//!
//! - one running app: a second start hands its arguments over and exits;
//! - reminders as toasts, whose click opens the task;
//! - links and attached files opened in their default programs;
//! - the system's file and folder choosers;
//! - the task dialog that confirms what can't be undone.
//!
//! The toasts carry the app's ID, which the installer also gives the
//! Start-menu shortcut: Windows shows toasts only for an app it knows.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc;

use gk_api::OpError;
use rfd::{
    AsyncFileDialog, AsyncMessageDialog, FileHandle, MessageButtons, MessageDialogResult,
    MessageLevel,
};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, WebviewWindow, Wry};

use crate::files::{Choose, Mode, Spec};
use crate::ipc::MessageBox;
use crate::messages::text;
use crate::toasts::{OPEN_ACTION, OnClick, Toast, ToastSystem};
use crate::{instance, main_window};

// ---------- one running app ----------

/// The plugin that keeps one app running. Register it before any other: a
/// second start hands its arguments to the running app while Tauri sets up,
/// lets that app's window come to the front, and exits before it opens one.
pub(crate) fn single_instance() -> TauriPlugin<Wry> {
    tauri_plugin_single_instance::init(|app, command_line, _cwd| {
        // On the main thread, from the plugin's message window.
        if let Some(activation) = instance::later_start(&command_line) {
            crate::on_activation(app, activation);
        }
    })
}

// ---------- toasts ----------

/// The system's toasts, as the app with ID `app_id`.
#[derive(Debug)]
pub(crate) struct Toasts {
    app_id: String,
}

impl Toasts {
    pub(crate) fn new(app_id: String) -> Self {
        Self { app_id }
    }
}

impl ToastSystem for Toasts {
    fn show(&self, toast: &Toast, on_click: OnClick) -> Result<(), String> {
        let mut shown = tauri_winrt_notification::Toast::new(&self.app_id)
            .title(&toast.title)
            .text1(&toast.body);
        if let Some(open) = &toast.open {
            shown = shown.add_button(open, OPEN_ACTION);
        }
        shown
            .on_activated(move |action| {
                on_click(action);
                Ok(())
            })
            .show()
            .map_err(|err| err.to_string())
    }
}

// ---------- links and files ----------

/// Opens a web or mail address in the default browser or mail program.
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|err| err.to_string())
}

/// Opens file `path` in the default program for its type.
pub(crate) fn open_path(path: &Path) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| err.to_string())
}

/// What Windows writes on a downloaded file: it came from the internet.
const FROM_THE_INTERNET: &str = "[ZoneTransfer]\r\nZoneId=3\r\n";

/// Marks the file at `path` as downloaded, the way browsers do, so that
/// Windows and the program that opens it handle it with the same care
/// (Office's Protected View, SmartScreen). Where the file system has no
/// room for the mark, the file opens without it.
pub(crate) fn mark_as_downloaded(path: &Path) {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    if let Err(err) = std::fs::write(&stream, FROM_THE_INTERNET) {
        eprintln!(
            "glyphkeel-app: cannot mark {} as downloaded: {err}",
            path.display()
        );
    }
}

// ---------- file choosers ----------

/// Shows the system's file or folder chooser over `window`, and waits for the
/// answer. Cancel gives no paths.
pub(crate) async fn choose(window: &WebviewWindow, mode: Mode) -> Result<Vec<PathBuf>, OpError> {
    let (answer, answered) = mpsc::channel();
    let parent = window.clone();
    let spec = mode.spec();
    window
        .run_on_main_thread(move || {
            let chosen = chooser(&parent, spec);
            std::thread::spawn(move || {
                let _ = answer.send(tauri::async_runtime::block_on(chosen));
            });
        })
        .map_err(|e| OpError::storage(format!("The file chooser can't be shown: {e}.")))?;
    crate::files::blocking(move || Ok(answered.recv().unwrap_or_default())).await
}

type Chosen = Pin<Box<dyn Future<Output = Vec<PathBuf>> + Send>>;

/// The chooser for `spec`, shown at once; its answer comes later.
fn chooser(parent: &WebviewWindow, spec: Spec) -> Chosen {
    let mut dialog = AsyncFileDialog::new()
        .set_title(text(spec.title, &[]))
        .set_parent(parent);
    if spec.zip_only {
        dialog = dialog.add_filter(text("chooser-zip-filter", &[]), &["zip"]);
    }
    if let Some(name) = spec.name {
        dialog = dialog.set_file_name(name);
    }
    fn path(handle: FileHandle) -> PathBuf {
        handle.path().to_path_buf()
    }
    match spec.choose {
        Choose::Files => {
            let chosen = dialog.pick_files();
            Box::pin(async move {
                chosen
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(path)
                    .collect()
            })
        }
        Choose::File => {
            let chosen = dialog.pick_file();
            Box::pin(async move { chosen.await.into_iter().map(path).collect() })
        }
        // The system's save dialog asks before it replaces a file.
        Choose::Save => {
            let chosen = dialog.save_file();
            Box::pin(async move { chosen.await.into_iter().map(path).collect() })
        }
        Choose::Folder => {
            let chosen = dialog.pick_folder();
            Box::pin(async move { chosen.await.into_iter().map(path).collect() })
        }
    }
}

// ---------- the confirmation ----------

/// The system's task dialog, over the main window.
#[derive(Debug)]
pub(crate) struct TaskDialog {
    app: AppHandle,
}

impl TaskDialog {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl MessageBox for TaskDialog {
    fn show(&self, title: &str, body: &str, buttons: [&str; 2]) -> Option<String> {
        let (answer, answered) = mpsc::channel();
        let app = self.app.clone();
        let [first, second] = buttons.map(str::to_owned);
        let mut dialog = AsyncMessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(body)
            // The first button is the default one.
            .set_buttons(MessageButtons::OkCancelCustom(first, second));
        // The window is reached on the main thread. The call waits on its
        // own thread, so the main loop keeps running while the dialog is open.
        let shown = self.app.run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window(main_window::LABEL) {
                dialog = dialog.set_parent(&window);
            }
            let pressed = dialog.show();
            std::thread::spawn(move || {
                let _ = answer.send(tauri::async_runtime::block_on(pressed));
            });
        });
        if shown.is_err() {
            // The event loop has ended: the app is quitting.
            return None;
        }
        // Each custom button answers with its label; Escape, closing the
        // dialog and a failure answer otherwise.
        match answered.recv() {
            Ok(MessageDialogResult::Custom(label)) => Some(label),
            _ => None,
        }
    }
}
