// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The main window's size, position and maximized state, remembered between
//! runs in `$XDG_STATE_HOME/glyphkeel/main-window.json`.
//!
//! The size stored is the size of the window's content, the page. GTK
//! reports the size of a window with a client-side frame including its shadow
//! margins, which would make the window grow on every restore; the content's
//! own allocation has no such margins. Wayland compositors don't let apps
//! read or choose a window's position, so it is kept only on X11.

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

/// Where the geometry is kept, or `None` without a home folder.
pub(crate) fn file() -> Option<PathBuf> {
    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state_home.join("glyphkeel").join("main-window.json"))
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
#[cfg_attr(
    not(any(target_os = "linux", test)),
    expect(dead_code, reason = "the geometry is followed only on Linux so far")
)]
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

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SIZE, Geometry, MIN_SIZE, load, save};

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
