// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The main window's size, position and maximized state, remembered between
//! runs in `main-window.json`: in `$XDG_STATE_HOME/glyphkeel` on Linux, and
//! in the person's configuration folder on Windows (`%APPDATA%\Glyphkeel`),
//! next to the app's settings. When the configuration folder is moved
//! elsewhere (`GLYPHKEEL_CONFIG_DIR`), the file goes there on every platform.
//!
//! The size stored is the size of the window's content, the page. GTK
//! reports the size of a window with a client-side frame including its shadow
//! margins, which would make the window grow on every restore; the content's
//! own allocation has no such margins. Wayland compositors don't let apps
//! read or choose a window's position, so it is kept only on X11 and on
//! Windows. There a position is used again only while some screen still
//! shows the window's top edge.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The default content size, from the design.
pub(crate) const DEFAULT_SIZE: (f64, f64) = (1280.0, 800.0);
/// The smallest content size the window allows.
pub(crate) const MIN_SIZE: (f64, f64) = (760.0, 520.0);
/// Anything larger than this is taken as a damaged file.
const MAX_SIDE: f64 = 16_384.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct Geometry {
    /// Content width and height in logical pixels, when not maximized.
    pub(crate) width: f64,
    pub(crate) height: f64,
    /// Position of the window on X11, in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<(i32, i32)>,
    #[serde(default)]
    pub(crate) maximized: bool,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            width: DEFAULT_SIZE.0,
            height: DEFAULT_SIZE.1,
            position: None,
            maximized: false,
        }
    }
}

impl Geometry {
    /// Keeps a stored geometry within sane limits; a size that makes no
    /// sense falls back to the default.
    pub(crate) fn sanitized(self) -> Self {
        let side_ok = |value: f64, min: f64| (min / 4.0..=MAX_SIDE).contains(&value);
        let (width, height) = if side_ok(self.width, MIN_SIZE.0) && side_ok(self.height, MIN_SIZE.1)
        {
            (self.width.max(MIN_SIZE.0), self.height.max(MIN_SIZE.1))
        } else {
            DEFAULT_SIZE
        };
        let position = self.position.filter(|(x, y)| {
            x.unsigned_abs() <= MAX_SIDE as u32 && y.unsigned_abs() <= MAX_SIDE as u32
        });
        Self {
            width,
            height,
            position,
            maximized: self.maximized,
        }
    }
}

/// The file's name.
const FILE_NAME: &str = "main-window.json";

/// Where the geometry is kept, or `None` without a home folder.
pub(crate) fn file() -> Option<PathBuf> {
    let config_moved = std::env::var_os(gk_store::CONFIG_DIR_ENV)
        .map(PathBuf::from)
        .is_some_and(|path| path.is_absolute());
    let folder = if cfg!(windows) || config_moved {
        gk_store::config_dir()
    } else {
        state_folder(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
    };
    Some(folder?.join(FILE_NAME))
}

/// The app's folder in `$XDG_STATE_HOME` when that is an absolute path,
/// else in `~/.local/state`.
fn state_folder(state_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let state_home = state_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state_home.join("glyphkeel"))
}

#[cfg(any(not(target_os = "linux"), test))]
/// A screen's work area, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[cfg(any(not(target_os = "linux"), test))]
/// How much of the window's top edge a screen must show for a stored
/// position to be used again: enough to grab the title bar.
const GRAB: (f64, f64) = (120.0, 24.0);

#[cfg(any(not(target_os = "linux"), test))]
/// Whether a window at `position` with content width `width` can still be
/// reached on one of `screens`: some screen shows a piece of its top edge
/// large enough to drag it by. A screen taken away since leaves the window
/// where the system puts new windows.
pub(crate) fn reachable(position: (i32, i32), width: f64, screens: &[Area]) -> bool {
    let (x, y) = (f64::from(position.0), f64::from(position.1));
    screens.iter().any(|screen| {
        let left = x.max(screen.x);
        let right = (x + width).min(screen.x + screen.width);
        right - left >= GRAB.0.min(width) && y >= screen.y && y + GRAB.1 <= screen.y + screen.height
    })
}

/// What the window looks like at one moment, as Tauri reports it.
#[cfg(any(not(target_os = "linux"), test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Seen {
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    /// The content's size, in logical pixels.
    pub(crate) size: (f64, f64),
    /// The window's position, in logical pixels.
    pub(crate) position: Option<(i32, i32)>,
}

#[cfg(any(not(target_os = "linux"), test))]
impl Geometry {
    /// Takes in what the window looks like now: the maximized state always,
    /// the size and position only in the normal state, so that the last
    /// normal geometry is kept for when the window is restored.
    pub(crate) fn follow(&mut self, seen: Seen) {
        self.maximized = seen.maximized;
        if seen.maximized || seen.minimized || seen.size.0 <= 1.0 || seen.size.1 <= 1.0 {
            return;
        }
        self.width = seen.size.0;
        self.height = seen.size.1;
        if seen.position.is_some() {
            self.position = seen.position;
        }
    }
}

/// The stored geometry, or the default when there is none or it can't be read.
pub(crate) fn load(path: &Path) -> Geometry {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Geometry>(&bytes) {
            Ok(geometry) => geometry.sanitized(),
            Err(err) => {
                eprintln!(
                    "glyphkeel-app: ignoring the stored window geometry in {}: {err}",
                    path.display()
                );
                Geometry::default()
            }
        },
        Err(_) => Geometry::default(),
    }
}

/// Stores the geometry, replacing the file in one step.
pub(crate) fn save(path: &Path, geometry: &Geometry) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(geometry).map_err(std::io::Error::other)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// Follows the window's geometry while it runs, and stores it on request.
#[cfg(target_os = "linux")]
pub(crate) mod track {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    use gtk::prelude::*;

    use super::{Geometry, save};

    /// Records the content size and position whenever GTK lays out the
    /// window, skipping maximized and minimized states so that the last
    /// normal geometry is kept. Returns the function that stores it.
    pub(crate) fn follow(
        window: &gtk::ApplicationWindow,
        start: Geometry,
        path: PathBuf,
    ) -> impl Fn() + use<> {
        let current = Rc::new(RefCell::new(start));
        let on_wayland = window.display().type_().name() == "GdkWaylandDisplay";

        if let Some(content) = window.child() {
            let window = window.clone();
            let current = Rc::clone(&current);
            content.connect_size_allocate(move |_, allocation| {
                let mut geometry = current.borrow_mut();
                geometry.maximized = window.is_maximized();
                let minimized = window
                    .window()
                    .is_some_and(|gdk| gdk.state().contains(gtk::gdk::WindowState::ICONIFIED));
                if geometry.maximized
                    || minimized
                    || allocation.width() <= 1
                    || allocation.height() <= 1
                {
                    return;
                }
                geometry.width = f64::from(allocation.width());
                geometry.height = f64::from(allocation.height());
                if !on_wayland {
                    geometry.position = Some(window.position());
                }
            });
        }
        {
            let current = Rc::clone(&current);
            window.connect_window_state_event(move |window, _| {
                current.borrow_mut().maximized = window.is_maximized();
                gtk::glib::Propagation::Proceed
            });
        }

        move || {
            let geometry = *current.borrow();
            if let Err(err) = save(&path, &geometry) {
                eprintln!(
                    "glyphkeel-app: cannot store the window geometry in {}: {err}",
                    path.display()
                );
            }
        }
    }
}

/// Follows the window's geometry through Tauri's window events, where GTK
/// isn't there to ask.
#[cfg(not(target_os = "linux"))]
pub(crate) mod track {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, PoisonError};

    use tauri::{WebviewWindow, WindowEvent};

    use super::{Area, Geometry, Seen, reachable, save};

    /// What `window` looks like now, or `None` when it can't be asked.
    fn seen(window: &WebviewWindow) -> Option<Seen> {
        let scale = window.scale_factor().ok()?;
        let size = window.inner_size().ok()?.to_logical::<f64>(scale);
        let position = window
            .outer_position()
            .ok()
            .map(|position| position.to_logical::<i32>(scale));
        Some(Seen {
            maximized: window.is_maximized().unwrap_or(false),
            minimized: window.is_minimized().unwrap_or(false),
            size: (size.width, size.height),
            position: position.map(|position| (position.x, position.y)),
        })
    }

    /// Records the geometry whenever the window is moved or resized, and
    /// stores it when the window goes, however it goes.
    pub(crate) fn follow(window: &WebviewWindow, start: Geometry, path: PathBuf) {
        let current = Arc::new(Mutex::new(start));
        let followed = window.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Resized(_)
            | WindowEvent::Moved(_)
            | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(seen) = seen(&followed) {
                    current
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .follow(seen);
                }
            }
            WindowEvent::Destroyed => {
                let geometry = *current.lock().unwrap_or_else(PoisonError::into_inner);
                if let Err(err) = save(&path, &geometry) {
                    eprintln!(
                        "glyphkeel-app: cannot store the window geometry in {}: {err}",
                        path.display()
                    );
                }
            }
            _ => {}
        });
    }

    /// The stored position, if one of the screens still shows it; the
    /// screens' work areas in logical pixels.
    pub(crate) fn position(app: &tauri::AppHandle, geometry: &Geometry) -> Option<(f64, f64)> {
        let position = geometry.position?;
        let screens: Vec<Area> = app
            .available_monitors()
            .ok()?
            .iter()
            .map(|monitor| {
                let scale = monitor.scale_factor();
                let area = monitor.work_area();
                let origin = area.position.to_logical::<f64>(scale);
                let size = area.size.to_logical::<f64>(scale);
                Area {
                    x: origin.x,
                    y: origin.y,
                    width: size.width,
                    height: size.height,
                }
            })
            .collect();
        reachable(position, geometry.width, &screens)
            .then(|| (f64::from(position.0), f64::from(position.1)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, DEFAULT_SIZE, Geometry, MIN_SIZE, Seen, load, reachable, save};

    /// XDG's paths are Unix paths; on Windows `/s` isn't absolute.
    #[test]
    #[cfg(not(windows))]
    fn the_state_folder_follows_xdg() {
        use std::ffi::OsString;
        use std::path::PathBuf;

        use super::state_folder;

        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            state_folder(os("/s"), os("/home/a")),
            Some(PathBuf::from("/s/glyphkeel"))
        );
        assert_eq!(
            state_folder(os("relative"), os("/home/a")),
            Some(PathBuf::from("/home/a/.local/state/glyphkeel"))
        );
        assert_eq!(state_folder(None, None), None);
    }

    #[test]
    fn the_file_is_in_the_moved_configuration_folder_or_the_platforms_own() {
        let file = super::file();
        if std::env::var_os(gk_store::CONFIG_DIR_ENV).is_some() || cfg!(windows) {
            assert_eq!(
                file,
                gk_store::config_dir().map(|dir| dir.join("main-window.json"))
            );
        } else if let Some(file) = file {
            assert!(
                file.ends_with("glyphkeel/main-window.json"),
                "{}",
                file.display()
            );
        }
    }

    #[test]
    fn the_last_normal_geometry_is_kept() {
        let mut geometry = Geometry::default();
        let normal = Seen {
            maximized: false,
            minimized: false,
            size: (1000.0, 700.0),
            position: Some((30, 40)),
        };
        geometry.follow(normal);
        assert_eq!((geometry.width, geometry.height), (1000.0, 700.0));
        assert_eq!(geometry.position, Some((30, 40)));
        // Maximized, then minimized: the normal size and place stay.
        geometry.follow(Seen {
            maximized: true,
            size: (2560.0, 1400.0),
            position: Some((0, 0)),
            ..normal
        });
        assert!(geometry.maximized);
        assert_eq!((geometry.width, geometry.height), (1000.0, 700.0));
        geometry.follow(Seen {
            minimized: true,
            size: (0.0, 0.0),
            position: Some((-32000, -32000)),
            ..normal
        });
        assert!(!geometry.maximized);
        assert_eq!(geometry.position, Some((30, 40)));
    }

    #[test]
    fn a_position_is_used_again_only_where_a_screen_shows_it() {
        let screens = [
            Area {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1040.0,
            },
            Area {
                x: 1920.0,
                y: -200.0,
                width: 1280.0,
                height: 984.0,
            },
        ];
        assert!(reachable((100, 100), 1280.0, &screens));
        assert!(reachable((2000, -150), 1000.0, &screens), "second screen");
        // Mostly off the left edge, but enough of the top edge shows.
        assert!(reachable((-1100, 50), 1280.0, &screens));
        assert!(
            !reachable((-1250, 50), 1280.0, &screens),
            "too little shows"
        );
        assert!(!reachable((100, -100), 1280.0, &screens), "top edge above");
        assert!(!reachable((100, 1030), 1280.0, &screens), "top edge below");
        assert!(!reachable((5000, 100), 1280.0, &screens), "screen gone");
        assert!(!reachable((100, 100), 1280.0, &[]));
    }

    #[test]
    fn a_stored_geometry_comes_back_unchanged() {
        let dir = std::env::temp_dir().join(format!("gk-app-window-state-{}", std::process::id()));
        let path = dir.join("main-window.json");
        let geometry = Geometry {
            width: 1000.0,
            height: 640.0,
            position: Some((40, 60)),
            maximized: true,
        };
        save(&path, &geometry).unwrap();
        assert_eq!(load(&path), geometry);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_or_damaged_file_gives_the_default() {
        let dir =
            std::env::temp_dir().join(format!("gk-app-window-state-bad-{}", std::process::id()));
        let path = dir.join("main-window.json");
        assert_eq!(load(&path), Geometry::default());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load(&path), Geometry::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sizes_are_kept_within_limits() {
        let tiny = Geometry {
            width: 400.0,
            height: 300.0,
            position: None,
            maximized: false,
        }
        .sanitized();
        assert_eq!((tiny.width, tiny.height), MIN_SIZE);
        let absurd = Geometry {
            width: f64::NAN,
            height: 1e9,
            position: Some((1 << 30, 0)),
            maximized: false,
        }
        .sanitized();
        assert_eq!((absurd.width, absurd.height), DEFAULT_SIZE);
        assert_eq!(absurd.position, None);
    }
}
