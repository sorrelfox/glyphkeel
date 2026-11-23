// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

/**
 * Opens a task in its own window, or brings its window forward. A plain
 * browser has one page only: there, `inPage` shows the task window's screen
 * instead.
 */
export async function openTaskWindow(key: string, inPage: () => void): Promise<void> {
  if (!inApp()) {
    inPage();
    return;
  }
  try {
    await invoke("open_task_window", { key });
  } catch (error: unknown) {
    throw new OpFailure("open_task_window", toCallError(error));
  }
}
