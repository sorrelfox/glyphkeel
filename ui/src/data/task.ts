// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The hooks of the task panel: a task with its subtasks, links and files, its
// comments, and its part of the activity log.

import { useQuery } from "@tanstack/react-query";

import { type ActivityEntry, type Call, type Task, type TaskPart } from "../generated/ops";
import { useCall, useOp } from "./hooks";
import { opPrefix } from "./queries";
import { type OpFailure } from "./transport";

/** What the panel reads with a task. */
export const PANEL_PARTS: readonly TaskPart[] = ["subtasks", "links", "files"];

/** One task with its subtasks, links and files. */
export function useTaskDetails(key: string) {
  return useOp("task.get", { keys: [key], include: [...PANEL_PARTS] });
}

/** A task's comments, oldest first, each with its earlier texts. */
export function useComments(key: string) {
  return useOp("comment.list", { key, revisions: true });
}

/** How far back the panel looks for a task's history. */
const HISTORY_PAGES = 5;
const HISTORY_PAGE = 200;

const record = (value: unknown): Record<string, unknown> => (typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {});

/**
 * Whether an entry of the activity log belongs to `task`: its own changes,
 * its links, and the comments and files on it. `comments` and `files` are
 * the IDs known to be the task's.
 */
export function belongsTo(entry: ActivityEntry, task: Pick<Task, "id" | "key">, comments: ReadonlySet<string>, files: ReadonlySet<string>): boolean {
  switch (entry.entity) {
    case "task":
      return entry.id === task.id;
    case "relation": {
      const link = record(entry.fields.link?.after ?? entry.fields.link?.before);
      return link.from === task.id || link.to === task.id;
    }
    case "comment":
      return comments.has(entry.id) || entry.fields.task?.after === task.key || entry.fields.task?.before === task.key;
    case "attachment":
      return files.has(entry.id);
    default:
      return false;
  }
}

/**
 * A task's history, newest first: the activity log read page by page (the
 * log has no filter by task) and kept to what belongs to the task. Reading
 * stops at the task's creation or after a few pages.
 */
export async function taskHistory(call: Call, task: Pick<Task, "id" | "key">, files: ReadonlySet<string>): Promise<ActivityEntry[]> {
  const entries: ActivityEntry[] = [];
  let cursor: string | undefined;
  for (let page = 0; page < HISTORY_PAGES; page += 1) {
    const answer = await call("history.feed", { limit: HISTORY_PAGE, ...(cursor === undefined ? {} : { cursor }) });
    entries.push(...answer.items);
    cursor = answer.next_cursor;
    const created = answer.items.some((entry) => entry.entity === "task" && entry.id === task.id && entry.fields.created !== undefined);
    if (cursor === undefined || created) {
      break;
    }
  }
  // A comment is the task's when it was added to it; its later changes name only the comment.
  const comments = new Set(entries.filter((entry) => entry.entity === "comment" && (entry.fields.task?.after === task.key || entry.fields.task?.before === task.key)).map((entry) => entry.id));
  return entries.filter((entry) => belongsTo(entry, task, comments, files));
}

/** The history of `task`, refreshed with the activity log. */
export function useTaskHistory(task: Pick<Task, "id" | "key">, files: readonly string[], enabled: boolean) {
  const call = useCall();
  return useQuery<ActivityEntry[], OpFailure>({
    queryKey: [...opPrefix("history.feed"), { task: task.id, files }],
    queryFn: () => taskHistory(call, task, new Set(files)),
    enabled,
  });
}

/** Whether a history group can still be offered for revert. */
export function revertible(entry: ActivityEntry): boolean {
  return entry.reverted_by === undefined && !entry.op.startsWith("history.") && entry.fields.purged === undefined;
}
