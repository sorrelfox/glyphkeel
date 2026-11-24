// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! How the web view draws: the workarounds for graphics drivers that leave
//! WebKitGTK's window blank, and the "safe rendering" switch.
//!
//! WebKitGTK composites on the GPU when it can. Some drivers, NVIDIA's
//! proprietary one above all, then show a blank or white window or end the
//! Wayland connection with a protocol error. WebKitGTK reads its rendering
//! settings from the environment once, when its processes start, so they
//! are added to the app's environment before anything else happens (see
//! [`crate::memory_env`]):
//!
//! - With NVIDIA's driver on Wayland, explicit sync is turned off, which
//!   fixes the protocol error without slowing drawing down.
//! - With NVIDIA's driver on X11, the DMA-BUF renderer is turned off. It is
//!   slower, but reliable.
//! - Safe rendering, chosen in Preferences and kept in the app's settings,
//!   or asked for once with `--safe-graphics`, turns GPU compositing off
//!   altogether and paints on the CPU. It is for any other driver that
//!   draws badly, and costs smoothness and memory.
//!
//! A value the person has set in the environment always wins.

use std::path::Path;

/// The argument that asks for safe rendering for one start.
pub(crate) const SAFE_ARG: &str = "--safe-graphics";

/// What safe rendering sets: no compositing on the GPU, no DMA-BUF renderer,
/// and painting on the CPU.
const SAFE: [(&str, &str); 3] = [
    ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
    ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
    ("WEBKIT_SKIA_ENABLE_CPU_RENDERING", "1"),
];
/// NVIDIA's driver on Wayland.
const NVIDIA_WAYLAND: (&str, &str) = ("__NV_DISABLE_EXPLICIT_SYNC", "1");
/// NVIDIA's driver on X11.
const NVIDIA_X11: (&str, &str) = ("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

/// Every name this module may add, so that programs the app opens for the
/// person get their environment without them.
pub(crate) const NAMES: [&str; 4] = [
    "WEBKIT_DISABLE_COMPOSITING_MODE",
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "WEBKIT_SKIA_ENABLE_CPU_RENDERING",
    "__NV_DISABLE_EXPLICIT_SYNC",
];

/// The machine and session the app starts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Session {
    /// NVIDIA's proprietary driver is loaded.
    pub(crate) nvidia: bool,
    /// The web view draws through Wayland, not X11.
    pub(crate) wayland: bool,
}

impl Session {
    /// The session as the environment and `/proc` describe it.
    pub(crate) fn detect() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        Self {
            nvidia: Path::new("/proc/driver/nvidia/version").exists(),
            wayland: is_wayland(
                var("WAYLAND_DISPLAY").as_deref(),
                var("GDK_BACKEND").as_deref(),
            ),
        }
    }
}

/// Whether GTK draws through Wayland: a Wayland display, unless GTK is
/// told to use X11 first.
fn is_wayland(display: Option<&str>, backend: Option<&str>) -> bool {
    display.is_some()
        && !backend.is_some_and(|backend| {
            backend
                .split(',')
                .next()
                .is_some_and(|first| first.trim() == "x11")
        })
}

/// The settings to start with: the driver's workaround, and with `safe`
/// the whole safe path. A name appears once.
pub(crate) fn settings(session: Session, safe: bool) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    let mut add = |setting: (&'static str, &'static str)| {
        if !out.iter().any(|(name, _)| *name == setting.0) {
            out.push(setting);
        }
    };
    if session.nvidia {
        add(if session.wayland {
            NVIDIA_WAYLAND
        } else {
            NVIDIA_X11
        });
    }
    if safe {
        SAFE.into_iter().for_each(&mut add);
    }
    out
}

/// Whether this start asks for safe rendering on its command line.
pub(crate) fn asked_on_command_line() -> bool {
    std::env::args().skip(1).any(|arg| arg == SAFE_ARG)
}

/// Whether this process runs with safe rendering: the app added its
/// settings, or the person set them.
pub(crate) fn safe_in_use() -> bool {
    std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_ok_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMD_WAYLAND: Session = Session {
        nvidia: false,
        wayland: true,
    };

    #[test]
    fn other_drivers_get_nothing_unless_safe_rendering_is_on() {
        assert!(settings(AMD_WAYLAND, false).is_empty());
        assert_eq!(settings(AMD_WAYLAND, true), SAFE.to_vec());
    }

    #[test]
    fn nvidia_gets_the_workaround_of_its_session() {
        let wayland = Session {
            nvidia: true,
            wayland: true,
        };
        let x11 = Session {
            nvidia: true,
            wayland: false,
        };
        assert_eq!(settings(wayland, false), [NVIDIA_WAYLAND]);
        assert_eq!(settings(x11, false), [NVIDIA_X11]);
        // Safe rendering adds the rest, each name once.
        let safe = settings(x11, true);
        assert_eq!(safe.len(), 3);
        assert_eq!(safe[0], NVIDIA_X11);
        let mut with_sync = settings(wayland, true);
        assert_eq!(with_sync.remove(0), NVIDIA_WAYLAND);
        assert_eq!(with_sync, SAFE.to_vec());
    }

    #[test]
    fn every_setting_is_named_for_the_programs_the_app_opens() {
        for session in [
            AMD_WAYLAND,
            Session {
                nvidia: true,
                wayland: false,
            },
        ] {
            for (name, _) in settings(session, true) {
                assert!(NAMES.contains(&name), "{name}");
            }
        }
        assert!(NAMES.contains(&NVIDIA_WAYLAND.0));
    }

    #[test]
    fn wayland_is_the_display_unless_gtk_is_told_x11_first() {
        assert!(is_wayland(Some("wayland-0"), None));
        assert!(is_wayland(Some("wayland-0"), Some("wayland,x11")));
        assert!(!is_wayland(Some("wayland-0"), Some("x11")));
        assert!(!is_wayland(Some("wayland-0"), Some("x11,wayland")));
        assert!(!is_wayland(None, None));
    }
}
