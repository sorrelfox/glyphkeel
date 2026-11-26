// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Closing a task that open tasks are marked as duplicates of: before the
// move, a small dialog asks whether to close them too. The answer goes to
// `task.move` (or a bulk change) as `close_duplicates`, so the task and its
// duplicates close in one change and one Undo opens them all again. Every
// way of closing asks the same question: the board, the task panel, the
// list's status cell, subtask checkboxes and bulk changes.
//
// The dialog lives once, in the app (`DuplicatePromptHost`). Without it, as
// in tests of single screens, nothing is asked and the engine's warning
// names the duplicates instead.

import { useSyncExternalStore } from "react";
import { Button, useLocale } from "react-aria-components";

import { useCall } from "../data/hooks";
import { type TaskRef } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { Dialog } from "./Dialog";

interface Pending {
  /** The tasks being closed that have open duplicates. */
  keys: readonly string[];
  duplicates: readonly TaskRef[];
  answer: (close: boolean) => void;
}

let hosts = 0;
let pending: Pending | null = null;
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

/** A mounted host listens; while one does, closing a task asks. */
function subscribeHost(listener: () => void): () => void {
  hosts += 1;
  listeners.add(listener);
  return () => {
    hosts -= 1;
    listeners.delete(listener);
  };
}

function current(): Pending | null {
  return pending;
}

/**
 * Asks, when a move closes task `key` (`closing`), whether to close its open
 * duplicates too: `true` or `false` for `close_duplicates`, or `undefined`
 * when there is nothing to ask.
 */
export function useAskCloseDuplicates(): (key: string, closing: boolean) => Promise<boolean | undefined> {
  const ask = useAskCloseDuplicatesOf();
  return (key, closing) => (closing ? ask([key]) : Promise.resolve(undefined));
}

/** The most tasks `task.get` reads at once. */
const READ_AT_ONCE = 20;

/**
 * As [`useAskCloseDuplicates`] for several tasks that a change closes (a
 * bulk change): one question for the open duplicates of them all, leaving
 * out those that close in the same change.
 */
export function useAskCloseDuplicatesOf(): (keys: readonly string[]) => Promise<boolean | undefined> {
  const call = useCall();
  return async (keys) => {
    if (hosts === 0 || keys.length === 0) {
      return undefined;
    }
    const closing = new Set(keys);
    const originals: string[] = [];
    const duplicates = new Map<string, TaskRef>();
    for (let start = 0; start < keys.length; start += READ_AT_ONCE) {
      const answer = await call("task.get", { keys: keys.slice(start, start + READ_AT_ONCE), include: ["links"] });
      for (const task of answer.items) {
        const open = (task.links ?? []).filter((link) => link.kind === "duplicated_by" && !link.task.closed && !closing.has(link.task.key));
        if (open.length > 0) {
          originals.push(task.key);
        }
        for (const link of open) {
          duplicates.set(link.task.key, link.task);
        }
      }
    }
    if (duplicates.size === 0) {
      return undefined;
    }
    return new Promise<boolean>((resolve) => {
      pending?.answer(false);
      pending = {
        keys: originals,
        duplicates: [...duplicates.values()],
        answer: (close) => {
          pending = null;
          emit();
          resolve(close);
        },
      };
      emit();
    });
  };
}

/** The dialog, mounted once in the app. */
export function DuplicatePromptHost() {
  const t = useT();
  const { locale } = useLocale();
  const shown = useSyncExternalStore(subscribeHost, current);
  if (shown === null) {
    return null;
  }
  const count = shown.duplicates.length;
  const keys = new Intl.ListFormat(locale, { type: "conjunction" }).format(shown.duplicates.map((task) => task.key));
  return (
    <Dialog
      title={t("duplicates-title", { count })}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          shown.answer(false);
        }
      }}
      footer={
        <>
          <Button
            className="btn"
            onPress={() => {
              shown.answer(false);
            }}
          >
            {t("duplicates-keep", { count })}
          </Button>
          <Button
            className="btn primary"
            autoFocus
            onPress={() => {
              shown.answer(true);
            }}
          >
            {t("duplicates-close", { count })}
          </Button>
        </>
      }
    >
      <p>
        {shown.keys.length === 1
          ? t("duplicates-body", { key: shown.keys[0] ?? "", count, keys })
          : t("duplicates-body-many", { tasks: shown.keys.length, originals: new Intl.ListFormat(locale, { type: "conjunction" }).format(shown.keys), count, keys })}
      </p>
      <ul>
        {shown.duplicates.map((task) => (
          <li key={task.key}>
            <span className="key">{task.key}</span> {task.title}
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
