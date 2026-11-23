// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The formatting commands: Markdown markers around the selection, or at the
 * start of the selected lines. Each is one change, so Undo takes it back in
 * one step, and it touches only the characters it adds or removes, so the
 * rest of the text stays exactly as it was.
 */

import { EditorSelection, type ChangeSpec, type EditorState, type SelectionRange } from "@codemirror/state";
import { type EditorView, type KeyBinding } from "@codemirror/view";

import { codeMirrorKey, FORMAT_SHORTCUTS, type FormatKind } from "./formats";

/**
 * Puts `before` and `after` around each selection, or takes them away when
 * they are already there (inside or just outside the selection). An empty
 * selection gets an empty pair with the cursor between.
 */
function toggleWrap(state: EditorState, before: string, after: string) {
  return state.changeByRange((range) => {
    const inner = state.sliceDoc(range.from, range.to);
    if (inner.length >= before.length + after.length && inner.startsWith(before) && inner.endsWith(after) && !range.empty) {
      return {
        changes: [
          { from: range.from, to: range.from + before.length },
          { from: range.to - after.length, to: range.to },
        ],
        range: EditorSelection.range(range.from, range.to - before.length - after.length),
      };
    }
    const outside = range.from >= before.length && state.sliceDoc(range.from - before.length, range.from) === before && state.sliceDoc(range.to, range.to + after.length) === after;
    // A single "*" next to "**" is part of bold, not italic.
    const partOfLonger = before === "*" && (state.sliceDoc(range.from - 2, range.from) === "**" || state.sliceDoc(range.to, range.to + 2) === "**");
    if (outside && !partOfLonger) {
      return {
        changes: [
          { from: range.from - before.length, to: range.from },
          { from: range.to, to: range.to + after.length },
        ],
        range: EditorSelection.range(range.from - before.length, range.to - before.length),
      };
    }
    return {
      changes: [
        { from: range.from, insert: before },
        { from: range.to, insert: after },
      ],
      range: range.empty ? EditorSelection.cursor(range.from + before.length) : EditorSelection.range(range.from + before.length, range.to + before.length),
    };
  });
}

/** A link: the selection becomes its text and the cursor goes where the address goes. */
function link(state: EditorState) {
  return state.changeByRange((range) => {
    if (range.empty) {
      return { changes: { from: range.from, insert: "[]()" }, range: EditorSelection.cursor(range.from + 1) };
    }
    const end = range.to + 3;
    return {
      changes: [
        { from: range.from, insert: "[" },
        { from: range.to, insert: "]()" },
      ],
      range: EditorSelection.cursor(end),
    };
  });
}

/** The line numbers a selection covers; a selection ending at a line's start leaves that line out. */
function linesOf(state: EditorState, range: SelectionRange): number[] {
  const first = state.doc.lineAt(range.from).number;
  let last = state.doc.lineAt(range.to).number;
  if (last > first && state.doc.line(last).from === range.to) {
    last--;
  }
  const numbers: number[] = [];
  for (let line = first; line <= last; line++) {
    numbers.push(line);
  }
  return numbers;
}

/** The marker a line starts with, after its indentation, for each kind of line format. */
const LINE_MARKERS = {
  heading: /^#{1,6}[ \t]+/u,
  "bullet-list": /^[-*+][ \t]+(?!\[[ xX]\])/u,
  "numbered-list": /^\d{1,9}[.)][ \t]+/u,
  checklist: /^[-*+][ \t]+\[[ xX]\][ \t]+/u,
  quote: /^>[ \t]?/u,
} as const;

type LineKind = keyof typeof LINE_MARKERS;

/** Every line marker, so that a list item can turn into another kind instead of getting two markers. */
const ANY_LIST_MARKER = /^(?:[-*+][ \t]+\[[ xX]\][ \t]+|[-*+][ \t]+|\d{1,9}[.)][ \t]+)/u;

/**
 * Adds the line marker to each selected line, or takes it away when every
 * selected line (blank lines aside) already has it. A list item of another
 * kind changes its marker.
 */
function toggleLines(state: EditorState, kind: LineKind) {
  const pattern = LINE_MARKERS[kind];
  const changes: ChangeSpec[] = [];
  const seen = new Set<number>();
  const lines = state.selection.ranges.flatMap((range) => linesOf(state, range)).filter((number) => (seen.has(number) ? false : (seen.add(number), true)));
  const content = lines.map((number) => state.doc.line(number)).filter((line) => line.text.trim() !== "");
  const indentOf = (text: string) => /^[ \t]*/u.exec(text)?.[0].length ?? 0;
  const all = content.length > 0 && content.every((line) => pattern.test(line.text.slice(indentOf(line.text))));
  let count = 0;
  for (const line of content) {
    const indent = indentOf(line.text);
    const rest = line.text.slice(indent);
    const from = line.from + indent;
    if (all) {
      const found = pattern.exec(rest);
      if (found) {
        changes.push({ from, to: from + found[0].length });
      }
      continue;
    }
    count++;
    const marker = kind === "heading" ? "## " : kind === "bullet-list" ? "- " : kind === "numbered-list" ? `${String(count)}. ` : kind === "checklist" ? "- [ ] " : "> ";
    const existing = kind === "heading" ? LINE_MARKERS.heading.exec(rest) : kind === "quote" ? null : ANY_LIST_MARKER.exec(rest);
    if (existing && kind !== "heading" && existing[0] === marker) {
      continue;
    }
    changes.push({ from, to: from + (existing?.[0].length ?? 0), insert: marker });
  }
  if (content.length === 0) {
    // An empty line: start it with the marker.
    const marker = kind === "heading" ? "## " : kind === "bullet-list" ? "- " : kind === "numbered-list" ? "1. " : kind === "checklist" ? "- [ ] " : "> ";
    for (const number of lines) {
      changes.push({ from: state.doc.line(number).from, insert: marker });
    }
  }
  return { changes: state.changes(changes) };
}

/**
 * A fenced code block around the selected lines, or an empty one at the
 * cursor. The fences use the text's own line break.
 */
function codeBlock(state: EditorState) {
  const range = state.selection.main;
  const lines = linesOf(state, range);
  const first = state.doc.line(lines[0] ?? 1);
  const last = state.doc.line(lines[lines.length - 1] ?? 1);
  const br = state.lineBreak;
  // Positions count a line break as one, whatever its characters; the selection is in the new text.
  const fence = 4;
  if (range.empty && first.text.trim() === "") {
    return { changes: { from: first.from, to: first.to, insert: `\`\`\`${br}${br}\`\`\`` }, selection: EditorSelection.cursor(first.from + fence) };
  }
  return {
    changes: [
      { from: first.from, insert: `\`\`\`${br}` },
      { from: last.to, insert: `${br}\`\`\`` },
    ],
    selection: EditorSelection.range(first.from + fence, last.to + fence),
  };
}

/** Runs a formatting command on the editor; false when the text can't change. */
export function applyFormat(view: EditorView, kind: FormatKind): boolean {
  const { state } = view;
  if (state.readOnly) {
    return false;
  }
  const spec =
    kind === "bold"
      ? toggleWrap(state, "**", "**")
      : kind === "italic"
        ? toggleWrap(state, "*", "*")
        : kind === "strikethrough"
          ? toggleWrap(state, "~~", "~~")
          : kind === "code"
            ? toggleWrap(state, "`", "`")
            : kind === "link"
              ? link(state)
              : kind === "code-block"
                ? codeBlock(state)
                : toggleLines(state, kind);
  view.dispatch(state.update(spec, { userEvent: "input.format", scrollIntoView: true }));
  return true;
}

/** The formatting shortcuts, as key bindings. */
export const formatKeys: KeyBinding[] = (Object.keys(FORMAT_SHORTCUTS) as FormatKind[]).map((kind) => ({
  key: codeMirrorKey(FORMAT_SHORTCUTS[kind]),
  run: (view) => applyFormat(view, kind),
  preventDefault: true,
}));
