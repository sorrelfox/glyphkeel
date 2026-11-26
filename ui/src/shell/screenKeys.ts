// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The keys a screen handles itself, inside its tree, chart or grid, rather
 * than through the window's actions: the shortcut sheet lists them while
 * that screen is on screen. Keep them in step with the screens' handlers.
 */

/** The screens with keys of their own. */
export type KeyedScreen = "wiki" | "gantt" | "calendar";

export interface ScreenKey {
  /** The message ID of what the keys do. */
  label: string;
  /** The key chords, in the keymap's text form. */
  keys: readonly string[];
}

export const SCREEN_KEYS: Readonly<Record<KeyedScreen, readonly ScreenKey[]>> = {
  wiki: [
    { label: "screen-key-wiki-edit", keys: ["E"] },
    { label: "screen-key-wiki-move-sibling", keys: ["Alt+ArrowUp", "Alt+ArrowDown"] },
    { label: "screen-key-wiki-move-in", keys: ["Alt+ArrowRight"] },
    { label: "screen-key-wiki-move-out", keys: ["Alt+ArrowLeft"] },
  ],
  gantt: [
    { label: "screen-key-gantt-move", keys: ["Alt+ArrowLeft", "Alt+ArrowRight"] },
    { label: "screen-key-gantt-resize", keys: ["Alt+Shift+ArrowLeft", "Alt+Shift+ArrowRight"] },
    { label: "screen-key-gantt-edit-start", keys: ["F2"] },
    { label: "screen-key-gantt-edit-due", keys: ["Shift+F2"] },
    { label: "screen-key-gantt-link", keys: ["L"] },
    { label: "screen-key-gantt-zoom-in", keys: ["+"] },
    { label: "screen-key-gantt-zoom-out", keys: ["-"] },
  ],
  calendar: [
    { label: "screen-key-calendar-open-day", keys: ["Enter"] },
    { label: "screen-key-calendar-move-day", keys: ["Alt+ArrowLeft", "Alt+ArrowRight"] },
    { label: "screen-key-calendar-move-week", keys: ["Alt+ArrowUp", "Alt+ArrowDown"] },
  ],
};

/** The screen with keys of its own at a route's path, if any. */
export function screenOf(pathname: string): KeyedScreen | null {
  if (/^\/projects\/[^/]+\/wiki(\/|$)/.test(pathname) || /^\/wiki\//.test(pathname)) {
    return "wiki";
  }
  if (/^\/projects\/[^/]+\/gantt\/?$/.test(pathname)) {
    return "gantt";
  }
  if (/^\/projects\/[^/]+\/calendar\/?$/.test(pathname)) {
    return "calendar";
  }
  return null;
}
