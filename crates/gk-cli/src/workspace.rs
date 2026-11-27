// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace a command works on, and `glyphkeel workspace list`,
//! `create`, `rename` and `open`, which work on the workspaces in the data
//! folder rather than inside one, and `encrypt` and `decrypt`, which change
//! how one is stored.

use std::path::PathBuf;

use clap::{Arg, ArgMatches, Command};
use gk_api::OpError;
use gk_engine::workspaces;
use gk_store::{Location, StoreError, WorkspaceLayout};
use serde_json::{Value, json};

use crate::messages::{self, text};

/// The words of the commands under `glyphkeel workspace`.
pub(crate) const LIST: &str = "list";
pub(crate) const CREATE: &str = "create";
pub(crate) const RENAME: &str = "rename";
pub(crate) const OPEN: &str = "open";
pub(crate) const ENCRYPT: &str = "encrypt";
pub(crate) const DECRYPT: &str = "decrypt";

/// Adds `list`, `create`, `rename` and `open` to the `workspace` command,
/// which on its own still shows the workspace's summary.
pub(crate) fn commands(workspace: Command) -> Command {
    let name = |id: &'static str, help: &str| {
        Arg::new(id)
            .required(true)
            .value_name("NAME")
            .help(text(help, &[]))
    };
    workspace
        .args_conflicts_with_subcommands(true)
        .subcommand(Command::new(LIST).about(text("cli-workspace-list-about", &[])))
        .subcommand(
            Command::new(CREATE)
                .about(text("cli-workspace-create-about", &[]))
                .arg(name("name", "cli-workspace-create-name-help")),
        )
        .subcommand(
            Command::new(RENAME)
                .about(text("cli-workspace-rename-about", &[]))
                .arg(name("old", "cli-workspace-rename-old-help"))
                .arg(name("new", "cli-workspace-rename-new-help")),
        )
        .subcommand(
            Command::new(ENCRYPT)
                .about(text("cli-workspace-encrypt-about", &[]))
                .long_about(text("cli-workspace-encrypt-long", &[])),
        )
        .subcommand(Command::new(DECRYPT).about(text("cli-workspace-decrypt-about", &[])))
        .subcommand(
            Command::new(OPEN)
                .about(text("cli-workspace-open-about", &[]))
                .arg(
                    Arg::new("workspace")
                        .required(true)
                        .value_name("NAME|PATH")
                        .help(text("cli-workspace-open-name-help", &[])),
                ),
        )
}

/// Runs one of the commands under `glyphkeel workspace`: what to print for
/// people, and the JSON for `--json`.
pub(crate) fn run(
    word: &str,
    m: &ArgMatches,
    chosen: Option<&str>,
) -> Result<(String, Value), OpError> {
    let data_dir = data_dir()?;
    let arg = |id: &str| {
        m.get_one::<String>(id)
            .map(String::as_str)
            .unwrap_or_default()
    };
    match word {
        LIST => {
            let current = Location::find(chosen).layout(Some(&data_dir)).ok();
            let list = workspaces::list(&data_dir);
            let mut lines = String::new();
            let mut items = Vec::new();
            for w in &list {
                let is_current = current
                    .as_ref()
                    .is_some_and(|c| c.root() == std::path::Path::new(&w.path));
                let id = if is_current {
                    "cli-workspace-list-current"
                } else {
                    "cli-workspace-list-other"
                };
                lines.push_str(&text(id, &[("name", w.name.clone())]));
                lines.push('\n');
                items.push(json!({ "name": w.name, "path": w.path, "current": is_current }));
            }
            if list.is_empty() {
                lines = text("cli-workspace-list-none", &[]);
                lines.push('\n');
            }
            Ok((lines, json!({ "items": items })))
        }
        CREATE => {
            let layout = workspaces::create(&data_dir, arg("name"))?;
            let name = workspaces::name_of(&layout);
            let line = text("cli-workspace-created", &[("name", name.clone())]);
            Ok((line + "\n", entry(&name, &layout)))
        }
        RENAME => {
            let layout = workspaces::rename(&data_dir, arg("old"), arg("new"))?;
            let name = workspaces::name_of(&layout);
            let line = text(
                "cli-workspace-renamed",
                &[("old", arg("old").trim().to_owned()), ("new", name.clone())],
            );
            Ok((line + "\n", entry(&name, &layout)))
        }
        ENCRYPT | DECRYPT => change_encryption(word == ENCRYPT, chosen),
        _ => {
            let layout = workspaces::choose(Some(&data_dir), arg("workspace"))?;
            let name = workspaces::name_of(&layout);
            let line = text("cli-workspace-opened", &[("name", name.clone())]);
            Ok((line + "\n", entry(&name, &layout)))
        }
    }
}

/// `glyphkeel workspace encrypt` (or `decrypt`): the workspace must be
/// closed everywhere else; a backup is made first.
fn change_encryption(encrypt: bool, chosen: Option<&str>) -> Result<(String, Value), OpError> {
    use gk_engine::encryption::{self, Change};
    let layout = resolve(chosen)?;
    if !layout.database().is_file() {
        return Err(gk_engine::store_error(&StoreError::NotAWorkspace(
            layout.database(),
        )));
    }
    crate::passphrase::unlock(&layout)?;
    let name = workspaces::name_of(&layout);
    let change = if encrypt {
        Change::Encrypt(crate::passphrase::new_passphrase()?)
    } else {
        Change::Decrypt
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    let changed = encryption::change(&layout, None, &change, now, env!("CARGO_PKG_VERSION"))?;
    let args = [
        ("name", name.clone()),
        ("backup", changed.backup.name.clone()),
    ];
    let mut lines = if encrypt {
        text("cli-workspace-encrypted", &args)
    } else {
        text("cli-workspace-decrypted", &args)
    };
    lines.push('\n');
    if changed.backups_left_plain > 0 {
        lines.push_str(&encryption::backups_left_plain_text(
            changed.backups_left_plain,
        ));
        lines.push('\n');
    }
    if changed.files_failed > 0 {
        lines.push_str(&encryption::files_left_text(changed.files_failed));
        lines.push('\n');
    }
    let mut value = entry(&name, &layout);
    value["encrypted"] = Value::from(encrypt);
    value["backup"] = Value::from(changed.backup.name);
    value["backups_left_plain"] = Value::from(changed.backups_left_plain);
    value["files_converted"] = Value::from(changed.files_converted);
    value["files_failed"] = Value::from(changed.files_failed);
    Ok((lines, value))
}

fn entry(name: &str, layout: &WorkspaceLayout) -> Value {
    json!({ "name": name, "path": layout.root().display().to_string() })
}

fn data_dir() -> Result<PathBuf, OpError> {
    gk_store::data_dir().ok_or_else(|| {
        messages::hint(
            messages::error("cli-no-home", &[], OpError::storage),
            "cli-no-home-hint",
            &[],
        )
    })
}

/// The workspace `arg` (`--workspace`) names, else the one
/// `GLYPHKEEL_WORKSPACE` names, else the one the person last chose, else
/// the default workspace, and whether it
/// may be created when it doesn't exist. Only a workspace chosen without
/// `--workspace` is created, as the app does on its first start; a mistyped
/// name must not make a new, empty workspace.
pub(crate) fn resolve(arg: Option<&str>) -> Result<WorkspaceLayout, OpError> {
    let may_create = arg.is_none_or(|a| a.trim().is_empty());
    let layout = Location::find(arg)
        .layout(gk_store::data_dir().as_deref())
        .map_err(|e| match e {
            StoreError::NoDataDir => messages::hint(
                messages::error("cli-no-home", &[], OpError::storage),
                "cli-no-home-hint",
                &[],
            ),
            StoreError::InvalidName(name) => messages::hint(
                messages::error("cli-workspace-name", &[("name", name)], |m| {
                    OpError::invalid_at("workspace", m)
                }),
                "cli-workspace-name-hint",
                &[],
            ),
            e => gk_engine::store_error(&e),
        })?;
    if !may_create && !layout.database().is_file() {
        return Err(messages::hint(
            messages::error(
                "cli-workspace-missing",
                &[("path", layout.root().display().to_string())],
                |m| OpError::not_found(m, &[]),
            ),
            "cli-workspace-missing-hint",
            &[],
        ));
    }
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_named_workspace_is_not_created() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("nothing-here");
        let err = resolve(Some(path.to_str().expect("utf-8"))).expect_err("missing");
        assert_eq!(err.code, gk_api::ErrorCode::NotFound);
        assert!(!path.exists());
    }

    #[test]
    fn a_bad_name_is_invalid() {
        let err = resolve(Some("a:b")).expect_err("bad name");
        assert_eq!(err.code, gk_api::ErrorCode::Invalid);
    }
}
