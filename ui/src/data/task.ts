// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The hooks of the task panel: a task with its subtasks, links and files, its
// comments, and its part of the activity log.

import { useInfiniteQuery, type InfiniteData } from "@tanstack/react-query";

import { type ActivityEntry, type ActivityPage, type TaskPart } from "../generated/ops";
import { useCall, useOp } from "./hooks";
import { opPrefix } from "./queries";
import { type OpFailure } from "./transport";

/** What the panel reads with a task. */
export const PANEL_PARTS: readonly TaskPart[] = ["subtasks", "links", "files", "time", "reminder"];

/** One task with its subtasks, links, files, time entries and reminder. */
export function useTaskDetails(key: string) {
  return useOp("task.get", { keys: [key], include: [...PANEL_PARTS] });
}

/** A task's comments, oldest first, each with its earlier texts. */
export function useComments(key: string) {
  return useOp("comment.list", { key, revisions: true });
}

/** How many history entries the panel reads at a time. */
export const HISTORY_PAGE = 100;

/**
 * A task's history, newest first: the activity log filtered by the engine to
 * the task (its fields and description, its comments, files and links), a
 * page at a time. Refreshed with the activity log.
 */
export function useTaskHistory(task: string, enabled: boolean) {
  const call = useCall();
  return useInfiniteQuery<ActivityPage, OpFailure, InfiniteData<ActivityPage>, readonly unknown[], string | undefined>({
    queryKey: [...opPrefix("history.feed"), { task, pages: HISTORY_PAGE }],
    queryFn: ({ pageParam }) => call("history.feed", { task, limit: HISTORY_PAGE, ...(pageParam === undefined ? {} : { cursor: pageParam }) }),
    initialPageParam: undefined,
    getNextPageParam: (page) => page.next_cursor,
    enabled,
  });
}

/** Whether a history group can still be offered for revert. */
export function revertible(entry: ActivityEntry): boolean {
  return entry.reverted_by === undefined && !entry.op.startsWith("history.") && entry.fields.purged === undefined;
}
