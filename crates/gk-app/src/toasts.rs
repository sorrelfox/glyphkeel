// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Reminders as toasts: notifications the system shows on its own, with an
//! "Open" button when the reminder names a task. A click on the toast or on
//! "Open" brings the task forward in the main window, also when the app runs
//! in the background without a window.
//!
//! What a toast says and what its click does are decided here; showing it is
//! the platform's [`ToastSystem`], so the decisions are tested without one.

use std::sync::Arc;

use crate::messages::text;
use crate::reminders::{Notice, Notifier};

/// The argument a click on "Open" comes back with. A click on the toast
/// itself comes back without one.
pub(crate) const OPEN_ACTION: &str = "open";

/// A toast as the system shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Toast {
    pub(crate) title: String,
    pub(crate) body: String,
    /// The label of the "Open" button, when there is a task to open.
    pub(crate) open: Option<String>,
}

/// What a click on a toast runs: with the argument of the button clicked, or
/// `None` for the toast itself.
pub(crate) type OnClick = Box<dyn Fn(Option<String>) + Send + 'static>;

/// What a click that opens a task asks for: that task.
pub(crate) type OnOpen = Box<dyn Fn(String) + Send + Sync>;

/// The system's toasts.
pub(crate) trait ToastSystem: Send + Sync {
    /// Shows `toast`; `on_click` runs, on a thread of the system's, each
    /// time it is clicked while the app runs.
    fn show(&self, toast: &Toast, on_click: OnClick) -> Result<(), String>;
}

/// Shows reminders as toasts.
pub(crate) struct ToastNotifier<S> {
    system: S,
    on_open: Arc<OnOpen>,
}

impl<S> std::fmt::Debug for ToastNotifier<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastNotifier").finish_non_exhaustive()
    }
}

impl<S: ToastSystem> ToastNotifier<S> {
    pub(crate) fn new(system: S, on_open: OnOpen) -> Self {
        Self {
            system,
            on_open: Arc::new(on_open),
        }
    }
}

/// The toast for `notice`.
pub(crate) fn toast(notice: &Notice) -> Toast {
    Toast {
        title: notice.title.clone(),
        body: notice.body.clone(),
        open: notice.task.as_ref().map(|_| text("notification-open", &[])),
    }
}

/// Whether a click with `action` opens the toast's task: a click on the
/// toast itself or on "Open".
pub(crate) fn opens(action: Option<&str>) -> bool {
    action.is_none_or(|action| action == OPEN_ACTION)
}

impl<S: ToastSystem> Notifier for ToastNotifier<S> {
    fn show(&self, notice: &Notice) {
        let task = notice.task.clone();
        let on_open = Arc::clone(&self.on_open);
        let on_click: OnClick = Box::new(move |action| {
            if let Some(task) = &task
                && opens(action.as_deref())
            {
                on_open(task.clone());
            }
        });
        if let Err(err) = self.system.show(&toast(notice), on_click) {
            eprintln!(
                "glyphkeel-app: cannot show the notification \"{}\": {err}",
                notice.title
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Keeps the toasts and their clicks, to click them in the test.
    #[derive(Default)]
    struct Fake {
        shown: Mutex<Vec<(Toast, OnClick)>>,
        fails: bool,
    }

    impl ToastSystem for Arc<Fake> {
        fn show(&self, toast: &Toast, on_click: OnClick) -> Result<(), String> {
            if self.fails {
                return Err("no notification service".into());
            }
            self.shown.lock().unwrap().push((toast.clone(), on_click));
            Ok(())
        }
    }

    fn notifier(fake: &Arc<Fake>) -> (ToastNotifier<Arc<Fake>>, Arc<Mutex<Vec<String>>>) {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&opened);
        let notifier = ToastNotifier::new(
            Arc::clone(fake),
            Box::new(move |task| record.lock().unwrap().push(task)),
        );
        (notifier, opened)
    }

    fn notice(task: Option<&str>) -> Notice {
        Notice {
            title: "Reminder: TM-41".into(),
            body: "Check fractional ranks".into(),
            task: task.map(String::from),
        }
    }

    #[test]
    fn a_reminder_of_a_task_shows_with_open_and_its_click_opens_the_task() {
        let fake = Arc::new(Fake::default());
        let (notifier, opened) = notifier(&fake);
        notifier.show(&notice(Some("TM-41")));
        let shown = fake.shown.lock().unwrap();
        let (toast, click) = &shown[0];
        assert_eq!(
            *toast,
            Toast {
                title: "Reminder: TM-41".into(),
                body: "Check fractional ranks".into(),
                open: Some("Open".into()),
            }
        );
        click(None);
        click(Some(OPEN_ACTION.into()));
        click(Some("snooze".into()));
        assert_eq!(*opened.lock().unwrap(), ["TM-41", "TM-41"]);
    }

    #[test]
    fn a_reminder_without_a_task_has_nothing_to_open() {
        let fake = Arc::new(Fake::default());
        let (notifier, opened) = notifier(&fake);
        notifier.show(&notice(None));
        let shown = fake.shown.lock().unwrap();
        assert_eq!(shown[0].0.open, None);
        (shown[0].1)(None);
        (shown[0].1)(Some(OPEN_ACTION.into()));
        assert!(opened.lock().unwrap().is_empty());
    }

    #[test]
    fn a_toast_that_cant_be_shown_is_only_logged() {
        let fake = Arc::new(Fake {
            fails: true,
            ..Fake::default()
        });
        let (notifier, opened) = notifier(&fake);
        notifier.show(&notice(Some("TM-1")));
        assert!(fake.shown.lock().unwrap().is_empty());
        assert!(opened.lock().unwrap().is_empty());
    }
}
