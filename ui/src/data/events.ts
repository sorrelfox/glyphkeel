// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type QueryClient } from "@tanstack/react-query";

import { type Changed, type OpId } from "../generated/ops";
import { OP_ROOT, opPrefix } from "./queries";

/**
 * The queries whose answers a change to each kind of item can alter. Events
 * carry no content: the UI re-reads what is on screen, so the engine stays
 * the one source of truth.
 */
export const ENTITY_QUERIES: Readonly<Record<string, readonly OpId[]>> = {
  // `workspace.get` carries the settings too.
  // Durations are told in the workspace's units, and the timer reminds after its hours.
  settings: ["settings.get", "workspace.get", "timer.get", "time.report"],
  // Open-task counts show in the project tree. Rendered texts link task keys
  // only while the task exists, and descriptions keep their revisions.
  // A task's reminder is one of its fields; the time report shows titles.
  task: ["task.get", "board.get", "project.list", "markdown.render", "description.history", "description.revision", "time.report", "timer.get", "task.search", "today.get", "search.query"],
  // Statuses, types and labels show on every card and in the task panel.
  project: ["project.list", "project.get", "board.get", "task.get", "layout.get", "markdown.render", "time.report", "task.search", "query.parse", "view.list", "today.get", "search.query", "wiki.search", "wiki.get"],
  status: ["project.get", "board.get", "task.get", "project.list", "task.search", "query.parse", "today.get", "search.query"],
  task_type: ["project.get", "board.get", "task.get", "task.search", "query.parse"],
  label: ["project.get", "board.get", "task.get", "task.search", "query.parse"],
  // The board leaves out the cards of hidden columns.
  layout: ["layout.get", "board.get"],
  // Cards count comments and files, and show the Blocked badge.
  comment: ["comment.list", "task.get", "board.get", "markdown.render", "task.search", "today.get", "search.query"],
  attachment: ["comment.list", "task.get", "board.get", "file.get", "file.read", "task.search", "search.query"],
  relation: ["task.get", "board.get", "task.search"],
  // Cards and the task show the time spent; the report adds it up.
  time_entry: ["task.get", "board.get", "time.report", "task.search"],
  // The one timer: its chip, the task's timer button, and `workspace.get`, which carries it.
  timer: ["timer.get", "workspace.get", "today.get"],
  // Saved views of the list.
  view: ["view.list"],
  // Wiki pages: their search, links to them and their text.
  wiki_page: ["wiki.search", "wiki.get", "wiki.list", "wiki.history", "search.query", "markdown.render"],
};

/** Every committed change shows in the activity log. */
const ALWAYS: readonly OpId[] = ["history.feed", "agent.sessions"];

/**
 * The query prefixes to invalidate for one event. A kind of item without an
 * entry invalidates everything: refetching too much is slow, too little is
 * wrong.
 */
export function invalidationsFor(event: Changed): readonly OpId[] | "all" {
  const ops = new Set<OpId>(ALWAYS);
  for (const change of event.changes) {
    const affected = ENTITY_QUERIES[change.entity];
    if (!affected) {
      return "all";
    }
    for (const op of affected) {
      ops.add(op);
    }
  }
  return [...ops];
}

/** Marks the queries an event affects as stale; those on screen refetch. */
export function applyEvent(queryClient: QueryClient, event: Changed): Promise<void> {
  const ops = invalidationsFor(event);
  if (ops === "all") {
    return queryClient.invalidateQueries({ queryKey: OP_ROOT });
  }
  return Promise.all(ops.map((op) => queryClient.invalidateQueries({ queryKey: opPrefix(op) }))).then(() => undefined);
}

/**
 * After (re)subscribing, events sent while the page wasn't listening are
 * lost: everything is refetched instead.
 */
export function refetchAll(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: OP_ROOT });
}
