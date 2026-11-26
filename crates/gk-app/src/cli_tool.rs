// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The command-line tool, for Preferences › Agents & CLI: where `glyphkeel`
//! is, whether the shell finds it on the `PATH`, the command an MCP client
//! should run, and a link to the copy that came with the app in
//! `~/.local/bin`, for installs that don't put it on the `PATH` themselves
//! (an AppImage or an unpacked archive).
//!
//! On Windows the installer puts `glyphkeel.exe` next to the app and its
//! folder on the person's `PATH`, so there is nothing to link. The `PATH`
//! looked at there is also the one kept in the registry: a terminal opened
//! now finds the tool even when this app was started before the installer
//! changed it.
//!
//! The page never names a path: the link's place is fixed here.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gk_api::OpError;
use serde::Serialize;

use crate::messages::text;

/// The tool's file name.
#[cfg(not(windows))]
const TOOL: &str = "glyphkeel";
#[cfg(windows)]
const TOOL: &str = "glyphkeel.exe";

/// Where the command-line tool is and how to run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CliStatus {
    /// The command an MCP client runs, before `mcp`: an absolute path, or
    /// the Flatpak's `flatpak run` command.
    pub(crate) command: Vec<String>,
    /// Where the shell finds `glyphkeel` on the `PATH`, if it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) on_path: Option<String>,
    /// The copy that came with the app, next to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bundled: Option<String>,
    /// Where installing puts a link to the bundled copy; absent when
    /// there is nothing to install.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) link: Option<String>,
    /// Whether the link's folder is on the `PATH`.
    pub(crate) link_dir_on_path: bool,
    /// The app runs as a Flatpak: the tool runs through `flatpak run`.
    pub(crate) flatpak: bool,
}

/// What the status depends on, gathered once so tests can pass their own.
#[derive(Debug, Clone, Default)]
pub(crate) struct Env {
    path: Option<OsString>,
    exe_dir: Option<PathBuf>,
    home: Option<PathBuf>,
    flatpak_id: Option<String>,
    /// The person's own `PATH` as the registry keeps it (Windows), with
    /// `%NAME%` references already filled in.
    user_path: Option<OsString>,
}

impl Env {
    /// This process's environment.
    pub(crate) fn current() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf)),
            home: std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from),
            flatpak_id: std::env::var("FLATPAK_ID").ok().filter(|id| !id.is_empty()),
            user_path: user_path(),
        }
    }

    /// The absolute folders of the `PATH`, in order, then those of the
    /// person's own `PATH` in the registry that this process doesn't have.
    fn path_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for path in [&self.path, &self.user_path].into_iter().flatten() {
            for dir in std::env::split_paths(path).filter(|dir| dir.is_absolute()) {
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
        }
        dirs
    }

    /// `~/.local/bin`, the folder for a person's own programs. None on
    /// Windows, where the installer puts the tool on the `PATH`.
    fn link_dir(&self) -> Option<PathBuf> {
        if cfg!(windows) {
            return None;
        }
        self.home
            .as_ref()
            .map(|home| home.join(".local").join("bin"))
    }
}

/// The person's own `PATH` from the registry (`HKEY_CURRENT_USER\Environment`),
/// where the installer adds the tool's folder.
#[cfg(windows)]
fn user_path() -> Option<OsString> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", winreg::enums::KEY_READ)
        .ok()?;
    let value: String = key.get_value("Path").ok()?;
    Some(OsString::from(expand(&value, |name| {
        std::env::var(name).ok()
    })))
}

#[cfg(not(windows))]
fn user_path() -> Option<OsString> {
    None
}

/// `text` with every `%NAME%` replaced by `lookup(NAME)`; unknown names
/// and a lone `%` stay as they are, as the system leaves them.
#[cfg(any(windows, test))]
fn expand(text: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match (!name.is_empty()).then(|| lookup(name)).flatten() {
                    Some(value) => {
                        out.push_str(&value);
                        rest = &after[end + 1..];
                    }
                    None => {
                        out.push('%');
                        rest = after;
                    }
                }
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether `path` is a file that can be run.
fn is_program(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// The status for `env`.
pub(crate) fn status(env: &Env) -> CliStatus {
    let dirs = env.path_dirs();
    if let Some(id) = &env.flatpak_id {
        return CliStatus {
            command: vec![
                "flatpak".to_owned(),
                "run".to_owned(),
                "--command=glyphkeel".to_owned(),
                id.clone(),
            ],
            on_path: None,
            bundled: None,
            link: None,
            link_dir_on_path: false,
            flatpak: true,
        };
    }
    let on_path = dirs
        .iter()
        .map(|dir| dir.join(TOOL))
        .find(|p| is_program(p));
    let bundled = env
        .exe_dir
        .as_ref()
        .map(|dir| dir.join(TOOL))
        .filter(|p| is_program(p));
    let link_dir = env.link_dir();
    let link_dir_on_path = link_dir.as_ref().is_some_and(|dir| dirs.contains(dir));
    // Installing is offered while the shell doesn't find the tool, and
    // there is a copy to link to.
    let link = match (&on_path, &bundled, &link_dir) {
        (None, Some(_), Some(dir)) => Some(dir.join(TOOL)),
        _ => None,
    };
    let program = on_path
        .clone()
        .or_else(|| bundled.clone())
        .map_or_else(|| TOOL.to_owned(), |path| shown(&path));
    CliStatus {
        command: vec![program],
        on_path: on_path.as_deref().map(shown),
        bundled: bundled.as_deref().map(shown),
        link: link.as_deref().map(shown),
        link_dir_on_path,
        flatpak: false,
    }
}

/// Puts a link to the bundled copy in `~/.local/bin`. A link that is
/// already there and points at it is fine; anything else in its place is
/// left alone and reported.
pub(crate) fn install(env: &Env) -> Result<CliStatus, OpError> {
    let before = status(env);
    let (Some(bundled), Some(link)) = (&before.bundled, &before.link) else {
        return Err(OpError::invalid(text("cli-install-nothing", &[])));
    };
    let (bundled, link) = (PathBuf::from(bundled), PathBuf::from(link));
    if std::fs::read_link(&link).is_ok_and(|target| target == bundled) {
        return Ok(status(env));
    }
    if std::fs::symlink_metadata(&link).is_ok() {
        return Err(OpError::invalid(text(
            "cli-install-exists",
            &[("path", shown(&link))],
        )));
    }
    let failed = |err: std::io::Error| {
        OpError::storage(text(
            "cli-install-failed",
            &[("path", shown(&link)), ("reason", err.to_string())],
        ))
    };
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    make_link(&bundled, &link).map_err(failed)?;
    Ok(status(env))
}

#[cfg(unix)]
fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn make_link(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// Where the command-line tool is, for Preferences.
#[tauri::command]
pub(crate) fn cli_status() -> CliStatus {
    status(&Env::current())
}

/// Links the bundled command-line tool into `~/.local/bin`.
#[tauri::command]
pub(crate) fn install_cli() -> Result<CliStatus, OpError> {
    install(&Env::current())
}

#[cfg(test)]
mod expand_tests {
    use super::expand;

    #[test]
    fn references_in_the_registrys_path_are_filled_in() {
        let lookup =
            |name: &str| (name == "LOCALAPPDATA").then(|| r"C:\Users\a\AppData\Local".to_owned());
        assert_eq!(
            expand(r"%LOCALAPPDATA%\Programs\Glyphkeel;C:\bin", lookup),
            r"C:\Users\a\AppData\Local\Programs\Glyphkeel;C:\bin"
        );
        assert_eq!(expand("%NOPE%;x", lookup), "%NOPE%;x");
        assert_eq!(expand("100%", lookup), "100%");
        assert_eq!(
            expand("%%LOCALAPPDATA%", lookup),
            r"%C:\Users\a\AppData\Local"
        );
        assert_eq!(expand("", lookup), "");
    }
}

/// The status on Windows: the tool next to the app, on the `PATH` or only
/// on the person's `PATH` in the registry, and never a link to make.
#[cfg(all(test, windows))]
mod windows_tests {
    use std::ffi::OsString;

    use super::{Env, TOOL, install, status};

    #[test]
    fn the_tool_next_to_the_app_is_found_on_the_users_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = dir.path().join("Programs").join("Glyphkeel");
        std::fs::create_dir_all(&app).expect("dir");
        std::fs::write(app.join(TOOL), "MZ").expect("write");
        let env = Env {
            path: Some(OsString::from(r"C:\Windows\System32")),
            exe_dir: Some(app.clone()),
            home: Some(dir.path().join("home")),
            flatpak_id: None,
            user_path: None,
        };
        let before = status(&env);
        assert_eq!(before.bundled, Some(app.join(TOOL).display().to_string()));
        assert_eq!(before.on_path, None);
        assert_eq!(before.link, None, "the installer puts it on the PATH");
        assert!(install(&env).is_err());
        // The installer added the folder to the person's PATH after the app
        // started: a new terminal finds it.
        let quoted = format!("C:\\x;\"{}\"", app.display());
        let env = Env {
            user_path: Some(OsString::from(quoted)),
            ..env
        };
        assert_eq!(
            status(&env).on_path,
            Some(app.join(TOOL).display().to_string())
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::{Env, TOOL, install, status};

    fn program(path: &Path) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn env(root: &Path, path: &[&Path]) -> Env {
        Env {
            path: Some(std::env::join_paths(path).expect("join")),
            exe_dir: Some(root.join("app")),
            home: Some(root.join("home")),
            flatpak_id: None,
            user_path: None,
        }
    }

    #[test]
    fn the_tool_on_the_path_is_the_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("bin");
        program(&bin.join(TOOL));
        program(&dir.path().join("app").join(TOOL));
        let found = status(&env(dir.path(), &[&bin]));
        assert_eq!(found.command, vec![bin.join(TOOL).display().to_string()]);
        assert_eq!(found.link, None, "nothing to install");
    }

    #[test]
    fn a_bundled_tool_off_the_path_can_be_linked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bundled = dir.path().join("app").join(TOOL);
        program(&bundled);
        let local_bin = dir.path().join("home/.local/bin");
        let env = env(dir.path(), &[Path::new("relative"), &local_bin]);
        let before = status(&env);
        assert_eq!(before.command, vec![bundled.display().to_string()]);
        assert_eq!(
            before.link,
            Some(local_bin.join(TOOL).display().to_string())
        );
        assert!(before.link_dir_on_path);

        let after = install(&env).expect("install");
        assert_eq!(
            std::fs::read_link(local_bin.join(TOOL)).expect("link"),
            bundled
        );
        assert_eq!(
            after.on_path,
            Some(local_bin.join(TOOL).display().to_string())
        );
        assert_eq!(after.link, None);
    }

    #[test]
    fn a_file_in_the_way_is_left_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        program(&dir.path().join("app").join(TOOL));
        let local_bin = dir.path().join("home/.local/bin");
        std::fs::create_dir_all(&local_bin).expect("dir");
        std::fs::write(local_bin.join(TOOL), "mine").expect("write");
        // Off the PATH, so the status still offers the link.
        let env = env(dir.path(), &[]);
        let err = install(&env).expect_err("refused");
        assert!(err.message.contains(".local/bin"), "{}", err.message);
        assert_eq!(
            std::fs::read_to_string(local_bin.join(TOOL)).expect("read"),
            "mine"
        );
    }

    #[test]
    fn without_a_bundled_copy_there_is_nothing_to_install() {
        let dir = tempfile::tempdir().expect("tempdir");
        let env = env(dir.path(), &[]);
        let found = status(&env);
        assert_eq!(found.command, vec![TOOL.to_owned()]);
        assert!(install(&env).is_err());
    }

    #[test]
    fn a_flatpak_runs_the_tool_through_flatpak() {
        let env = Env {
            path: Some(OsString::from("/usr/bin")),
            flatpak_id: Some("io.github.sorrelfox.Glyphkeel".to_owned()),
            ..Env::default()
        };
        let found = status(&env);
        assert!(found.flatpak);
        assert_eq!(
            found.command,
            [
                "flatpak",
                "run",
                "--command=glyphkeel",
                "io.github.sorrelfox.Glyphkeel"
            ]
        );
    }
}
