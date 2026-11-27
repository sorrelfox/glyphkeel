// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Links in rendered text that lead outside the app: web and mail
//! addresses. The web view never navigates to them; the desktop's browser or
//! mail program opens them.

use gk_api::OpError;
use tauri::{AppHandle, Url};

/// The longest address the page may hand over.
const MAX_LINK: usize = 8 * 1024;

/// The address, if it is a web or mail address the desktop may open.
pub(crate) fn external(url: &str) -> Result<Url, OpError> {
    if url.len() > MAX_LINK {
        return Err(crate::messages::error("link-too-long", &[], |m| {
            OpError::limit(m, MAX_LINK as u64)
        }));
    }
    let parsed = Url::parse(url).map_err(|e| {
        crate::messages::error(
            "link-not-an-address",
            &[("error", e.to_string())],
            OpError::invalid,
        )
    })?;
    let usable = match parsed.scheme() {
        "http" | "https" => parsed.host_str().is_some_and(|host| !host.is_empty()),
        "mailto" => true,
        _ => false,
    };
    if !usable || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(crate::messages::error(
            "link-not-external",
            &[],
            OpError::forbidden,
        ));
    }
    Ok(parsed)
}

/// Opens a web or mail address in the desktop's browser or mail program.
#[tauri::command]
pub(crate) async fn open_link(app: AppHandle, url: String) -> Result<(), OpError> {
    let url = external(&url)?;
    system::open(&app, url.as_str()).await
}

#[cfg(target_os = "linux")]
mod system {
    use gk_api::OpError;
    use gtk::gio;
    use tauri::AppHandle;

    /// Hands the address to the desktop (through the portal inside a sandbox).
    pub(crate) async fn open(app: &AppHandle, uri: &str) -> Result<(), OpError> {
        let uri = uri.to_owned();
        let (answer, answered) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let result = gio::AppInfo::launch_default_for_uri(
                &uri,
                Some(&crate::memory_env::launch_context()),
            )
            .map_err(|e| e.to_string());
            let _ = answer.send(result);
        })
        .map_err(|e| {
            crate::messages::error(
                "link-open-failed",
                &[("error", e.to_string())],
                OpError::storage,
            )
        })?;
        tauri::async_runtime::spawn_blocking(move || {
            answered
                .recv()
                .map_err(|_| crate::messages::error("app-quitting", &[], OpError::storage))?
                .map_err(|e| {
                    crate::messages::error("link-no-program", &[("error", e)], OpError::storage)
                })
        })
        .await
        .map_err(|e| {
            crate::messages::error(
                "command-stopped",
                &[("error", e.to_string())],
                OpError::storage,
            )
        })?
    }
}

#[cfg(windows)]
mod system {
    use gk_api::OpError;
    use tauri::AppHandle;

    /// Hands the address to the default browser or mail program.
    pub(crate) async fn open(_app: &AppHandle, uri: &str) -> Result<(), OpError> {
        let uri = uri.to_owned();
        crate::files::blocking(move || {
            crate::platform::windows::open_url(&uri).map_err(|e| {
                crate::messages::error(
                    "link-no-program",
                    &[("error", e.to_string())],
                    OpError::storage,
                )
            })
        })
        .await
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod system {
    use gk_api::OpError;
    use tauri::AppHandle;

    pub(crate) async fn open(_app: &AppHandle, _uri: &str) -> Result<(), OpError> {
        Err(crate::messages::error(
            "link-unsupported",
            &[],
            OpError::forbidden,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::external;

    #[test]
    fn web_and_mail_addresses_are_opened() {
        for url in [
            "https://example.com/a?b=c#d",
            "http://www.example.com",
            "mailto:someone@example.com",
        ] {
            assert!(external(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn everything_else_is_refused() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<p>x</p>",
            "gk-file://localhost/abc",
            "tauri://localhost/",
            "glyphkeel://open/TM-1",
            "https://user:secret@example.com/",
            "smb://server/share",
            "not an address",
            "",
        ] {
            assert!(external(url).is_err(), "{url}");
        }
        assert!(external(&format!("https://example.com/{}", "a".repeat(9000))).is_err());
    }
}
