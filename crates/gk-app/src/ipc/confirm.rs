// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The native confirmation of `permanent` operations, and of raising the
//! most an agent may do or the limits on agents.
//!
//! The engine asks inside the call, before the transaction, and the call
//! waits for the answer. The dialog is the system's own (GTK's on Linux, a
//! task dialog on Windows), shown by Rust over the window: script in the page
//! can neither see nor answer it, and no answer the page passes in can stand
//! in for it.

use std::sync::Arc;

use gk_engine::{ConfirmRequest, Confirmation};
use serde_json::Value;
use tauri::AppHandle;

use crate::messages::text;

/// What the dialog says: its title, the question and the confirming button.
#[cfg_attr(
    not(any(target_os = "linux", windows, test)),
    expect(dead_code, reason = "no native dialog shows it on this platform yet")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wording {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) accept: String,
    pub(crate) cancel: String,
}

/// The dialog's words for `op` with its `input`, from the app's messages:
/// what exactly is deleted or replaced, named from the input.
#[cfg_attr(
    not(any(target_os = "linux", windows, test)),
    expect(dead_code, reason = "no native dialog shows it on this platform yet")
)]
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
        "agent.policy.update" if input.get("agent_http").and_then(Value::as_bool) == Some(true) => {
            (text("confirm-agent-http-on", &[]), "confirm-agent-raise")
        }
        "agent.policy.update" => {
            // Only a raise asks; to anything but the full profile it is to
            // the standard one. Setting the same profile again asks only to
            // let agents ask the person.
            let question = match input.get("max_profile").and_then(Value::as_str) {
                Some("full") => "confirm-agent-raise-full",
                _ if input.get("confirmations").and_then(Value::as_bool) == Some(true) => {
                    "confirm-agent-confirmations-on"
                }
                _ if input.get("approvals").is_some() => "confirm-agent-approvals-off",
                _ => "confirm-agent-raise-standard",
            };
            (text(question, &[]), "confirm-agent-raise")
        }
        "agent.limits.update" => (
            text("confirm-agent-limits-raise", &[]),
            "confirm-agent-raise",
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
    let title = if matches!(op, "agent.policy.update" | "agent.limits.update") {
        "confirm-agent-raise-title"
    } else {
        "confirm-title"
    };
    Wording {
        title: text(title, &[]),
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

#[cfg(windows)]
fn ask(app: &AppHandle, request: &ConfirmRequest) -> bool {
    let wording = wording(request.op.id, request.op.summary, &request.input);
    ask_with(
        &crate::platform::windows::TaskDialog::new(app.clone()),
        &wording,
    )
}

#[cfg(not(any(target_os = "linux", windows)))]
fn ask(_app: &AppHandle, _request: &ConfirmRequest) -> bool {
    // No native dialog on this platform yet: refuse rather than guess.
    false
}

/// A message box of the system's with two buttons, the first one the
/// default.
#[cfg(any(windows, test))]
pub(crate) trait MessageBox {
    /// Shows the box and waits; answers the label of the button pressed, or
    /// `None` when the box went away without one (Escape, closed, failed).
    fn show(&self, title: &str, body: &str, buttons: [&str; 2]) -> Option<String>;
}

/// Asks with `wording` in `message_box`. Cancel comes first, so Enter on an
/// untouched box cancels, as on Linux. Only the confirming button confirms.
#[cfg(any(windows, test))]
pub(crate) fn ask_with(message_box: &impl MessageBox, wording: &Wording) -> bool {
    let cancel = access_keys(&wording.cancel);
    let accept = access_keys(&wording.accept);
    if cancel == accept {
        // Two buttons that look the same can't be told apart: refuse.
        return false;
    }
    message_box
        .show(&wording.title, &wording.body, [&cancel, &accept])
        .is_some_and(|pressed| pressed == accept)
}

/// A button label with GTK's access key (`_Delete`) written as Windows
/// writes it (`&Delete`); `__` is an underscore, and a `&` stays one.
#[cfg(any(windows, test))]
pub(crate) fn access_keys(label: &str) -> String {
    let mut out = String::with_capacity(label.len() + 1);
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
                out.push('_');
            }
            '_' if chars.peek().is_some() => out.push('&'),
            '&' => out.push_str("&&"),
            c => out.push(c),
        }
    }
    out
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
    use std::cell::RefCell;

    use serde_json::json;

    use super::{MessageBox, Wording, access_keys, ask_with, wording};

    /// Presses the button it is told to, and keeps what it showed.
    struct Fake {
        press: Option<usize>,
        shown: RefCell<Vec<(String, String, [String; 2])>>,
    }

    impl Fake {
        fn pressing(press: Option<usize>) -> Self {
            Self {
                press,
                shown: RefCell::new(Vec::new()),
            }
        }
    }

    impl MessageBox for Fake {
        fn show(&self, title: &str, body: &str, buttons: [&str; 2]) -> Option<String> {
            self.shown.borrow_mut().push((
                title.to_owned(),
                body.to_owned(),
                buttons.map(str::to_owned),
            ));
            self.press.map(|at| buttons[at].to_owned())
        }
    }

    #[test]
    fn the_message_box_confirms_only_with_its_confirming_button() {
        let words = wording("backup.delete", "", &json!({ "name": "manual-x" }));
        let cancel = Fake::pressing(Some(0));
        assert!(!ask_with(&cancel, &words));
        let (title, body, buttons) = cancel.shown.borrow()[0].clone();
        assert_eq!(title, "This can't be undone");
        assert!(body.contains("“manual-x”"), "{body}");
        // Cancel first, as the default button.
        assert_eq!(buttons, ["&Cancel", "&Delete for good"]);
        assert!(ask_with(&Fake::pressing(Some(1)), &words));
        assert!(!ask_with(&Fake::pressing(None), &words));
    }

    #[test]
    fn buttons_that_look_the_same_never_confirm() {
        let same = Wording {
            title: "t".into(),
            body: "b".into(),
            accept: "_OK".into(),
            cancel: "_OK".into(),
        };
        let fake = Fake::pressing(Some(1));
        assert!(!ask_with(&fake, &same));
        assert!(fake.shown.borrow().is_empty());
    }

    #[test]
    fn access_keys_are_written_the_windows_way() {
        assert_eq!(access_keys("_Delete for good"), "&Delete for good");
        assert_eq!(access_keys("Save __as"), "Save _as");
        assert_eq!(access_keys("Fish & _chips"), "Fish && &chips");
        assert_eq!(access_keys("trailing_"), "trailing_");
        assert_eq!(access_keys(""), "");
    }

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

    #[test]
    fn raising_what_agents_may_do_says_what_they_could_then_do() {
        let full = wording("agent.policy.update", "", &json!({ "max_profile": "full" }));
        assert!(full.body.contains("deleting for good"), "{full:?}");
        assert_eq!(full.title, "Let agents do more?");
        assert_eq!(full.accept, "_Allow");
        let standard = wording(
            "agent.policy.update",
            "",
            &json!({ "max_profile": "standard" }),
        );
        assert!(standard.body.contains("change tasks"), "{standard:?}");
        let limits = wording(
            "agent.limits.update",
            "",
            &json!({ "no_limit": ["writes_per_minute"] }),
        );
        assert_eq!(limits.title, "Let agents do more?");
        assert_eq!(limits.accept, "_Allow");
        assert!(limits.body.contains("limits on agents"), "{limits:?}");
    }
}
