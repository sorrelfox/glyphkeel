// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The native confirmation of `permanent` operations.
//!
//! The engine asks inside the call, before the transaction, and the call
//! waits for the answer. The dialog is GTK's own, shown by Rust over the
//! window: script in the page can neither see nor answer it, and no answer
//! the page passes in can stand in for it.

use std::sync::Arc;

use gk_engine::{ConfirmRequest, Confirmation};
use tauri::AppHandle;

/// Asks the person in a native dialog over the main window.
pub(crate) fn native(app: AppHandle) -> Confirmation {
    Confirmation::Ask(Arc::new(move |request| ask(&app, request)))
}

#[cfg(target_os = "linux")]
fn ask(app: &AppHandle, request: &ConfirmRequest) -> bool {
    let (answer, answered) = std::sync::mpsc::channel();
    let handle = app.clone();
    let summary = request.op.summary;
    // GTK lives on the main thread. The call waits on its own blocking
    // thread, so the main loop keeps running while the dialog is open.
    let shown = app.run_on_main_thread(move || dialog::show(&handle, summary, answer));
    if shown.is_err() {
        // The event loop has ended: the app is quitting.
        return false;
    }
    // A dialog that goes away without an answer counts as Cancel.
    answered.recv().unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
fn ask(_app: &AppHandle, _request: &ConfirmRequest) -> bool {
    // No native dialog on this platform yet: refuse rather than guess.
    false
}

#[cfg(target_os = "linux")]
mod dialog {
    use std::cell::RefCell;
    use std::sync::mpsc::Sender;

    use gtk::prelude::*;
    use gtk::{ButtonsType, DialogFlags, MessageDialog, MessageType, ResponseType};
    use tauri::{AppHandle, Manager};

    use crate::main_window;

    // The dialog's text is English until the Rust side reads the message
    // files the page uses.
    const TITLE: &str = "This can't be undone";
    const CANCEL: &str = "_Cancel";
    const CONTINUE: &str = "_Continue";

    /// Shows the dialog, modal over the main window, and sends the answer
    /// once.
    pub(super) fn show(app: &AppHandle, summary: &str, answer: Sender<bool>) {
        let parent = app
            .get_webview_window(main_window::LABEL)
            .and_then(|window| window.gtk_window().ok());
        let dialog = MessageDialog::new(
            parent.as_ref(),
            DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
            MessageType::Warning,
            ButtonsType::None,
            TITLE,
        );
        dialog.set_secondary_text(Some(summary));
        dialog.add_button(CANCEL, ResponseType::Cancel);
        if let Some(button) = dialog
            .add_button(CONTINUE, ResponseType::Accept)
            .downcast_ref::<gtk::Button>()
        {
            button.style_context().add_class("destructive-action");
        }
        // Enter on an untouched dialog cancels.
        dialog.set_default_response(ResponseType::Cancel);
        let answer = RefCell::new(Some(answer));
        dialog.connect_response(move |dialog, response| {
            if let Some(answer) = answer.borrow_mut().take() {
                let _ = answer.send(response == ResponseType::Accept);
            }
            dialog.close();
        });
        dialog.show_all();
    }
}
