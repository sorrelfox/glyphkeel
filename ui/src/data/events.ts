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
  settings: ["settings.get", "workspace.get"],
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
