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
        "save_export",
        "export_archive",
        "export_wiki_page",
        "choose_import",
        "choose_wiki_folder",
        "import_wiki_folder",
        "import_archive",
        "choose_csv",
        "import_csv",
        "choose_backup_folder",
        "add_agent_folder",
        "remove_agent_folder",
        "open_link",
        "watch_open_links",
        "open_task_window",
        "open_wiki_window",
        "app_settings",
        "update_app_settings",
        "set_ui_pref",
        "time_zones",
        "set_autostart",
        "restart_app",
        "agent_http",
        "update_agent_http",
        "regenerate_agent_token",
        "show_quick_add",
        "hide_quick_add",
        "watch_quick_add",
        "quick_add_shortcut",
        "set_quick_add_shortcut",
        "cli_status",
        "install_cli",
        "workspaces",
        "create_workspace",
        "open_workspace",
        "rename_workspace",
        "quit_app",
        "legal_text",
    ]);
    // On Windows: UTF-8 code page, long paths and per-monitor DPI, besides the
    // common controls that Tauri's own manifest asks for.
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("packaging/windows/glyphkeel-app.manifest"));
    println!("cargo:rerun-if-changed=packaging/windows/glyphkeel-app.manifest");
    let attributes = tauri_build::Attributes::new()
        .app_manifest(manifest)
        .windows_attributes(windows);
    if let Err(err) = tauri_build::try_build(attributes) {
        panic!("tauri-build: {err:#}");
    }
}
