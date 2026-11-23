// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! WebKitGTK features the app never uses, switched off so that a page can't
//! reach the network or the user's devices through them.

use webkit2gtk::{SettingsExt, WebView, WebViewExt};

pub(crate) fn restrict(view: &WebView) {
    let Some(settings) = WebViewExt::settings(view) else {
        return;
    };
    // Nothing is looked up ahead of a click. (Hyperlink auditing needs no
    // setting: current WebKitGTK ignores it, and the page's content security
    // policy blocks its requests anyway.)
    settings.set_enable_dns_prefetching(false);
    // No real-time connections, cameras, microphones or 3D contexts.
    settings.set_enable_webrtc(false);
    settings.set_enable_media_stream(false);
    settings.set_enable_webgl(false);
    // Script can't open windows, and the caret never browses the chrome.
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_enable_caret_browsing(false);
    // No web storage and no offline caches: the UI keeps its state in Rust.
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_offline_web_application_cache(false);
    settings.set_enable_page_cache(false);
    // Pages loaded from files may not read other files.
    settings.set_allow_file_access_from_file_urls(false);
    settings.set_allow_universal_access_from_file_urls(false);
}
