// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! One running app per session.
//!
//! The first process owns the app's name on the session bus and serves the
//! standard `org.freedesktop.Application` interface there. A second start
//! finds the name taken, hands its links and its activation token to the
//! first process, and exits before it opens any window.
//!
//! The activation token matters on Wayland: without it the compositor may
//! refuse to bring the running window forward. Tauri's single-instance plugin
//! passes only the arguments, so the hand-off is done here.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;
use zbus::interface;
use zbus::zvariant::{OwnedValue, Value};

/// The app's well-known bus name. Development builds use their own, so they
/// can run next to an installed release.
const BUS_NAME: &str = if cfg!(debug_assertions) {
    "io.github.sorrelfox.Glyphkeel.Devel"
} else {
    "io.github.sorrelfox.Glyphkeel"
};
const OBJECT_PATH: &str = if cfg!(debug_assertions) {
    "/io/github/sorrelfox/Glyphkeel/Devel"
} else {
    "/io/github/sorrelfox/Glyphkeel"
};
const INTERFACE: &str = "org.freedesktop.Application";

/// What a later start asked the running app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Activation {
    /// `glyphkeel://` links to open, in order.
    pub(crate) links: Vec<String>,
    /// The token that lets the window come forward (xdg-activation on
    /// Wayland, the startup id on X11), when the launcher gave one.
    pub(crate) token: Option<String>,
}

/// The outcome of trying to become the running app.
pub(crate) enum Claim {
    /// This process is the running app. `activations` brings the requests of
    /// later starts, in order. Keep `bus` alive as long as the app runs:
    /// dropping it releases the name.
    Primary {
        bus: Connection,
        activations: Receiver<Activation>,
    },
    /// Another process runs the app and has received this start's request.
    Forwarded,
    /// The session bus is unreachable; the app runs without the check.
    Unavailable(zbus::Error),
}

/// Becomes the running app, or hands `activation` to the one that runs.
pub(crate) fn claim(activation: &Activation) -> Claim {
    let (sender, activations) = mpsc::channel();
    let built = Builder::session()
        .and_then(|builder| builder.name(BUS_NAME))
        .map(|builder| {
            builder
                .replace_existing_names(false)
                .allow_name_replacements(false)
        })
        .and_then(|builder| builder.serve_at(OBJECT_PATH, Application { sender }))
        .and_then(Builder::build);
    match built {
        Ok(bus) => Claim::Primary { bus, activations },
        Err(zbus::Error::NameTaken) => match forward(activation) {
            Ok(()) => Claim::Forwarded,
            Err(err) => Claim::Unavailable(err),
        },
        Err(err) => Claim::Unavailable(err),
    }
}

fn forward(activation: &Activation) -> zbus::Result<()> {
    let connection = Connection::session()?;
    let mut platform_data: HashMap<&str, Value<'_>> = HashMap::new();
    if let Some(token) = &activation.token {
        platform_data.insert("activation-token", Value::from(token.as_str()));
        platform_data.insert("desktop-startup-id", Value::from(token.as_str()));
    }
    if activation.links.is_empty() {
        connection.call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(INTERFACE),
            "Activate",
            &(platform_data,),
        )?;
    } else {
        connection.call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(INTERFACE),
            "Open",
            &(&activation.links, platform_data),
        )?;
    }
    Ok(())
}

/// This start's request, from its arguments and environment.
pub(crate) fn this_start() -> Activation {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let token = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    Activation {
        links: links_in(&args),
        token,
    }
}

/// The app's own links among the arguments.
pub(crate) fn links_in(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|arg| arg.starts_with("glyphkeel://"))
        .cloned()
        .collect()
}

struct Application {
    sender: Sender<Activation>,
}

impl Application {
    fn send(&self, links: Vec<String>, platform_data: &HashMap<String, OwnedValue>) {
        let token = ["activation-token", "desktop-startup-id"]
            .iter()
            .find_map(|key| {
                platform_data
                    .get(*key)
                    .and_then(|value| String::try_from(value.clone()).ok())
                    .filter(|value| !value.is_empty())
            });
        // The receiver lives as long as the app; a send fails only while it
        // shuts down, when there is nothing left to bring forward.
        let _ = self.sender.send(Activation {
            links: links_in(&links),
            token,
        });
    }
}

#[interface(name = "org.freedesktop.Application")]
impl Application {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        self.send(Vec::new(), &platform_data);
    }

    fn open(&self, uris: Vec<String>, platform_data: HashMap<String, OwnedValue>) {
        self.send(uris, &platform_data);
    }

    fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        // The app has no actions to start from outside yet; bring it forward.
        let _ = (action_name, parameter);
        self.send(Vec::new(), &platform_data);
    }
}

#[cfg(test)]
mod tests {
    use super::links_in;

    #[test]
    fn only_the_apps_own_links_are_taken_from_the_arguments() {
        let args = [
            "--background",
            "glyphkeel://open/TM-41",
            "https://example.com/",
            "glyphkeel://open/TM-2",
        ]
        .map(String::from);
        assert_eq!(
            links_in(&args),
            ["glyphkeel://open/TM-41", "glyphkeel://open/TM-2"]
        );
        assert!(links_in(&[]).is_empty());
    }
}
