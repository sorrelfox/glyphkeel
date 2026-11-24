// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Unsaved texts of the editors. While the window is open they are held in
// memory, so closing the panel, opening another task or switching tabs
// doesn't lose them. Each change is also saved in the engine as the
// person's draft, a moment after the typing stops, so a crash or a restart
// doesn't lose them either: the editor offers the engine's draft again.
// Drafts are the person's own: they are not history, Undo doesn't touch
// them, and the engine forgets them after 30 days.

import { useEffect, useSyncExternalStore } from "react";

import { type Call, type Draft as KeptDraft, type DraftKind } from "../generated/ops";
import { useCall, useOp } from "./hooks";

export interface Draft {
  text: string;
  /** The description revision the text was edited from. */
  base?: string | undefined;
  /** The conflict copy this text resolves; kept in memory only. */
  resolving?: string | undefined;
}

/** The editor a draft belongs to: its kind and the ID of the task, comment, project or page it edits. */
export interface DraftTarget {
  kind: DraftKind;
  id: string;
}

/** How long typing must pause before a draft is saved in the engine. */
export const SAVE_AFTER_MS = 600;

/** Drafts of this window, by target; `null` is one given up here, which the engine's copy doesn't bring back. */
const memory = new Map<string, Draft | null>();
/** Saves waiting for a pause in the typing, by target. */
const waiting = new Map<string, { timer: ReturnType<typeof setTimeout>; send: () => void }>();
/** The last call sent for each target: the next one waits for it, so the engine sees them in order. */
const sent = new Map<string, Promise<unknown>>();
const listeners = new Set<() => void>();
let generation = 0;

const keyOf = (target: DraftTarget) => `${target.kind}:${target.id}`;

function changed(): void {
  generation += 1;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Sends one call about `key` after the calls before it. A draft that can't be kept is no reason to interrupt. */
function inOrder(key: string, run: () => Promise<unknown>): void {
  const next = (sent.get(key) ?? Promise.resolve()).then(run).catch(() => undefined);
  sent.set(key, next);
  void next.then(() => {
    if (sent.get(key) === next) {
      sent.delete(key);
    }
  });
}

/** Sends the saves still waiting for a pause, now: when an editor closes or the page goes away. */
export function flushDrafts(): void {
  for (const [key, entry] of [...waiting]) {
    clearTimeout(entry.timer);
    waiting.delete(key);
    entry.send();
  }
}

if (typeof window !== "undefined") {
  window.addEventListener("pagehide", flushDrafts);
}

/**
 * The draft of `target`: this window's, or else the one the engine kept
 * from earlier (`kept`, the person's drafts as `draft.list` answers them).
 */
export function readDraft(target: DraftTarget, kept: readonly KeptDraft[] | undefined): Draft | undefined {
  const key = keyOf(target);
  if (memory.has(key)) {
    return memory.get(key) ?? undefined;
  }
  const found = kept?.find((draft) => draft.kind === target.kind && draft.target === target.id);
  return found === undefined ? undefined : { text: found.body, base: found.base_revision };
}

/** {@link readDraft} at a version of this window's drafts, which only says when to read again. */
function readDraftAt(_version: number, target: DraftTarget, kept: readonly KeptDraft[] | undefined): Draft | undefined {
  return readDraft(target, kept);
}

/** Keeps a draft, or gives it up with `null`, here and in the engine. */
export function writeDraft(call: Call, target: DraftTarget, draft: Draft | null): void {
  const key = keyOf(target);
  memory.set(key, draft);
  changed();
  const before = waiting.get(key);
  if (before) {
    clearTimeout(before.timer);
    waiting.delete(key);
  }
  if (draft === null || draft.text === "") {
    inOrder(key, () => call("draft.discard", { kind: target.kind, target: target.id }));
    return;
  }
  const send = () => {
    inOrder(key, () => call("draft.save", { kind: target.kind, target: target.id, body: draft.text, base_revision: draft.base ?? null }));
  };
  waiting.set(key, {
    timer: setTimeout(() => {
      waiting.delete(key);
      send();
    }, SAVE_AFTER_MS),
    send,
  });
}

/** Forgets this window's drafts, as a restart does; the engine's stay. For tests. */
export function forgetDraftsInMemory(): void {
  for (const entry of waiting.values()) {
    clearTimeout(entry.timer);
  }
  waiting.clear();
  memory.clear();
  changed();
}

/** The drafts the engine kept about a task: its description, a new comment and edits of its comments. */
export function useTaskDrafts(taskKey: string): readonly KeptDraft[] | undefined {
  return useOp("draft.list", { task: taskKey }).data?.items;
}

/**
 * The draft of `target` and how to change it. Waiting saves are sent when
 * the editor goes away, so closing the panel right after typing keeps the
 * text.
 */
export function useDraft(target: DraftTarget, kept: readonly KeptDraft[] | undefined): [Draft | undefined, (draft: Draft | null) => void] {
  const call = useCall();
  // The version of this window's drafts is an input of the reading, so that it is read again after each change.
  const version = useSyncExternalStore(subscribe, () => generation);
  useEffect(() => flushDrafts, []);
  return [readDraftAt(version, target, kept), (draft) => { writeDraft(call, target, draft); }];
}
