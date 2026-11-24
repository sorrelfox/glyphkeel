// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// History and data safety: the workspace's activity feed, the trash, the
// backups, and moving tasks to the trash with an Undo.

import { useInfiniteQuery, useQueryClient, type InfiniteData } from "@tanstack/react-query";
import { useLocale } from "react-aria-components";

import { useToasts } from "../components/Toasts";
import { type ActivityPage, type TrashKind, type TrashList } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { errorToast, useCall } from "./hooks";
import { opPrefix } from "./queries";
import { OpFailure } from "./transport";

/** How many changes the activity feed reads at a time: the engine's most. */
export const FEED_PAGE = 200;

/** How many entries of the trash are read at a time. */
export const TRASH_PAGE = 100;

/** The workspace's activity log, newest first, a page at a time. */
export function useActivityFeed() {
  const call = useCall();
  return useInfiniteQuery<ActivityPage, OpFailure, InfiniteData<ActivityPage>, readonly unknown[], string | undefined>({
    queryKey: [...opPrefix("history.feed"), { feed: FEED_PAGE }],
    queryFn: ({ pageParam }) => call("history.feed", { limit: FEED_PAGE, ...(pageParam === undefined ? {} : { cursor: pageParam }) }),
    initialPageParam: undefined,
    getNextPageParam: (page) => page.next_cursor,
  });
}

/** The trash, newest deletion first, optionally of some kinds or one project. */
export function useTrashList(filter: { kinds: readonly TrashKind[]; project?: string | undefined }) {
  const call = useCall();
  const { kinds, project } = filter;
  return useInfiniteQuery<TrashList, OpFailure, InfiniteData<TrashList>, readonly unknown[], string | undefined>({
    queryKey: [...opPrefix("trash.list"), { kinds, project }],
    queryFn: ({ pageParam }) =>
      call("trash.list", {
        limit: TRASH_PAGE,
        kinds: [...kinds],
        ...(project === undefined ? {} : { project }),
        ...(pageParam === undefined ? {} : { cursor: pageParam }),
      }),
    initialPageParam: undefined,
    getNextPageParam: (page) => page.next_cursor,
  });
}

/** Whole days until `purgeAt`, never below zero: a day that has begun counts. */
export function daysLeft(purgeAt: string, now: Date): number {
  const ms = new Date(purgeAt).getTime() - now.getTime();
  return Math.max(0, Math.ceil(ms / 86_400_000));
}

const UNITS = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte"] as const;

/** A size in bytes as people read it, in the locale: "12 kB", "1.4 GB". */
export function formatBytes(bytes: number, locale: string): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, { style: "unit", unit: UNITS[unit], unitDisplay: "short", maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
}

/** Sizes in the window's locale. */
export function useBytes(): (bytes: number) => string {
  const { locale } = useLocale();
  return (bytes) => formatBytes(bytes, locale);
}

/**
 * Whether a failure means only that the person said no in the native
 * confirmation: a permanent operation comes back `forbidden`, without a
 * message of its own. Saying no needs no error message.
 */
export function declined(error: unknown): boolean {
  return error instanceof OpFailure && error.error.code === "forbidden" && error.error.details?.message_id === undefined;
}

/** Shows an error toast, unless the person only said no to a confirmation. */
export function useFailureToast(): (error: unknown) => void {
  const t = useT();
  const toasts = useToasts();
  return (error) => {
    if (!declined(error)) {
      toasts.show(errorToast(t, error));
    }
  };
}

/**
 * Moves tasks to the trash, each with its subtasks, and offers Undo in a
 * toast. Undo restores exactly what this deletion moved, by its history
 * group, so a later change elsewhere doesn't get in the way.
 */
export function useTrashTasks(): (keys: readonly string[], onDone?: () => void) => void {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  return (keys, onDone) => {
    if (keys.length === 0) {
      return;
    }
    call("task.delete", { keys: [...keys] }).then(
      (change) => {
        onDone?.();
        const group = change.group;
        toasts.show({
          tone: "info",
          text: t("trash-tasks-moved", { count: change.items.length, total: change.total }),
          action:
            group === undefined
              ? undefined
              : {
                  label: t("toast-undo"),
                  run: () => {
                    call("history.revert", { groups: [group] }).then((result) => {
                      const skipped = result.skipped[0];
                      if (skipped) {
                        toasts.show({ tone: "error", text: t("history-not-reverted"), detail: skipped.reason });
                      }
                    }, fail);
                  },
                },
        });
      },
      fail,
    );
  };
}

/** Marks the list of backups as stale, after one was made, deleted or restored. */
export function useRefreshBackups(): () => void {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: opPrefix("backup.list") });
  };
}
