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
//! - Without a GPU to render on (no render node, as in a virtual machine
//!   or a container), the DMA-BUF renderer is turned off too. It would hand
//!   every frame over in buffers the software renderer fills in the web
//!   view's own memory: about 200 MB more for a large board, and slower
//!   scrolling, than with shared-memory buffers.
//! - Safe rendering, chosen in Preferences and kept in the app's settings,
//!   or asked for once with `--safe-graphics`, turns GPU compositing off
//!   altogether and paints on the CPU. It is for any other driver that
//!   draws badly, and costs smoothness and memory.
//!
//! A value the person has set in the environment always wins.
//!
//! On Windows the page is drawn by WebView2, which takes its rendering
//! switches as browser arguments when its environment is made, so nothing
//! has to be started again: safe rendering adds `--disable-gpu` to them. The
//! switches have to be the same for every web view that shares a data
//! folder, so safe rendering is decided once per run and gets a data folder
//! of its own. The web views are also built here without the WebView2
//! features the app never uses (see [`web_view`]).

#[cfg(target_os = "linux")]
use std::path::Path;

/// The argument that asks for safe rendering for one start.
pub(crate) const SAFE_ARG: &str = "--safe-graphics";

/// What safe rendering sets: no compositing on the GPU, no DMA-BUF renderer,
/// and painting on the CPU.
#[cfg(target_os = "linux")]
const SAFE: [(&str, &str); 3] = [
    ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
    ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
    ("WEBKIT_SKIA_ENABLE_CPU_RENDERING", "1"),
];
/// NVIDIA's driver on Wayland.
#[cfg(target_os = "linux")]
const NVIDIA_WAYLAND: (&str, &str) = ("__NV_DISABLE_EXPLICIT_SYNC", "1");
/// NVIDIA's driver on X11.
#[cfg(target_os = "linux")]
const NVIDIA_X11: (&str, &str) = ("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
/// No GPU to render on.
#[cfg(target_os = "linux")]
const NO_RENDER_NODE: (&str, &str) = ("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

/// Every name this module may add, so that programs the app opens for the
/// person get their environment without them.
#[cfg(target_os = "linux")]
pub(crate) const NAMES: [&str; 4] = [
    "WEBKIT_DISABLE_COMPOSITING_MODE",
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "WEBKIT_SKIA_ENABLE_CPU_RENDERING",
    "__NV_DISABLE_EXPLICIT_SYNC",
];

/// The machine and session the app starts in.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Session {
    /// NVIDIA's proprietary driver is loaded.
    pub(crate) nvidia: bool,
    /// The web view draws through Wayland, not X11.
    pub(crate) wayland: bool,
    /// A GPU can render: the kernel offers a DRM render node.
    pub(crate) render_node: bool,
}

#[cfg(target_os = "linux")]
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
            render_node: has_render_node(Path::new("/dev/dri")),
        }
    }
}

/// Whether `dri` (the kernel's DRM device folder) holds a render node,
/// `renderD128` and up. A display-only device (`card0` of a simple frame
/// buffer) isn't one.
#[cfg(target_os = "linux")]
fn has_render_node(dri: &Path) -> bool {
    std::fs::read_dir(dri).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
    })
}

/// Whether GTK draws through Wayland: a Wayland display, unless GTK is
/// told to use X11 first.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
    if !session.render_node {
        add(NO_RENDER_NODE);
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

/// Whether this process runs with safe rendering: on Linux the app added
/// its settings, or the person set them; on Windows it was asked for when
/// the run began.
pub(crate) fn safe_in_use() -> bool {
    #[cfg(windows)]
    {
        safe_this_run()
    }
    #[cfg(not(windows))]
    {
        std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_ok_and(|value| value == "1")
    }
}

/// Whether this run draws safely: asked for on the command line or kept in
/// the app's settings, decided on first use and kept for the whole run.
#[cfg(windows)]
fn safe_this_run() -> bool {
    static SAFE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SAFE.get_or_init(|| {
        asked_on_command_line()
            || crate::app_settings::file()
                .is_some_and(|file| crate::app_settings::load(&file).safe_rendering)
    })
}

/// The switches WebView2 gets without the app's own: wry passes these only
/// when the app passes none, so they are repeated here. They keep the
/// browser's own pop-ups and SmartScreen's look-ups out.
#[cfg(any(windows, test))]
const WEBVIEW2_DEFAULT_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// What safe rendering adds on Windows: no GPU, no GPU compositing.
#[cfg(any(windows, test))]
const WEBVIEW2_SAFE_ARGS: &str = "--disable-gpu --disable-gpu-compositing";

/// The data folder of safe rendering's web views, inside Tauri's own:
/// WebView2 refuses a data folder that is in use with other switches.
#[cfg(windows)]
const SAFE_DATA_FOLDER: &str = "safe-rendering";

/// WebView2's browser arguments, with or without safe rendering.
#[cfg(any(windows, test))]
pub(crate) fn browser_args(safe: bool) -> String {
    if safe {
        format!("{WEBVIEW2_DEFAULT_ARGS} {WEBVIEW2_SAFE_ARGS}")
    } else {
        WEBVIEW2_DEFAULT_ARGS.to_owned()
    }
}

/// Sets up a web view the way this platform needs. On Windows: the
/// rendering switches of this run, and WebView2's own features off that the
/// app never uses and that could keep or send what is typed (form autofill;
/// saving passwords is off by default and stays off), no developer tools in
/// release builds and no browser extensions. Its status bar is off already,
/// the page keeps the browser's context menu and keys away (`native.ts`),
/// and the page's navigation is held to the app by [`crate::page`]. On
/// Linux nothing changes here: WebKitGTK reads its switches from the
/// environment, and its settings are tightened once the view exists
/// (`webkit`).
pub(crate) fn web_view<'a>(
    app: &tauri::AppHandle,
    builder: tauri::WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle>,
) -> tauri::WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle> {
    #[cfg(windows)]
    {
        use tauri::Manager;

        let safe = safe_this_run();
        let mut builder = builder
            .additional_browser_args(&browser_args(safe))
            .general_autofill_enabled(false)
            .browser_extensions_enabled(false)
            .devtools(cfg!(debug_assertions));
        if safe {
            match app.path().app_local_data_dir() {
                Ok(dir) => builder = builder.data_directory(dir.join(SAFE_DATA_FOLDER)),
                Err(err) => {
                    eprintln!("glyphkeel-app: no data folder of its own for safe rendering: {err}")
                }
            }
        }
        builder
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        builder
    }
}

/// The arguments a restart from Preferences starts with: this run's,
/// without those that only made sense for how this run began (in the
/// background, for quick add, with safe rendering asked for once). The
/// restart is for the settings just changed, and the person is looking.
#[cfg(any(not(target_os = "linux"), test))]
pub(crate) fn restart_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Vec<std::ffi::OsString> {
    args.into_iter()
        .filter(|arg| {
            ![crate::autostart::BACKGROUND_ARG, "--quick-add", SAFE_ARG]
                .iter()
                .any(|once| arg == *once)
        })
        .collect()
}

/// Starts the app again after this run ends, for a restart from
/// Preferences; on Linux the process replaces itself instead
/// (`memory_env::start_again`). Call when the app exits, after the single
/// instance has been let go, or the new run would hand itself to this one.
#[cfg(not(target_os = "linux"))]
pub(crate) fn start_again() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .args(restart_args(std::env::args_os().skip(1)))
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod web_view_tests {
    use std::ffi::OsString;

    use super::{SAFE_ARG, browser_args, restart_args};

    #[test]
    fn safe_rendering_adds_its_switches_to_webview2s_own() {
        let normal = browser_args(false);
        assert!(normal.contains("msSmartScreenProtection"), "{normal}");
        assert!(!normal.contains("--disable-gpu"), "{normal}");
        let safe = browser_args(true);
        assert!(safe.starts_with(&normal), "{safe}");
        assert!(safe.contains("--disable-gpu "), "{safe}");
        assert!(safe.ends_with("--disable-gpu-compositing"), "{safe}");
    }

    #[test]
    fn a_restart_leaves_out_what_only_this_start_asked_for() {
        let args = [
            "--background",
            "--quick-add",
            SAFE_ARG,
            "glyphkeel://open/TM-1",
            "--x",
        ]
        .map(OsString::from);
        assert_eq!(
            restart_args(args),
            ["glyphkeel://open/TM-1", "--x"].map(OsString::from)
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    const AMD_WAYLAND: Session = Session {
        nvidia: false,
        wayland: true,
        render_node: true,
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
            render_node: true,
        };
        let x11 = Session {
            nvidia: true,
            wayland: false,
            render_node: true,
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
                render_node: true,
            },
            Session {
                nvidia: false,
                wayland: false,
                render_node: false,
            },
        ] {
            for (name, _) in settings(session, true) {
                assert!(NAMES.contains(&name), "{name}");
            }
        }
        assert!(NAMES.contains(&NVIDIA_WAYLAND.0));
    }

    #[test]
    fn without_a_render_node_the_dma_buf_renderer_is_off() {
        let software = Session {
            render_node: false,
            ..AMD_WAYLAND
        };
        assert_eq!(settings(software, false), [NO_RENDER_NODE]);
        // Safe rendering names it once.
        let safe = settings(software, true);
        assert_eq!(safe.len(), SAFE.len());
        assert!(SAFE.iter().all(|setting| safe.contains(setting)));
    }

    #[test]
    fn a_render_node_is_a_renderd_entry() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_render_node(dir.path()));
        assert!(!has_render_node(&dir.path().join("missing")));
        std::fs::write(dir.path().join("card0"), "").unwrap();
        assert!(!has_render_node(dir.path()));
        std::fs::write(dir.path().join("renderD128"), "").unwrap();
        assert!(has_render_node(dir.path()));
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
