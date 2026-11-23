// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace a command works on.

use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_store::WorkspaceLayout;

/// The workspace used when `--workspace` is not given. It is created on
/// first use, as the app does on its first start.
pub(crate) const DEFAULT_NAME: &str = "Personal";

/// The folder of the program's data: `$XDG_DATA_HOME/glyphkeel` on Linux,
/// `%LOCALAPPDATA%\Glyphkeel` on Windows.
fn data_dir() -> Result<PathBuf, OpError> {
    let base = directories::BaseDirs::new().ok_or_else(|| {
        OpError::storage("Can't find the home folder, so there is no default data folder.")
            .with_hint("Name the workspace folder with --workspace <path>.")
    })?;
    let name = if cfg!(windows) {
        "Glyphkeel"
    } else {
        "glyphkeel"
    };
    Ok(base.data_local_dir().join(name))
}

/// Whether `arg` names a folder rather than a workspace in the data folder.
fn is_path(arg: &str) -> bool {
    arg == "."
        || arg == ".."
        || arg.contains('/')
        || arg.contains(std::path::MAIN_SEPARATOR)
        || Path::new(arg).is_absolute()
}

/// The workspace `arg` names, and whether it may be created when it doesn't
/// exist (only the default one is; a mistyped name must not make a new,
/// empty workspace).
pub(crate) fn resolve(arg: Option<&str>) -> Result<WorkspaceLayout, OpError> {
    let (layout, may_create) = match arg {
        None => (in_data_dir(DEFAULT_NAME)?, true),
        Some(a) if is_path(a) => (WorkspaceLayout::new(a), false),
        Some(a) => (in_data_dir(a)?, false),
    };
    if !may_create && !layout.database().is_file() {
        return Err(OpError::not_found(
            format!("There is no workspace at {}.", layout.root().display()),
            &[],
        )
        .with_hint("Check the name or path given with --workspace."));
    }
    Ok(layout)
}

fn in_data_dir(name: &str) -> Result<WorkspaceLayout, OpError> {
    WorkspaceLayout::in_data_dir(&data_dir()?, name).map_err(|_| {
        OpError::invalid_at(
            "workspace",
            format!("'{name}' is not a workspace name."),
        )
        .with_hint(
            "A name has letters, digits, spaces, '-' and '_'; to name a folder, give a path such as ./my-workspace.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_names_are_told_apart() {
        assert!(is_path("."));
        assert!(is_path("./work"));
        assert!(is_path("/tmp/work"));
        assert!(!is_path("Personal"));
        assert!(!is_path("Side project"));
    }

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
        let err = resolve(Some("..hidden")).expect_err("bad name");
        assert_eq!(err.code, gk_api::ErrorCode::Invalid);
    }
}
