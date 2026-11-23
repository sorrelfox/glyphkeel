// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Which workspace the app opens: the default one under the data folder
//! (`$XDG_DATA_HOME/glyphkeel/workspaces/default` on Linux), created on the
//! first run, or the one `GLYPHKEEL_WORKSPACE` names, by name or by path.

use std::path::{Path, PathBuf};

use gk_api::OpError;
use gk_store::WorkspaceLayout;

/// The variable that picks another workspace, for development and tests.
pub(crate) const ENV: &str = "GLYPHKEEL_WORKSPACE";

/// The name of the workspace the app opens when nothing else is asked for.
pub(crate) const DEFAULT_NAME: &str = "default";

/// The workspace to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Location {
    /// A workspace in the data folder, by name.
    Named(String),
    /// A workspace folder anywhere.
    Path(PathBuf),
}

/// The workspace `GLYPHKEEL_WORKSPACE` names, or the default one.
pub(crate) fn from_env() -> Location {
    parse(std::env::var(ENV).ok().as_deref())
}

/// A value with a path separator, or one that starts with `.` or `~`, is
/// a folder; any other value is a name in the data folder.
pub(crate) fn parse(value: Option<&str>) -> Location {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Location::Named(DEFAULT_NAME.to_owned()),
        Some(v) if v.contains(std::path::MAIN_SEPARATOR) || v.contains('/') => {
            Location::Path(PathBuf::from(v))
        }
        Some(v) if v.starts_with('.') || v.starts_with('~') => Location::Path(PathBuf::from(v)),
        Some(v) => Location::Named(v.to_owned()),
    }
}

impl Location {
    /// The workspace folder's layout. `data_dir` is the app's data folder
    /// (`$XDG_DATA_HOME/glyphkeel`); a named workspace needs it.
    pub(crate) fn layout(&self, data_dir: Option<&Path>) -> Result<WorkspaceLayout, OpError> {
        match self {
            Self::Path(path) => Ok(WorkspaceLayout::new(expand_home(path))),
            Self::Named(name) => {
                let data_dir = data_dir.ok_or_else(|| {
                    OpError::storage("The data folder could not be found.")
                        .with_hint("Set XDG_DATA_HOME or HOME.")
                })?;
                WorkspaceLayout::in_data_dir(data_dir, name).map_err(|e| {
                    OpError::invalid(format!("\"{name}\" can't be a workspace name: {e}."))
                })
            }
        }
    }
}

fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => std::env::var_os("HOME")
            .map_or_else(|| path.to_path_buf(), |home| PathBuf::from(home).join(rest)),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{DEFAULT_NAME, Location, parse};

    #[test]
    fn nothing_set_means_the_default_workspace() {
        assert_eq!(parse(None), Location::Named(DEFAULT_NAME.to_owned()));
        assert_eq!(parse(Some("  ")), Location::Named(DEFAULT_NAME.to_owned()));
    }

    #[test]
    fn names_and_paths_are_told_apart() {
        assert_eq!(parse(Some("work")), Location::Named("work".to_owned()));
        assert_eq!(
            parse(Some("/tmp/ws")),
            Location::Path(PathBuf::from("/tmp/ws"))
        );
        assert_eq!(parse(Some("./ws")), Location::Path(PathBuf::from("./ws")));
        assert_eq!(parse(Some("~/ws")), Location::Path(PathBuf::from("~/ws")));
    }

    #[test]
    fn a_named_workspace_lives_under_the_data_folder() {
        let data = Path::new("/data/glyphkeel");
        let layout = Location::Named("default".to_owned())
            .layout(Some(data))
            .unwrap();
        assert_eq!(
            layout.root(),
            Path::new("/data/glyphkeel/workspaces/default")
        );
        assert!(Location::Named("default".to_owned()).layout(None).is_err());
    }
}
