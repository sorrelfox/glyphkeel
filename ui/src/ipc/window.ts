// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * The window commands the header bar needs. The app's capability file allows
 * exactly these and nothing else of the window API.
 */
export const appWindow = {
  minimize: (): Promise<void> => getCurrentWindow().minimize(),
  toggleMaximize: (): Promise<void> => getCurrentWindow().toggleMaximize(),
  close: (): Promise<void> => getCurrentWindow().close(),
};
