// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Line breaks and other text rules that keep the editor's text exactly as
 * stored.
 *
 * CodeMirror joins lines with one separator. The editor therefore sets that
 * separator to the text's own line break, reads the text only through
 * `state.sliceDoc()` (never `doc.toString()`, which always joins with "\n"),
 * and converts every text it inserts to that break. A text that mixes several
 * kinds of line break can't be kept exactly, so it is converted to its main
 * kind when it is opened, and the person is told once.
 */

export type LineBreak = "\n" | "\r\n" | "\r";

export interface LineBreakCounts {
  lf: number;
  crlf: number;
  cr: number;
}

const ANY_BREAK = /\r\n?|\n/g;

/** Counts each kind of line break in a text. */
export function countLineBreaks(text: string): LineBreakCounts {
  const counts: LineBreakCounts = { lf: 0, crlf: 0, cr: 0 };
  for (const match of text.matchAll(ANY_BREAK)) {
    if (match[0] === "\n") {
      counts.lf++;
    } else if (match[0] === "\r\n") {
      counts.crlf++;
    } else {
      counts.cr++;
    }
  }
  return counts;
}

/** Whether a text uses more than one kind of line break. */
export function hasMixedLineBreaks(text: string): boolean {
  const { lf, crlf, cr } = countLineBreaks(text);
  return [lf, crlf, cr].filter((n) => n > 0).length > 1;
}

/**
 * The text's main line break: the kind it uses most. A tie goes to the kind
 * that comes first in the text; a text without line breaks uses "\n".
 */
export function mainLineBreak(text: string): LineBreak {
  const counts = countLineBreaks(text);
  const order: LineBreak[] = [];
  for (const match of text.matchAll(ANY_BREAK)) {
    const kind = match[0] as LineBreak;
    if (!order.includes(kind)) {
      order.push(kind);
    }
    if (order.length === 3) {
      break;
    }
  }
  const count = (kind: LineBreak) => (kind === "\n" ? counts.lf : kind === "\r\n" ? counts.crlf : counts.cr);
  let best: LineBreak = "\n";
  let bestCount = 0;
  for (const kind of order) {
    if (count(kind) > bestCount) {
      best = kind;
      bestCount = count(kind);
    }
  }
  return best;
}

/** Converts every line break in a text to the given one. */
export function convertLineBreaks(text: string, lineBreak: string): string {
  return text.replace(ANY_BREAK, lineBreak);
}

/** What opening a text for editing does to its line breaks. */
export interface OpenedText {
  /** The text the editor holds: the original, or its conversion. */
  text: string;
  /** The line break the editor uses for the text. */
  lineBreak: LineBreak;
  /** Set when mixed line breaks were converted to the main kind. */
  converted?: { from: LineBreakCounts; to: LineBreak };
}

/**
 * Prepares a text for editing: picks its line break and, when it mixes
 * several kinds, converts it to the main one.
 */
export function openText(text: string): OpenedText {
  const lineBreak = mainLineBreak(text);
  if (!hasMixedLineBreaks(text)) {
    return { text, lineBreak };
  }
  return { text: convertLineBreaks(text, lineBreak), lineBreak, converted: { from: countLineBreaks(text), to: lineBreak } };
}

/** Whether a UTF-16 position falls between the two halves of a surrogate pair. */
export function splitsSurrogatePair(text: string, pos: number): boolean {
  if (pos <= 0 || pos >= text.length) {
    return false;
  }
  const before = text.charCodeAt(pos - 1);
  const after = text.charCodeAt(pos);
  return before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff;
}

/** One replacement in a text, in UTF-16 positions. */
export interface TextEdit {
  from: number;
  to: number;
  insert: string;
}

/** Why a set of edits was refused, or `null` when it can be applied. */
export type EditProblem = "out-of-range" | "overlapping" | "splits-surrogate-pair" | null;

/**
 * Checks edits that come from outside the editor (an agent's targeted edits,
 * for example) before they are applied: each must lie inside the text, none
 * may overlap, and no position may split a surrogate pair, which would leave
 * a lone surrogate that can't travel as JSON.
 */
export function checkEdits(text: string, edits: readonly TextEdit[]): EditProblem {
  const sorted = [...edits].sort((a, b) => a.from - b.from);
  let end = -1;
  for (const edit of sorted) {
    if (!Number.isInteger(edit.from) || !Number.isInteger(edit.to) || edit.from < 0 || edit.to < edit.from || edit.to > text.length) {
      return "out-of-range";
    }
    if (edit.from < end) {
      return "overlapping";
    }
    end = edit.to;
    if (splitsSurrogatePair(text, edit.from) || splitsSurrogatePair(text, edit.to)) {
      return "splits-surrogate-pair";
    }
  }
  return null;
}
