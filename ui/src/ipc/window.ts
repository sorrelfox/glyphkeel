// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Runs a window command in the app; in a plain browser there is no window to command. */
function inApp(command: () => Promise<void>): Promise<void> {
  return isTauri() ? command() : Promise.resolve();
}

/**
 * The window commands the header bar needs, and quitting the app. The app's
 * capability file allows exactly these and nothing else of the window API.
 */
export const appWindow = {
  minimize: (): Promise<void> => inApp(() => getCurrentWindow().minimize()),
  toggleMaximize: (): Promise<void> => inApp(() => getCurrentWindow().toggleMaximize()),
  close: (): Promise<void> => inApp(() => getCurrentWindow().close()),
  /** Ends the app, also when closing its windows would keep it running in the background. */
  quit: (): Promise<void> => (isTauri() ? invoke("quit_app") : Promise.resolve()),
  setTitle: (title: string): Promise<void> => {
    document.title = title;
    return inApp(() => getCurrentWindow().setTitle(title));
  },
};
