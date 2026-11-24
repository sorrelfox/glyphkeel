// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Links to comments and quoting them in a reply.
//
// A comment is linked as its task's key, `#c-` and the last eight
// hexadecimal digits of its ID: `TM-41#c-1a2b3c4d`. The engine renders such
// a link like a task key, with the comment named in `data-comment`; opening
// it opens the task and scrolls to the comment.

import { useSyncExternalStore } from "react";

/** What comes between the task's key and the comment's short ID. */
export const COMMENT_ANCHOR = "#c-";

/** The part of a comment's ID that its link names: the last eight hexadecimal digits. */
export function shortCommentId(id: string): string {
  return id.replace(/-/g, "").slice(-8).toLowerCase();
}

/** The link to comment `id` of the task `taskKey`: `TM-41#c-1a2b3c4d`. */
export function commentLink(taskKey: string, id: string): string {
  return `${taskKey}${COMMENT_ANCHOR}${shortCommentId(id)}`;
}

/** Whether comment `id` is the one a link names by `short`. */
export function isComment(id: string, short: string): boolean {
  return shortCommentId(id) === short.toLowerCase();
}

/** The short IDs of the task's comments that `text` links to, in order. */
export function linkedComments(text: string, taskKey: string): string[] {
  const escaped = taskKey.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`(?:^|[^A-Za-z0-9_-])${escaped}#c-([0-9a-f]{8})(?![A-Za-z0-9_-])`, "g");
  return [...text.matchAll(pattern)].map((match) => match[1] ?? "");
}

/** Fluent's invisible marks around placeables, which don't belong in stored text. */
const ISOLATION = /[⁨⁩]/g;

/**
 * A quote of `text` for a reply: every line a block quote, then `source`
 * (the attribution with the link) as its last line, and an empty line to
 * write the answer after.
 */
export function quoteMarkdown(text: string, source: string): string {
  const lines = text.replace(/\r\n?/g, "\n").trim().split("\n");
  const quoted = lines.map((line) => (line.trim() === "" ? ">" : `> ${line}`));
  return [...quoted, ">", `> ${source.replace(ISOLATION, "")}`, "", ""].join("\n");
}

/** Where a quote goes in a text already written: after it, on a paragraph of its own. */
export function appendQuote(before: string, quote: string): string {
  if (before.trim() === "") {
    return quote;
  }
  const gap = before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
  return before + gap + quote;
}

// ---------- opening a linked comment ----------

/** How long a request to show a comment waits for its task to open. */
const FOCUS_WAITS_MS = 10_000;

let focus: { task: string; short: string } | null = null;
let focusGeneration = 0;
const focusListeners = new Set<() => void>();

function focusChanged(): void {
  focusGeneration += 1;
  for (const listener of focusListeners) {
    listener();
  }
}

function subscribeFocus(listener: () => void): () => void {
  focusListeners.add(listener);
  return () => {
    focusListeners.delete(listener);
  };
}

/** Asks the task panel of `taskKey`, open or about to open, to show the comment named by `short`. */
export function requestComment(taskKey: string, short: string): void {
  const request = { task: taskKey.toUpperCase(), short: short.toLowerCase() };
  focus = request;
  focusChanged();
  // A task that doesn't open soon isn't shown at the comment later.
  setTimeout(() => {
    if (focus === request) {
      focus = null;
    }
  }, FOCUS_WAITS_MS);
}

/** The comment of `taskKey` a link asked to show, if any. */
export function useRequestedComment(taskKey: string): string | null {
  const request = useSyncExternalStore(subscribeFocus, () => focus);
  return request !== null && request.task === taskKey.toUpperCase() ? request.short : null;
}

/** How many times a comment was asked to be shown: grows with each request. */
export function useCommentRequests(): number {
  return useSyncExternalStore(subscribeFocus, () => focusGeneration);
}

/** The request has been answered. */
export function commentShown(): void {
  focus = null;
}
