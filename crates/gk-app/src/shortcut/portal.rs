// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The quick-add shortcut through the desktop's GlobalShortcuts portal.
//!
//! Every portal call of the app is in this file, because of one rule: the
//! portal ties a bus connection to an app ID at its first call. A program
//! that isn't sandboxed tells the portal its ID with `register_host_app`,
//! and that has to be the very first call, or KDE takes the ID from the
//! process's control group and names the terminal that started the app in
//! its dialog, and files the shortcut under it.
//!
//! KDE keeps a shortcut's entry (in `kglobalshortcutsrc`) even after a
//! refused request and after the session closes. [`Portal::release`] and a
//! refusal remove it through KDE's own shortcut service, so that turning
//! the shortcut off leaves nothing behind.

use ashpd::desktop::ResponseError;
use ashpd::desktop::Session;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use tauri::AppHandle;
use tauri::async_runtime::{self, JoinHandle};

use super::{Keys, SHORTCUT_ID};

/// The app's ID with the portal: the name of its desktop entry. Development
/// builds use their own, as on the session bus.
const APP_ID: &str = if cfg!(debug_assertions) {
    "io.github.sorrelfox.Glyphkeel.Devel"
} else {
    "io.github.sorrelfox.Glyphkeel"
};

// English until the Rust side reads the message files the page uses. The
// desktop shows it in its dialog and its shortcut settings.
const DESCRIPTION: &str = "Quick add a task";

/// The portal connection's state: whether the app's ID is registered yet.
#[derive(Debug, Default)]
pub(crate) struct Portal {
    registered: bool,
}

/// Why binding didn't work.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The person refused it in the desktop's dialog.
    Refused,
    /// No portal, or it failed.
    Unavailable(String),
}

/// A bound shortcut: its session, which keeps it, and the task that opens
/// the quick-add window when it is pressed.
pub(crate) struct Bound {
    session: Session<GlobalShortcuts>,
    listener: JoinHandle<()>,
    /// The key as the desktop describes it.
    pub(crate) trigger: String,
}

impl Portal {
    /// Registers the app's ID, once, before any other portal call. A
    /// failure is only logged: the portal then guesses the app.
    async fn register(&mut self) {
        if self.registered {
            return;
        }
        self.registered = true;
        let registered = match APP_ID.parse() {
            Ok(id) => ashpd::register_host_app(id).await,
            Err(err) => Err(err),
        };
        if let Err(err) = registered {
            eprintln!("glyphkeel-app: the portal does not know the app's ID: {err}");
        }
    }

    /// Asks the desktop for the shortcut. The desktop may show a dialog and
    /// wait for the person's answer; this waits with it.
    pub(crate) async fn bind(&mut self, app: &AppHandle, keys: &Keys) -> Result<Bound, Failure> {
        self.register().await;
        let unavailable = |err: ashpd::Error| Failure::Unavailable(err.to_string());
        let portal = GlobalShortcuts::new().await.map_err(unavailable)?;
        let session = portal
            .create_session(Default::default())
            .await
            .map_err(unavailable)?;
        let trigger = keys.portal_trigger();
        let shortcut =
            NewShortcut::new(SHORTCUT_ID, DESCRIPTION).preferred_trigger(Some(trigger.as_str()));
        let request = portal
            .bind_shortcuts(&session, &[shortcut], None, BindShortcutsOptions::default())
            .await
            .map_err(unavailable)?;
        let bound = match request.response() {
            Ok(response) => response
                .shortcuts()
                .iter()
                .find(|bound| bound.id() == SHORTCUT_ID)
                .map(|bound| bound.trigger_description().to_owned()),
            Err(ashpd::Error::Response(ResponseError::Cancelled)) => None,
            Err(err) => {
                let _ = session.close().await;
                return Err(unavailable(err));
            }
        };
        let Some(trigger) = bound else {
            let _ = session.close().await;
            forget().await;
            return Err(Failure::Refused);
        };
        let activations = portal.receive_activated().await.map_err(unavailable)?;
        let app = app.clone();
        let listener = async_runtime::spawn(async move {
            let mut activations = activations;
            while let Some(activated) = activations.next().await {
                if activated.shortcut_id() != SHORTCUT_ID {
                    continue;
                }
                let token = activated
                    .options()
                    .get("activation_token")
                    .and_then(|value| String::try_from(value.clone()).ok())
                    .filter(|token| !token.is_empty());
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || crate::quick_add::show(&handle, token));
            }
        });
        Ok(Bound {
            session,
            listener,
            trigger,
        })
    }

    /// Gives the shortcut back: the session closes and the desktop forgets
    /// the key.
    pub(crate) async fn release(&mut self, bound: Bound) {
        bound.listener.abort();
        if let Err(err) = bound.session.close().await {
            eprintln!("glyphkeel-app: cannot close the shortcut's portal session: {err}");
        }
        forget().await;
    }
}

/// Removes the shortcut's entry from KDE's shortcut service, which keeps it
/// otherwise. Elsewhere there is no such service and nothing to do.
async fn forget() {
    let Ok(connection) = ashpd::zbus::Connection::session().await else {
        return;
    };
    let answer = connection
        .call_method(
            Some("org.kde.kglobalaccel"),
            "/kglobalaccel",
            Some("org.kde.KGlobalAccel"),
            "unregister",
            &(APP_ID, SHORTCUT_ID),
        )
        .await;
    if let Err(err) = answer {
        // No KDE: the service isn't there. Anything else is worth a line.
        if !matches!(&err, ashpd::zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown")
        {
            eprintln!("glyphkeel-app: cannot remove the shortcut from KDE: {err}");
        }
    }
}
