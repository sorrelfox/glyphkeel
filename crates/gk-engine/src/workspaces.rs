// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The workspaces in the data folder: listing, making and renaming them,
//! and remembering which one the person works in.
//!
//! These are not operations of a workspace: they work on the folders,
//! outside any one workspace, and belong to the person, not to the data. So
//! the app and the command line call them directly, and agents can't reach
//! them: an agent works in the workspace it was started on.
//!
//! The choice is the personal setting `workspace` (see
//! [`gk_store::UserSettings`]). The app keeps it when the person opens
//! another workspace, `glyphkeel workspace open` sets it too, and the
//! command line and the agent server follow it when they are not told
//! another workspace ([`gk_store::Location::find`]).

use std::fs;
use std::path::Path;

use gk_api::OpError;
use gk_core::fold;
use gk_store::{DATABASE_FILE, Location, LockFile, UserSettings, WorkspaceLayout};
use serde::Serialize;

use crate::engine::{Engine, EngineConfig, Role};
use crate::messages;

/// One workspace in the data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceEntry {
    /// Its name, which is its folder's name.
    pub name: String,
    /// Its folder.
    pub path: String,
}

/// The workspaces in `data_dir`: the folders under `workspaces/` that hold
/// a workspace database, by name.
#[must_use]
pub fn list(data_dir: &Path) -> Vec<WorkspaceEntry> {
    let Ok(entries) = fs::read_dir(data_dir.join("workspaces")) else {
        return Vec::new();
    };
    let mut out: Vec<WorkspaceEntry> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().join(DATABASE_FILE).is_file())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            gk_store::check_workspace_name(&name).ok()?;
            Some(WorkspaceEntry {
                path: e.path().display().to_string(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| fold(&a.name).cmp(&fold(&b.name)).then(a.name.cmp(&b.name)));
    out
}

/// The name a workspace folder goes by: its folder's name.
#[must_use]
pub fn name_of(layout: &WorkspaceLayout) -> String {
    layout
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| layout.root().display().to_string())
}

/// Makes the workspace `name` in `data_dir`, empty and ready to open.
///
/// # Errors
/// `invalid` for a name that can't be a folder name, `conflict` when a
/// workspace or folder of that name exists, `storage` when it can't be made.
pub fn create(data_dir: &Path, name: &str) -> Result<WorkspaceLayout, OpError> {
    let name = name.trim();
    let layout = named(data_dir, name)?;
    if layout.root().exists() || exists_ignoring_case(data_dir, name, None) {
        return Err(messages::conflict(
            "workspace-exists",
            &[("name", name.to_owned())],
        ));
    }
    Engine::open(&layout, EngineConfig::new(Role::Cli))?.close();
    Ok(layout)
}

/// Renames the workspace `old` in `data_dir` to `new`. Nobody may have it
/// open: close it in the app first (the app does so itself for the
/// workspace it shows). When the person's choice named it, the choice
/// follows the new name.
///
/// # Errors
/// `invalid` for a name that can't be a folder name, `not_found` when there
/// is no workspace `old`, `conflict` when `new` is taken or the workspace
/// is open, `storage` when the folder can't be renamed.
pub fn rename(data_dir: &Path, old: &str, new: &str) -> Result<WorkspaceLayout, OpError> {
    rename_with(
        data_dir,
        old,
        new,
        gk_store::user_settings_file().as_deref(),
    )
}

/// As [`rename`], with the personal settings in `settings`.
fn rename_with(
    data_dir: &Path,
    old: &str,
    new: &str,
    settings: Option<&Path>,
) -> Result<WorkspaceLayout, OpError> {
    let (old, new) = (old.trim(), new.trim());
    let from = existing(data_dir, old)?;
    let to = named(data_dir, new)?;
    if old == new {
        return Ok(from);
    }
    // Only the case changes: the same folder on a system that ignores case.
    let same_folder = fold(old) == fold(new);
    if !same_folder && (to.root().exists() || exists_ignoring_case(data_dir, new, Some(old))) {
        return Err(messages::conflict(
            "workspace-exists",
            &[("name", new.to_owned())],
        ));
    }
    {
        let lock = |path: &Path| LockFile::try_acquire(path).ok().flatten();
        let app = lock(&from.app_lock());
        let jobs = lock(&from.jobs_lock());
        if app.is_none() || jobs.is_none() {
            return Err(messages::hint(
                messages::conflict("workspace-in-use", &[("name", old.to_owned())]),
                "workspace-in-use-hint",
                &[],
            ));
        }
    }
    fs::rename(from.root(), to.root()).map_err(|e| {
        messages::storage(
            "workspace-rename-failed",
            &[("name", old.to_owned()), ("error", e.to_string())],
        )
    })?;
    if let Some(file) = settings {
        let mut stored = UserSettings::load(file);
        if stored.workspace.as_deref().map(str::trim) == Some(old) {
            stored.workspace = Some(new.to_owned());
            save(&stored, Some(file))?;
        }
    }
    Ok(to)
}

/// Remembers `workspace` (a name in the data folder or a folder) as the
/// one the person works in, for the app's next start, the command line and
/// the agent server.
///
/// # Errors
/// `not_found` when there is no workspace there, `invalid` for a bad
/// name, `storage` when the personal settings can't be written.
pub fn choose(data_dir: Option<&Path>, workspace: &str) -> Result<WorkspaceLayout, OpError> {
    let workspace = workspace.trim();
    let layout = Location::parse(workspace)
        .layout(data_dir)
        .map_err(|e| crate::store_error(&e))?;
    if !layout.database().is_file() {
        return Err(messages::not_found(
            "workspace-not-found",
            &[("name", workspace.to_owned())],
        ));
    }
    remember(workspace)?;
    Ok(layout)
}

/// Keeps `workspace` as the person's choice, without checking it.
///
/// # Errors
/// `storage` when the personal settings can't be written.
pub fn remember(workspace: &str) -> Result<(), OpError> {
    let mut settings = UserSettings::load_default();
    if settings.workspace.as_deref() == Some(workspace) {
        return Ok(());
    }
    settings.workspace = Some(workspace.to_owned());
    save(&settings, gk_store::user_settings_file().as_deref())
}

fn save(settings: &UserSettings, file: Option<&Path>) -> Result<(), OpError> {
    let Some(file) = file else {
        return Err(messages::storage("workspace-choice-no-folder", &[]));
    };
    settings
        .save(file)
        .map_err(|e| messages::storage("workspace-choice-not-saved", &[("error", e.to_string())]))
}

/// The error for a workspace `name` that isn't there.
#[must_use]
pub fn not_found(name: &str) -> OpError {
    messages::not_found("workspace-not-found", &[("name", name.to_owned())])
}

/// The layout of the workspace `name` in `data_dir`, checking the name.
fn named(data_dir: &Path, name: &str) -> Result<WorkspaceLayout, OpError> {
    WorkspaceLayout::in_data_dir(data_dir, name).map_err(|_| {
        messages::hint(
            messages::invalid_at(
                "name",
                "workspace-name-invalid",
                &[("name", name.to_owned())],
            ),
            "workspace-name-invalid-hint",
            &[],
        )
    })
}

/// The workspace `name` in `data_dir`, which must exist.
fn existing(data_dir: &Path, name: &str) -> Result<WorkspaceLayout, OpError> {
    let layout = named(data_dir, name)?;
    if layout.database().is_file() {
        Ok(layout)
    } else {
        Err(messages::not_found(
            "workspace-not-found",
            &[("name", name.to_owned())],
        ))
    }
}

/// Whether another folder in the data folder has `name` in another case,
/// which would be the same folder where case doesn't count. `except` is the
/// workspace being renamed.
fn exists_ignoring_case(data_dir: &Path, name: &str, except: Option<&str>) -> bool {
    let Ok(entries) = fs::read_dir(data_dir.join("workspaces")) else {
        return false;
    };
    let wanted = fold(name);
    entries.filter_map(Result::ok).any(|e| {
        let found = e.file_name().to_string_lossy().into_owned();
        fold(&found) == wanted && except.is_none_or(|x| fold(x) != wanted)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_are_made_listed_and_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        assert!(list(data).is_empty());
        create(data, "Personal").unwrap();
        create(data, " Side project ").unwrap();
        let names: Vec<String> = list(data).into_iter().map(|w| w.name).collect();
        assert_eq!(names, ["Personal", "Side project"]);

        let err = create(data, "side PROJECT").unwrap_err();
        assert_eq!(err.code, gk_api::ErrorCode::Conflict);
        let err = create(data, "a/b").unwrap_err();
        assert_eq!(err.code, gk_api::ErrorCode::Invalid);

        let settings = data.join("settings.json");
        let rename = |old: &str, new: &str| rename_with(data, old, new, Some(&settings));
        UserSettings {
            workspace: Some("Side project".into()),
            ..UserSettings::default()
        }
        .save(&settings)
        .unwrap();
        let renamed = rename("Side project", "Hobby").unwrap();
        // The choice follows the new name.
        assert_eq!(
            UserSettings::load(&settings).workspace.as_deref(),
            Some("Hobby")
        );
        assert!(renamed.database().is_file());
        let names: Vec<String> = list(data).into_iter().map(|w| w.name).collect();
        assert_eq!(names, ["Hobby", "Personal"]);
        assert_eq!(
            rename("Hobby", "Personal").unwrap_err().code,
            gk_api::ErrorCode::Conflict
        );
        assert_eq!(
            rename("Nothing", "Else").unwrap_err().code,
            gk_api::ErrorCode::NotFound
        );
        // Only the case changes.
        rename("Hobby", "hobby").unwrap();
        assert_eq!(list(data)[0].name, "hobby");
    }

    #[test]
    fn an_open_workspace_is_not_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let layout = create(data, "Busy").unwrap();
        let engine = Engine::open(&layout, EngineConfig::new(Role::App)).unwrap();
        let err = rename_with(data, "Busy", "Free", None).unwrap_err();
        assert_eq!(err.code, gk_api::ErrorCode::Conflict);
        engine.close();
        rename_with(data, "Busy", "Free", None).unwrap();
    }

    #[test]
    fn names_come_from_the_folder() {
        assert_eq!(
            name_of(&WorkspaceLayout::new("/data/workspaces/Side project")),
            "Side project"
        );
    }
}
