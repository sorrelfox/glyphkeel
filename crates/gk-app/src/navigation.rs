// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Where the web view may navigate: only to the app's own pages.

use tauri::Url;

/// The development server's origin, used only by debug builds that load the
/// UI from it instead of from the embedded files.
const DEV_ORIGIN: (&str, &str, u16) = ("http", "127.0.0.1", 5310);

/// Whether the web view may load `url`. Anything else is refused; external
/// links are opened by the app in the user's browser instead.
pub(crate) fn is_app_url(url: &Url) -> bool {
    is_embedded(url) || (cfg!(dev) && is_dev_server(url))
}

/// The embedded UI: `tauri://localhost` on Linux, `http://tauri.localhost`
/// with WebView2 on Windows.
fn is_embedded(url: &Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) => true,
        ("http" | "https", Some("tauri.localhost")) => url.port().is_none(),
        _ => false,
    }
}

fn is_dev_server(url: &Url) -> bool {
    let (scheme, host, port) = DEV_ORIGIN;
    url.scheme() == scheme && url.host_str() == Some(host) && url.port() == Some(port)
}

#[cfg(test)]
mod tests {
    use super::{is_dev_server, is_embedded};
    use tauri::Url;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn embedded_pages_are_allowed() {
        assert!(is_embedded(&url("tauri://localhost/index.html")));
        assert!(is_embedded(&url("http://tauri.localhost/")));
    }

    #[test]
    fn other_pages_are_refused() {
        for text in [
            "https://example.com/",
            "http://localhost:5310/",
            "http://tauri.localhost:8080/",
            "tauri://example.com/",
            "file:///etc/passwd",
            "data:text/html,<p>x</p>",
            "javascript:alert(1)",
        ] {
            assert!(!is_embedded(&url(text)), "{text}");
        }
    }

    #[test]
    fn only_the_exact_dev_origin_counts_as_the_dev_server() {
        assert!(is_dev_server(&url("http://127.0.0.1:5310/src/main.tsx")));
        assert!(!is_dev_server(&url("http://127.0.0.1:5311/")));
        assert!(!is_dev_server(&url("http://localhost:5310/")));
        assert!(!is_dev_server(&url("https://127.0.0.1:5310/")));
    }
}
