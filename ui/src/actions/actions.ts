// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { contextOf, matches, parseChord, typesText, type Chord } from "./keymap";

/** Groups in the shortcut sheet and menus. */
export type ActionGroup = "app" | "navigation" | "tasks" | "window";

/**
 * A command of the app. One registry feeds the keymap, the menus, the
 * shortcut sheet and the tooltips; later also the command palette.
 */
export interface Action {
  id: string;
  /** The message ID of its name. */
  label: string;
  group: ActionGroup;
  /** Its key chords, from the keymap. */
  keys: readonly Chord[];
  /**
   * Whether it also fires while a text field or editor has the focus.
   * Defaults to yes, except for shortcuts that type text there (`N`, `?`).
   */
  inText?: boolean;
  /** Present but not available yet, or not now: shown dimmed, never run. */
  disabled?: boolean;
  run: () => void;
}

/**
 * The default keymap. Every command of the app has its keys here, including
 * the ones whose screens don't exist yet; an action takes its keys from this
 * table by ID.
 */
export const DEFAULT_KEYMAP: Readonly<Record<string, readonly string[]>> = {
  "palette.open": ["Ctrl+K"],
  "task.new": ["Ctrl+N", "N"],
  "view.filter": ["Ctrl+F"],
  "view.board": ["Ctrl+1"],
  "view.list": ["Ctrl+2"],
  "view.gantt": ["Ctrl+3"],
  "view.wiki": ["Ctrl+4"],
  "view.calendar": ["Ctrl+5"],
  "view.previousPeriod": ["Alt+PageUp"],
  "view.nextPeriod": ["Alt+PageDown"],
  "view.today": ["Alt+Home"],
  "sidebar.toggle": ["F9"],
  "app.preferences": ["Ctrl+,"],
  "app.undo": ["Ctrl+Z"],
  "app.redo": ["Ctrl+Shift+Z"],
  "selection.open": ["Enter"],
  "selection.openWindow": ["Ctrl+Enter"],
  "selection.clear": ["Escape"],
  "card.moveLeft": ["Alt+ArrowLeft"],
  "card.moveRight": ["Alt+ArrowRight"],
  "card.moveUp": ["Alt+ArrowUp"],
  "card.moveDown": ["Alt+ArrowDown"],
  "column.moveLeft": ["Alt+Shift+ArrowLeft"],
  "column.moveRight": ["Alt+Shift+ArrowRight"],
  "task.edit": ["E"],
  "selection.rename": ["F2"],
  "task.setStatus": ["S"],
  "task.setPriority": ["P"],
  "task.setLabels": ["L"],
  "task.setDue": ["D"],
  "timer.toggle": ["T"],
  "task.toggleDone": ["Space"],
  "selection.trash": ["Delete"],
  "help.shortcuts": ["?"],
  "window.close": ["Ctrl+W"],
  "app.quit": ["Ctrl+Q"],
  "focus.nextRegion": ["F6"],
  "focus.previousRegion": ["Shift+F6"],
};

/** The default keys of an action, parsed. */
export function keysOf(id: string): Chord[] {
  return (DEFAULT_KEYMAP[id] ?? []).map(parseChord);
}

/** Whether an action fires in the given context for the chord that was pressed. */
function firesIn(action: Action, chord: Chord, inText: boolean): boolean {
  if (!inText) {
    return true;
  }
  return action.inText ?? !typesText(chord);
}

/**
 * Runs the action bound to a key press, if any. Returns whether one matched;
 * the event's default is then prevented. A disabled action still takes its
 * keys, so they don't fall through to the web view.
 */
export function dispatchKey(actions: readonly Action[], event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.isComposing) {
    return false;
  }
  const inText = contextOf(event.target) === "text";
  for (const action of actions) {
    const chord = action.keys.find((candidate) => matches(candidate, event));
    if (!chord || !firesIn(action, chord, inText)) {
      continue;
    }
    event.preventDefault();
    if (!action.disabled) {
      action.run();
    }
    return true;
  }
  return false;
}
