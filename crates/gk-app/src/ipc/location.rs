// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace the app opens: the one `GLYPHKEEL_WORKSPACE` names, by
//! name or by path, else the one the person last opened, else the default
//! workspace "Personal" in the data folder, created on the first run. The rule is the storage layer's, shared
//! with the command line and the agent server.

use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_store::{Location, StoreError, WorkspaceLayout};

/// The workspace to open, as the environment names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppLocation(Location);

/// The workspace `GLYPHKEEL_WORKSPACE` names, else the one last opened,
/// else the default one.
pub(crate) fn from_env() -> AppLocation {
    AppLocation(Location::find(None))
}

impl AppLocation {
    /// The workspace folder's layout. A named workspace lives in the shared
    /// data folder; `fallback_data_dir` is used only when that can't be
    /// found.
    pub(crate) fn layout(
        &self,
        fallback_data_dir: Option<&Path>,
    ) -> Result<WorkspaceLayout, OpError> {
        let data_dir: Option<PathBuf> =
            gk_store::data_dir().or_else(|| fallback_data_dir.map(Path::to_path_buf));
        self.0.layout(data_dir.as_deref()).map_err(|e| match e {
            StoreError::NoDataDir => OpError::storage("The data folder could not be found.")
                .with_hint("Set XDG_DATA_HOME or HOME, or name a folder in GLYPHKEEL_WORKSPACE."),
            e => OpError::invalid(format!("{e}.")),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gk_store::Location;

    use super::AppLocation;

    #[test]
    fn the_default_is_the_shared_personal_workspace() {
        let location = AppLocation(Location::choose(None, None));
        assert!(location.0.is_default());
        if gk_store::data_dir().is_some() {
            let layout = location.layout(None).expect("layout");
            assert!(layout.root().ends_with("workspaces/Personal"));
        }
    }

    #[test]
    fn a_path_needs_no_data_folder() {
        let location = AppLocation(Location::choose(None, Some("/tmp/ws")));
        let layout = location.layout(None).expect("layout");
        assert_eq!(layout.root(), Path::new("/tmp/ws"));
        let bad = AppLocation(Location::choose(None, Some("a:b")));
        assert!(bad.layout(Some(Path::new("/data"))).is_err());
    }
}
