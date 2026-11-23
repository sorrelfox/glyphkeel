// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke, isTauri } from "@tauri-apps/api/core";

/**
 * How the page's calls reach the app: over Tauri's `ipc:` protocol, over the
 * `postMessage` fallback (a broken set-up: larger responses never arrive), or
 * not at all in a plain browser.
 */
export type Transport = "ipc" | "postMessage" | "none";

interface Ready {
  transport: "ipc" | "postMessage";
}

/**
 * Tells the app that the page has committed its first screen, so the window
 * can be shown, and learns how the call travelled. Called from React's first
 * commit; `requestAnimationFrame` never runs while the window is hidden.
 */
export async function windowReady(): Promise<Transport> {
  if (!isTauri()) {
    return "none";
  }
  const ready = await invoke<Ready>("window_ready");
  return ready.transport;
}
