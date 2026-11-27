// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Wiki windows: one wiki page in a window of its own, the way a task opens
//! in a task window.
//!
//! The page is named by its ID, which stays the same through renames and
//! moves. Like task windows, each window shares the engine, the change
//! events and the undo stack with every other window; opening a page that
//! has a window already brings that window forward.

use gk_api::OpError;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::start::{self, StartData};
use crate::task_window;
use crate::title_bar::TitleBar;

/// The start of every wiki window's label. The capability file grants the
/// same commands to `wiki-*` as to the main window.
pub(crate) const PREFIX: &str = "wiki-";

/// The size a wiki window opens with: wide enough for the text and its outline.
const SIZE: (f64, f64) = (980.0, 860.0);

/// Whether `id` is a page ID: a UUID in its usual form (36 characters,
/// hexadecimal digits and hyphens at the usual places).
pub(crate) fn is_page_id(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(at, c)| match at {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The label of the window of page `id`.
fn label(id: &str) -> String {
    format!("{PREFIX}{}", id.to_ascii_lowercase())
}

/// Opens wiki page `id` in its own window, or brings its window forward.
#[tauri::command]
pub(crate) fn open_wiki_window(app: AppHandle, id: String) -> Result<(), OpError> {
    if !is_page_id(&id) {
        return Err(crate::messages::error(
            "wiki-window-bad-id",
            &[("id", id.clone())],
            |m| OpError::invalid_at("id", m),
        ));
    }
    if let Some(window) = app.get_webview_window(&label(&id)) {
        task_window::show(&window);
        return Ok(());
    }
    open(&app, &id).map(drop).map_err(|e| {
        crate::messages::error(
            "wiki-window-failed",
            &[("error", e.to_string())],
            OpError::storage,
        )
    })
}

fn open(app: &AppHandle, id: &str) -> tauri::Result<WebviewWindow> {
    let start = StartData {
        // As task windows: the system's title bar.
        title_bar: TitleBar::System,
        locale: start::locale_from_env(),
        task: None,
        wiki: Some(id.to_ascii_lowercase()),
        open: None,
        prefs: app
            .try_state::<crate::app_settings::AppSettings>()
            .map(|settings| settings.ui()),
        quick_add: false,
        first_run: false,
    };
    // The page sets the title to the page's own once it has read it.
    task_window::build(app, label(id), &start, "Glyphkeel", SIZE)
}

#[cfg(test)]
mod tests {
    use super::{is_page_id, label};
    use crate::start::StartData;
    use crate::title_bar::TitleBar;

    #[test]
    fn page_ids_are_recognised() {
        for id in [
            "01a1ee56-5e01-7406-9f8b-e8c1fe48b90b",
            "01A1EE56-5E01-7406-9F8B-E8C1FE48B90B",
        ] {
            assert!(is_page_id(id), "{id}");
        }
        for id in [
            "",
            "TM-41",
            "01a1ee56-5e01-7406-9f8b-e8c1fe48b90",
            "01a1ee56-5e01-7406-9f8b-e8c1fe48b90bb",
            "01a1ee565e01-7406-9f8b-e8c1fe48b90b-",
            "../1ee56-5e01-7406-9f8b-e8c1fe48b90b",
            "01a1ee56-5e01-7406-9f8b-e8c1fe48b9zz",
        ] {
            assert!(!is_page_id(id), "{id}");
        }
    }

    #[test]
    fn a_wiki_windows_label_is_covered_by_the_capability() {
        let label = label("01A1EE56-5E01-7406-9F8B-E8C1FE48B90B");
        assert_eq!(label, "wiki-01a1ee56-5e01-7406-9f8b-e8c1fe48b90b");
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/item-windows.json")).unwrap();
        let windows = capability["windows"].as_array().unwrap();
        assert!(windows.iter().any(|w| w == "wiki-*"));
        let permissions = capability["permissions"].as_array().unwrap();
        assert!(permissions.iter().any(|p| p == "allow-open-wiki-window"));
    }

    #[test]
    fn a_wiki_window_names_its_page() {
        let script = StartData {
            title_bar: TitleBar::System,
            locale: None,
            task: None,
            wiki: Some("01a1ee56-5e01-7406-9f8b-e8c1fe48b90b".into()),
            open: None,
            prefs: None,
            quick_add: false,
            first_run: false,
        }
        .script();
        assert!(
            script.contains(r#""wiki":"01a1ee56-5e01-7406-9f8b-e8c1fe48b90b""#),
            "{script}"
        );
    }
}
