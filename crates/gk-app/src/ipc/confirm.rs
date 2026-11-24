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
use serde_json::Value;
use tauri::AppHandle;

use crate::messages::text;

/// What the dialog says: its title, the question and the confirming button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wording {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) accept: String,
    pub(crate) cancel: String,
}

/// The dialog's words for `op` with its `input`, from the app's messages:
/// what exactly is deleted or replaced, named from the input.
pub(crate) fn wording(op: &str, summary: &str, input: &Value) -> Wording {
    let name = || {
        input
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let (body, accept) = match op {
        "trash.purge" if input.get("all").and_then(Value::as_bool) == Some(true) => {
            (text("confirm-purge-all", &[]), "confirm-delete")
        }
        "trash.purge" => {
            let count = input
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            (
                text("confirm-purge-items", &[("count", count.to_string())]),
                "confirm-delete",
            )
        }
        "backup.delete" => (
            text("confirm-backup-delete", &[("name", name())]),
            "confirm-delete",
        ),
        "backup.restore" => (
            text("confirm-backup-restore", &[("name", name())]),
            "confirm-restore",
        ),
        _ => (
            text("confirm-other", &[("summary", summary.to_owned())]),
            "confirm-continue",
        ),
    };
    Wording {
        title: text("confirm-title", &[]),
        body,
        accept: text(accept, &[]),
        cancel: text("confirm-cancel", &[]),
    }
}

/// Asks the person in a native dialog over the main window.
pub(crate) fn native(app: AppHandle) -> Confirmation {
    Confirmation::Ask(Arc::new(move |request| ask(&app, request)))
}

#[cfg(target_os = "linux")]
fn ask(app: &AppHandle, request: &ConfirmRequest) -> bool {
    let (answer, answered) = std::sync::mpsc::channel();
    let handle = app.clone();
    let wording = wording(request.op.id, request.op.summary, &request.input);
    // GTK lives on the main thread. The call waits on its own blocking
    // thread, so the main loop keeps running while the dialog is open.
    let shown = app.run_on_main_thread(move || dialog::show(&handle, &wording, answer));
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

    use super::Wording;
    use crate::main_window;

    /// Shows the dialog, modal over the main window, and sends the answer
    /// once.
    pub(super) fn show(app: &AppHandle, wording: &Wording, answer: Sender<bool>) {
        let parent = app
            .get_webview_window(main_window::LABEL)
            .and_then(|window| window.gtk_window().ok());
        let dialog = MessageDialog::new(
            parent.as_ref(),
            DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
            MessageType::Warning,
            ButtonsType::None,
            &wording.title,
        );
        dialog.set_secondary_text(Some(&wording.body));
        dialog.add_button(&wording.cancel, ResponseType::Cancel);
        if let Some(button) = dialog
            .add_button(&wording.accept, ResponseType::Accept)
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::wording;

    #[test]
    fn the_dialog_names_what_goes_for_good() {
        let all = wording("trash.purge", "", &json!({ "all": true }));
        assert!(all.body.starts_with("Empty the whole trash?"), "{all:?}");
        assert_eq!(all.accept, "_Delete for good");
        let two = wording("trash.purge", "", &json!({ "items": ["a", "b"] }));
        assert!(two.body.starts_with("Delete 2 items"), "{two:?}");
        let backup = wording("backup.delete", "", &json!({ "name": "manual-x" }));
        assert!(backup.body.contains("“manual-x”"), "{backup:?}");
        let restore = wording("backup.restore", "", &json!({ "name": "daily-y" }));
        assert!(restore.body.contains("“daily-y”"), "{restore:?}");
        assert_eq!(restore.accept, "_Restore");
        let other = wording("label.delete", "Delete a label.", &json!({}));
        assert_eq!(other.body, "Delete a label.");
        assert_eq!(other.title, "This can't be undone");
        assert_eq!(other.cancel, "_Cancel");
    }
}
