// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which title bar the main window gets.
//!
//! - **The app's header bar** (the default, as on GNOME): GTK keeps its
//!   client-side frame for the shadow and the resize edges, and its own title
//!   bar is replaced by an empty widget, so only the page's header bar shows.
//!   The page draws the window buttons.
//! - **The system title bar** (KDE Plasma and tiling window managers, where
//!   apps don't draw their own): the window manager decorates the window, and
//!   the page's header bar has no window buttons.
//!
//! On Windows the window always has the system title bar: it gives the
//! window Snap Layouts and the system's own buttons, which a page can't draw
//! with the same behaviour. The page learns it from the start data
//! (`titleBar: "system"`, and `platform: "windows"`), and leaves out its
//! window buttons.

use serde::Serialize;

/// Overrides the choice until Preferences can store it: `app` or `system`.
pub(crate) const OVERRIDE_VAR: &str = "GLYPHKEEL_TITLE_BAR";

/// Desktops whose apps leave the title bar to the window manager. Matched
/// against the entries of `XDG_CURRENT_DESKTOP`, ignoring case.
const SYSTEM_TITLE_BAR_DESKTOPS: &[&str] = &[
    "KDE",
    "LXQt",
    "sway",
    "Hyprland",
    "i3",
    "bspwm",
    "river",
    "niri",
    "awesome",
    "qtile",
    "dwm",
    "xmonad",
    "herbstluftwm",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TitleBar {
    /// The page's header bar with its own window buttons.
    App,
    /// The window manager's title bar.
    System,
}

impl TitleBar {
    /// Chooses from the override, if it names a known value, and otherwise
    /// from the desktop.
    pub(crate) fn choose(override_value: Option<&str>, current_desktop: Option<&str>) -> Self {
        match override_value.map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("app") => return Self::App,
            Some(value) if value.eq_ignore_ascii_case("system") => return Self::System,
            _ => {}
        }
        let system = current_desktop.is_some_and(|desktops| {
            desktops.split(':').any(|desktop| {
                SYSTEM_TITLE_BAR_DESKTOPS
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(desktop.trim()))
            })
        });
        if system { Self::System } else { Self::App }
    }

    /// The choice on a platform: Windows always has the system title bar,
    /// whatever the override says.
    pub(crate) fn choose_on(
        windows: bool,
        override_value: Option<&str>,
        current_desktop: Option<&str>,
    ) -> Self {
        if windows {
            Self::System
        } else {
            Self::choose(override_value, current_desktop)
        }
    }

    /// The choice for this process, from its environment.
    pub(crate) fn from_env() -> Self {
        let override_value = std::env::var(OVERRIDE_VAR).ok();
        if cfg!(windows) {
            if override_value.is_some() {
                eprintln!(
                    "glyphkeel-app: {OVERRIDE_VAR} is ignored on Windows, where the window has the system title bar"
                );
            }
            return Self::choose_on(true, None, None);
        }
        if let Some(value) = &override_value
            && !value.trim().eq_ignore_ascii_case("app")
            && !value.trim().eq_ignore_ascii_case("system")
        {
            eprintln!(
                "glyphkeel-app: {OVERRIDE_VAR}={value:?} is neither \"app\" nor \"system\"; ignored"
            );
        }
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
        Self::choose(override_value.as_deref(), desktop.as_deref())
    }
}

/// Replaces GTK's title bar with an empty widget, keeping the client-side
/// frame. Must run before the window is first shown.
#[cfg(target_os = "linux")]
pub(crate) fn hide_gtk_title_bar(window: &gtk::ApplicationWindow) {
    use gtk::prelude::*;

    let empty = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    // Adwaita gives every title bar a minimum height and a background, which
    // would show as an empty strip above the page.
    let css = gtk::CssProvider::new();
    if let Err(err) = css.load_from_data(
        b"box { min-height: 0; padding: 0; margin: 0; border: none; background: none; box-shadow: none; }",
    ) {
        eprintln!("glyphkeel-app: cannot style the empty title bar: {err}");
    }
    empty
        .style_context()
        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    empty.show();
    window.set_titlebar(Some(&empty));
}

#[cfg(test)]
mod tests {
    use super::TitleBar;

    #[test]
    fn gnome_and_unknown_desktops_get_the_app_header_bar() {
        assert_eq!(TitleBar::choose(None, Some("GNOME")), TitleBar::App);
        assert_eq!(TitleBar::choose(None, Some("ubuntu:GNOME")), TitleBar::App);
        assert_eq!(TitleBar::choose(None, Some("")), TitleBar::App);
        assert_eq!(TitleBar::choose(None, None), TitleBar::App);
    }

    #[test]
    fn kde_and_tiling_window_managers_get_the_system_title_bar() {
        assert_eq!(TitleBar::choose(None, Some("KDE")), TitleBar::System);
        assert_eq!(TitleBar::choose(None, Some("sway")), TitleBar::System);
        assert_eq!(TitleBar::choose(None, Some("Hyprland")), TitleBar::System);
        assert_eq!(TitleBar::choose(None, Some("foo:kde")), TitleBar::System);
    }

    #[test]
    fn windows_always_gets_the_system_title_bar() {
        assert_eq!(
            TitleBar::choose_on(true, Some("app"), Some("GNOME")),
            TitleBar::System
        );
        assert_eq!(TitleBar::choose_on(true, None, None), TitleBar::System);
        assert_eq!(TitleBar::choose_on(false, None, None), TitleBar::App);
        assert_eq!(
            TitleBar::choose_on(false, Some("system"), None),
            TitleBar::System
        );
    }

    #[test]
    fn the_override_wins_and_unknown_values_are_ignored() {
        assert_eq!(TitleBar::choose(Some("app"), Some("KDE")), TitleBar::App);
        assert_eq!(
            TitleBar::choose(Some(" System "), Some("GNOME")),
            TitleBar::System
        );
        assert_eq!(
            TitleBar::choose(Some("fancy"), Some("KDE")),
            TitleBar::System
        );
        assert_eq!(
            TitleBar::choose(Some("fancy"), Some("GNOME")),
            TitleBar::App
        );
    }
}
