// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Word-level differences inside a changed line: which words of the old line
 * went and which words of the new line came, so a history diff can mark them
 * within the red and green lines.
 */

/** A piece of a line, marked when it changed. */
export interface WordPart {
  text: string;
  changed: boolean;
}

/** The most tokens compared on one side; longer lines are shown as whole changed lines. */
const MAX_TOKENS = 600;

let segmenter: Intl.Segmenter | null | undefined;

/** The line's words, spaces and punctuation, in order, by the platform's word rules (which know scripts without spaces). */
export function tokens(line: string): string[] {
  if (segmenter === undefined) {
    // Word breaks follow Unicode's rules, which don't depend on the
    // interface's language; the text compared may be in any language.
    // eslint-disable-next-line no-restricted-syntax -- no locale to pass
    segmenter = typeof Intl.Segmenter === "function" ? new Intl.Segmenter(undefined, { granularity: "word" }) : null;
  }
  if (segmenter !== null) {
    return Array.from(segmenter.segment(line), (part) => part.segment);
  }
  return line.match(/[\p{L}\p{N}_]+|\s+|[^\p{L}\p{N}_\s]/gu) ?? [];
}

/** Joins neighbouring parts with the same mark. */
function merge(parts: WordPart[]): WordPart[] {
  const out: WordPart[] = [];
  for (const part of parts) {
    const last = out.at(-1);
    if (last?.changed === part.changed) {
      last.text += part.text;
    } else {
      out.push({ ...part });
    }
  }
  return out;
}

/**
 * The old and the new line in parts, the words that differ marked; `null`
 * when the lines share no word (the whole line changed) or are too long to
 * compare.
 */
export function wordDiff(before: string, after: string): { removed: WordPart[]; added: WordPart[] } | null {
  const a = tokens(before);
  const b = tokens(after);
  if (a.length > MAX_TOKENS || b.length > MAX_TOKENS) {
    return null;
  }
  // The longest common subsequence of tokens, by dynamic programming over the suffixes.
  const width = b.length + 1;
  const lengths = new Uint16Array((a.length + 1) * width);
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      lengths[i * width + j] = a[i] === b[j] ? (lengths[(i + 1) * width + j + 1] ?? 0) + 1 : Math.max(lengths[(i + 1) * width + j] ?? 0, lengths[i * width + j + 1] ?? 0);
    }
  }
  const removed: WordPart[] = [];
  const added: WordPart[] = [];
  let shared = false;
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    const left = a[i];
    const right = b[j];
    if (left !== undefined && left === right) {
      removed.push({ text: left, changed: false });
      added.push({ text: left, changed: false });
      shared ||= /[\p{L}\p{N}]/u.test(left);
      i += 1;
      j += 1;
    } else if (right === undefined || (left !== undefined && (lengths[(i + 1) * width + j] ?? 0) >= (lengths[i * width + j + 1] ?? 0))) {
      removed.push({ text: left ?? "", changed: true });
      i += 1;
    } else {
      added.push({ text: right, changed: true });
      j += 1;
    }
  }
  if (!shared) {
    return null;
  }
  // A space between two changed words reads as part of the change.
  const bridge = (parts: WordPart[]) =>
    parts.map((part, index) => (!part.changed && part.text.trim() === "" && parts[index - 1]?.changed === true && parts[index + 1]?.changed === true ? { ...part, changed: true } : part));
  return { removed: merge(bridge(removed)), added: merge(bridge(added)) };
}
