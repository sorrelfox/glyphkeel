// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Exporting to an archive and importing one, where the person chooses.
//!
//! The page never names a path: Rust shows the native file chooser, keeps
//! the chosen archive for the import under a token, and runs the
//! operation with the path itself. The `op` command refuses every
//! operation of [`CHOSEN_PATHS`] with a path from the page.

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
                crate::messages::hint(
                    crate::messages::error("archive-unknown", &[], |m| OpError::not_found(m, &[])),
                    "archive-unknown-hint",
                    &[],
                )
            })
    }
}

/// An archive the person chose to import: its token and its name.
#[derive(Debug, Serialize)]
pub(crate) struct ChosenArchive {
    token: String,
    name: String,
}

/// The inputs of operations that name a file or a folder on the person's
/// computer, as `(operation, field)`. The page never fills them in: the app
/// shows the system's chooser and runs the operation itself, with the path
/// the person chose. A page that could name a path would make the engine
/// read or write any file the person can (attaching `~/.ssh/id_ed25519` and
/// then reading it back, say). A test checks every input of the catalog
/// against this list, so a new path field can't be missed.
pub(crate) const CHOSEN_PATHS: &[(&str, &str)] = &[
    ("transfer.export", "path"),
    ("transfer.import", "path"),
    ("transfer.import_csv", "path"),
    ("wiki.export", "path"),
    ("wiki.import", "path"),
    ("file.attach", "path"),
    // Setting it back to the default, with no folder, is fine.
    ("backup.folder.update", "folder"),
];

/// Whether the page may run `op` with `input` through the `op` command:
/// never with a path or a folder of [`CHOSEN_PATHS`] that the page names
/// itself.
pub(crate) fn page_may_run(op: &str, input: &Value) -> Result<(), OpError> {
    let names = |field: &str| input.get(field).is_some_and(|value| !value.is_null());
    let refused = CHOSEN_PATHS
        .iter()
        .find(|(o, field)| *o == op && names(field));
    match refused {
        Some((op, field)) => Err(crate::messages::error(
            "page-path-refused",
            &[("op", (*op).to_owned()), ("field", (*field).to_owned())],
            OpError::forbidden,
        )
        .with_detail("path", *field)),
        None => Ok(()),
    }
}

/// Exports `projects` (all when empty) to a `.zip` file, or to a new
/// folder when `folder`, where the person chooses, starting with `name`.
/// `encrypt` asks for an encrypted `.zip` (left out: the engine's default,
/// on for an encrypted workspace). Returns the export's result, or `null`
/// when the person cancelled.
#[tauri::command]
pub(crate) async fn export_archive(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    projects: Vec<String>,
    folder: bool,
    name: String,
    encrypt: Option<bool>,
) -> Result<Option<Value>, OpError> {
    let workspace = Arc::clone(&workspace);
    // An encrypted export is one file.
    let folder = folder && encrypt != Some(true);
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
            "encrypt": encrypt,
        });
        workspace.call("transfer.export", input, None).map(Some)
    })
    .await
}

/// Exports wiki page `page` (with the pages inside it when `subpages`), or
/// without a page the whole wiki of `project`, as Markdown files (or web
/// pages when `html`) in a `.zip` file where the person chooses, starting
/// with `name`. Returns the export's result, or `null` when the person
/// cancelled.
#[tauri::command]
pub(crate) async fn export_wiki_page(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    page: Option<String>,
    project: Option<String>,
    subpages: bool,
    name: String,
    html: Option<bool>,
) -> Result<Option<Value>, OpError> {
    let workspace = Arc::clone(&workspace);
    let mode = dialog::Mode::Export {
        name: safe_name(&name),
        folder: false,
    };
    let Some(path) = dialog::choose(&window, mode).await?.into_iter().next() else {
        return Ok(None);
    };
    blocking(move || {
        let input = json!({
            "page": page,
            "project": project,
            "subpages": subpages,
            "path": path.to_string_lossy(),
            "format": "zip",
            "text_format": if html == Some(true) { "html" } else { "markdown" },
        });
        workspace.call("wiki.export", input, None).map(Some)
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

/// Lets the person choose a folder of Markdown files to import into a
/// wiki. Returns its token for [`import_wiki_folder`] and its name, or
/// `null` when the person cancelled.
#[tauri::command]
pub(crate) async fn choose_wiki_folder(
    window: WebviewWindow,
    choices: State<'_, Arc<ImportChoices>>,
) -> Result<Option<ChosenArchive>, OpError> {
    let Some(path) = dialog::choose(&window, dialog::Mode::WikiFolder)
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

/// Checks (`dry_run`) or imports the folder chosen under `token` as pages
/// of `project`'s wiki, inside page `parent` when given.
#[tauri::command]
pub(crate) async fn import_wiki_folder(
    workspace: State<'_, Arc<Workspace>>,
    choices: State<'_, Arc<ImportChoices>>,
    token: String,
    project: String,
    parent: Option<String>,
    dry_run: bool,
) -> Result<Value, OpError> {
    let path = choices.path(&token)?;
    let workspace = Arc::clone(&workspace);
    blocking(move || {
        let input = json!({
            "project": project,
            "path": path.to_string_lossy(),
            "parent": parent,
            "dry_run": dry_run,
        });
        workspace.call("wiki.import", input, None)
    })
    .await
}

/// Lets the person choose the folder backups go to, in the native folder
/// chooser, and keeps it; the page never names the folder. Returns the
/// settings change, or `null` when the person cancelled. Backups made
/// before stay where they are.
#[tauri::command]
pub(crate) async fn choose_backup_folder(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
) -> Result<Option<Value>, OpError> {
    let workspace = Arc::clone(&workspace);
    let Some(path) = dialog::choose(&window, dialog::Mode::BackupFolder)
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    blocking(move || {
        let input = json!({ "folder": path.to_string_lossy() });
        workspace
            .call("backup.folder.update", input, None)
            .map(Some)
    })
    .await
}

/// Lets the person choose a CSV file to import tasks from. Returns its
/// token for [`import_csv`] and its name, or `null` when the person
/// cancelled.
#[tauri::command]
pub(crate) async fn choose_csv(
    window: WebviewWindow,
    choices: State<'_, Arc<ImportChoices>>,
) -> Result<Option<ChosenArchive>, OpError> {
    let Some(path) = dialog::choose(&window, dialog::Mode::Csv)
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

/// Checks (when `input.dry_run`) or runs a CSV import of the file chosen
/// under `token`; `input` is the operation's input without its path.
#[tauri::command]
pub(crate) async fn import_csv(
    workspace: State<'_, Arc<Workspace>>,
    choices: State<'_, Arc<ImportChoices>>,
    token: String,
    input: Value,
) -> Result<Value, OpError> {
    let path = choices.path(&token)?;
    let Value::Object(mut input) = input else {
        return Err(crate::messages::error(
            "csv-input-not-an-object",
            &[],
            OpError::invalid,
        ));
    };
    input.insert("path".to_owned(), json!(path.to_string_lossy()));
    let workspace = Arc::clone(&workspace);
    blocking(move || workspace.call("transfer.import_csv", Value::Object(input), None)).await
}

/// Checks (`dry_run`) or imports the archive chosen under `token`, with new
/// keys for projects whose key is taken. `passphrase` opens an encrypted
/// export (the error `details.passphrase = "export"` asks for it); the app
/// keeps it in memory until it quits.
#[tauri::command]
pub(crate) async fn import_archive(
    workspace: State<'_, Arc<Workspace>>,
    choices: State<'_, Arc<ImportChoices>>,
    token: String,
    dry_run: bool,
    keys: Option<Value>,
    passphrase: Option<String>,
) -> Result<Value, OpError> {
    let path = choices.path(&token)?;
    if let Some(passphrase) = passphrase.and_then(gk_store::Passphrase::new) {
        gk_store::remember_passphrase(&passphrase);
    }
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
        assert!(
            page_may_run(
                "wiki.export",
                &json!({ "page": "Home", "path": "/tmp/h.zip" })
            )
            .is_err()
        );
        assert!(page_may_run("wiki.export", &json!({ "page": "Home" })).is_ok());
        assert!(page_may_run("wiki.import", &json!({ "project": "TM", "path": "/home" })).is_err());
        assert!(
            page_may_run(
                "transfer.import_csv",
                &json!({ "project": "TM", "path": "/etc/passwd" })
            )
            .is_err()
        );
        assert!(page_may_run("transfer.export", &json!({ "projects": [] })).is_ok());
        assert!(page_may_run("transfer.export", &json!({ "path": null })).is_ok());
        assert!(page_may_run("task.create", &json!({ "path": "x" })).is_ok());
        assert!(page_may_run("backup.folder.update", &json!({ "folder": "/mnt" })).is_err());
        assert!(page_may_run("backup.folder.update", &json!({})).is_ok());
    }

    /// The page attaches what it has in hand (pasted, dragged from the page)
    /// as Base64; a path would let it read any of the person's files.
    #[test]
    fn the_page_never_names_a_file_to_attach() {
        let refused = page_may_run(
            "file.attach",
            &json!({ "task": "TM-1", "path": "/home/me/.ssh/id_ed25519" }),
        )
        .unwrap_err();
        assert_eq!(refused.code, gk_api::ErrorCode::Forbidden);
        assert!(
            refused.message.contains("file.attach"),
            "{}",
            refused.message
        );
        assert!(
            page_may_run(
                "file.attach",
                &json!({ "task": "TM-1", "name": "a.txt", "data_base64": "aGk=" })
            )
            .is_ok()
        );
    }

    /// Words in an input field's name that say it may be a place on disk.
    const PATH_WORDS: &[&str] = &["path", "folder", "dir", "file_name", "location"];

    /// Fields with such a name that are not a place on disk: the field and
    /// what it is instead.
    const NOT_ON_DISK: &[(&str, &str, &str)] = &[
        (
            "folder.update",
            "folder",
            "a folder of projects in the sidebar, by ID or name",
        ),
        (
            "folder.rename",
            "folder",
            "a folder of projects in the sidebar, by ID or name",
        ),
        (
            "folder.delete",
            "folder",
            "a folder of projects in the sidebar, by ID or name",
        ),
        (
            "folder.move",
            "folder",
            "a folder of projects in the sidebar, by ID or name",
        ),
    ];

    /// The top-level fields of an input schema, also those of each variant.
    fn fields(schema: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            out.extend(props.keys().cloned());
        }
        for key in ["anyOf", "oneOf", "allOf"] {
            for variant in schema
                .get(key)
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                fields(variant, out);
            }
        }
    }

    /// Every operation's input that could name a file or a folder is either
    /// refused from the page or known not to be one.
    #[test]
    fn every_path_in_the_catalog_is_chosen_by_the_person() {
        let mut missing = Vec::new();
        for desc in gk_api::CATALOG {
            let schema = (desc.input_schema)().to_value();
            let mut names = Vec::new();
            fields(&schema, &mut names);
            for name in names {
                let lower = name.to_ascii_lowercase();
                if !PATH_WORDS.iter().any(|w| lower.contains(w)) {
                    continue;
                }
                let chosen = super::CHOSEN_PATHS.contains(&(desc.id, name.as_str()));
                let known = NOT_ON_DISK
                    .iter()
                    .any(|(op, field, _)| *op == desc.id && *field == name);
                if !chosen && !known {
                    missing.push(format!("{}.{name}", desc.id));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "add these to CHOSEN_PATHS (or NOT_ON_DISK with the reason): {missing:?}"
        );
        for (op, field) in super::CHOSEN_PATHS {
            let desc =
                gk_api::descriptor(op).unwrap_or_else(|| panic!("{op} is not in the catalog"));
            let mut names = Vec::new();
            fields(&(desc.input_schema)().to_value(), &mut names);
            assert!(
                names.iter().any(|n| n == field),
                "{op} has no input {field}"
            );
        }
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
