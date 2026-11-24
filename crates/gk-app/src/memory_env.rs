// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Starts the app with the memory settings it needs in its environment.
//!
//! The app's process and the web view's processes read these memory settings
//! from the environment once, when they start:
//!
//! - The web view's script engine lets its heap grow in proportion to the
//!   machine's memory before it collects, so on a large machine one scroll
//!   through a long list leaves hundreds of megabytes of garbage behind.
//!   `JSC_forceRAMSize` makes it size its heap as if the machine had 256 MB.
//!   The engine runs in the web view's own process, which inherits the app's
//!   environment.
//! - glibc's allocator keeps freed memory in one arena per busy thread. With
//!   `MALLOC_ARENA_MAX` the engine's blocking threads share two arenas, so
//!   the memory one answer freed is reused by the next instead of staying
//!   with an idle thread.
//!
//! glibc's `MALLOC_MMAP_THRESHOLD_` is left alone on purpose. A low value
//! (8 KiB) would give the memory of large answers back to the system as
//! soon as they are freed, but then every block above it sits in its own
//! mapping, which ends right after the block and is gone once it is freed.
//! The web view's process inherits the setting. On Debian 12 (WebKitGTK
//! 2.50 with Mesa's software renderer, as in a virtual machine or a
//! container without a GPU) its compositor thread then died with a
//! segmentation fault in the renderer two to three seconds after a large
//! workspace opened, on every start; without the setting it didn't. The
//! renderer seems to read a little outside the blocks it was given, which
//! in the heap lands on memory that is still there. The setting can't be
//! kept for the app's process alone: the web process starts later, from
//! the app's environment, and the app can't safely take a setting out of
//! its own environment once it runs. The app's process keeps somewhat more
//! memory after a large answer instead.
//!
//! A running program can't safely change its own environment, so the app
//! replaces itself once, at the very start, with a fresh start of the
//! same program that has the missing settings added. Settings the user has
//! set already are kept as they are. The names it added are noted in
//! [`ADDED`], which also stops it from starting itself again; programs the
//! app opens for the user (a browser, a file's app) get the environment
//! without them.
//!
//! The web view's rendering settings (see [`crate::graphics`]) are added
//! the same way, in the same new start.

/// Notes which settings this module added, separated by spaces.
const ADDED: &str = "GLYPHKEEL_ADDED_ENV";

/// The settings, with the values the app starts with.
const SETTINGS: [(&str, &str); 2] = [
    // 256 MB, in bytes.
    ("JSC_forceRAMSize", "268435456"),
    ("MALLOC_ARENA_MAX", "2"),
];

/// The settings to add: those of [`SETTINGS`] and `extra` that `is_set`
/// doesn't find, none if this start already is the one with the settings
/// added.
fn missing(
    is_set: impl Fn(&str) -> bool,
    extra: &[(&'static str, &'static str)],
) -> Vec<(&'static str, &'static str)> {
    if is_set(ADDED) {
        return Vec::new();
    }
    SETTINGS
        .iter()
        .chain(extra)
        .copied()
        .filter(|(name, _)| !is_set(name))
        .collect()
}

/// Starts the app again in this process with the missing settings added,
/// the memory settings and `extra`. Called first thing in `main`, before
/// any thread or connection exists; returns only if nothing is missing or
/// the new start failed, and the app then runs without them.
pub(crate) fn restart_with_settings(extra: &[(&'static str, &'static str)]) {
    use std::os::unix::process::CommandExt;

    let missing = missing(|name| std::env::var_os(name).is_some(), extra);
    if missing.is_empty() {
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("glyphkeel-app: starts without its memory settings: {err}");
            return;
        }
    };
    let mut args = std::env::args_os();
    let mut command = std::process::Command::new(exe);
    if let Some(name) = args.next() {
        command.arg0(name);
    }
    let added: Vec<&str> = missing.iter().map(|(name, _)| *name).collect();
    let err = command
        .args(args)
        .envs(missing)
        .env(ADDED, added.join(" "))
        .exec();
    eprintln!("glyphkeel-app: starts without its memory settings: {err}");
}

/// Replaces this process with a new start of the app, without the
/// settings this module added, so that the new start chooses them again
/// (after safe rendering was switched on or off). Called as the app ends,
/// after the workspace has closed; the new start opens the main window.
/// Returns only if the new start failed.
pub(crate) fn start_again() -> std::io::Error {
    use std::os::unix::process::CommandExt;

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => return err,
    };
    let mut command = std::process::Command::new(exe);
    if let Some(name) = std::env::args_os().next() {
        command.arg0(name);
    }
    let note = std::env::var(ADDED).ok();
    for name in added_names(note.as_deref()) {
        command.env_remove(name);
    }
    command.exec()
}

/// Whether `name` is one of the settings this module may add.
fn is_known(name: &str) -> bool {
    SETTINGS.iter().any(|(setting, _)| *setting == name) || crate::graphics::NAMES.contains(&name)
}

/// The names to leave out of the environment of programs the app opens: the
/// settings this module added, and its note.
fn added_names(note: Option<&str>) -> Vec<&str> {
    match note {
        Some(note) => note
            .split(' ')
            .filter(|name| is_known(name))
            .chain([ADDED])
            .collect(),
        None => Vec::new(),
    }
}

/// A launch context for programs the app opens for the user: their
/// environment is the user's, without the settings the app added for itself.
pub(crate) fn launch_context() -> gtk::gio::AppLaunchContext {
    use gtk::prelude::AppLaunchContextExt;

    let context = gtk::gio::AppLaunchContext::new();
    let note = std::env::var(ADDED).ok();
    for name in added_names(note.as_deref()) {
        context.unsetenv(name);
    }
    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_are_added_and_the_users_are_kept() {
        assert_eq!(missing(|_| false, &[]), SETTINGS.to_vec());
        assert_eq!(
            missing(|name| name == "JSC_forceRAMSize", &[]),
            vec![("MALLOC_ARENA_MAX", "2")]
        );
        assert!(missing(|name| name != ADDED, &[]).is_empty());
    }

    #[test]
    fn rendering_settings_are_added_unless_the_user_set_them() {
        let extra = [
            ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ];
        let added = missing(|name| name == "WEBKIT_DISABLE_DMABUF_RENDERER", &extra);
        assert_eq!(added.len(), SETTINGS.len() + 1);
        assert_eq!(
            added.last(),
            Some(&("WEBKIT_DISABLE_COMPOSITING_MODE", "1"))
        );
    }

    #[test]
    fn the_second_start_adds_nothing() {
        assert!(
            missing(
                |name| name == ADDED,
                &[("WEBKIT_DISABLE_COMPOSITING_MODE", "1")]
            )
            .is_empty()
        );
    }

    #[test]
    fn opened_programs_lose_only_what_the_app_added() {
        assert!(added_names(None).is_empty());
        assert_eq!(
            added_names(Some("MALLOC_ARENA_MAX")),
            vec!["MALLOC_ARENA_MAX", ADDED]
        );
        assert_eq!(
            added_names(Some("JSC_forceRAMSize MALLOC_ARENA_MAX PATH")),
            vec!["JSC_forceRAMSize", "MALLOC_ARENA_MAX", ADDED]
        );
        assert_eq!(
            added_names(Some("MALLOC_ARENA_MAX __NV_DISABLE_EXPLICIT_SYNC HOME")),
            vec!["MALLOC_ARENA_MAX", "__NV_DISABLE_EXPLICIT_SYNC", ADDED]
        );
    }
}
