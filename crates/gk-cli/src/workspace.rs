// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace a command works on.

use gk_api::OpError;
use gk_store::{Location, StoreError, WorkspaceLayout};

use crate::messages;

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
            e => OpError::invalid_at("workspace", format!("{e}.")),
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
