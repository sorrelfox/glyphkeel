// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Channel, invoke } from "@tauri-apps/api/core";

import { isTaskKey } from "../start";
import { inApp } from "./files";

const listeners = new Set<(key: string) => void>();
let watching = false;

/**
 * Listens for `glyphkeel://open/<KEY>` links handed to the running app (by
 * another start of it): each brings the key of the task to open. The app
 * keeps links that arrive before the page listens and sends them then. Only
 * the main window hears them; a plain browser never does.
 */
export function onOpenLink(listener: (key: string) => void): () => void {
  listeners.add(listener);
  if (!watching && inApp()) {
    watching = true;
    const channel = new Channel<unknown>((key) => {
      if (!isTaskKey(key)) {
        return;
      }
      for (const each of listeners) {
        each(key);
      }
    });
    invoke("watch_open_links", { channel }).catch((error: unknown) => {
      watching = false;
      console.error("the app did not take the link channel", error);
    });
  }
  return () => {
    listeners.delete(listener);
  };
}
