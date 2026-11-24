// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Starting with the system: an XDG autostart entry.
//!
//! Installed from a package, the app writes its own entry in
//! `$XDG_CONFIG_HOME/autostart/` and removes it again; the entry itself is
//! the setting, so a change made in the desktop's own autostart settings
//! shows in Preferences. The entry starts the app with `--background`: in
//! background mode it then runs with its tray icon only, for reminders,
//! and otherwise opens its window as usual.
//!
//! Inside Flatpak the app can't write there. It asks the Background portal
//! instead, which writes the entry for it, and keeps the answer in its own
//! settings, since the entry is out of its sight.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use gk_api::OpError;

use crate::messages;

/// The argument the entry starts the app with.
pub(crate) const BACKGROUND_ARG: &str = "--background";

/// The entry's file name: the app's ID. Development builds use their own,
/// so they never replace an installed release's entry.
const FILE_NAME: &str = if cfg!(debug_assertions) {
    "io.github.sorrelfox.Glyphkeel.Devel.desktop"
} else {
    "io.github.sorrelfox.Glyphkeel.desktop"
};

/// Whether the app runs inside Flatpak's sandbox.
pub(crate) fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Where the entry goes: the autostart folder in the person's
/// configuration folder.
pub(crate) fn entry_file() -> Option<PathBuf> {
    let config_home = gk_store::config_dir()?.parent()?.to_path_buf();
    Some(config_home.join("autostart").join(FILE_NAME))
}

/// The program the entry starts: the AppImage itself when the app runs
/// from one (its mounted copy moves on every start), otherwise this program.
fn program() -> Option<PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(image));
    }
    std::env::current_exe().ok()
}

/// `arg` as one argument of an `Exec` line: quoted when it holds a
/// character the desktop entry specification reserves, with `"`, `` ` ``,
/// `$` and `\` escaped inside the quotes, and `%` doubled. The result is
/// then written as a string value, so its backslashes are doubled too.
fn exec_arg(arg: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    let quoted = if arg.is_empty() || arg.contains(RESERVED) {
        let mut out = String::from("\"");
        for c in arg.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        arg.to_owned()
    };
    quoted.replace('%', "%%").replace('\\', "\\\\")
}

/// The entry that starts `program` in the background.
fn entry(program: &Path) -> String {
    let comment = messages::text("autostart-comment", &[]).replace('\n', " ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Glyphkeel\n\
         Comment={comment}\n\
         Exec={} {BACKGROUND_ARG}\n\
         Icon=io.github.sorrelfox.Glyphkeel\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n\
         X-KDE-autostart-after=panel\n",
        exec_arg(&program.to_string_lossy()),
    )
}

/// Whether the entry in `file` starts the app: it exists, and neither the
/// specification's `Hidden` nor GNOME's own switch turns it off.
fn starts(file: &Path) -> bool {
    let Ok(text) = fs::read_to_string(file) else {
        return false;
    };
    !text.lines().map(str::trim).any(|line| {
        matches!(
            line.replace(' ', "").as_str(),
            "Hidden=true" | "X-GNOME-Autostart-enabled=false"
        )
    })
}

/// Writes or removes the entry in `file`.
fn write(file: &Path, on: bool, program: Option<&Path>) -> std::io::Result<()> {
    if !on {
        return match fs::remove_file(file) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        };
    }
    let program = program.ok_or_else(|| std::io::Error::other("the program's path is unknown"))?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = file.with_extension("desktop.tmp");
    let mut out = fs::File::create(&temporary)?;
    out.write_all(entry(program).as_bytes())?;
    out.sync_all()?;
    fs::rename(&temporary, file)
}

/// Whether the app starts with the system. `remembered` is the answer
/// the portal gave inside Flatpak.
pub(crate) fn enabled(remembered: Option<bool>) -> bool {
    if in_flatpak() {
        return remembered.unwrap_or(false);
    }
    entry_file().is_some_and(|file| starts(&file))
}

/// Switches starting with the system on or off; answers whether it is on
/// now. Inside Flatpak the desktop may ask the person first.
pub(crate) async fn set(on: bool) -> Result<bool, OpError> {
    #[cfg(target_os = "linux")]
    if in_flatpak() {
        return crate::shortcut::portal::request_autostart(on, BACKGROUND_ARG)
            .await
            .map_err(|reason| {
                OpError::storage(messages::text(
                    "autostart-portal-failed",
                    &[("reason", reason)],
                ))
            });
    }
    let file =
        entry_file().ok_or_else(|| OpError::storage(messages::text("autostart-no-home", &[])))?;
    let program = program();
    write(&file, on, program.as_deref()).map_err(|err| {
        OpError::storage(messages::text(
            "autostart-failed",
            &[
                ("path", file.display().to_string()),
                ("reason", err.to_string()),
            ],
        ))
    })?;
    Ok(starts(&file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_quoted_only_when_they_need_it() {
        assert_eq!(exec_arg("/usr/bin/glyphkeel-app"), "/usr/bin/glyphkeel-app");
        assert_eq!(
            exec_arg("/home/a b/Glyphkeel.AppImage"),
            "\"/home/a b/Glyphkeel.AppImage\""
        );
        // `$` is escaped inside the quotes, and the backslash is doubled again
        // as the value is a string.
        assert_eq!(exec_arg("/opt/$x"), "\"/opt/\\\\$x\"");
        assert_eq!(exec_arg("/opt/100%"), "/opt/100%%");
        assert_eq!(exec_arg(""), "\"\"");
    }

    #[test]
    fn the_entry_starts_the_program_in_the_background() {
        let text = entry(Path::new("/home/a b/glyphkeel-app"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("\nExec=\"/home/a b/glyphkeel-app\" --background\n"));
        assert!(text.contains("\nType=Application\n"));
        let comment = text.lines().find(|l| l.starts_with("Comment=")).unwrap();
        assert!(comment.len() > "Comment=".len());
        assert!(!comment.contains('{'));
    }

    #[test]
    fn switching_on_and_off_writes_and_removes_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("autostart").join(FILE_NAME);
        assert!(!starts(&file));
        let program = Path::new("/usr/bin/glyphkeel-app");
        write(&file, true, Some(program)).unwrap();
        assert!(starts(&file));
        // Written twice, it is the same entry.
        write(&file, true, Some(program)).unwrap();
        assert!(starts(&file));
        assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1);
        write(&file, false, Some(program)).unwrap();
        assert!(!file.exists());
        assert!(!starts(&file));
        // Off twice is fine.
        write(&file, false, None).unwrap();
        assert!(write(&file, true, None).is_err());
    }

    #[test]
    fn an_entry_the_desktop_switched_off_does_not_start() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE_NAME);
        let program = Path::new("/usr/bin/glyphkeel-app");
        for off in ["Hidden=true", "X-GNOME-Autostart-enabled=false"] {
            let text = entry(program).replace("X-GNOME-Autostart-enabled=true", off);
            fs::write(&file, text).unwrap();
            assert!(!starts(&file), "{off}");
        }
    }
}
