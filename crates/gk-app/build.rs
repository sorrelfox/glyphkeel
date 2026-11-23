// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

fn main() {
    // Declaring the app's commands puts them under the capability files: a
    // command that no capability grants is refused, even when registered.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "window_ready",
        "op",
        "subscribe",
        "spell_check",
        "spell_suggest",
        "attach_files",
        "watch_drops",
        "attach_drop",
        "open_file",
        "save_file",
        "open_link",
        "watch_open_links",
        "open_task_window",
        "app_settings",
        "update_app_settings",
        "set_ui_pref",
        "time_zones",
        "quit_app",
    ]);
    if let Err(err) = tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
    {
        panic!("tauri-build: {err:#}");
    }
}
