// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The keys a screen handles itself, inside its tree, chart or grid, rather
 * than through the window's actions: the shortcut sheet lists them while
 * that screen is on screen. Keep them in step with the screens' handlers.
 */

/** The screens with keys of their own. */
export type KeyedScreen = "board" | "list" | "wiki" | "gantt" | "calendar";

export interface ScreenKey {
  /** The message ID of what the keys do. */
  label: string;
  /** The key chords, in the keymap's text form. */
  keys: readonly string[];
}

export const SCREEN_KEYS: Readonly<Record<KeyedScreen, readonly ScreenKey[]>> = {
  board: [
    { label: "screen-key-board-select", keys: ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"] },
    { label: "screen-key-board-open", keys: ["Enter"] },
    { label: "screen-key-board-pick-up", keys: ["Space"] },
    { label: "screen-key-board-trash", keys: ["Delete"] },
    { label: "screen-key-board-menu", keys: ["ContextMenu", "Shift+F10"] },
  ],
  list: [
    { label: "screen-key-list-move", keys: ["ArrowUp", "ArrowDown", "Home", "End"] },
    { label: "screen-key-list-open", keys: ["Enter"] },
    { label: "screen-key-list-select", keys: ["Space"] },
    { label: "screen-key-list-select-all", keys: ["Ctrl+A"] },
    { label: "screen-key-list-clear", keys: ["Escape"] },
    { label: "screen-key-list-rename", keys: ["F2"] },
    { label: "screen-key-list-trash", keys: ["Delete"] },
  ],
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

/** What a key does in an open task, in its panel or its own window. */
export type TaskKeyId = "edit" | "status" | "priority" | "labels" | "due" | "submit" | "window";

/**
 * The keys of an open task, while the focus is in its panel or window. The
 * single letters work outside text fields; `Ctrl+Enter` saves or sends in
 * the description and comment editors, and elsewhere in a docked panel
 * opens the task in its own window. The panel reads its keys from here.
 */
export const TASK_KEYS: readonly (ScreenKey & { id: TaskKeyId })[] = [
  { id: "edit", label: "task-key-edit", keys: ["E"] },
  { id: "status", label: "task-key-status", keys: ["S"] },
  { id: "priority", label: "task-key-priority", keys: ["P"] },
  { id: "labels", label: "task-key-labels", keys: ["L"] },
  { id: "due", label: "task-key-due", keys: ["D"] },
  { id: "submit", label: "task-key-submit", keys: ["Ctrl+Enter"] },
  { id: "window", label: "task-key-window", keys: ["Ctrl+Enter"] },
];

/** The screen with keys of its own at a route's path, if any. */
export function screenOf(pathname: string): KeyedScreen | null {
  if (/^\/projects\/[^/]+\/?$/.test(pathname)) {
    return "board";
  }
  if (/^\/projects\/[^/]+\/list\/?$/.test(pathname) || /^\/all\/?$/.test(pathname)) {
    return "list";
  }
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
