// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Key chords: parsing them from the keymap's text form ("Ctrl+Shift+Z"),
 * matching them against key events, and telling where a key press happened.
 */

/** A key combination. `key` is a `KeyboardEvent.key` value, letters in lower case. */
export interface Chord {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  key: string;
}

/**
 * Where a key press happens. In a text field or editor, single keys type
 * text, so the shortcuts made of a single key don't fire there.
 */
export type KeyContext = "text" | "outside-text";

const MODIFIERS: Readonly<Record<string, keyof Omit<Chord, "key">>> = {
  ctrl: "ctrl",
  alt: "alt",
  shift: "shift",
  super: "meta",
  meta: "meta",
};

/** Keymap names for keys whose `KeyboardEvent.key` is awkward to write. */
const KEY_NAMES: Readonly<Record<string, string>> = { Space: " ", Esc: "Escape" };

/** Parses "Ctrl+Shift+Z", "F9", "?", "Alt+ArrowLeft" or "Ctrl++". */
export function parseChord(text: string): Chord {
  const chord: Chord = { ctrl: false, alt: false, shift: false, meta: false, key: "" };
  // The key itself may be "+", so split off modifiers from the left only.
  let rest = text;
  for (;;) {
    const plus = rest.indexOf("+");
    if (plus <= 0) {
      break;
    }
    const modifier = MODIFIERS[rest.slice(0, plus).toLowerCase()];
    if (!modifier) {
      break;
    }
    chord[modifier] = true;
    rest = rest.slice(plus + 1);
  }
  if (rest === "") {
    throw new Error(`no key in the shortcut "${text}"`);
  }
  chord.key = KEY_NAMES[rest] ?? (rest.length === 1 ? rest.toLowerCase() : rest);
  return chord;
}

/** Whether the key is one character that may need Shift to be typed, such as "?" or "+". */
function isSymbol(key: string): boolean {
  return key.length === 1 && key.toLowerCase() === key.toUpperCase();
}

export function matches(chord: Chord, event: Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): boolean {
  const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
  if (key !== chord.key) {
    return false;
  }
  // Shift is part of typing "?" on most layouts; for such symbols it is not
  // compared, so the shortcut works on every layout that has the symbol.
  const shiftMatters = !isSymbol(chord.key);
  return (
    event.ctrlKey === chord.ctrl &&
    event.altKey === chord.alt &&
    event.metaKey === chord.meta &&
    (!shiftMatters || event.shiftKey === chord.shift)
  );
}

/** Whether a chord types text when pressed in a text field: a printable key without Ctrl, Alt or Super. */
export function typesText(chord: Chord): boolean {
  return chord.key.length === 1 && !chord.ctrl && !chord.alt && !chord.meta;
}

const TEXT_INPUT_TYPES = new Set(["text", "search", "email", "url", "tel", "password", "number", "date", "time", "datetime-local", "month", "week"]);

/** The context of a key press, from the element that has the focus. */
export function contextOf(target: EventTarget | null): KeyContext {
  if (!(target instanceof Element)) {
    return "outside-text";
  }
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return "text";
  }
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUT_TYPES.has(target.type) ? "text" : "outside-text";
  }
  if (target instanceof HTMLElement && target.isContentEditable) {
    return "text";
  }
  return target.closest('[contenteditable=""], [contenteditable="true"], [role="textbox"], [role="combobox"]') ? "text" : "outside-text";
}

/** The parts of a chord's hint, in order: modifiers, then the key. */
export function chordParts(chord: Chord): string[] {
  const parts: string[] = [];
  if (chord.ctrl) parts.push("ctrl");
  if (chord.alt) parts.push("alt");
  if (chord.shift) parts.push("shift");
  if (chord.meta) parts.push("super");
  parts.push(chord.key);
  return parts;
}
