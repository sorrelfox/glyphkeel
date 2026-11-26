// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Desktop notifications through the freedesktop notification server on the
//! session bus, with an "Open" action that brings the task forward.
//!
//! The server sends `ActivationToken` just before `ActionInvoked` when the
//! person clicks; the token lets the window take focus on Wayland. Crates
//! that wrap the interface don't pass it on, so the bus is used directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;

use crate::messages::text;
use crate::reminders::{Notice, Notifier};

const DESTINATION: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const APP_NAME: &str = "Glyphkeel";
/// The desktop file the server takes the app's name and icon from.
const DESKTOP_ENTRY: &str = "io.github.sorrelfox.Glyphkeel";
/// The action keys: `default` is a click on the notification itself.
const OPEN_ACTIONS: [&str; 2] = ["default", "open"];
/// The most notifications remembered for their action.
const KEPT: usize = 64;

/// What "Open" on a notification asks for: the task, and the activation
/// token when the server sent one.
pub(crate) type OnOpen = Box<dyn Fn(String, Option<String>) + Send + Sync>;

#[derive(Default)]
struct Shown {
    /// Notification ID to its task, in the order shown.
    tasks: Vec<(u32, String)>,
    tokens: HashMap<u32, String>,
}

/// The session bus's notification server.
pub(crate) struct DesktopNotifier {
    connection: Connection,
    shown: Arc<Mutex<Shown>>,
}

impl std::fmt::Debug for DesktopNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopNotifier").finish_non_exhaustive()
    }
}

fn lock(shown: &Mutex<Shown>) -> MutexGuard<'_, Shown> {
    shown.lock().unwrap_or_else(PoisonError::into_inner)
}

impl DesktopNotifier {
    /// Connects to the session bus and listens for the actions of the
    /// notifications it shows; `on_open` runs for "Open", on a thread of
    /// its own.
    pub(crate) fn connect(on_open: OnOpen) -> zbus::Result<Self> {
        let connection = Connection::session()?;
        let shown = Arc::new(Mutex::new(Shown::default()));
        let signals = MessageIterator::for_match_rule(
            format!("type='signal',interface='{INTERFACE}',path='{PATH}'").as_str(),
            &connection,
            Some(64),
        )?;
        let listening = Arc::clone(&shown);
        std::thread::Builder::new()
            .name("gk-app-notifications".into())
            .spawn(move || listen(signals, &listening, &on_open))
            .map_err(|err| zbus::Error::Failure(err.to_string()))?;
        Ok(Self { connection, shown })
    }

    /// Shows `notice`; answers the server's ID for it.
    pub(crate) fn notify(&self, notice: &Notice) -> zbus::Result<u32> {
        let open = text("notification-open", &[]);
        let actions: Vec<&str> = if notice.task.is_some() {
            vec![OPEN_ACTIONS[0], &open, OPEN_ACTIONS[1], &open]
        } else {
            Vec::new()
        };
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        hints.insert("desktop-entry", Value::from(DESKTOP_ENTRY));
        // Normal urgency: reminders the person asked for.
        hints.insert("urgency", Value::from(1u8));
        let reply = self.connection.call_method(
            Some(DESTINATION),
            PATH,
            Some(INTERFACE),
            "Notify",
            &(
                APP_NAME,
                0u32,
                "",
                notice.title.as_str(),
                notice.body.as_str(),
                actions,
                hints,
                // The server's own timeout.
                -1i32,
            ),
        )?;
        let id: u32 = reply.body().deserialize()?;
        if let Some(task) = &notice.task {
            let mut shown = lock(&self.shown);
            shown.tasks.push((id, task.clone()));
            let excess = shown.tasks.len().saturating_sub(KEPT);
            let gone: Vec<u32> = shown.tasks.drain(..excess).map(|(id, _)| id).collect();
            for id in gone {
                shown.tokens.remove(&id);
            }
        }
        Ok(id)
    }

    /// Takes a notification off the screen.
    #[cfg(test)]
    pub(crate) fn close(&self, id: u32) -> zbus::Result<()> {
        self.connection.call_method(
            Some(DESTINATION),
            PATH,
            Some(INTERFACE),
            "CloseNotification",
            &(id,),
        )?;
        Ok(())
    }
}

impl Notifier for DesktopNotifier {
    fn show(&self, notice: &Notice) {
        if let Err(err) = self.notify(notice) {
            eprintln!(
                "glyphkeel-app: cannot show the notification \"{}\": {err}",
                notice.title
            );
        }
    }
}

fn listen(signals: MessageIterator, shown: &Mutex<Shown>, on_open: &OnOpen) {
    for message in signals {
        let Ok(message) = message else {
            continue;
        };
        let header = message.header();
        let Some(member) = header.member() else {
            continue;
        };
        match member.as_str() {
            "ActivationToken" => {
                if let Ok((id, token)) = message.body().deserialize::<(u32, String)>() {
                    let mut shown = lock(shown);
                    if shown.tasks.iter().any(|(known, _)| *known == id) {
                        shown.tokens.insert(id, token);
                    }
                }
            }
            "ActionInvoked" => {
                let Ok((id, action)) = message.body().deserialize::<(u32, String)>() else {
                    continue;
                };
                if !OPEN_ACTIONS.contains(&action.as_str()) {
                    continue;
                }
                let found = {
                    let mut shown = lock(shown);
                    let task = shown
                        .tasks
                        .iter()
                        .find(|(known, _)| *known == id)
                        .map(|(_, task)| task.clone());
                    task.map(|task| (task, shown.tokens.remove(&id)))
                };
                if let Some((task, token)) = found {
                    on_open(task, token);
                }
            }
            "NotificationClosed" => {
                // An expired notification may still be clicked in the
                // desktop's history; one dismissed or taken back can't.
                if let Ok((id, reason)) = message.body().deserialize::<(u32, u32)>()
                    && (reason == 2 || reason == 3)
                {
                    let mut shown = lock(shown);
                    shown.tasks.retain(|(known, _)| *known != id);
                    shown.tokens.remove(&id);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Shows one real notification on this desktop and takes it back. Needs
    /// a session bus with a notification server, so it runs only when asked:
    /// `cargo test -p gk-app notifications -- --ignored`.
    #[test]
    #[ignore = "shows a notification on the desktop"]
    fn a_notification_shows_with_its_open_action_and_is_taken_back() {
        let notifier = DesktopNotifier::connect(Box::new(|_, _| {})).unwrap();
        let id = notifier
            .notify(&Notice {
                title: "Reminder: TM-41".into(),
                body: "Check fractional ranks".into(),
                task: Some("TM-41".into()),
            })
            .unwrap();
        assert!(id > 0);
        assert_eq!(lock(&notifier.shown).tasks, [(id, "TM-41".to_owned())]);
        std::thread::sleep(std::time::Duration::from_secs(2));
        notifier.close(id).unwrap();
    }
}
