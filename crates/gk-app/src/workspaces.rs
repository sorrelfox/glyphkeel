// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The workspace switcher: the workspaces in the data folder, making and
//! renaming them, and opening another one in place of the one shown.
//!
//! The page names workspaces by name only, never by folder, so it reaches
//! nothing outside the data folder. Opening one closes the engine and opens
//! the other folder in the same windows (the main window hears that
//! everything changed; the task and wiki windows of the old workspace
//! close). The choice is kept in the personal settings, so the app opens
//! it at its next start and the command line and the agent server work in
//! it unless told another one; `GLYPHKEEL_WORKSPACE` and `--workspace`
//! still come first.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gk_api::OpError;
use gk_engine::workspaces;
use gk_store::WorkspaceLayout;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::files::blocking;
use crate::ipc::Workspace;

/// The data folder the workspaces are in: the shared one, else the app's
/// own when the shared one can't be found.
pub(crate) fn data_dir(app: &AppHandle) -> Option<PathBuf> {
    gk_store::data_dir().or_else(|| app.path().data_dir().ok().map(|dir| dir.join("glyphkeel")))
}

/// What the switcher shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkspacesView {
    /// The name of the workspace open now; absent when none is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current: Option<String>,
    /// Whether the open workspace is outside the data folder (named by
    /// `GLYPHKEEL_WORKSPACE` or `--workspace` as a folder), so it is not in
    /// `items` and can't be renamed here.
    pub(crate) elsewhere: bool,
    /// The workspaces in the data folder, by name.
    pub(crate) items: Vec<WorkspaceItem>,
}

/// One workspace of the switcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WorkspaceItem {
    pub(crate) name: String,
    /// Whether it is the one open now.
    pub(crate) current: bool,
}

/// The switcher's view of `data_dir` with `open` open.
fn view(data_dir: Option<&Path>, open: Option<&WorkspaceLayout>) -> WorkspacesView {
    let items = data_dir.map(workspaces::list).unwrap_or_default();
    let is_open = |path: &str| open.is_some_and(|layout| layout.root() == Path::new(path));
    let listed = items.iter().any(|w| is_open(&w.path));
    WorkspacesView {
        current: open.map(workspaces::name_of),
        elsewhere: open.is_some() && !listed,
        items: items
            .iter()
            .map(|w| WorkspaceItem {
                name: w.name.clone(),
                current: is_open(&w.path),
            })
            .collect(),
    }
}

fn no_data_dir() -> OpError {
    crate::messages::hint(
        crate::messages::error("workspace-no-data-folder", &[], OpError::storage),
        "workspace-no-data-folder-hint",
        &[],
    )
}

/// Closes the windows of the workspace that was open: its tasks' and wiki
/// pages' own windows. The main window stays, and shows the new one.
fn close_other_windows(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(crate::task_window::PREFIX)
            || label.starts_with(crate::wiki_window::PREFIX)
        {
            let _ = window.close();
        }
    }
}

/// Keeps `name` as the person's choice; a failure only loses the choice for
/// the next start, so it is logged, not reported.
fn remember(name: &str) {
    if let Err(err) = workspaces::remember(name) {
        eprintln!("glyphkeel-app: {}", err.message);
    }
}

/// Opens `layout` in place of the workspace open now.
fn switch(
    app: &AppHandle,
    workspace: &Workspace,
    name: &str,
    layout: &WorkspaceLayout,
) -> Result<(), OpError> {
    workspace.switch(layout)?;
    close_other_windows(app);
    remember(name);
    Ok(())
}

/// The workspaces in the data folder, and the one open now.
#[tauri::command]
pub(crate) async fn workspaces(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<WorkspacesView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || Ok(view(data_dir(&app).as_deref(), workspace.layout().as_ref()))).await
}

/// Makes the workspace `name` in the data folder and opens it.
#[tauri::command]
pub(crate) async fn create_workspace(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    name: String,
) -> Result<WorkspacesView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let data = data_dir(&app).ok_or_else(no_data_dir)?;
        let layout = workspaces::create(&data, &name)?;
        switch(&app, &workspace, &workspaces::name_of(&layout), &layout)?;
        Ok(view(Some(&data), workspace.layout().as_ref()))
    })
    .await
}

/// Opens the workspace `name` of the data folder in place of the one open
/// now.
#[tauri::command]
pub(crate) async fn open_workspace(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    name: String,
) -> Result<WorkspacesView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let data = data_dir(&app).ok_or_else(no_data_dir)?;
        let name = name.trim();
        // Only a workspace of the list: a name, never a folder.
        let Some(found) = workspaces::list(&data).into_iter().find(|w| w.name == name) else {
            return Err(workspaces::not_found(name));
        };
        let layout = WorkspaceLayout::new(&found.path);
        if workspace.layout().as_ref() != Some(&layout) {
            switch(&app, &workspace, &found.name, &layout)?;
        }
        Ok(view(Some(&data), workspace.layout().as_ref()))
    })
    .await
}

/// Renames the workspace `old` of the data folder to `new`. The one open
/// now closes for it and opens again under its new name.
#[tauri::command]
pub(crate) async fn rename_workspace(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    old: String,
    new: String,
) -> Result<WorkspacesView, OpError> {
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let data = data_dir(&app).ok_or_else(no_data_dir)?;
        let open = workspace.layout();
        let is_open = open.as_ref().is_some_and(|layout| {
            workspaces::list(&data)
                .iter()
                .any(|w| w.name == old.trim() && layout.root() == Path::new(&w.path))
        });
        if is_open {
            workspace.closed_for(|_| workspaces::rename(&data, &old, &new))?;
            close_other_windows(&app);
        } else {
            workspaces::rename(&data, &old, &new)?;
        }
        Ok(view(Some(&data), workspace.layout().as_ref()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_marks_the_open_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let personal = workspaces::create(data, "Personal").unwrap();
        workspaces::create(data, "Side").unwrap();
        let shown = view(Some(data), Some(&personal));
        assert_eq!(shown.current.as_deref(), Some("Personal"));
        assert!(!shown.elsewhere);
        assert_eq!(
            shown.items,
            [
                WorkspaceItem {
                    name: "Personal".into(),
                    current: true
                },
                WorkspaceItem {
                    name: "Side".into(),
                    current: false
                },
            ]
        );
        let outside = WorkspaceLayout::new(dir.path().join("elsewhere"));
        let shown = view(Some(data), Some(&outside));
        assert!(shown.elsewhere);
        assert!(shown.items.iter().all(|w| !w.current));
    }

    #[test]
    fn the_open_workspace_moves_to_another_folder_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let first = workspaces::create(data, "First").unwrap();
        let second = workspaces::create(data, "Second").unwrap();
        let ws = Workspace::open(
            &first,
            gk_engine::EngineConfig::new(gk_engine::Role::App),
            gk_engine::Confirmation::Given,
        );
        ws.call(
            "project.create",
            serde_json::json!({ "name": "One", "key": "ONE" }),
            None,
        )
        .unwrap();
        ws.switch(&second).unwrap();
        assert_eq!(ws.layout().as_ref(), Some(&second));
        let projects = ws
            .call("project.list", serde_json::json!({}), None)
            .unwrap();
        assert_eq!(projects["items"].as_array().map(Vec::len), Some(0));
        // A folder that can't open leaves the open one open.
        let bad = data.join("not-a-folder");
        std::fs::write(&bad, b"x").unwrap();
        ws.switch(&WorkspaceLayout::new(&bad)).unwrap_err();
        assert_eq!(ws.layout().as_ref(), Some(&second));
        ws.call("project.list", serde_json::json!({}), None)
            .unwrap();
        // The open one is renamed by closing it first; a refused rename
        // opens it again as it was.
        ws.closed_for(|_| workspaces::rename(data, "Second", "First"))
            .unwrap_err();
        assert_eq!(ws.layout().as_ref(), Some(&second));
        ws.closed_for(|_| workspaces::rename(data, "Second", "Renamed"))
            .unwrap();
        assert!(ws.layout().unwrap().root().ends_with("Renamed"));
        ws.call("project.list", serde_json::json!({}), None)
            .unwrap();
        ws.close();
    }
}
