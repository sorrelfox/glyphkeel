// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke } from "@tauri-apps/api/core";

import { OpFailure, toCallError } from "../data/transport";
import { inApp } from "./files";

/** The addresses the app hands to the person's browser or mail program. */
export function isExternalLink(url: string): boolean {
  return /^(?:https?:\/\/|mailto:)/i.test(url);
}

/**
 * Opens a web or mail address outside the app: the page itself never
 * navigates away. In a plain browser it opens in a new tab.
 */
export async function openLink(url: string): Promise<void> {
  if (!isExternalLink(url)) {
    return;
  }
  if (!inApp()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  try {
    await invoke("open_link", { url });
  } catch (error: unknown) {
    throw new OpFailure("open_link", toCallError(error));
  }
}
