// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Personal settings: the ones that belong to the person using this
//! computer, not to a workspace, and that the desktop app, the `glyphkeel`
//! command and `glyphkeel mcp` all follow. Several people can share one
//! workspace; each keeps their own personal settings.
//!
//! They are kept in one JSON file in the program's configuration folder
//! ([`config_dir`]):
//!
//! ```text
//! $XDG_CONFIG_HOME/glyphkeel/settings.json     (normally ~/.config/glyphkeel)
//! {
//!   "time_zone": "Europe/Berlin"
//! }
//! ```
//!
//! - `time_zone`: the zone every time is shown in, and in which "today",
//!   reminder times and the days of time entries are counted: `"system"`
//!   (the computer's zone; also when the field is left out) or an IANA name
//!   such as `"Europe/Berlin"`. Stored times stay in UTC.
//!
//! Fields this program doesn't know are kept when it writes the file, so an
//! older and a newer program can share it. A missing or damaged file means
//! the defaults. The app's own settings (`app.json`, the window and page
//! preferences of this computer) live next to it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The file name of the personal settings in [`config_dir`].
pub const USER_SETTINGS_FILE: &str = "settings.json";

/// The value of `time_zone` that means the computer's own zone.
pub const SYSTEM_TIME_ZONE: &str = "system";

/// The environment variable that puts the configuration folder, with the
/// personal settings, elsewhere: an absolute path, used as it is. The data
/// folder has its own variable, [`DATA_DIR_ENV`](crate::DATA_DIR_ENV), so
/// personal settings and shared workspace data stay apart.
pub const CONFIG_DIR_ENV: &str = "GLYPHKEEL_CONFIG_DIR";

/// The program's configuration folder: the folder [`CONFIG_DIR_ENV`] names,
/// else `$XDG_CONFIG_HOME/glyphkeel` (normally `~/.config/glyphkeel`) on
/// Linux and `%APPDATA%\Glyphkeel` on Windows. `None` when no home folder
/// can be found.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = crate::layout::folder_from_env(CONFIG_DIR_ENV) {
        return Some(dir);
    }
    let base = directories::BaseDirs::new()?;
    let name = if cfg!(windows) {
        "Glyphkeel"
    } else {
        "glyphkeel"
    };
    Some(base.config_dir().join(name))
}

/// Where the personal settings are kept, or `None` without a home folder.
#[must_use]
pub fn user_settings_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(USER_SETTINGS_FILE))
}

/// The personal settings, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSettings {
    /// `"system"`, or an IANA zone name; `None` (left out) is `"system"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// Fields of other versions of the program, kept as they are.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl UserSettings {
    /// The settings in `path`; the defaults when the file is missing or
    /// can't be read (a damaged file is reported in the log).
    #[must_use]
    pub fn load(path: &Path) -> Self {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                tracing::warn!(file = %path.display(), error = %err, "ignoring the personal settings");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// The settings in [`user_settings_file`], or the defaults.
    #[must_use]
    pub fn load_default() -> Self {
        user_settings_file()
            .as_deref()
            .map(Self::load)
            .unwrap_or_default()
    }

    /// Stores the settings in `path`, replacing the file in one step.
    ///
    /// # Errors
    /// When the folder or the file can't be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    }

    /// The chosen zone's IANA name; `None` for the computer's own zone
    /// (`"system"`, blank or left out).
    #[must_use]
    pub fn zone_name(&self) -> Option<&str> {
        self.time_zone
            .as_deref()
            .map(str::trim)
            .filter(|z| !z.is_empty() && !z.eq_ignore_ascii_case(SYSTEM_TIME_ZONE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_back_with_fields_of_other_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("glyphkeel").join(USER_SETTINGS_FILE);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, br#"{"time_zone": "Asia/Tokyo", "later": {"x": 1}}"#).unwrap();
        let mut settings = UserSettings::load(&path);
        assert_eq!(settings.zone_name(), Some("Asia/Tokyo"));
        settings.time_zone = Some("Europe/Berlin".into());
        settings.save(&path).unwrap();
        let again = UserSettings::load(&path);
        assert_eq!(again.zone_name(), Some("Europe/Berlin"));
        assert_eq!(again.other["later"], serde_json::json!({"x": 1}));
    }

    #[test]
    fn missing_damaged_or_system_means_the_computers_zone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(USER_SETTINGS_FILE);
        assert_eq!(UserSettings::load(&path).zone_name(), None);
        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(UserSettings::load(&path), UserSettings::default());
        for system in ["system", "System", " ", ""] {
            let s = UserSettings {
                time_zone: Some(system.into()),
                ..UserSettings::default()
            };
            assert_eq!(s.zone_name(), None, "{system:?}");
        }
    }
}
