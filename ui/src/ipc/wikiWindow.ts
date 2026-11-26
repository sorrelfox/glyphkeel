// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

/**
 * Opens a wiki page, by its ID, in its own window, or brings its window
 * forward. A plain browser has one page only: there, `inPage` shows the wiki
 * window's screen instead.
 */
export async function openWikiWindow(id: string, inPage: () => void): Promise<void> {
  if (!inApp()) {
    inPage();
    return;
  }
  try {
    await invoke("open_wiki_window", { id });
  } catch (error: unknown) {
    throw new OpFailure("open_wiki_window", toCallError(error));
  }
}
