// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! One running app per session.
//!
//! On Linux, the first process owns the app's name on the session bus and
//! serves the standard `org.freedesktop.Application` interface there. A
//! second start finds the name taken, hands its links and its activation
//! token to the first process, and exits before it opens any window.
//!
//! The activation token matters on Wayland: without it the compositor may
//! refuse to bring the running window forward. Tauri's single-instance plugin
//! passes only the arguments, so the hand-off is done here.
//!
//! On Windows, Tauri's single-instance plugin does the hand-off: a second
//! start sends its arguments to the running app, lets it come to the front,
//! and exits. What those arguments ask for is read here, the same way on
//! every platform.

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::sync::mpsc::{self, Receiver, Sender};

#[cfg(target_os = "linux")]
use zbus::blocking::Connection;
#[cfg(target_os = "linux")]
use zbus::blocking::connection::Builder;
#[cfg(target_os = "linux")]
use zbus::interface;
#[cfg(target_os = "linux")]
use zbus::zvariant::{OwnedValue, Value};

/// The app's well-known bus name. Development builds use their own, so they
/// can run next to an installed release.
#[cfg(target_os = "linux")]
const BUS_NAME: &str = if cfg!(debug_assertions) {
    "io.github.sorrelfox.Glyphkeel.Devel"
} else {
    "io.github.sorrelfox.Glyphkeel"
};
#[cfg(target_os = "linux")]
const OBJECT_PATH: &str = if cfg!(debug_assertions) {
    "/io/github/sorrelfox/Glyphkeel/Devel"
} else {
    "/io/github/sorrelfox/Glyphkeel"
};
#[cfg(target_os = "linux")]
const INTERFACE: &str = "org.freedesktop.Application";

/// What a later start asked the running app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Activation {
    /// `glyphkeel://` links to open, in order.
    pub(crate) links: Vec<String>,
    /// The token that lets the window come forward (xdg-activation on
    /// Wayland, the startup id on X11), when the launcher gave one.
    pub(crate) token: Option<String>,
    /// Show the quick-add window instead of the main one (`--quick-add`).
    pub(crate) quick_add: bool,
    /// Started with the system (`--background`): a running app has nothing
    /// to do for it. Never sent to the running app.
    pub(crate) background: bool,
}

impl Activation {
    /// Whether this start asks a running app for nothing: started with the
    /// system, without links or quick add.
    fn asks_nothing(&self) -> bool {
        self.background && self.links.is_empty() && !self.quick_add
    }
}

/// The argument that asks for the quick-add window, and its action name on
/// the bus.
pub(crate) const QUICK_ADD: &str = "--quick-add";
#[cfg(target_os = "linux")]
const QUICK_ADD_ACTION: &str = "quick-add";

/// The outcome of trying to become the running app.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
        // Started with the system while the app runs already (it was
        // started by hand first): leave the running app as it is.
        Err(zbus::Error::NameTaken) if activation.asks_nothing() => Claim::Forwarded,
        Err(zbus::Error::NameTaken) => match forward(activation) {
            Ok(()) => Claim::Forwarded,
            Err(err) => Claim::Unavailable(err),
        },
        Err(err) => Claim::Unavailable(err),
    }
}

#[cfg(target_os = "linux")]
fn forward(activation: &Activation) -> zbus::Result<()> {
    let connection = Connection::session()?;
    let mut platform_data: HashMap<&str, Value<'_>> = HashMap::new();
    if let Some(token) = &activation.token {
        platform_data.insert("activation-token", Value::from(token.as_str()));
        platform_data.insert("desktop-startup-id", Value::from(token.as_str()));
    }
    if activation.quick_add {
        let parameter: Vec<Value<'_>> = Vec::new();
        connection.call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(INTERFACE),
            "ActivateAction",
            &(QUICK_ADD_ACTION, parameter, platform_data),
        )?;
    } else if activation.links.is_empty() {
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
        token,
        ..from_args(&args)
    }
}

/// The request in a start's arguments (without the program's own name).
pub(crate) fn from_args(args: &[String]) -> Activation {
    Activation {
        links: links_in(args),
        token: None,
        quick_add: args.iter().any(|arg| arg == QUICK_ADD),
        background: args
            .iter()
            .any(|arg| arg == crate::autostart::BACKGROUND_ARG),
    }
}

/// What a later start asks of the running app, from its whole command line
/// as the single-instance plugin passes it (the program first); `None` when
/// it asks for nothing.
#[cfg(any(windows, test))]
pub(crate) fn later_start(command_line: &[String]) -> Option<Activation> {
    let activation = from_args(command_line.get(1..).unwrap_or_default());
    (!activation.asks_nothing()).then_some(activation)
}

/// The app's own links among the arguments.
pub(crate) fn links_in(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|arg| arg.starts_with("glyphkeel://"))
        .cloned()
        .collect()
}

#[cfg(target_os = "linux")]
struct Application {
    sender: Sender<Activation>,
}

#[cfg(target_os = "linux")]
impl Application {
    fn send(
        &self,
        links: Vec<String>,
        quick_add: bool,
        platform_data: &HashMap<String, OwnedValue>,
    ) {
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
            quick_add,
            background: false,
        });
    }
}

#[cfg(target_os = "linux")]
#[interface(name = "org.freedesktop.Application")]
impl Application {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        self.send(Vec::new(), false, &platform_data);
    }

    fn open(&self, uris: Vec<String>, platform_data: HashMap<String, OwnedValue>) {
        self.send(uris, false, &platform_data);
    }

    fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        // Quick add is the one action; any other brings the app forward.
        let _ = parameter;
        self.send(Vec::new(), action_name == QUICK_ADD_ACTION, &platform_data);
    }
}

#[cfg(test)]
mod tests {
    use super::{Activation, from_args, later_start, links_in};

    #[test]
    fn a_start_with_the_system_asks_a_running_app_for_nothing() {
        let start = |links: &[&str], quick_add, background| Activation {
            links: links.iter().map(|l| (*l).to_owned()).collect(),
            token: None,
            quick_add,
            background,
        };
        assert!(start(&[], false, true).asks_nothing());
        assert!(!start(&[], false, false).asks_nothing());
        assert!(!start(&["glyphkeel://open/TM-1"], false, true).asks_nothing());
        assert!(!start(&[], true, true).asks_nothing());
    }

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

    #[test]
    fn a_later_start_asks_for_its_links_or_quick_add() {
        let line = |args: &[&str]| -> Vec<String> {
            std::iter::once("C:\\Glyphkeel\\glyphkeel-app.exe")
                .chain(args.iter().copied())
                .map(String::from)
                .collect()
        };
        let open = later_start(&line(&["glyphkeel://open/TM-1"])).unwrap();
        assert_eq!(open.links, ["glyphkeel://open/TM-1"]);
        assert!(!open.quick_add);
        assert_eq!(open.token, None);
        assert!(later_start(&line(&["--quick-add"])).unwrap().quick_add);
        // A plain second start brings the window forward.
        assert_eq!(later_start(&line(&[])).unwrap(), from_args(&[]));
        // Started with the system while the app runs: nothing to do.
        assert_eq!(later_start(&line(&["--background"])), None);
        // The program's own name is never taken for a link.
        assert!(
            later_start(&["glyphkeel://open/TM-9".to_owned()])
                .unwrap()
                .links
                .is_empty()
        );
        assert!(later_start(&[]).is_some());
    }
}
