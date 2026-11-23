// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace a command works on.

use gk_api::OpError;
use gk_store::{Location, StoreError, WorkspaceLayout};

/// The workspace `arg` (`--workspace`) names, else the one
/// `GLYPHKEEL_WORKSPACE` names, else the default workspace, and whether it
/// may be created when it doesn't exist. Only a workspace chosen without
/// `--workspace` is created, as the app does on its first start; a mistyped
/// name must not make a new, empty workspace.
pub(crate) fn resolve(arg: Option<&str>) -> Result<WorkspaceLayout, OpError> {
    let may_create = arg.is_none_or(|a| a.trim().is_empty());
    let layout = Location::find(arg)
        .layout(gk_store::data_dir().as_deref())
        .map_err(|e| match e {
            StoreError::NoDataDir => OpError::storage(
                "Can't find the home folder, so there is no default data folder.",
            )
            .with_hint("Name the workspace folder with --workspace <path>."),
            e => OpError::invalid_at("workspace", format!("{e}.")).with_hint(
                "A name has letters, digits, spaces, '-' and '_'; to name a folder, give a path such as ./my-workspace.",
            ),
        })?;
    if !may_create && !layout.database().is_file() {
        return Err(OpError::not_found(
            format!("There is no workspace at {}.", layout.root().display()),
            &[],
        )
        .with_hint("Check the name or path given with --workspace."));
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
