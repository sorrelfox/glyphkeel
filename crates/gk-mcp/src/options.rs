// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The options of `glyphkeel mcp` and the workspace they name.

use std::ffi::OsString;

use clap::Parser;
use gk_api::{OpError, Profile, Toolset};
use gk_store::{Location, StoreError, WorkspaceLayout};

use crate::messages;

/// The workspace opened when neither `--workspace` nor
/// `GLYPHKEEL_WORKSPACE` names one, shared with the app and the command
/// line.
pub const DEFAULT_WORKSPACE: &str = gk_store::DEFAULT_WORKSPACE;

/// Serve Glyphkeel to an AI agent over MCP, on standard input and output.
///
/// The agent's MCP client starts this command. Logs go to standard error;
/// standard output carries the protocol only.
#[derive(Debug, Clone, Parser)]
#[command(name = "glyphkeel mcp", no_binary_name = true, version)]
pub struct Options {
    /// Which workspace: a name under the data folder, or a path to a
    /// workspace folder. Default: the one `GLYPHKEEL_WORKSPACE` names, else
    /// "Personal".
    #[arg(long, value_name = "NAME|PATH")]
    pub workspace: Option<String>,

    /// Limit the agent to this project and its sub-projects.
    #[arg(long, value_name = "KEY")]
    pub project: Option<String>,

    /// What the agent may do. The workspace setting "most an agent may do"
    /// caps it.
    #[arg(long, value_name = "PROFILE", default_value = "standard", value_parser = parse_profile)]
    pub profile: Profile,

    /// Which groups of tools to offer, separated by commas. Default: every
    /// group except `admin`.
    #[arg(long, value_name = "LIST", value_delimiter = ',', value_parser = parse_toolset)]
    pub toolsets: Option<Vec<Toolset>>,

    /// The name shown in history when the MCP client sends none.
    #[arg(long, value_name = "NAME")]
    pub agent_name: Option<String>,

    /// The time zone of "today" and local times: "system" or an IANA name
    /// such as Europe/Berlin. Default: the personal setting shared with the
    /// app and the command line.
    #[arg(long, value_name = "ZONE")]
    pub tz: Option<String>,

    /// How much to log on standard error: off, error, warn, info, debug or
    /// trace.
    #[arg(long, value_name = "LEVEL", default_value = "warn", value_parser = parse_level)]
    pub log_level: tracing::level_filters::LevelFilter,
}

impl Options {
    /// Parses the arguments that follow `mcp`.
    ///
    /// # Errors
    /// The usage error, including `--help` and `--version`, which clap
    /// reports as errors that print to standard output.
    pub fn parse_args(args: &[OsString]) -> Result<Self, clap::Error> {
        Self::try_parse_from(args)
    }

    /// The options with every default, for tests and embedders.
    #[must_use]
    pub fn with_workspace(workspace: impl Into<String>) -> Self {
        Self {
            workspace: Some(workspace.into()),
            project: None,
            profile: Profile::Standard,
            toolsets: None,
            agent_name: None,
            tz: None,
            log_level: tracing::level_filters::LevelFilter::WARN,
        }
    }

    /// The toolsets switched on, in listing order.
    #[must_use]
    pub fn toolsets(&self) -> Vec<Toolset> {
        match &self.toolsets {
            Some(chosen) => Toolset::ALL
                .into_iter()
                .filter(|t| chosen.contains(t))
                .collect(),
            None => Toolset::defaults(),
        }
    }

    /// The workspace folder these options name.
    ///
    /// # Errors
    /// `invalid` when the name can't be a folder name, `storage` when no
    /// data folder can be found.
    pub fn layout(&self) -> Result<WorkspaceLayout, OpError> {
        workspace_layout(self.workspace.as_deref())
    }
}

/// The workspace named by `--workspace`, else by `GLYPHKEEL_WORKSPACE`,
/// else the default one: a path when it contains a path separator or starts
/// with `.` or `~`, otherwise a name under the data folder
/// (`$XDG_DATA_HOME/glyphkeel/workspaces/<name>`).
///
/// # Errors
/// As [`Options::layout`].
pub fn workspace_layout(arg: Option<&str>) -> Result<WorkspaceLayout, OpError> {
    Location::find(arg)
        .layout(gk_store::data_dir().as_deref())
        .map_err(|e| match e {
            StoreError::NoDataDir => messages::hint(
                messages::error("mcp-no-data-folder", &[], OpError::storage),
                "mcp-no-data-folder-hint",
                &[],
            ),
            e => OpError::invalid(format!("{e}.")),
        })
}

fn parse_profile(s: &str) -> Result<Profile, String> {
    s.parse()
}

fn parse_toolset(s: &str) -> Result<Toolset, String> {
    Toolset::parse(s.trim()).ok_or_else(|| {
        let names: Vec<&str> = Toolset::ALL.iter().map(|t| t.as_str()).collect();
        messages::text(
            "mcp-unknown-toolset",
            &[("toolset", s.to_owned()), ("toolsets", names.join(", "))],
        )
    })
}

fn parse_level(s: &str) -> Result<tracing::level_filters::LevelFilter, String> {
    s.parse()
        .map_err(|_| messages::text("mcp-unknown-log-level", &[("level", s.to_owned())]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, clap::Error> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        Options::parse_args(&args)
    }

    #[test]
    fn defaults() {
        let o = parse(&[]).expect("parses");
        assert_eq!(o.profile, Profile::Standard);
        assert_eq!(o.toolsets(), Toolset::defaults());
        assert!(o.workspace.is_none());
    }

    #[test]
    fn every_option() {
        let o = parse(&[
            "--workspace",
            "Work",
            "--project",
            "TM",
            "--profile",
            "read-only",
            "--toolsets",
            "admin,workspace",
            "--agent-name",
            "Helper",
            "--log-level",
            "debug",
        ])
        .expect("parses");
        assert_eq!(o.workspace.as_deref(), Some("Work"));
        assert_eq!(o.project.as_deref(), Some("TM"));
        assert_eq!(o.profile, Profile::ReadOnly);
        // Listing order, whatever order they were given in.
        assert_eq!(o.toolsets(), [Toolset::Workspace, Toolset::Admin]);
        assert_eq!(o.agent_name.as_deref(), Some("Helper"));
    }

    #[test]
    fn bad_values_are_usage_errors() {
        assert!(parse(&["--profile", "admin"]).is_err());
        assert!(parse(&["--toolsets", "workspace,nope"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
    }

    #[test]
    fn workspace_names_and_paths() {
        use std::path::Path;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("ws");
        let layout = workspace_layout(Some(path.to_str().expect("utf-8"))).expect("path");
        assert_eq!(layout.root(), path);
        let layout = workspace_layout(Some("./ws")).expect("relative path");
        assert_eq!(layout.root(), Path::new("./ws"));
        if gk_store::data_dir().is_some() {
            let layout = workspace_layout(Some("Work")).expect("name");
            assert!(layout.root().ends_with("glyphkeel/workspaces/Work"));
            assert!(workspace_layout(None).is_ok());
        }
    }
}
