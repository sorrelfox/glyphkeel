// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Starting the desktop app. `glyphkeel` itself never links the desktop
//! toolkit or the web view; it starts `glyphkeel-app`, found next to its own
//! binary first, then on `PATH`, and returns without waiting for it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gk_api::OpError;

/// The app's executable name.
const APP: &str = "glyphkeel-app";
/// The scheme of the app's links.
const SCHEME: &str = "glyphkeel://";
/// The app's argument for its quick-add window. A running app is handed
/// it and shows the window; otherwise the app starts with only that window.
pub(crate) const QUICK_ADD: &str = "--quick-add";

/// The links to hand to the app: `glyphkeel://` links as given, task keys
/// as `glyphkeel://open/<KEY>`.
pub(crate) fn links(args: &[String]) -> Result<Vec<String>, OpError> {
    args.iter()
        .map(|a| {
            if a.starts_with(SCHEME) {
                Ok(a.clone())
            } else if !a.is_empty()
                && a.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(format!("{SCHEME}open/{a}"))
            } else {
                Err(
                    OpError::invalid(format!("'{a}' is neither a task key nor a {SCHEME} link."))
                        .with_hint(
                            "Give a key such as TM-41, or a link such as glyphkeel://open/TM-41.",
                        ),
                )
            }
        })
        .collect()
}

/// Starts the app with `links` and returns once it has started.
pub(crate) fn start(links: &[String]) -> Result<(), OpError> {
    let app = find().ok_or_else(|| {
        OpError::not_found(
            format!("The desktop app ({APP}) is neither next to this program nor on PATH."),
            &[],
        )
        .with_hint("Install the desktop app, or use the commands listed by `glyphkeel --help`.")
    })?;
    let mut cmd = Command::new(&app);
    cmd.args(links)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut cmd);
    cmd.spawn()
        .map(drop)
        .map_err(|e| OpError::storage(format!("Can't start {}: {e}.", app.display())))
}

/// Its own process group, so closing the terminal or Ctrl+C there leaves
/// the app running.
#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// No console window, and not in the console's Ctrl+C group.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn detach(_cmd: &mut Command) {}

fn file_name() -> OsString {
    let mut name = OsString::from(APP);
    name.push(std::env::consts::EXE_SUFFIX);
    name
}

/// Next to this program, then on `PATH`.
fn find() -> Option<PathBuf> {
    let name = file_name();
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)));
    beside.filter(|p| is_executable(p)).or_else(|| {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join(&name))
            .find(|p| is_executable(p))
    })
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_become_links_and_links_stay() {
        let args = ["TM-41".to_owned(), "glyphkeel://open/TM-2".to_owned()];
        assert_eq!(
            links(&args).expect("valid"),
            ["glyphkeel://open/TM-41", "glyphkeel://open/TM-2"]
        );
    }

    #[test]
    fn other_text_is_refused() {
        let err = links(&["https://example.org".to_owned()]).expect_err("refused");
        assert_eq!(err.code, gk_api::ErrorCode::Invalid);
    }
}
