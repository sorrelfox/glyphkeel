// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type OpId } from "../generated/ops";
import { createQueryClient } from "./DataProvider";
import { applyEvent, invalidationsFor, refetchAll } from "./events";
import { opKey, type QueryOp } from "./queries";
import { changed } from "./testing";

/** A cache with one answer for each of these queries. */
function primed(ops: QueryOp[]) {
  const queryClient = createQueryClient();
  for (const op of ops) {
    queryClient.setQueryData(opKey(op, {}), { cached: op });
  }
  return queryClient;
}

const ALL_QUERIES: QueryOp[] = ["workspace.get", "settings.get", "history.feed", "agent.sessions"];

function invalidated(queryClient: ReturnType<typeof createQueryClient>): OpId[] {
  return queryClient
    .getQueryCache()
    .getAll()
    .filter((query) => query.state.isInvalidated)
    .map((query) => query.queryKey[1] as OpId)
    .sort();
}

describe("change events", () => {
  it("invalidate exactly the queries a settings change can alter", async () => {
    const queryClient = primed(ALL_QUERIES);
    await applyEvent(queryClient, changed(1, "settings", ["week_start"]));
    expect(invalidated(queryClient)).toEqual(["agent.sessions", "history.feed", "settings.get", "workspace.get"]);
  });

  it("invalidate a query whatever its input", async () => {
    const queryClient = createQueryClient();
    queryClient.setQueryData(opKey("history.feed", { limit: 10 }), {});
    queryClient.setQueryData(opKey("history.feed", { limit: 50, actor: "You" }), {});
    await applyEvent(queryClient, changed(1, "settings", ["trash_days"]));
    expect(invalidated(queryClient)).toEqual(["history.feed", "history.feed"]);
  });

  it("leave everything else alone", async () => {
    const queryClient = primed(["workspace.get", "settings.get"]);
    queryClient.setQueryData(["not-an-op"], 1);
    await applyEvent(queryClient, changed(1, "settings", ["week_start"]));
    expect(queryClient.getQueryState(["not-an-op"])?.isInvalidated).toBe(false);
  });

  it("invalidate every operation's answers for a kind of item they don't know", async () => {
    expect(invalidationsFor(changed(1, "comet", ["tail"]))).toBe("all");
    const queryClient = primed(ALL_QUERIES);
    await applyEvent(queryClient, changed(1, "comet", ["tail"]));
    expect(invalidated(queryClient)).toEqual([...ALL_QUERIES].sort());
  });

  it("after subscribing again, refetch everything", async () => {
    const queryClient = primed(ALL_QUERIES);
    await refetchAll(queryClient);
    expect(invalidated(queryClient)).toEqual([...ALL_QUERIES].sort());
  });
});
