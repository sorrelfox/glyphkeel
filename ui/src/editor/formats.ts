// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The formatting commands of the editor and their shortcuts, shared by the
 * editor's key bindings and the formatting toolbar. Kept free of CodeMirror
 * imports, so that the toolbar can draw before the editor's chunk loads.
 */

/** A formatting command: a toolbar button and a shortcut. */
export type FormatKind =
  | "bold"
  | "italic"
  | "strikethrough"
  | "code"
  | "link"
  | "heading"
  | "bullet-list"
  | "numbered-list"
  | "checklist"
  | "quote"
  | "code-block";

export const FORMAT_KINDS: readonly FormatKind[] = [
  "heading",
  "bold",
  "italic",
  "strikethrough",
  "code",
  "link",
  "bullet-list",
  "numbered-list",
  "checklist",
  "quote",
  "code-block",
];

/**
 * Each command's shortcut in the keymap's text form ("Ctrl+Shift+X"). In the
 * editor, Ctrl stands for Cmd on a Mac.
 */
export const FORMAT_SHORTCUTS: Readonly<Record<FormatKind, string>> = {
  bold: "Ctrl+B",
  italic: "Ctrl+I",
  strikethrough: "Ctrl+Shift+X",
  code: "Ctrl+E",
  link: "Ctrl+K",
  heading: "Ctrl+Shift+H",
  "bullet-list": "Ctrl+Shift+8",
  "numbered-list": "Ctrl+Shift+7",
  checklist: "Ctrl+Shift+9",
  quote: "Ctrl+Shift+.",
  "code-block": "Ctrl+Alt+C",
};

/** A shortcut as CodeMirror names it: "Ctrl+Shift+X" becomes "Mod-Shift-x". */
export function codeMirrorKey(shortcut: string): string {
  return shortcut
    .split("+")
    .map((part) => (part === "Ctrl" ? "Mod" : part.length === 1 ? part.toLowerCase() : part))
    .join("-");
}
