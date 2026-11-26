// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's Markdown: a small stand-in for the engine's
// renderer, checklist, merge and diff. It covers what the sample data and
// the tests write (paragraphs, headings, lists with checkboxes, quotes,
// callouts, code, math, tables, links, images, task keys and wiki links), and
// escapes everything else. The real engine uses a full CommonMark parser.

import { type ChecklistItem } from "../../generated/ops";

export interface FakeChecklistItem extends ChecklistItem {
  /** Where its "[" is in the text, in UTF-16 code units. */
  offset: number;
}

const FENCE = /^ {0,3}(`{3,}|~{3,})\s*([^`\s]*)/;
const ITEM = /^((?: {0,3}> ?)*)([ \t]*)([-*+]|\d{1,9}[.)])[ \t]+(?:\[([ xX])\](?=[ \t]|$)[ \t]*)?(.*)$/;

/** The text's lines, each with its own line break and its offset. */
function linesOf(text: string): { line: string; start: number; end: string }[] {
  const lines: { line: string; start: number; end: string }[] = [];
  const pattern = /([^\r\n]*)(\r\n|\r|\n|$)/g;
  let found: RegExpExecArray | null;
  while ((found = pattern.exec(text)) !== null) {
    const line = found[1] ?? "";
    const end = found[2] ?? "";
    lines.push({ line, start: found.index, end });
    if (end === "") {
      break;
    }
  }
  return lines;
}

/** Inline Markdown as plain text, as the engine's checklist reports item texts. */
export function plainInline(text: string): string {
  return text
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/g, (_all, page: string, label?: string) => label ?? page)
    .replace(/(\*\*|__|~~|\*|_|`)/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** The checkboxes of a text, numbered from 1 in reading order; none inside code. */
export function checklistOf(text: string): FakeChecklistItem[] {
  const items: FakeChecklistItem[] = [];
  let fence: string | null = null;
  for (const { line, start } of linesOf(text)) {
    const opening = FENCE.exec(line);
    if (fence !== null) {
      if (opening?.[1]?.startsWith(fence)) {
        fence = null;
      }
      continue;
    }
    if (opening) {
      fence = opening[1] ?? "```";
      continue;
    }
    const found = ITEM.exec(line);
    if (!found || found[4] === undefined) {
      continue;
    }
    const indent = (found[2] ?? "").replace(/\t/g, "    ").length;
    items.push({
      number: items.length + 1,
      checked: found[4] !== " ",
      depth: Math.floor(indent / 2),
      text: plainInline(found[5] ?? ""),
      offset: start + line.indexOf("[", (found[1] ?? "").length + (found[2] ?? "").length + (found[3] ?? "").length),
    });
  }
  return items;
}

/** The checklist as the engine reports it. */
export function publicChecklist(text: string): ChecklistItem[] {
  return checklistOf(text).map(({ number, checked, depth, text: itemText }) => ({ number, checked, depth, text: itemText }));
}

/** Ticks or clears one item: exactly one character changes. */
export function setChecked(text: string, item: FakeChecklistItem, checked: boolean): string {
  if (item.checked === checked) {
    return text;
  }
  return `${text.slice(0, item.offset + 1)}${checked ? "x" : " "}${text.slice(item.offset + 2)}`;
}

// ---------------------------------------------------------------- rendering

export const escapeHtml = (text: string) => text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);

// A key may name one of the task's comments: `TM-41#c-1a2b3c4d`.
const TASK_KEY = /(^|[^A-Za-z0-9_-])([A-Z][A-Z0-9]{1,9}-\d+)(?:#c-([0-9a-f]{8}))?(?![A-Za-z0-9_-])/g;

function inline(text: string, knownKey: (key: string) => boolean): string {
  const held: string[] = [];
  const hold = (html: string) => {
    held.push(html);
    return `\u0000${String(held.length - 1)}\u0000`;
  };
  let out = text
    .replace(/`([^`]+)`/g, (_all, code: string) => hold(`<code>${escapeHtml(code)}</code>`))
    .replace(/\$([^$\s][^$]*?)\$/g, (_all, tex: string) => hold(`<span data-math="inline">${escapeHtml(tex)}</span>`))
    .replace(/!\[([^\]]*)\]\(([^)\s]*)\)/g, (_all, alt: string, url: string) => {
      if (url.startsWith("gk-file://")) {
        return hold(`<img src="${escapeHtml(url)}" alt="${escapeHtml(alt)}">`);
      }
      if (/^https?:\/\//.test(url)) {
        return hold(`<img data-external-src="${escapeHtml(url)}" alt="${escapeHtml(alt)}">`);
      }
      return hold(`<img alt="${escapeHtml(alt)}">`);
    })
    .replace(/\[\[([^\]|]+?)(?:\|([^\]]*))?\]\]/g, (_all, page: string, label?: string) =>
      hold(`<a data-wiki="${escapeHtml(page)}" rel="noopener noreferrer">${escapeHtml(label ?? page)}</a>`),
    )
    .replace(/\[([^\]]+)\]\(([^)\s]*)\)/g, (_all, label: string, url: string) =>
      hold(/^(https?:|mailto:)/.test(url) || /^gk-file:\/\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(url) ? `<a href="${escapeHtml(url)}" rel="noopener noreferrer">${escapeHtml(label)}</a>` : `<a rel="noopener noreferrer">${escapeHtml(label)}</a>`),
    );
  out = escapeHtml(out)
    .replace(TASK_KEY, (all, before: string, key: string, comment: string | undefined) => {
      if (!knownKey(key)) {
        return all;
      }
      return comment === undefined ? `${before}<a data-ref="${key}">${key}</a>` : `${before}<a data-ref="${key}" data-comment="${comment}">${key}#c-${comment}</a>`;
    })
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/~~([^~]+)~~/g, "<del>$1</del>")
    .replace(/(^|[^*])\*([^*]+)\*/g, "$1<em>$2</em>");
  // eslint-disable-next-line no-control-regex
  return out.replace(/\u0000(\d+)\u0000/g, (_all, index: string) => held[Number(index)] ?? "");
}

const CALLOUTS: Record<string, string> = { note: "Note", tip: "Tip", important: "Important", warning: "Warning", caution: "Caution" };

/** A text as the reader's HTML: the same shapes the engine writes, for the subset above. */
export function renderFake(text: string, knownKey: (key: string) => boolean): string {
  const lines = linesOf(text).map((entry) => entry.line);
  const out: string[] = [];
  let paragraph: string[] = [];
  let number = 0;
  const flush = () => {
    if (paragraph.length > 0) {
      out.push(`<p>${paragraph.map((line) => inline(line.trim(), knownKey)).join("\n")}</p>`);
      paragraph = [];
    }
  };
  let index = 0;
  while (index < lines.length) {
    const line = lines[index] ?? "";
    const fence = FENCE.exec(line);
    if (fence) {
      flush();
      const marker = fence[1] ?? "```";
      const lang = (fence[2] ?? "").toLowerCase();
      const body: string[] = [];
      index += 1;
      while (index < lines.length && !(lines[index] ?? "").trimStart().startsWith(marker)) {
        body.push(lines[index] ?? "");
        index += 1;
      }
      index += 1;
      const code = escapeHtml(body.map((row) => `${row}\n`).join(""));
      out.push(lang === "" ? `<pre><code>${code}</code></pre>` : `<pre data-lang="${escapeHtml(lang)}"><code>${code}</code></pre>`);
      continue;
    }
    if (line.trim() === "$$") {
      flush();
      const body: string[] = [];
      index += 1;
      while (index < lines.length && (lines[index] ?? "").trim() !== "$$") {
        body.push(lines[index] ?? "");
        index += 1;
      }
      index += 1;
      out.push(`<p><span data-math="display">${escapeHtml(body.map((row) => `${row}\n`).join(""))}</span></p>`);
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      const level = String((heading[1] ?? "#").length);
      out.push(`<h${level}>${inline(heading[2] ?? "", knownKey)}</h${level}>`);
      index += 1;
      continue;
    }
    if (/^\s*>/.test(line) && !ITEM.exec(line)?.[4]) {
      flush();
      const body: string[] = [];
      while (index < lines.length && /^\s*>/.test(lines[index] ?? "")) {
        body.push((lines[index] ?? "").replace(/^\s*> ?/, ""));
        index += 1;
      }
      const callout = /^\[!(\w+)\]\s*(.*)$/.exec(body[0] ?? "");
      const kind = callout?.[1]?.toLowerCase();
      if (kind !== undefined && kind in CALLOUTS) {
        const title = callout?.[2] === "" || callout?.[2] === undefined ? (CALLOUTS[kind] ?? kind) : callout[2];
        out.push(`<div class="markdown-alert markdown-alert-${kind}">\n<p class="markdown-alert-title">${escapeHtml(title)}</p>\n${renderFake(body.slice(1).join("\n"), knownKey)}</div>`);
      } else {
        out.push(`<blockquote>\n${renderFake(body.join("\n"), knownKey)}</blockquote>`);
      }
      continue;
    }
    if (/^\s*\|/.test(line) && /^\s*\|?\s*:?-{3,}/.test(lines[index + 1] ?? "")) {
      flush();
      const cells = (row: string) => row.trim().replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim());
      const head = cells(line);
      index += 2;
      const rows: string[][] = [];
      while (index < lines.length && /^\s*\|/.test(lines[index] ?? "")) {
        rows.push(cells(lines[index] ?? ""));
        index += 1;
      }
      out.push(
        `<table>\n<thead>\n<tr>\n${head.map((cell) => `<th>${inline(cell, knownKey)}</th>`).join("\n")}\n</tr>\n</thead>\n<tbody>\n${rows
          .map((row) => `<tr>\n${row.map((cell) => `<td>${inline(cell, knownKey)}</td>`).join("\n")}\n</tr>`)
          .join("\n")}\n</tbody>\n</table>`,
      );
      continue;
    }
    const item = ITEM.exec(line);
    if (item && (item[1] ?? "") === "") {
      flush();
      const ordered = /\d/.test(item[3] ?? "");
      const rows: string[] = [];
      while (index < lines.length) {
        const next = ITEM.exec(lines[index] ?? "");
        if (!next || (next[1] ?? "") !== "") {
          break;
        }
        if (next[4] === undefined) {
          rows.push(`<li>${inline(next[5] ?? "", knownKey)}</li>`);
        } else {
          number += 1;
          rows.push(`<li class="task-item"><input type="checkbox" data-item="${String(number)}"${next[4] === " " ? "" : ' checked=""'} /> ${inline(next[5] ?? "", knownKey)}</li>`);
        }
        index += 1;
      }
      out.push(`<${ordered ? "ol" : "ul"}>\n${rows.join("\n")}\n</${ordered ? "ol" : "ul"}>`);
      continue;
    }
    if (line.trim() === "") {
      flush();
    } else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flush();
      out.push("<hr />");
    } else {
      paragraph.push(line);
    }
    index += 1;
  }
  flush();
  return out.join("\n") + (out.length > 0 ? "\n" : "");
}

// ---------------------------------------------------------------- merge and diff

/** The lines of a text, each with its own line break. */
export function splitLines(text: string): string[] {
  return text.match(/[^\r\n]*(?:\r\n|\r|\n)|[^\r\n]+$/g) ?? [];
}

/**
 * Merges two edits of one text, line by line: a line changed on one side
 * only takes that side's change. Anything else (both sides changed the same
 * line, or lines were added or removed while the other side changed too) is
 * a conflict, which the engine would sometimes still merge.
 */
export function merge3(base: string, ours: string, theirs: string): string | null {
  if (ours === theirs || theirs === base) {
    return ours;
  }
  if (ours === base) {
    return theirs;
  }
  const [b, o, t] = [splitLines(base), splitLines(ours), splitLines(theirs)];
  if (b.length !== o.length || b.length !== t.length) {
    return null;
  }
  const merged: string[] = [];
  for (let index = 0; index < b.length; index += 1) {
    const [lineB, lineO, lineT] = [b[index], o[index], t[index]];
    if (lineO === lineB) {
      merged.push(lineT ?? "");
    } else if (lineT === lineB || lineT === lineO) {
      merged.push(lineO ?? "");
    } else {
      return null;
    }
  }
  return merged.join("");
}

type DiffOp = { kind: " " | "-" | "+"; line: string };

function diffLines(a: string[], b: string[]): DiffOp[] {
  const rows = a.length + 1;
  const columns = b.length + 1;
  const lcs = new Uint32Array(rows * columns);
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      lcs[i * columns + j] = a[i] === b[j] ? (lcs[(i + 1) * columns + j + 1] ?? 0) + 1 : Math.max(lcs[(i + 1) * columns + j] ?? 0, lcs[i * columns + j + 1] ?? 0);
    }
  }
  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      ops.push({ kind: " ", line: a[i] ?? "" });
      i += 1;
      j += 1;
    } else if (i < a.length && (j === b.length || (lcs[(i + 1) * columns + j] ?? 0) >= (lcs[i * columns + j + 1] ?? 0))) {
      ops.push({ kind: "-", line: a[i] ?? "" });
      i += 1;
    } else {
      ops.push({ kind: "+", line: b[j] ?? "" });
      j += 1;
    }
  }
  return ops;
}

/** A unified diff with three lines of context, as the engine writes it. */
export function unifiedDiff(before: string, after: string, beforeName: string, afterName: string): string {
  if (before === after) {
    return "";
  }
  const ops = diffLines(splitLines(before), splitLines(after));
  const out = [`--- ${beforeName}\n`, `+++ ${afterName}\n`];
  const changed = ops.map((op, index) => (op.kind === " " ? -1 : index)).filter((index) => index >= 0);
  let index = 0;
  while (index < changed.length) {
    const first = changed[index] ?? 0;
    let last = first;
    while (index + 1 < changed.length && (changed[index + 1] ?? 0) - last <= 7) {
      index += 1;
      last = changed[index] ?? last;
    }
    index += 1;
    const from = Math.max(0, first - 3);
    const to = Math.min(ops.length, last + 4);
    const oldStart = ops.slice(0, from).filter((op) => op.kind !== "+").length;
    const newStart = ops.slice(0, from).filter((op) => op.kind !== "-").length;
    const hunk = ops.slice(from, to);
    const oldCount = hunk.filter((op) => op.kind !== "+").length;
    const newCount = hunk.filter((op) => op.kind !== "-").length;
    out.push(`@@ -${String(oldStart + 1)},${String(oldCount)} +${String(newStart + 1)},${String(newCount)} @@\n`);
    for (const op of hunk) {
      out.push(`${op.kind}${op.line.replace(/\r?\n$|\r$/, "")}\n`);
    }
  }
  return out.join("");
}
