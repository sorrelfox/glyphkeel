// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Starting with the system.
//!
//! - **Linux:** an XDG autostart entry. Installed from a package, the app
//!   writes its own entry in `$XDG_CONFIG_HOME/autostart/` and removes it
//!   again; the entry itself is the setting, so a change made in the
//!   desktop's own autostart settings shows in Preferences. Inside Flatpak
//!   the app can't write there. It asks the Background portal instead, which
//!   writes the entry for it, and keeps the answer in its own settings, since
//!   the entry is out of its sight.
//! - **Windows:** a value in the person's `Run` key in the registry. The
//!   value is the setting, as the entry is on Linux; when the person turns
//!   the app off in the system's list of startup apps, Windows keeps that in
//!   a key of its own, which counts too.
//!
//! Either way the app starts with `--background`: in background mode it then
//! runs with its tray icon only, for reminders, and otherwise opens its
//! window as usual.

use std::path::PathBuf;

use gk_api::OpError;

use crate::messages;

/// The argument the entry starts the app with.
pub(crate) const BACKGROUND_ARG: &str = "--background";

/// Whether the app runs inside Flatpak's sandbox.
pub(crate) fn in_flatpak() -> bool {
    cfg!(target_os = "linux") && std::path::Path::new("/.flatpak-info").exists()
}

/// The program the entry starts: the AppImage itself when the app runs
/// from one (its mounted copy moves on every start), otherwise this program.
fn program() -> Option<PathBuf> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(image));
    }
    std::env::current_exe().ok()
}

/// Whether the app starts with the system. `remembered` is the answer
/// the portal gave inside Flatpak.
pub(crate) fn enabled(remembered: Option<bool>) -> bool {
    #[cfg(windows)]
    {
        let _ = remembered;
        run_key::starts(&run_key::CurrentUser::RUN_KEYS)
    }
    #[cfg(not(windows))]
    {
        if in_flatpak() {
            return remembered.unwrap_or(false);
        }
        desktop_entry::file().is_some_and(|file| desktop_entry::starts(&file))
    }
}

/// Switches starting with the system on or off; answers whether it is on
/// now. Inside Flatpak the desktop may ask the person first.
pub(crate) async fn set(on: bool) -> Result<bool, OpError> {
    #[cfg(windows)]
    {
        let keys = run_key::CurrentUser::RUN_KEYS;
        run_key::write(&keys, on, program().as_deref()).map_err(|err| {
            messages::error(
                "autostart-run-key-failed",
                &[("reason", err.to_string())],
                OpError::storage,
            )
        })?;
        Ok(run_key::starts(&keys))
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "linux")]
        if in_flatpak() {
            return crate::shortcut::portal::request_autostart(on, BACKGROUND_ARG)
                .await
                .map_err(|reason| {
                    messages::error(
                        "autostart-portal-failed",
                        &[("reason", reason)],
                        OpError::storage,
                    )
                });
        }
        let file = desktop_entry::file()
            .ok_or_else(|| messages::error("autostart-no-home", &[], OpError::storage))?;
        let program = program();
        desktop_entry::write(&file, on, program.as_deref()).map_err(|err| {
            messages::error(
                "autostart-failed",
                &[
                    ("path", file.display().to_string()),
                    ("reason", err.to_string()),
                ],
                OpError::storage,
            )
        })?;
        Ok(desktop_entry::starts(&file))
    }
}

/// The XDG autostart entry.
#[cfg(not(windows))]
mod desktop_entry {
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::BACKGROUND_ARG;
    use crate::messages;

    /// The entry's file name: the app's ID. Development builds use their own,
    /// so they never replace an installed release's entry.
    pub(super) const FILE_NAME: &str = if cfg!(debug_assertions) {
        "io.github.sorrelfox.Glyphkeel.Devel.desktop"
    } else {
        "io.github.sorrelfox.Glyphkeel.desktop"
    };

    /// Where the entry goes: the autostart folder in the person's
    /// configuration folder. It is the desktop's folder, so the app's own
    /// configuration folder, which may be moved elsewhere, plays no part.
    pub(super) fn file() -> Option<PathBuf> {
        folder(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
        .map(|dir| dir.join(FILE_NAME))
    }

    /// The autostart folder: in `$XDG_CONFIG_HOME` when it is an absolute
    /// path, else in `~/.config`.
    pub(super) fn folder(config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
        let config_home = config_home
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                home.map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|home| home.join(".config"))
            })?;
        Some(config_home.join("autostart"))
    }

    /// `arg` as one argument of an `Exec` line: quoted when it holds a
    /// character the desktop entry specification reserves, with `"`, `` ` ``,
    /// `$` and `\` escaped inside the quotes, and `%` doubled. The result is
    /// then written as a string value, so its backslashes are doubled too.
    pub(super) fn exec_arg(arg: &str) -> String {
        const RESERVED: &[char] = &[
            ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#',
            '(', ')', '`',
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
    pub(super) fn entry(program: &Path) -> String {
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
    pub(super) fn starts(file: &Path) -> bool {
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
    pub(super) fn write(file: &Path, on: bool, program: Option<&Path>) -> std::io::Result<()> {
        if !on {
            return match fs::remove_file(file) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            };
        }
        let program =
            program.ok_or_else(|| std::io::Error::other("the program's path is unknown"))?;
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        let temporary = file.with_extension("desktop.tmp");
        let mut out = fs::File::create(&temporary)?;
        out.write_all(entry(program).as_bytes())?;
        out.sync_all()?;
        fs::rename(&temporary, file)
    }
}

/// The value in the person's `Run` key on Windows. The logic works on any
/// [`Registry`], so that it is tested with a stand-in on every platform.
#[cfg(any(windows, test))]
mod run_key {
    use std::io;
    use std::path::Path;

    use super::BACKGROUND_ARG;

    /// The value's name: the app's name. Development builds use their own,
    /// so they never replace an installed release's value.
    pub(super) const VALUE_NAME: &str = if cfg!(debug_assertions) {
        "Glyphkeel (development)"
    } else {
        "Glyphkeel"
    };

    /// The keys the value is read from and written to.
    pub(super) trait Registry {
        /// The command in the `Run` key's value `name`, if there is one.
        fn command(&self, name: &str) -> io::Result<Option<String>>;
        /// Sets the `Run` key's value `name`.
        fn set_command(&self, name: &str, command: &str) -> io::Result<()>;
        /// Removes the `Run` key's value `name`; a missing value is fine.
        fn remove_command(&self, name: &str) -> io::Result<()>;
        /// The data Windows keeps for value `name` in its list of startup
        /// apps, if it keeps any.
        fn approval(&self, name: &str) -> io::Result<Option<Vec<u8>>>;
        /// Removes what Windows keeps for value `name` in that list; a
        /// missing value is fine.
        fn remove_approval(&self, name: &str) -> io::Result<()>;
    }

    /// The value's command: the quoted program and the background argument.
    /// Windows paths can't hold `"`, so quoting is enough.
    pub(super) fn command(program: &Path) -> String {
        format!("\"{}\" {BACKGROUND_ARG}", program.display())
    }

    /// Whether the person switched the app off in the system's list of
    /// startup apps: the first byte of what Windows keeps is odd then (2 is
    /// on, 3 is off).
    fn switched_off(approval: Option<&[u8]>) -> bool {
        approval
            .and_then(|bytes| bytes.first())
            .is_some_and(|first| first & 1 == 1)
    }

    /// Whether the app starts with the system: its value is there, and the
    /// person hasn't switched it off in the system's list.
    pub(super) fn starts(registry: &impl Registry) -> bool {
        let present = registry
            .command(VALUE_NAME)
            .ok()
            .flatten()
            .is_some_and(|command| !command.trim().is_empty());
        present && !switched_off(registry.approval(VALUE_NAME).ok().flatten().as_deref())
    }

    /// Writes or removes the value. Switching on also clears a "switched
    /// off" the system's list kept, since the person has just asked for it.
    pub(super) fn write(
        registry: &impl Registry,
        on: bool,
        program: Option<&Path>,
    ) -> io::Result<()> {
        if !on {
            registry.remove_command(VALUE_NAME)?;
            return registry.remove_approval(VALUE_NAME);
        }
        let program = program.ok_or_else(|| io::Error::other("the program's path is unknown"))?;
        registry.set_command(VALUE_NAME, &command(program))?;
        if switched_off(registry.approval(VALUE_NAME)?.as_deref()) {
            registry.remove_approval(VALUE_NAME)?;
        }
        Ok(())
    }

    /// The registry of the person who runs the app.
    #[cfg(windows)]
    #[derive(Debug, Clone, Copy)]
    pub(super) struct CurrentUser {
        /// The `Run` key, under `HKEY_CURRENT_USER`.
        run: &'static str,
        /// The key of the system's list of startup apps for it.
        approved: &'static str,
    }

    #[cfg(windows)]
    impl CurrentUser {
        /// The keys Windows starts programs from at sign-in.
        pub(super) const RUN_KEYS: Self = Self {
            run: r"Software\Microsoft\Windows\CurrentVersion\Run",
            approved: r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        };

        fn open(path: &str, flags: u32) -> io::Result<Option<winreg::RegKey>> {
            let user = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            match user.open_subkey_with_flags(path, flags) {
                Ok(key) => Ok(Some(key)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        }

        fn remove(path: &str, name: &str) -> io::Result<()> {
            let Some(key) = Self::open(path, winreg::enums::KEY_SET_VALUE)? else {
                return Ok(());
            };
            match key.delete_value(name) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            }
        }
    }

    #[cfg(windows)]
    impl Registry for CurrentUser {
        fn command(&self, name: &str) -> io::Result<Option<String>> {
            let Some(key) = Self::open(self.run, winreg::enums::KEY_READ)? else {
                return Ok(None);
            };
            match key.get_value::<String, _>(name) {
                Ok(command) => Ok(Some(command)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        }

        fn set_command(&self, name: &str, command: &str) -> io::Result<()> {
            let user = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let (key, _) = user.create_subkey(self.run)?;
            key.set_value(name, &command)
        }

        fn remove_command(&self, name: &str) -> io::Result<()> {
            Self::remove(self.run, name)
        }

        fn approval(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
            let Some(key) = Self::open(self.approved, winreg::enums::KEY_READ)? else {
                return Ok(None);
            };
            match key.get_raw_value(name) {
                Ok(value) => Ok(Some(value.bytes.into_owned())),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        }

        fn remove_approval(&self, name: &str) -> io::Result<()> {
            Self::remove(self.approved, name)
        }
    }

    #[cfg(test)]
    mod tests {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::io;
        use std::path::Path;

        use super::{Registry, VALUE_NAME, command, starts, write};

        /// A registry in memory.
        #[derive(Default)]
        struct Fake {
            run: RefCell<HashMap<String, String>>,
            approved: RefCell<HashMap<String, Vec<u8>>>,
        }

        impl Registry for Fake {
            fn command(&self, name: &str) -> io::Result<Option<String>> {
                Ok(self.run.borrow().get(name).cloned())
            }
            fn set_command(&self, name: &str, command: &str) -> io::Result<()> {
                self.run
                    .borrow_mut()
                    .insert(name.to_owned(), command.to_owned());
                Ok(())
            }
            fn remove_command(&self, name: &str) -> io::Result<()> {
                self.run.borrow_mut().remove(name);
                Ok(())
            }
            fn approval(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
                Ok(self.approved.borrow().get(name).cloned())
            }
            fn remove_approval(&self, name: &str) -> io::Result<()> {
                self.approved.borrow_mut().remove(name);
                Ok(())
            }
        }

        const PROGRAM: &str = r"C:\Users\A B\AppData\Local\Programs\Glyphkeel\glyphkeel-app.exe";

        #[test]
        fn the_command_quotes_the_program_and_starts_in_the_background() {
            assert_eq!(
                command(Path::new(PROGRAM)),
                format!("\"{PROGRAM}\" --background")
            );
        }

        #[test]
        fn switching_on_and_off_writes_and_removes_the_value() {
            let registry = Fake::default();
            assert!(!starts(&registry));
            write(&registry, true, Some(Path::new(PROGRAM))).unwrap();
            assert!(starts(&registry));
            assert_eq!(
                registry.run.borrow().get(VALUE_NAME),
                Some(&command(Path::new(PROGRAM)))
            );
            // Written twice, it is the same value.
            write(&registry, true, Some(Path::new(PROGRAM))).unwrap();
            assert_eq!(registry.run.borrow().len(), 1);
            write(&registry, false, None).unwrap();
            assert!(!starts(&registry));
            assert!(registry.run.borrow().is_empty());
            // Off twice is fine; on without a program is not.
            write(&registry, false, None).unwrap();
            assert!(write(&registry, true, None).is_err());
        }

        #[test]
        fn the_systems_list_of_startup_apps_counts() {
            let registry = Fake::default();
            write(&registry, true, Some(Path::new(PROGRAM))).unwrap();
            let mut off = vec![3, 0, 0, 0];
            off.extend([0x10; 8]);
            registry
                .approved
                .borrow_mut()
                .insert(VALUE_NAME.to_owned(), off);
            assert!(!starts(&registry), "switched off in the system's list");
            registry
                .approved
                .borrow_mut()
                .insert(VALUE_NAME.to_owned(), vec![2, 0, 0, 0]);
            assert!(starts(&registry), "switched on in the system's list");
            // Switching on in Preferences undoes the system's "off".
            registry
                .approved
                .borrow_mut()
                .insert(VALUE_NAME.to_owned(), vec![3, 0, 0, 0]);
            write(&registry, true, Some(Path::new(PROGRAM))).unwrap();
            assert!(starts(&registry));
            // Switching off leaves nothing behind.
            write(&registry, false, None).unwrap();
            assert!(registry.approved.borrow().is_empty());
        }

        #[test]
        fn an_empty_value_does_not_start() {
            let registry = Fake::default();
            registry
                .run
                .borrow_mut()
                .insert(VALUE_NAME.to_owned(), "  ".to_owned());
            assert!(!starts(&registry));
        }
    }

    /// The real registry, in keys of the tests' own (under Wine in the
    /// container, or on Windows), never the person's `Run` key.
    #[cfg(all(test, windows))]
    mod registry_tests {
        use std::path::Path;

        use super::{CurrentUser, Registry, VALUE_NAME, starts, write};

        #[test]
        fn the_value_is_written_to_and_removed_from_the_registry() {
            let root = format!(r"Software\Glyphkeel tests\autostart-{}", std::process::id());
            let run: &'static str = Box::leak(format!(r"{root}\Run").into_boxed_str());
            let approved: &'static str = Box::leak(format!(r"{root}\Approved").into_boxed_str());
            let registry = CurrentUser { run, approved };
            let program = Path::new(r"C:\Program Files\Glyphkeel\glyphkeel-app.exe");
            assert!(!starts(&registry), "no keys yet");
            write(&registry, false, None).expect("off without keys");
            write(&registry, true, Some(program)).expect("on");
            assert!(starts(&registry));
            assert_eq!(
                registry.command(VALUE_NAME).expect("read").as_deref(),
                Some(r#""C:\Program Files\Glyphkeel\glyphkeel-app.exe" --background"#)
            );
            let user = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let (list, _) = user.create_subkey(approved).expect("approved key");
            list.set_raw_value(
                VALUE_NAME,
                &winreg::RegValue {
                    bytes: vec![3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into(),
                    vtype: winreg::enums::RegType::REG_BINARY,
                },
            )
            .expect("switched off");
            assert!(!starts(&registry));
            write(&registry, true, Some(program)).expect("on again");
            assert!(starts(&registry));
            assert_eq!(registry.approval(VALUE_NAME).expect("read"), None);
            write(&registry, false, None).expect("off");
            assert!(!starts(&registry));
            assert_eq!(registry.command(VALUE_NAME).expect("read"), None);
            user.delete_subkey_all(&root).expect("clean up");
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::desktop_entry::{FILE_NAME, entry, exec_arg, folder, starts, write};

    #[test]
    fn the_entry_goes_to_the_desktops_autostart_folder() {
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            folder(os("/home/a/.cfg"), os("/home/a")),
            Some(PathBuf::from("/home/a/.cfg/autostart"))
        );
        // A relative or empty XDG_CONFIG_HOME is ignored, as the
        // specification says.
        assert_eq!(
            folder(os("cfg"), os("/home/a")),
            Some(PathBuf::from("/home/a/.config/autostart"))
        );
        assert_eq!(
            folder(os(""), os("/home/a")),
            Some(PathBuf::from("/home/a/.config/autostart"))
        );
        assert_eq!(folder(None, None), None);
        assert_eq!(folder(None, os("")), None);
    }

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
