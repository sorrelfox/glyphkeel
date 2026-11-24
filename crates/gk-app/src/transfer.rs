// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Exporting to an archive and importing one, where the person chooses.
//!
//! The page never names a path: Rust shows the native file chooser, keeps
//! the chosen archive for the import under a token, and runs the
//! operation with the path itself. The `op` command refuses these two
//! operations with a path from the page.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gk_api::OpError;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{State, WebviewWindow};

use crate::files::{blocking, dialog, safe_name};
use crate::ipc::Workspace;

/// Archives chosen for an import that the page may still name, newest last.
const KEPT_CHOICES: usize = 4;

/// The archives the person chose to import, by token.
#[derive(Debug, Default)]
pub(crate) struct ImportChoices {
    chosen: Mutex<VecDeque<(String, PathBuf)>>,
    next: Mutex<u64>,
}

impl ImportChoices {
    /// Keeps `path` and returns its token.
    fn keep(&self, path: PathBuf) -> String {
        let token = {
            let mut next = lock(&self.next);
            *next += 1;
            format!("import-{next}")
        };
        let mut chosen = lock(&self.chosen);
        chosen.push_back((token.clone(), path));
        while chosen.len() > KEPT_CHOICES {
            chosen.pop_front();
        }
        token
    }

    /// The archive chosen under `token`.
    fn path(&self, token: &str) -> Result<PathBuf, OpError> {
        lock(&self.chosen)
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| {
                OpError::not_found("The chosen archive is no longer known.", &[])
                    .with_hint("Choose the archive again.")
            })
    }
}

/// An archive the person chose to import: its token and its name.
#[derive(Debug, Serialize)]
pub(crate) struct ChosenArchive {
    token: String,
    name: String,
}

/// Whether the page may run `op` with `input` through the `op` command:
/// not an export or import that names a path.
pub(crate) fn page_may_run(op: &str, input: &Value) -> Result<(), OpError> {
    let names_path = input.get("path").is_some_and(|path| !path.is_null());
    if matches!(op, "transfer.export" | "transfer.import") && names_path {
        return Err(OpError::forbidden(format!(
            "{op} takes its path from the file chooser, not from the page."
        )));
    }
    Ok(())
}

/// Exports `projects` (all when empty) to a `.zip` file, or to a new
/// folder when `folder`, where the person chooses, starting with `name`.
/// Returns the export's result, or `null` when the person cancelled.
#[tauri::command]
pub(crate) async fn export_archive(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    projects: Vec<String>,
    folder: bool,
    name: String,
) -> Result<Option<Value>, OpError> {
    let workspace = Arc::clone(&workspace);
    let mode = dialog::Mode::Export {
        name: safe_name(&name),
        folder,
    };
    let Some(path) = dialog::choose(&window, mode).await?.into_iter().next() else {
        return Ok(None);
    };
    blocking(move || {
        let input = json!({
            "path": path.to_string_lossy(),
            "format": if folder { "folder" } else { "zip" },
            "projects": projects,
        });
        workspace.call("transfer.export", input, None).map(Some)
    })
    .await
}

/// Lets the person choose an archive to import: a `.zip` file, or a folder
/// when `folder`. Returns its token for [`import_archive`] and its name,
/// or `null` when the person cancelled.
#[tauri::command]
pub(crate) async fn choose_import(
    window: WebviewWindow,
    choices: State<'_, Arc<ImportChoices>>,
    folder: bool,
) -> Result<Option<ChosenArchive>, OpError> {
    let Some(path) = dialog::choose(&window, dialog::Mode::Import { folder })
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(ChosenArchive {
        token: choices.keep(path),
        name,
    }))
}

/// Checks (`dry_run`) or imports the archive chosen under `token`, with new
/// keys for projects whose key is taken.
#[tauri::command]
pub(crate) async fn import_archive(
    workspace: State<'_, Arc<Workspace>>,
    choices: State<'_, Arc<ImportChoices>>,
    token: String,
    dry_run: bool,
    keys: Option<Value>,
) -> Result<Value, OpError> {
    let path = choices.path(&token)?;
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let input = json!({
            "path": path.to_string_lossy(),
            "dry_run": dry_run,
            "keys": keys.unwrap_or_else(|| json!([])),
        });
        workspace.call("transfer.import", input, None)
    })
    .await
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ImportChoices, page_may_run};

    #[test]
    fn the_page_never_names_an_archive_path() {
        assert!(page_may_run("transfer.export", &json!({ "path": "/tmp/x.zip" })).is_err());
        assert!(page_may_run("transfer.import", &json!({ "path": "/etc" })).is_err());
        assert!(page_may_run("transfer.export", &json!({ "projects": [] })).is_ok());
        assert!(page_may_run("transfer.export", &json!({ "path": null })).is_ok());
        assert!(page_may_run("task.create", &json!({ "path": "x" })).is_ok());
    }

    #[test]
    fn chosen_archives_are_kept_by_token_for_a_while() {
        let choices = ImportChoices::default();
        let first = choices.keep("/a.zip".into());
        assert_eq!(
            choices.path(&first).unwrap(),
            std::path::Path::new("/a.zip")
        );
        for n in 0..super::KEPT_CHOICES {
            choices.keep(format!("/{n}.zip").into());
        }
        assert!(choices.path(&first).is_err());
        assert!(choices.path("unknown").is_err());
    }
}
