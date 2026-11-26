// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The operating system the app runs on, for the few things the page does
 * differently there: key names, fonts, Preferences rows and spelling. The
 * app says which in its start data; a plain browser counts as Linux.
 */

export type Platform = "linux" | "windows";

const PLATFORMS: readonly Platform[] = ["linux", "windows"];

let current: Platform = "linux";

/** The platform the page was started on. */
export function platform(): Platform {
  return current;
}

/** Whether the value names a platform the page knows. */
export function isPlatform(value: unknown): value is Platform {
  return typeof value === "string" && (PLATFORMS as readonly string[]).includes(value);
}

/**
 * Sets the platform for the page and marks `<html>` with its class
 * (`platform-linux`, `platform-windows`), which the styles read. Called once
 * before the first render; tests call it to switch.
 */
export function applyPlatform(value: Platform, root?: HTMLElement): void {
  current = value;
  if (root) {
    for (const each of PLATFORMS) {
      root.classList.toggle(`platform-${each}`, each === value);
    }
  }
}

/** Whether the web view checks spelling itself (WebView2 on Windows), so the app sends no text for it. */
export function webViewChecksSpelling(): boolean {
  return current === "windows";
}
