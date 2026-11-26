// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The web view every window of the app starts from: the embedded page,
//! the start data, and the rules on where the page may go.
//!
//! All windows share one web context. On Linux the app's URL schemes
//! (`tauri:` for the embedded UI, the attachment scheme for images) are
//! registered on the web context, and Tauri registers them only for the
//! first web view of a context. A private ("incognito") web view gets a web
//! context of its own, so every window after the first would have no
//! schemes at all and show WebKit's "The URL can't be shown" instead of the
//! app. The page keeps nothing anyway: web storage and offline caches are
//! switched off (see `webkit`), and the UI keeps its state in Rust.

use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

use crate::navigation;
use crate::start::StartData;

/// The page every window loads.
pub(crate) const PAGE: &str = "index.html";

/// A window with label `label` that loads the app's page with `start`. The
/// caller adds the window's size, title and page-load handling.
pub(crate) fn window<'a>(
    app: &'a AppHandle,
    label: impl Into<String>,
    start: &StartData,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(PAGE.into()))
        .initialization_script(start.script())
        .zoom_hotkeys_enabled(false)
        // The page never leaves the app and never opens windows of its own.
        .on_navigation(navigation::is_app_url)
        .on_new_window(|_url, _features| NewWindowResponse::Deny);
    // The platform's rendering switches and the web view's features off.
    crate::graphics::web_view(app, builder)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tauri::Url;

    use super::PAGE;
    use crate::navigation::is_app_url;

    /// The address the embedded page has in every window on Linux.
    #[test]
    fn every_windows_page_is_an_app_url() {
        let url = Url::parse("tauri://localhost/")
            .unwrap()
            .join(PAGE)
            .unwrap();
        assert!(is_app_url(&url), "{url}");
    }

    /// A window built elsewhere, or a private web view, would lose the app's
    /// URL schemes from the second window on: the page would not load.
    #[test]
    fn windows_are_built_only_here_and_never_private() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut offenders = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                // This file and the IPC tests (mock runtime, no web view)
                // are allowed to name the builder.
                if path.extension().is_none_or(|e| e != "rs")
                    || name == "page.rs"
                    || path.ends_with("ipc/tests.rs")
                {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                let code: String = text
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("//"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if code.contains("WebviewWindowBuilder::new") || code.contains(".incognito(") {
                    offenders.push(path.display().to_string());
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "build windows with page::window, without incognito: {offenders:?}"
        );
    }

    fn capability(text: &str) -> (Vec<String>, Vec<String>) {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        let list = |key: &str| {
            value[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        (list("windows"), list("permissions"))
    }

    /// Each window gets only what its page needs: a task or wiki window,
    /// which shows text that may come from anywhere, can't reach the
    /// commands that concern the whole app.
    #[test]
    fn each_kind_of_window_has_its_own_least_capability() {
        let (main_windows, main) = capability(include_str!("../capabilities/main.json"));
        let (item_windows, item) = capability(include_str!("../capabilities/item-windows.json"));
        let (quick_windows, quick) = capability(include_str!("../capabilities/quick-add.json"));
        assert_eq!(main_windows, ["main"]);
        assert_eq!(item_windows, ["task-*", "wiki-*"]);
        assert_eq!(quick_windows, [crate::quick_add::LABEL]);
        for permission in &item {
            assert!(main.contains(permission), "{permission} is not in main");
        }
        for app_wide in [
            "allow-workspaces",
            "allow-create-workspace",
            "allow-open-workspace",
            "allow-rename-workspace",
            "allow-install-cli",
            "allow-cli-status",
            "allow-set-autostart",
            "allow-restart-app",
            "allow-update-app-settings",
            "allow-set-quick-add-shortcut",
            "allow-choose-backup-folder",
            "allow-choose-import",
            "allow-import-archive",
            "allow-export-archive",
            "allow-choose-csv",
            "allow-import-csv",
            "allow-choose-wiki-folder",
            "allow-import-wiki-folder",
            "allow-watch-open-links",
            "allow-quit-app",
        ] {
            assert!(main.contains(&app_wide.to_owned()), "{app_wide}");
            assert!(!item.contains(&app_wide.to_owned()), "{app_wide}");
            assert!(!quick.contains(&app_wide.to_owned()), "{app_wide}");
        }
        // No capability reaches beyond the app's own commands and the few
        // window calls of the header bar.
        for permission in main.iter().chain(&item).chain(&quick) {
            assert!(
                permission.starts_with("allow-") || permission.starts_with("core:window:allow-"),
                "{permission}"
            );
        }
    }

    /// Every command the app declares is granted to some window, and every
    /// permission names a declared command.
    #[test]
    fn capabilities_name_only_declared_commands() {
        let build = include_str!("../build.rs");
        let list = &build[build.find(".commands(&[").unwrap()..];
        let list = &list[..list.find("]);").unwrap()];
        let declared: Vec<String> = list
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|name| format!("allow-{}", name.replace('_', "-")))
            .collect();
        let mut granted = Vec::new();
        for text in [
            include_str!("../capabilities/main.json"),
            include_str!("../capabilities/item-windows.json"),
            include_str!("../capabilities/quick-add.json"),
        ] {
            granted.extend(capability(text).1);
        }
        for command in &declared {
            assert!(
                granted.contains(command),
                "{command} is granted to no window"
            );
        }
        for permission in granted.iter().filter(|p| !p.starts_with("core:")) {
            assert!(
                declared.contains(permission),
                "{permission} is not a declared command"
            );
        }
    }
}
