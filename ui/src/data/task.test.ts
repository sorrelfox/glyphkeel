// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ActivityEntry } from "../generated/ops";
import { fakeTransport } from "./fake/transport";
import { revertible } from "./task";
import { createClient } from "./transport";

const TODAY = new Date(2026, 10, 23, 9, 0);

const entry = (fields: Partial<ActivityEntry>): ActivityEntry => ({
  actor: { kind: "user", name: "You" },
  at: "2026-11-23T09:00:00Z",
  entity: "task",
  fields: {},
  group: "g",
  id: "t-1",
  op: "task.update",
  seq: 1,
  via: "app",
  ...fields,
});

describe("a task's history", () => {
  it("is the activity log the engine filters to the task, newest first, including later changes to its comments and links", async () => {
    const { call } = createClient(fakeTransport({ today: () => TODAY }));
    const made = await call("task.create", { project: "PER", title: "Watched" });
    const other = await call("task.create", { project: "PER", title: "Other" });
    await call("task.update", { key: made.task.key, priority: "high" });
    await call("task.update", { key: other.task.key, priority: "low" });
    const comment = await call("comment.add", { key: made.task.key, body: "hi" });
    await call("comment.add", { key: other.task.key, body: "elsewhere" });
    await call("comment.delete", { comment: comment.comment.id });
    await call("relation.add", { key: other.task.key, kind: "blocks", other: made.task.key });
    const history = await call("history.feed", { task: made.task.key });
    expect(history.items.map((item) => item.op)).toEqual(["relation.add", "comment.delete", "comment.add", "task.update", "task.create"]);
    expect(history.total).toBe(5);
    const page = await call("history.feed", { task: made.task.id, limit: 2 });
    expect(page.items).toHaveLength(2);
    expect(page.next_cursor).toBeDefined();
  });

  it("offers a revert only for changes not reverted yet", () => {
    expect(revertible(entry({}))).toBe(true);
    expect(revertible(entry({ reverted_by: "g-2" }))).toBe(false);
    expect(revertible(entry({ op: "history.revert" }))).toBe(false);
  });
});
