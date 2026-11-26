// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Task windows: one task in a window of its own, beside the main window.
//!
//! Each window shows the same page with the task named in its start data,
//! and shares the engine, the change events and the undo stack with every
//! other window. Opening a task that has a window already brings that window
//! forward.

use std::time::Duration;

use gk_api::OpError;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use crate::page;
use crate::start::{self, StartData};
use crate::title_bar::TitleBar;

/// The start of every task window's label. The capability file grants the
/// same commands to `task-*` as to the main window.
pub(crate) const PREFIX: &str = "task-";

/// The size a task window opens with.
const SIZE: (f64, f64) = (720.0, 860.0);
const MIN_SIZE: (f64, f64) = (420.0, 480.0);
/// How long the page may take to draw before the window is shown anyway.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether `key` is a task key: a project key (a capital letter, then 1 to
/// 9 capitals or digits), a hyphen and a number.
pub(crate) fn is_task_key(key: &str) -> bool {
    let Some((project, number)) = key.rsplit_once('-') else {
        return false;
    };
    let mut chars = project.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && (2..=10).contains(&project.len())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !number.is_empty()
        && number.len() <= 12
        && number.chars().all(|c| c.is_ascii_digit())
}

/// The label of the window of task `key`.
fn label(key: &str) -> String {
    format!("{PREFIX}{key}")
}

/// Opens task `key` in its own window, or brings its window forward.
#[tauri::command]
pub(crate) fn open_task_window(app: AppHandle, key: String) -> Result<(), OpError> {
    if !is_task_key(&key) {
        return Err(OpError::invalid_at(
            "key",
            format!("{key} isn't a task key."),
        ));
    }
    if let Some(window) = app.get_webview_window(&label(&key)) {
        show(&window);
        return Ok(());
    }
    open(&app, &key)
        .map(drop)
        .map_err(|e| OpError::storage(format!("The task window can't be opened: {e}.")))
}

fn open(app: &AppHandle, key: &str) -> tauri::Result<WebviewWindow> {
    let start = StartData {
        // Task windows keep the system's title bar: they are small, and the
        // window manager's controls are what people expect there.
        title_bar: TitleBar::System,
        locale: start::locale_from_env(),
        task: Some(key.to_owned()),
        wiki: None,
        open: None,
        prefs: app
            .try_state::<crate::app_settings::AppSettings>()
            .map(|settings| settings.ui()),
        quick_add: false,
        first_run: false,
    };
    build(app, label(key), &start, key, SIZE)
}

/// Makes a window of its own for one item (a task, a wiki page), hidden
/// until its page has drawn.
pub(crate) fn build(
    app: &AppHandle,
    label: String,
    start: &StartData,
    title: &str,
    size: (f64, f64),
) -> tauri::Result<WebviewWindow> {
    let window = page::window(app, label, start)
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .decorations(true)
        .visible(false)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                show_if_page_stays_silent(&window);
            }
        })
        .build()?;
    #[cfg(target_os = "linux")]
    window.with_webview(|webview| crate::webkit::restrict(&webview.inner()))?;
    Ok(window)
}

/// Shows a task or wiki window and gives it the focus.
pub(crate) fn show<R: Runtime>(window: &WebviewWindow<R>) {
    for result in [window.unminimize(), window.show(), window.set_focus()] {
        if let Err(err) = result {
            eprintln!("glyphkeel-app: cannot bring a task window forward: {err}");
        }
    }
}

fn show_if_page_stays_silent(window: &WebviewWindow) {
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_TIMEOUT);
        if window.is_visible().unwrap_or(true) {
            return;
        }
        eprintln!("glyphkeel-app: a task window's page did not signal; showing it anyway");
        let target = window.clone();
        if let Err(err) = window.run_on_main_thread(move || show(&target)) {
            eprintln!("glyphkeel-app: cannot show a task window: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{is_task_key, label};

    #[test]
    fn task_keys_are_recognised() {
        for key in ["TM-41", "CLI2-1", "ABCDEFGHIJ-999"] {
            assert!(is_task_key(key), "{key}");
        }
        for key in [
            "",
            "TM",
            "TM-",
            "tm-41",
            "T-1",
            "1TM-4",
            "TM-4a",
            "TM_41",
            "../TM-1",
            "ABCDEFGHIJK-1",
        ] {
            assert!(!is_task_key(key), "{key}");
        }
    }

    #[test]
    fn a_task_windows_label_is_covered_by_the_capability() {
        let label = label("TM-41");
        assert_eq!(label, "task-TM-41");
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        assert!(
            capability["windows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w == "task-*")
        );
    }
}
