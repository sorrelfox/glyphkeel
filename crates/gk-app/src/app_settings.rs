// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The app's own settings: the ones that belong to this computer and not to
//! the workspace, so the engine doesn't keep them. They live in
//! `$XDG_CONFIG_HOME/glyphkeel/app.json`, and Preferences reads and changes
//! them through `app_settings` and `update_app_settings`.
//!
//! The same two commands read and change the person's time zone. It is a
//! personal setting that the command line and the agent server follow too,
//! so it is kept in the shared personal settings file next to this one
//! (`settings.json`, see [`gk_store::UserSettings`]), not in `app.json`. It
//! is not a workspace operation: it isn't workspace data, has no place in
//! the workspace's history, and an agent must not change it.
//!
//! The same file keeps the page's own preferences (the appearance, the
//! sidebar's closed projects, the board's card size…): the web view runs
//! without storage of its own, so they come back through the start script,
//! before the page's first frame, and the page writes them through
//! `set_ui_pref`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use gk_api::OpError;
use gk_store::UserSettings;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, State};

use crate::background::{self, BackgroundMode};
use crate::ipc::Workspace;
use crate::reminders::{Notice, Notifier};

/// The settings as stored. A field left out of the file keeps its default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Stored {
    /// Whether closing the last window keeps the app running for reminders.
    /// Unset, `GLYPHKEEL_BACKGROUND` decides, and without it the app does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) background: Option<bool>,
    /// Whether reminders show as desktop notifications.
    pub(crate) notifications: bool,
    /// The page's preferences, by name; the page checks their values.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub(crate) ui: Map<String, Value>,
}

impl Default for Stored {
    fn default() -> Self {
        Self {
            background: None,
            notifications: true,
            ui: Map::new(),
        }
    }
}

/// Limits on the page's preferences, so a page can't fill the disk.
const MAX_UI_PREFS: usize = 1000;
const MAX_UI_NAME: usize = 200;
const MAX_UI_VALUE_BYTES: usize = 64 * 1024;

/// Checks one of the page's preferences: a name of letters, digits, `.`,
/// `_` and `-`, and a value of at most 64 KiB as JSON.
fn check_ui_pref(name: &str, value: &Value) -> Result<(), OpError> {
    let name_ok = !name.is_empty()
        && name.len() <= MAX_UI_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !name_ok {
        return Err(OpError::invalid(format!(
            "\"{name}\" is not a preference name."
        )));
    }
    let size = serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len());
    if size > MAX_UI_VALUE_BYTES {
        return Err(OpError::invalid(format!(
            "The preference \"{name}\" is too large."
        )));
    }
    Ok(())
}

/// Where the settings are kept, or `None` without a home folder: in the
/// program's configuration folder, next to the personal settings.
pub(crate) fn file() -> Option<PathBuf> {
    gk_store::config_dir().map(|dir| dir.join("app.json"))
}

/// The stored settings, or the defaults when there are none or they can't be read.
pub(crate) fn load(path: &Path) -> Stored {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            eprintln!(
                "glyphkeel-app: ignoring the settings in {}: {err}",
                path.display()
            );
            Stored::default()
        }),
        Err(_) => Stored::default(),
    }
}

/// Stores the settings, replacing the file in one step.
pub(crate) fn save(path: &Path, stored: &Stored) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(stored).map_err(std::io::Error::other)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// The app's settings while it runs, with the file they are kept in.
#[derive(Debug)]
pub(crate) struct AppSettings {
    file: Option<PathBuf>,
    /// The personal settings shared with the command line and the agent
    /// server.
    user_file: Option<PathBuf>,
    stored: Mutex<Stored>,
    /// Read by the reminders' notifier on its own thread.
    notifications: Arc<AtomicBool>,
}

impl AppSettings {
    /// Reads the settings file; without a home folder the defaults apply
    /// and changes last until the app ends.
    pub(crate) fn load() -> Self {
        let file = file();
        let stored = file.as_deref().map(load).unwrap_or_default();
        let mut settings = Self::new(file, stored);
        settings.user_file = gk_store::user_settings_file();
        settings
    }

    fn new(file: Option<PathBuf>, stored: Stored) -> Self {
        Self {
            file,
            user_file: None,
            notifications: Arc::new(AtomicBool::new(stored.notifications)),
            stored: Mutex::new(stored),
        }
    }

    pub(crate) fn stored(&self) -> Stored {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The page's preferences, for the start script of a new window.
    pub(crate) fn ui(&self) -> Map<String, Value> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .ui
            .clone()
    }

    /// Changes the settings with `change` and stores the result. A file
    /// that can't be written is reported; the change still holds until the
    /// app ends.
    fn change(&self, change: impl FnOnce(&mut Stored)) -> Stored {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut stored);
        self.notifications
            .store(stored.notifications, Ordering::SeqCst);
        if let Some(path) = &self.file
            && let Err(err) = save(path, &stored)
        {
            eprintln!(
                "glyphkeel-app: cannot store the settings in {}: {err}",
                path.display()
            );
        }
        stored.clone()
    }

    /// The personal time zone as chosen: `"system"` or an IANA name.
    pub(crate) fn time_zone(&self) -> String {
        let user = self
            .user_file
            .as_deref()
            .map(UserSettings::load)
            .unwrap_or_default();
        user.zone_name()
            .unwrap_or(gk_store::SYSTEM_TIME_ZONE)
            .to_owned()
    }

    /// Keeps the personal time zone `zone` (`None`: the system's) in the
    /// shared file, keeping the file's other fields.
    fn set_time_zone(&self, zone: Option<&str>) -> Result<(), OpError> {
        let Some(path) = &self.user_file else {
            return Ok(());
        };
        let mut user = UserSettings::load(path);
        user.time_zone = Some(zone.unwrap_or(gk_store::SYSTEM_TIME_ZONE).to_owned());
        user.save(path).map_err(|err| {
            OpError::storage(format!(
                "Can't store the time zone in {}: {err}.",
                path.display()
            ))
        })
    }

    /// Applies a change from Preferences.
    fn apply(&self, patch: &Patch) -> Stored {
        self.change(|stored| {
            if let Some(background) = patch.background {
                stored.background = Some(background);
            }
            if let Some(notifications) = patch.notifications {
                stored.notifications = notifications;
            }
        })
    }

    /// Sets one of the page's preferences; `null` removes it.
    fn set_ui(&self, name: String, value: Value) -> Result<(), OpError> {
        check_ui_pref(&name, &value)?;
        let mut full = false;
        self.change(|stored| {
            if value.is_null() {
                stored.ui.remove(&name);
            } else if stored.ui.len() < MAX_UI_PREFS || stored.ui.contains_key(&name) {
                stored.ui.insert(name, value);
            } else {
                full = true;
            }
        });
        if full {
            return Err(OpError::invalid("There are too many preferences."));
        }
        Ok(())
    }

    /// A notifier that shows through `inner` while notifications are on,
    /// and otherwise only writes to the log.
    pub(crate) fn switched(&self, inner: Arc<dyn Notifier>) -> Arc<dyn Notifier> {
        Arc::new(Switched {
            on: Arc::clone(&self.notifications),
            inner,
        })
    }
}

struct Switched {
    on: Arc<AtomicBool>,
    inner: Arc<dyn Notifier>,
}

impl Notifier for Switched {
    fn show(&self, notice: &Notice) {
        if self.on.load(Ordering::SeqCst) {
            self.inner.show(notice);
        } else {
            eprintln!(
                "glyphkeel-app: reminder (notifications off): {}: {}",
                notice.title, notice.body
            );
        }
    }
}

/// What the page sees: the settings in effect, and the open workspace's folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct View {
    pub(crate) background: bool,
    pub(crate) notifications: bool,
    /// The personal time zone as chosen: `"system"` or an IANA name.
    pub(crate) time_zone: String,
    /// The zone times are shown in now, by its IANA name: the chosen one,
    /// or the system's; absent when the system's zone has no name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) zone_in_use: Option<String>,
    /// The workspace folder; absent when the workspace didn't open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace_path: Option<String>,
}

/// A change from Preferences; fields left out keep their value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Patch {
    #[serde(default)]
    pub(crate) background: Option<bool>,
    #[serde(default)]
    pub(crate) notifications: Option<bool>,
    /// `"system"` or an IANA name such as `"Europe/Berlin"`.
    #[serde(default)]
    pub(crate) time_zone: Option<String>,
}

fn view(
    settings: &AppSettings,
    stored: &Stored,
    mode: &BackgroundMode,
    workspace: &Workspace,
) -> View {
    View {
        background: mode.enabled(),
        notifications: stored.notifications,
        time_zone: settings.time_zone(),
        zone_in_use: workspace.time_zone(),
        workspace_path: workspace
            .layout()
            .map(|layout| layout.root().display().to_string()),
    }
}

/// The app's settings, for Preferences.
#[tauri::command]
pub(crate) fn app_settings(
    settings: State<'_, AppSettings>,
    mode: State<'_, BackgroundMode>,
    workspace: State<'_, Arc<Workspace>>,
) -> View {
    view(&settings, &settings.stored(), &mode, &workspace)
}

/// Changes the app's settings from Preferences; they apply at once.
/// Runs on the main thread, where the tray icon is made and removed. An
/// unknown time zone refuses the whole change.
#[tauri::command]
pub(crate) fn update_app_settings(
    app: AppHandle,
    settings: State<'_, AppSettings>,
    mode: State<'_, BackgroundMode>,
    workspace: State<'_, Arc<Workspace>>,
    patch: Patch,
) -> Result<View, OpError> {
    let zone = match &patch.time_zone {
        Some(name) => Some(gk_engine::check_time_zone(name)?),
        None => None,
    };
    let stored = settings.apply(&patch);
    if let Some(on) = patch.background {
        background::switch(&app, on);
    }
    if let Some(zone) = zone {
        settings.set_time_zone(zone.as_deref())?;
        workspace.set_time_zone(zone.as_deref().unwrap_or(gk_store::SYSTEM_TIME_ZONE))?;
    }
    Ok(view(&settings, &stored, &mode, &workspace))
}

/// Every time zone a person can choose, with its offset from UTC now, for
/// the picker in Preferences.
#[tauri::command]
pub(crate) fn time_zones() -> Vec<gk_engine::ZoneChoice> {
    gk_engine::time_zones((gk_engine::system_clock())())
}

/// Keeps one of the page's preferences for the next start; `null` removes it.
#[tauri::command]
pub(crate) fn set_ui_pref(
    settings: State<'_, AppSettings>,
    name: String,
    value: Value,
) -> Result<(), OpError> {
    settings.set_ui(name, value)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};

    use super::{AppSettings, Patch, Stored, load, save};
    use crate::reminders::{Notice, Notifier};

    #[test]
    fn stored_settings_come_back_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("glyphkeel").join("app.json");
        let mut ui = serde_json::Map::new();
        ui.insert("appearance".into(), serde_json::json!({ "theme": "slate" }));
        let stored = Stored {
            background: Some(false),
            notifications: false,
            ui,
        };
        save(&path, &stored).unwrap();
        assert_eq!(load(&path), stored);
    }

    #[test]
    fn a_missing_damaged_or_partial_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.json");
        assert_eq!(load(&path), Stored::default());
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load(&path), Stored::default());
        std::fs::write(&path, br#"{"background": true}"#).unwrap();
        assert_eq!(
            load(&path),
            Stored {
                background: Some(true),
                ..Stored::default()
            }
        );
    }

    #[test]
    fn a_change_is_kept_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.json");
        let settings = AppSettings::new(Some(path.clone()), Stored::default());
        let patch: Patch = serde_json::from_str(r#"{"notifications": false}"#).unwrap();
        let stored = settings.apply(&patch);
        assert!(!stored.notifications);
        assert_eq!(stored.background, None);
        assert!(!settings.notifications.load(Ordering::SeqCst));
        assert_eq!(load(&path), stored);
        assert!(serde_json::from_str::<Patch>(r#"{"theme": "dark"}"#).is_err());
    }

    #[test]
    fn the_time_zone_is_kept_in_the_shared_personal_settings() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join(gk_store::USER_SETTINGS_FILE);
        std::fs::write(&user, br#"{"time_zone": "Asia/Tokyo", "other": 1}"#).unwrap();
        let mut settings = AppSettings::new(Some(dir.path().join("app.json")), Stored::default());
        settings.user_file = Some(user.clone());
        assert_eq!(settings.time_zone(), "Asia/Tokyo");
        settings.set_time_zone(Some("Europe/Berlin")).unwrap();
        assert_eq!(settings.time_zone(), "Europe/Berlin");
        // The command line and the agent server read the same file.
        let shared = gk_store::UserSettings::load(&user);
        assert_eq!(
            gk_engine::personal_time_zone(None, &shared)
                .unwrap()
                .as_deref(),
            Some("Europe/Berlin")
        );
        assert_eq!(shared.other["other"], 1);
        settings.set_time_zone(None).unwrap();
        assert_eq!(settings.time_zone(), "system");
        let patch: Patch = serde_json::from_str(r#"{"time_zone": "UTC"}"#).unwrap();
        assert_eq!(patch.time_zone.as_deref(), Some("UTC"));
    }

    #[test]
    fn the_pages_preferences_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.json");
        let running = AppSettings::new(Some(path.clone()), load(&path));
        running
            .set_ui("appearance".into(), serde_json::json!({ "theme": "sand" }))
            .unwrap();
        running
            .set_ui("board.cardSize.p-1".into(), serde_json::json!("compact"))
            .unwrap();
        running
            .set_ui("sidebar.collapsed".into(), serde_json::json!(["TM"]))
            .unwrap();
        running
            .set_ui("sidebar.collapsed".into(), serde_json::Value::Null)
            .unwrap();
        drop(running);

        // The next start reads them back, for the start script.
        let restarted = AppSettings::new(Some(path.clone()), load(&path));
        let ui = restarted.ui();
        assert_eq!(ui["appearance"], serde_json::json!({ "theme": "sand" }));
        assert_eq!(ui["board.cardSize.p-1"], serde_json::json!("compact"));
        assert!(!ui.contains_key("sidebar.collapsed"));
    }

    #[test]
    fn the_page_cannot_store_odd_names_or_huge_values() {
        let settings = AppSettings::new(None, Stored::default());
        assert!(
            settings
                .set_ui("../x".into(), serde_json::json!(1))
                .is_err()
        );
        assert!(
            settings
                .set_ui(String::new(), serde_json::json!(1))
                .is_err()
        );
        let huge = serde_json::json!("x".repeat(super::MAX_UI_VALUE_BYTES));
        assert!(settings.set_ui("big".into(), huge).is_err());
        assert!(settings.ui().is_empty());
    }

    struct Shown(Mutex<Vec<String>>);

    impl Notifier for Shown {
        fn show(&self, notice: &Notice) {
            self.0.lock().unwrap().push(notice.title.clone());
        }
    }

    #[test]
    fn notifications_off_keeps_reminders_off_the_desktop() {
        let settings = AppSettings::new(None, Stored::default());
        let shown = Arc::new(Shown(Mutex::new(Vec::new())));
        let notifier = settings.switched(Arc::clone(&shown) as Arc<dyn Notifier>);
        let notice = |title: &str| Notice {
            title: title.to_owned(),
            body: String::new(),
            task: None,
        };
        notifier.show(&notice("first"));
        settings.apply(&Patch {
            notifications: Some(false),
            ..Patch::default()
        });
        notifier.show(&notice("second"));
        assert_eq!(*shown.0.lock().unwrap(), ["first"]);
    }
}
