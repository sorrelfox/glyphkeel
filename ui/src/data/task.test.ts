// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ActivityEntry } from "../generated/ops";
import { fakeTransport } from "./fake/transport";
import { belongsTo, revertible, taskHistory } from "./task";
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
  const task = { id: "t-1", key: "TM-1" };

  it("takes the task's own changes, its links, and the comments and files on it", () => {
    const none = new Set<string>();
    expect(belongsTo(entry({}), task, none, none)).toBe(true);
    expect(belongsTo(entry({ id: "t-2" }), task, none, none)).toBe(false);
    expect(belongsTo(entry({ entity: "relation", id: "r", fields: { link: { after: { from: "t-2", to: "t-1", kind: "blocks" } } } }), task, none, none)).toBe(true);
    expect(belongsTo(entry({ entity: "relation", id: "r", fields: { link: { before: { from: "t-2", to: "t-3", kind: "blocks" } } } }), task, none, none)).toBe(false);
    expect(belongsTo(entry({ entity: "comment", id: "c", fields: { task: { after: "TM-1" } } }), task, none, none)).toBe(true);
    expect(belongsTo(entry({ entity: "comment", id: "c", fields: { body: { after: "x" } } }), task, new Set(["c"]), none)).toBe(true);
    expect(belongsTo(entry({ entity: "attachment", id: "f" }), task, none, new Set(["f"]))).toBe(true);
    expect(belongsTo(entry({ entity: "project", id: "t-1" }), task, none, none)).toBe(false);
  });

  it("is read from the activity log, newest first, including later changes to its comments", async () => {
    const { call } = createClient(fakeTransport({ today: () => TODAY }));
    const made = await call("task.create", { project: "PER", title: "Watched" });
    await call("task.create", { project: "PER", title: "Other" });
    await call("task.update", { key: made.task.key, priority: "high" });
    const comment = await call("comment.add", { key: made.task.key, body: "hi" });
    await call("comment.delete", { comment: comment.comment.id });
    const history = await taskHistory(call, made.task, new Set());
    expect(history.map((item) => item.op)).toEqual(["comment.delete", "comment.add", "task.update", "task.create"]);
  });

  it("offers a revert only for changes not reverted yet", () => {
    expect(revertible(entry({}))).toBe(true);
    expect(revertible(entry({ reverted_by: "g-2" }))).toBe(false);
    expect(revertible(entry({ op: "history.revert" }))).toBe(false);
  });
});
