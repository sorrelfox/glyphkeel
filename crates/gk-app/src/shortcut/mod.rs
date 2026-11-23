// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The quick-add shortcut: one key that opens the quick-add window from
//! anywhere on the desktop, while the app runs (also in the background).
//!
//! Desktops differ in how a program may have such a key, so the app tries,
//! in this order:
//!
//! 1. **The GlobalShortcuts portal**, where the desktop offers it (KDE
//!    Plasma, recent GNOME; on Wayland and on X11). The desktop asks the
//!    person once to allow the shortcut, and may let them pick another key.
//!    Before the first portal call the app registers its ID with the portal
//!    ([`portal`]), or KDE files the shortcut under the program that started
//!    the app (a terminal, say).
//! 2. **A key grab on the X server**: on X11, and on KDE Plasma's Wayland
//!    session, whose compositor passes key combinations on to X11 programs
//!    through XWayland ([`grab`]).
//! 3. Otherwise **the person binds `glyphkeel-app --quick-add`** to a key in
//!    the desktop's own settings; Preferences says how.
//!
//! When the person refuses the portal's request, nothing else is tried (they
//! said no): the setting is switched off, and the entry KDE keeps even for a
//! refused request is removed. Turning the shortcut off, or choosing another
//! key, removes it from the desktop the same way.

pub(crate) mod grab;
#[cfg(target_os = "linux")]
pub(crate) mod portal;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gk_api::OpError;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::{self, Receiver, Sender};
use tauri::{AppHandle, Manager, State as TauriState};

use crate::app_settings::AppSettings;

/// The shortcut's ID with the desktop.
pub(crate) const SHORTCUT_ID: &str = "quick-add";

/// The keys Preferences offers. Each is free on the common desktops'
/// default layouts.
pub(crate) const CHOICES: [&str; 6] = [
    "Ctrl+Alt+Space",
    "Ctrl+Alt+N",
    "Ctrl+Alt+A",
    "Ctrl+Shift+Space",
    "Super+Alt+N",
    "Ctrl+Alt+Shift+N",
];

/// The key until the person chooses another.
pub(crate) const DEFAULT_KEYS: &str = CHOICES[0];

/// The command a person binds to a key where the app can't have one.
pub(crate) const COMMAND: &str = "glyphkeel-app --quick-add";

/// The shortcut as chosen in Preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ShortcutSetting {
    pub(crate) enabled: bool,
    /// One of [`CHOICES`], written like the page's keymap (`Ctrl+Alt+N`).
    pub(crate) keys: String,
}

impl Default for ShortcutSetting {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: DEFAULT_KEYS.to_owned(),
        }
    }
}

/// A key combination of the choices: modifiers and one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Keys {
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
    /// `A`–`Z`, `0`–`9`, `Space` or `F1`–`F12`.
    key: String,
}

impl Keys {
    /// Reads `Ctrl+Alt+N`. Needs at least one modifier besides Shift, so
    /// that the shortcut can't swallow typing.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut keys = Self {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            key: String::new(),
        };
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts.pop()?;
        for part in parts {
            let flag = match part.to_ascii_lowercase().as_str() {
                "ctrl" => &mut keys.ctrl,
                "alt" => &mut keys.alt,
                "shift" => &mut keys.shift,
                "super" => &mut keys.super_key,
                _ => return None,
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
        let upper = key.to_ascii_uppercase();
        let valid = (upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()))
            || upper == "SPACE"
            || upper
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=12).contains(&n));
        if !valid || !(keys.ctrl || keys.alt || keys.super_key) {
            return None;
        }
        keys.key = if upper == "SPACE" {
            "Space".to_owned()
        } else {
            upper
        };
        Some(keys)
    }

    /// The form the portal takes as the preferred trigger (the XDG
    /// shortcuts format: `CTRL+ALT+space`, key names as XKB key symbols).
    pub(crate) fn portal_trigger(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (on, name) in [
            (self.ctrl, "CTRL"),
            (self.alt, "ALT"),
            (self.shift, "SHIFT"),
            (self.super_key, "LOGO"),
        ] {
            if on {
                parts.push(name.to_owned());
            }
        }
        parts.push(match self.key.as_str() {
            "Space" => "space".to_owned(),
            key if key.len() == 1 => key.to_ascii_lowercase(),
            key => key.to_owned(),
        });
        parts.join("+")
    }

    /// The form the X server grab takes: `CTRL+ALT+SPACE`.
    pub(crate) fn grab_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (on, name) in [
            (self.ctrl, "CTRL"),
            (self.alt, "ALT"),
            (self.shift, "SHIFT"),
            (self.super_key, "SUPER"),
        ] {
            if on {
                parts.push(name.to_owned());
            }
        }
        parts.push(self.key.to_ascii_uppercase());
        parts.join("+")
    }
}

/// The kind of session the app runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Session {
    X11,
    Wayland,
    Other,
}

/// What the desktop offers for a shortcut, from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Desktop {
    pub(crate) session: Session,
    /// KDE Plasma, whose compositor passes key combinations to XWayland.
    pub(crate) kde: bool,
    /// An X server to grab keys on (X11 itself, or XWayland).
    pub(crate) x_display: bool,
    /// A route forced for testing by `GLYPHKEEL_SHORTCUT_ROUTE`.
    pub(crate) forced: Option<Forced>,
}

/// The route `GLYPHKEEL_SHORTCUT_ROUTE` forces: `portal`, `grab` or `manual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forced {
    Portal,
    Grab,
    Manual,
}

impl Desktop {
    /// Reads `XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`, `WAYLAND_DISPLAY`,
    /// `DISPLAY` and `GLYPHKEEL_SHORTCUT_ROUTE` through `var`.
    pub(crate) fn from_env(var: impl Fn(&str) -> Option<String>) -> Self {
        let set = |name: &str| var(name).is_some_and(|value| !value.is_empty());
        let session = match var("XDG_SESSION_TYPE").as_deref() {
            Some("x11") => Session::X11,
            Some("wayland") => Session::Wayland,
            _ if set("WAYLAND_DISPLAY") => Session::Wayland,
            _ if set("DISPLAY") => Session::X11,
            _ => Session::Other,
        };
        let kde = var("XDG_CURRENT_DESKTOP").is_some_and(|desktops| {
            desktops
                .split(':')
                .any(|desktop| desktop.eq_ignore_ascii_case("KDE"))
        });
        let forced = match var("GLYPHKEEL_SHORTCUT_ROUTE").as_deref() {
            Some("portal") => Some(Forced::Portal),
            Some("grab") => Some(Forced::Grab),
            Some("manual") => Some(Forced::Manual),
            _ => None,
        };
        Self {
            session,
            kde,
            x_display: set("DISPLAY"),
            forced,
        }
    }

    /// Whether a key grab on the X server sees the keys pressed in every
    /// window: on X11, and on KDE's Wayland session through XWayland. Not
    /// on other Wayland compositors, which keep keys from X11 programs.
    pub(crate) fn grabs_work(&self) -> bool {
        self.x_display
            && match self.session {
                Session::X11 => true,
                Session::Wayland => self.kde,
                Session::Other => false,
            }
    }
}

/// A way to have the shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Portal,
    Grab,
}

/// How a route went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The shortcut works now.
    Bound,
    /// The person refused it in the desktop's dialog.
    Refused,
    /// The route isn't there or failed; the next one may work.
    Unavailable,
}

/// Where the shortcut stands, for Preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    /// Switched off.
    Off,
    /// Asked the desktop; it may be waiting for the person's answer.
    Waiting,
    /// Bound through the portal.
    Portal,
    /// Bound as a key grab on the X server.
    Grab,
    /// The app can't have the key here; the person binds the command.
    Manual,
    /// The person refused the desktop's request.
    Refused,
}

/// What to do next while registering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Try(Route),
    Done(State),
}

/// The next step after the routes `tried` so far: the portal first, then a
/// key grab where it works, else the person binds the command. A refusal
/// ends it; nothing else is tried behind the person's back.
pub(crate) fn next_step(desktop: &Desktop, tried: &[(Route, Outcome)]) -> Step {
    if let Some(&(route, outcome)) = tried.last() {
        match outcome {
            Outcome::Bound => {
                return Step::Done(match route {
                    Route::Portal => State::Portal,
                    Route::Grab => State::Grab,
                });
            }
            Outcome::Refused => return Step::Done(State::Refused),
            Outcome::Unavailable => {}
        }
    }
    let routes: &[Route] = match desktop.forced {
        Some(Forced::Portal) => &[Route::Portal],
        Some(Forced::Grab) => &[Route::Grab],
        Some(Forced::Manual) => &[],
        None if desktop.grabs_work() => &[Route::Portal, Route::Grab],
        None => &[Route::Portal],
    };
    routes
        .iter()
        .find(|route| !tried.iter().any(|(done, _)| done == *route))
        .map_or(Step::Done(State::Manual), |route| Step::Try(*route))
}

/// What Preferences shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct View {
    pub(crate) enabled: bool,
    pub(crate) keys: String,
    pub(crate) choices: Vec<String>,
    pub(crate) state: State,
    /// The key as the desktop describes it, when the portal bound it (the
    /// person may have picked another there).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) trigger: Option<String>,
    /// Why the last route failed, for the log line in Preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
    /// The command to bind where the app can't have the key.
    pub(crate) command: String,
}

#[derive(Debug, Clone)]
struct Status {
    state: State,
    trigger: Option<String>,
    detail: Option<String>,
}

/// The shortcut while the app runs: its status, and the queue of settings
/// to apply, which one task works through in order.
#[derive(Debug)]
pub(crate) struct Shortcut {
    status: Arc<Mutex<Status>>,
    requests: Sender<ShortcutSetting>,
}

impl Shortcut {
    fn lock(&self) -> MutexGuard<'_, Status> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn view(&self, setting: &ShortcutSetting) -> View {
        let status = self.lock().clone();
        View {
            enabled: setting.enabled,
            keys: setting.keys.clone(),
            choices: CHOICES.iter().map(|&choice| choice.to_owned()).collect(),
            state: status.state,
            trigger: status.trigger,
            detail: status.detail,
            command: COMMAND.to_owned(),
        }
    }

    /// Queues `setting` to be applied: the old shortcut goes, the new one
    /// is registered.
    fn apply(&self, setting: ShortcutSetting) {
        {
            let mut status = self.lock();
            status.state = if setting.enabled {
                State::Waiting
            } else {
                State::Off
            };
            status.trigger = None;
            status.detail = None;
        }
        if self.requests.try_send(setting).is_err() {
            eprintln!("glyphkeel-app: the quick-add shortcut's queue is full or closed");
        }
    }
}

/// Starts the task that registers the shortcut, and registers the stored
/// one. Call once, in the app's setup.
pub(crate) fn start(app: &AppHandle) {
    let (requests, queue) = async_runtime::channel(16);
    let status = Arc::new(Mutex::new(Status {
        state: State::Off,
        trigger: None,
        detail: None,
    }));
    let shortcut = Shortcut {
        status: Arc::clone(&status),
        requests,
    };
    let setting = app.state::<AppSettings>().shortcut();
    if setting.enabled {
        shortcut.apply(setting);
    }
    app.manage(shortcut);
    let desktop = Desktop::from_env(|name| std::env::var(name).ok());
    async_runtime::spawn(work(app.clone(), desktop, status, queue));
}

/// The shortcut registered now, to take back before the next.
enum Active {
    #[cfg(target_os = "linux")]
    Portal(portal::Bound),
    Grab(String),
}

/// Applies the queued settings one after another.
async fn work(
    app: AppHandle,
    desktop: Desktop,
    status: Arc<Mutex<Status>>,
    mut queue: Receiver<ShortcutSetting>,
) {
    let mut active: Option<Active> = None;
    #[cfg(target_os = "linux")]
    let mut portal = portal::Portal::default();
    while let Some(setting) = queue.recv().await {
        // Only the latest setting matters.
        let mut setting = setting;
        while let Ok(later) = queue.try_recv() {
            setting = later;
        }
        match active.take() {
            #[cfg(target_os = "linux")]
            Some(Active::Portal(bound)) => portal.release(bound).await,
            Some(Active::Grab(keys)) => grab::release(&app, &keys),
            None => {}
        }
        let Some(keys) = setting
            .enabled
            .then(|| Keys::parse(&setting.keys))
            .flatten()
        else {
            set(&status, State::Off, None, None);
            continue;
        };
        let mut tried = Vec::new();
        let mut trigger = None;
        let mut detail = None;
        let state = loop {
            match next_step(&desktop, &tried) {
                Step::Done(state) => break state,
                Step::Try(Route::Portal) => {
                    #[cfg(target_os = "linux")]
                    let outcome = match portal.bind(&app, &keys).await {
                        Ok(bound) => {
                            trigger = Some(bound.trigger.clone());
                            active = Some(Active::Portal(bound));
                            Outcome::Bound
                        }
                        Err(portal::Failure::Refused) => Outcome::Refused,
                        Err(portal::Failure::Unavailable(why)) => {
                            eprintln!("glyphkeel-app: the shortcut portal: {why}");
                            detail = Some(why);
                            Outcome::Unavailable
                        }
                    };
                    #[cfg(not(target_os = "linux"))]
                    let outcome = Outcome::Unavailable;
                    tried.push((Route::Portal, outcome));
                }
                Step::Try(Route::Grab) => {
                    let text = keys.grab_text();
                    let outcome = match grab::register(&app, &text) {
                        Ok(()) => {
                            active = Some(Active::Grab(text));
                            Outcome::Bound
                        }
                        Err(why) => {
                            eprintln!("glyphkeel-app: the shortcut's key grab: {why}");
                            detail = Some(why);
                            Outcome::Unavailable
                        }
                    };
                    tried.push((Route::Grab, outcome));
                }
            }
        };
        if state == State::Refused {
            // Asking again at every start would be a nuisance: the person
            // turns it on again when they want it.
            app.state::<AppSettings>().set_shortcut(ShortcutSetting {
                enabled: false,
                ..setting
            });
        }
        if matches!(state, State::Portal | State::Grab) {
            detail = None;
        }
        set(&status, state, trigger, detail);
    }
}

fn set(status: &Mutex<Status>, state: State, trigger: Option<String>, detail: Option<String>) {
    *status.lock().unwrap_or_else(PoisonError::into_inner) = Status {
        state,
        trigger,
        detail,
    };
}

/// The shortcut's setting and where it stands, for Preferences.
#[tauri::command]
pub(crate) fn quick_add_shortcut(
    settings: TauriState<'_, AppSettings>,
    shortcut: TauriState<'_, Shortcut>,
) -> View {
    shortcut.view(&settings.shortcut())
}

/// Changes the shortcut from Preferences. The answer says `waiting` while
/// the desktop registers it, which may need the person's OK in a dialog;
/// Preferences asks again until it settles.
#[tauri::command]
pub(crate) fn set_quick_add_shortcut(
    settings: TauriState<'_, AppSettings>,
    shortcut: TauriState<'_, Shortcut>,
    enabled: bool,
    keys: String,
) -> Result<View, OpError> {
    if !CHOICES.contains(&keys.as_str()) {
        return Err(OpError::invalid_at(
            "keys",
            format!("\"{keys}\" is not one of the shortcuts to choose from."),
        ));
    }
    let setting = ShortcutSetting { enabled, keys };
    settings.set_shortcut(setting.clone());
    shortcut.apply(setting.clone());
    Ok(shortcut.view(&setting))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn kde_wayland() -> Desktop {
        Desktop::from_env(env(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "KDE"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DISPLAY", ":0"),
        ]))
    }

    fn gnome_wayland() -> Desktop {
        Desktop::from_env(env(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "GNOME"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DISPLAY", ":0"),
        ]))
    }

    fn x11() -> Desktop {
        Desktop::from_env(env(&[
            ("XDG_SESSION_TYPE", "x11"),
            ("XDG_CURRENT_DESKTOP", "XFCE"),
            ("DISPLAY", ":0"),
        ]))
    }

    #[test]
    fn every_choice_parses_and_converts_for_both_routes() {
        for choice in CHOICES {
            let keys = Keys::parse(choice).expect(choice);
            assert!(!keys.portal_trigger().is_empty());
            assert!(!keys.grab_text().is_empty());
        }
        assert!(CHOICES.contains(&DEFAULT_KEYS));
        let keys = Keys::parse("Ctrl+Alt+Space").unwrap();
        assert_eq!(keys.portal_trigger(), "CTRL+ALT+space");
        assert_eq!(keys.grab_text(), "CTRL+ALT+SPACE");
        let keys = Keys::parse("Super+Alt+N").unwrap();
        assert_eq!(keys.portal_trigger(), "ALT+LOGO+n");
        assert_eq!(keys.grab_text(), "ALT+SUPER+N");
        assert_eq!(
            Keys::parse("Ctrl+Alt+Shift+F10").unwrap().portal_trigger(),
            "CTRL+ALT+SHIFT+F10"
        );
    }

    #[test]
    fn keys_that_would_swallow_typing_or_are_unknown_are_refused() {
        for text in [
            "",
            "N",
            "Shift+N",
            "Ctrl+",
            "Ctrl+Ctrl+N",
            "Hyper+N",
            "Ctrl+Tab",
            "Ctrl+F13",
            "Ctrl+NN",
        ] {
            assert!(Keys::parse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn the_session_is_read_from_the_environment() {
        let desktop = kde_wayland();
        assert_eq!(desktop.session, Session::Wayland);
        assert!(desktop.kde && desktop.x_display);
        assert!(desktop.grabs_work());
        assert!(!gnome_wayland().grabs_work());
        assert!(x11().grabs_work());
        // Without XDG_SESSION_TYPE, the displays tell.
        let guessed = Desktop::from_env(env(&[("WAYLAND_DISPLAY", "wayland-1")]));
        assert_eq!(guessed.session, Session::Wayland);
        assert!(!guessed.grabs_work());
        let plasma = Desktop::from_env(env(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("XDG_CURRENT_DESKTOP", "ubuntu:KDE"),
        ]));
        assert!(plasma.kde);
        // Without XWayland there is nothing to grab on.
        assert!(!plasma.grabs_work());
        assert_eq!(Desktop::from_env(env(&[])).session, Session::Other);
    }

    #[test]
    fn the_portal_comes_first_then_a_grab_where_it_works() {
        let kde = kde_wayland();
        assert_eq!(next_step(&kde, &[]), Step::Try(Route::Portal));
        assert_eq!(
            next_step(&kde, &[(Route::Portal, Outcome::Bound)]),
            Step::Done(State::Portal)
        );
        assert_eq!(
            next_step(&kde, &[(Route::Portal, Outcome::Unavailable)]),
            Step::Try(Route::Grab)
        );
        assert_eq!(
            next_step(
                &kde,
                &[
                    (Route::Portal, Outcome::Unavailable),
                    (Route::Grab, Outcome::Bound)
                ]
            ),
            Step::Done(State::Grab)
        );
        assert_eq!(
            next_step(
                &kde,
                &[
                    (Route::Portal, Outcome::Unavailable),
                    (Route::Grab, Outcome::Unavailable)
                ]
            ),
            Step::Done(State::Manual)
        );
    }

    #[test]
    fn a_refusal_ends_it_without_trying_the_grab() {
        assert_eq!(
            next_step(&kde_wayland(), &[(Route::Portal, Outcome::Refused)]),
            Step::Done(State::Refused)
        );
        assert_eq!(
            next_step(&x11(), &[(Route::Portal, Outcome::Refused)]),
            Step::Done(State::Refused)
        );
    }

    #[test]
    fn where_grabs_dont_work_the_person_binds_the_command() {
        let gnome = gnome_wayland();
        assert_eq!(next_step(&gnome, &[]), Step::Try(Route::Portal));
        assert_eq!(
            next_step(&gnome, &[(Route::Portal, Outcome::Unavailable)]),
            Step::Done(State::Manual)
        );
        assert_eq!(
            next_step(&x11(), &[(Route::Portal, Outcome::Unavailable)]),
            Step::Try(Route::Grab)
        );
    }

    #[test]
    fn a_forced_route_is_the_only_one_tried() {
        let mut desktop = kde_wayland();
        desktop.forced = Some(Forced::Grab);
        assert_eq!(next_step(&desktop, &[]), Step::Try(Route::Grab));
        assert_eq!(
            next_step(&desktop, &[(Route::Grab, Outcome::Unavailable)]),
            Step::Done(State::Manual)
        );
        desktop.forced = Some(Forced::Manual);
        assert_eq!(next_step(&desktop, &[]), Step::Done(State::Manual));
        let forced = Desktop::from_env(env(&[("GLYPHKEEL_SHORTCUT_ROUTE", "portal")]));
        assert_eq!(forced.forced, Some(Forced::Portal));
    }

    #[test]
    fn the_view_serializes_as_the_page_reads_it() {
        let view = View {
            enabled: true,
            keys: "Ctrl+Alt+N".into(),
            choices: vec!["Ctrl+Alt+N".into()],
            state: State::Waiting,
            trigger: None,
            detail: None,
            command: COMMAND.into(),
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["state"], "waiting");
        assert_eq!(json["command"], "glyphkeel-app --quick-add");
        assert!(json.get("trigger").is_none());
    }

    /// The portal fixes a connection's app ID at its first call, so the
    /// host-app registration in `portal` must come before any other portal
    /// use. Keeping every portal call in that one file makes it checkable.
    #[test]
    fn only_the_portal_module_talks_to_the_portal() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut offenders = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs")
                    || path.ends_with("shortcut/portal.rs")
                    || path.ends_with("shortcut/mod.rs")
                {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("ashpd") {
                    offenders.push(path.display().to_string());
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "portal calls outside shortcut/portal.rs: {offenders:?}"
        );
    }
}
