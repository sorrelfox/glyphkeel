// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! What the page needs before its first frame, and the page's signal that it
//! has drawn it.
//!
//! The window starts hidden. Rust hands the page its start data through an
//! initialization script; the page sets its theme from it, renders, and calls
//! `window_ready` from React's first commit. Only then is the window shown, so
//! it never flashes white or in the wrong theme. (`requestAnimationFrame`
//! can't be the signal: it never runs while the window is hidden.)
//!
//! `window_ready` also checks how the call arrived. Tauri sends calls over its
//! `ipc:` protocol, and falls back to `postMessage` without a word when the
//! content security policy blocks that protocol. Over the fallback, larger
//! raw responses never arrive, so start-up reports it loudly instead.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::ipc::Request;
use tauri::{Runtime, State, WebviewWindow};

use crate::title_bar::TitleBar;

/// The data the page reads as `window.__GLYPHKEEL_START__`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartData {
    pub(crate) title_bar: TitleBar,
    /// The user's locale from the environment, as a BCP 47 tag. WebKit takes
    /// its own from `LANG` alone, so the page gets it from here.
    pub(crate) locale: Option<String>,
    /// A task window's task: the page shows that task alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) task: Option<String>,
    /// A wiki window's page, by its ID: the page shows that page alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wiki: Option<String>,
    /// The task to open first, from a `glyphkeel://open/<KEY>` link the app
    /// was started with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) open: Option<String>,
    /// The page's own preferences, kept by the app, since the web view
    /// keeps nothing between starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prefs: Option<serde_json::Map<String, serde_json::Value>>,
    /// The quick-add window: the page shows the quick-add field alone.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) quick_add: bool,
    /// The workspace was made at this start: the main window offers the
    /// first-run set-up.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) first_run: bool,
}

impl StartData {
    /// The script that hands the data to the page before any of its own
    /// scripts run.
    pub(crate) fn script(&self) -> String {
        // Serializing these plain fields can't fail; an empty object would
        // only make the page use its defaults.
        /// The data with the platform, for what the page does differently
        /// there (window buttons, key names): `linux` or `windows`.
        #[derive(Serialize)]
        struct WithPlatform<'a> {
            #[serde(flatten)]
            data: &'a StartData,
            platform: &'static str,
        }
        let json = serde_json::to_string(&WithPlatform {
            data: self,
            platform: std::env::consts::OS,
        })
        .unwrap_or_else(|_| "{}".to_owned());
        format!(
            "Object.defineProperty(window, \"__GLYPHKEEL_START__\", {{ value: Object.freeze({json}) }});"
        )
    }
}

/// The locale for dates and numbers: `LC_ALL`, then `LC_TIME`, then `LANG`,
/// as the C library picks it, turned into a BCP 47 tag.
pub(crate) fn locale_from_env() -> Option<String> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .and_then(|value| posix_to_bcp47(&value))
}

/// `de_DE.UTF-8@euro` becomes `de-DE`; `C` and `POSIX` give no locale.
pub(crate) fn posix_to_bcp47(value: &str) -> Option<String> {
    let base = value.split(['.', '@']).next().unwrap_or_default();
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    let mut parts = base.split('_');
    let language = parts.next().filter(|lang| {
        (2..=3).contains(&lang.len()) && lang.chars().all(|c| c.is_ascii_alphabetic())
    })?;
    let region = parts
        .next()
        .filter(|region| !region.is_empty() && region.chars().all(|c| c.is_ascii_alphanumeric()));
    Some(match region {
        Some(region) => format!(
            "{}-{}",
            language.to_ascii_lowercase(),
            region.to_ascii_uppercase()
        ),
        None => language.to_ascii_lowercase(),
    })
}

/// How the page's calls reach Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Transport {
    /// Tauri's `ipc:` protocol: the expected path.
    Ipc,
    /// The `postMessage` fallback: a misconfiguration.
    PostMessage,
}

impl Transport {
    /// Calls over the `ipc:` protocol are HTTP requests that carry Tauri's
    /// invoke key as a header. Over `postMessage` the key travels in the
    /// message body, and the headers are only those the page added itself.
    pub(crate) fn of(headers: &tauri::http::HeaderMap) -> Self {
        if headers.contains_key("tauri-invoke-key") {
            Self::Ipc
        } else {
            Self::PostMessage
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ready {
    transport: Transport,
}

/// Whether the main window's page has signalled its first commit.
#[derive(Debug, Default)]
pub(crate) struct Shown(pub(crate) AtomicBool);

/// The page has committed its first screen: show the window.
#[tauri::command]
pub(crate) fn window_ready(
    window: WebviewWindow,
    shown: State<'_, Shown>,
    request: Request<'_>,
) -> Ready {
    let transport = Transport::of(request.headers());
    if transport == Transport::PostMessage {
        eprintln!(
            "glyphkeel-app: ERROR: the page's calls fall back to postMessage, because the content \
             security policy blocks Tauri's ipc: protocol. Larger responses would never arrive. \
             Check connect-src in tauri.conf.json."
        );
    }
    if window.label() == crate::main_window::LABEL {
        show(&window, &shown);
    } else if window.label() == crate::quick_add::LABEL {
        // Shown now if it was asked for while its page loaded.
        crate::quick_add::on_ready(&window);
    } else {
        // Task and wiki windows come forward each time they are opened.
        crate::task_window::show(&window);
    }
    Ready { transport }
}

/// The task key in a `glyphkeel://open/<KEY>` link.
pub(crate) fn task_in_link(link: &str) -> Option<String> {
    let key = link
        .strip_prefix("glyphkeel://open/")?
        .trim_end_matches('/');
    crate::task_window::is_task_key(key).then(|| key.to_owned())
}

/// Shows the window once.
pub(crate) fn show<R: Runtime>(window: &WebviewWindow<R>, shown: &Shown) {
    if shown.0.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(err) = window.show() {
        eprintln!("glyphkeel-app: cannot show the main window: {err}");
    }
    if let Err(err) = window.set_focus() {
        eprintln!("glyphkeel-app: cannot focus the main window: {err}");
    }
}

#[cfg(test)]
mod tests {
    use tauri::http::{HeaderMap, HeaderValue};

    use super::{StartData, Transport, posix_to_bcp47, task_in_link};
    use crate::title_bar::TitleBar;

    #[test]
    fn posix_locales_become_bcp47_tags() {
        assert_eq!(posix_to_bcp47("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(posix_to_bcp47("de_DE.UTF-8@euro").as_deref(), Some("de-DE"));
        assert_eq!(posix_to_bcp47("en_US").as_deref(), Some("en-US"));
        assert_eq!(posix_to_bcp47("fi").as_deref(), Some("fi"));
        assert_eq!(posix_to_bcp47("C.UTF-8"), None);
        assert_eq!(posix_to_bcp47("POSIX"), None);
        assert_eq!(posix_to_bcp47(""), None);
        assert_eq!(posix_to_bcp47("not a locale"), None);
    }

    #[test]
    fn the_transport_is_told_by_the_invoke_key_header() {
        let mut headers = HeaderMap::new();
        assert_eq!(Transport::of(&headers), Transport::PostMessage);
        headers.insert("Tauri-Invoke-Key", HeaderValue::from_static("key"));
        assert_eq!(Transport::of(&headers), Transport::Ipc);
    }

    #[test]
    fn the_start_script_defines_a_frozen_global() {
        let script = StartData {
            title_bar: TitleBar::System,
            locale: Some("de-DE".into()),
            task: None,
            wiki: None,
            open: None,
            prefs: None,
            quick_add: false,
            first_run: false,
        }
        .script();
        assert_eq!(
            script,
            format!(
                r#"Object.defineProperty(window, "__GLYPHKEEL_START__", {{ value: Object.freeze({{"titleBar":"system","locale":"de-DE","platform":"{}"}}) }});"#,
                std::env::consts::OS
            )
        );
    }

    #[test]
    fn a_task_window_names_its_task() {
        let script = StartData {
            title_bar: TitleBar::System,
            locale: None,
            task: Some("TM-41".into()),
            wiki: None,
            open: None,
            prefs: None,
            quick_add: false,
            first_run: false,
        }
        .script();
        assert!(script.contains(r#""task":"TM-41""#), "{script}");
    }

    #[test]
    fn the_quick_add_window_says_so() {
        let script = StartData {
            title_bar: TitleBar::System,
            locale: None,
            task: None,
            wiki: None,
            open: None,
            prefs: None,
            quick_add: true,
            first_run: false,
        }
        .script();
        assert!(script.contains(r#""quickAdd":true"#), "{script}");
    }

    #[test]
    fn only_open_links_with_a_task_key_name_a_task() {
        assert_eq!(
            task_in_link("glyphkeel://open/TM-41").as_deref(),
            Some("TM-41")
        );
        assert_eq!(
            task_in_link("glyphkeel://open/TM-41/").as_deref(),
            Some("TM-41")
        );
        assert_eq!(task_in_link("glyphkeel://open/tm-41"), None);
        assert_eq!(task_in_link("glyphkeel://open/TM"), None);
        assert_eq!(task_in_link("glyphkeel://view/TM-41"), None);
        assert_eq!(task_in_link("https://open/TM-41"), None);
    }
}
