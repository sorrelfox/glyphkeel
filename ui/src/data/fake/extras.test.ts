// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The sample workspace's history conditions in queries, the lag of a
// dependency, the WIP minimum, kinds of work and closing duplicates: the
// same rules as the engine, small.

import { describe, expect, it } from "vitest";

import { type Board, type Schedule, type TaskChange, type TaskList, type TimeEntryChange, type TimeReport, type WorkKindList } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function failure(run: () => unknown) {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

const keys = (ws: FakeWorkspace, query: string) => call<TaskList>(ws, "task.search", { query, limit: 500 }).items.map((task) => task.key);

function newTask(ws: FakeWorkspace, title: string, extra: Record<string, unknown> = {}): string {
  return call<TaskChange>(ws, "task.create", { project: "LAB", title, ...extra }).task.key;
}

describe("history conditions", () => {
  it("find what a task was and when its fields changed", () => {
    const ws = workspace();
    const key = newTask(ws, "Moves around");
    call(ws, "task.move", { key, status: "In progress" });
    call(ws, "task.move", { key, status: "To do" });
    // Tasks in progress now match too; this one was in progress before.
    expect(keys(ws, 'was:status:"in progress" project:LAB')).toContain(key);
    expect(keys(ws, 'status:"in progress" project:LAB')).not.toContain(key);
    expect(keys(ws, "was:category:in_progress project:LAB")).toContain(key);
    expect(keys(ws, "changed:status project:LAB")).toEqual([key]);
    expect(keys(ws, "changed:status:today project:LAB")).toEqual([key]);
    expect(keys(ws, "changed:status<-7d project:LAB")).toEqual([]);
    expect(keys(ws, "changed-by:agent project:LAB")).toEqual([]);
    expect(keys(ws, "changed-by:me project:LAB")).toContain(key);
    expect(failure(() => keys(ws, "was:due:today")).details?.message_id).toBe("query-history-field");
    expect(failure(() => keys(ws, "was:status")).details?.message_id).toBe("query-was-value");
  });
});

describe("dependencies", () => {
  it("keep a lag, which the schedule and its problems count", () => {
    const ws = workspace();
    const a = newTask(ws, "Pour", { start_date: "2026-12-01", due_date: "2026-12-02" });
    const b = newTask(ws, "Build", { start_date: "2026-12-04", due_date: "2026-12-05" });
    call(ws, "relation.add", { key: a, kind: "blocks", other: b, lag_days: 2 });
    const schedule = call<Schedule>(ws, "schedule.get", { project: "LAB" });
    const link = schedule.dependencies.find((each) => each.from === a);
    expect(link?.lag_days).toBe(2);
    expect(link?.violated).toBe(true);
    call(ws, "relation.add", { key: a, kind: "blocks", other: b, lag_days: 0 });
    expect(call<Schedule>(ws, "schedule.get", { project: "LAB" }).dependencies.find((each) => each.from === a)?.lag_days).toBeUndefined();
    expect(failure(() => call(ws, "relation.add", { key: a, kind: "relates", other: b, lag_days: 1 })).details?.message_id).toBe("link-lag-blocks-only");
  });
});

describe("board columns", () => {
  it("say when they hold fewer tasks than their WIP minimum", () => {
    const ws = workspace();
    call(ws, "status.update", { project: "LAB", status: "Review", wip_min: 50 });
    const column = call<Board>(ws, "board.get", { project: "LAB" }).columns.find((each) => each.name === "Review");
    expect(column?.wip_min).toBe(50);
    expect(column?.under_minimum).toBe(true);
    expect(failure(() => call(ws, "status.update", { project: "LAB", status: "Review", wip_limit: 3 })).details?.message_id).toBe("status-wip-min-above-limit");
  });
});

describe("kinds of work", () => {
  it("mark time, group the report and can't be deleted while used", () => {
    const ws = workspace();
    const key = newTask(ws, "Timed");
    const entry = call<TimeEntryChange>(ws, "time.log", { key, duration: "1h", work_kind: "design" });
    expect(entry.entry.work_kind?.name).toBe("Design");
    call(ws, "time.log", { key, duration: "30m" });
    const report = call<TimeReport>(ws, "time.report", { group_by: "work_kind", task: key });
    expect(report.items.map((row) => [row.title, row.minutes])).toEqual([
      ["Design", 60],
      ["", 30],
    ]);
    expect(call<TimeReport>(ws, "time.report", { task: key, work_kind: "none" }).total_minutes).toBe(30);
    expect(failure(() => call(ws, "work_kind.delete", { kind: "Design" })).details?.message_id).toBe("work-kind-in-use");
    call(ws, "work_kind.update", { kind: "Design", archived: true });
    expect(call<WorkKindList>(ws, "work_kind.list").items.map((kind) => kind.name)).toEqual(["Development", "Meeting"]);
    expect(failure(() => call(ws, "time.log", { key, duration: "5m", work_kind: "Design" })).details?.message_id).toBe("work-kind-archived");
  });
});

describe("duplicates", () => {
  it("close with their original when asked, and are named when not", () => {
    const ws = workspace();
    const original = newTask(ws, "Crash");
    const copy = newTask(ws, "Crash again");
    call(ws, "relation.add", { key: copy, kind: "duplicates", other: original });
    const warned = call<TaskChange>(ws, "task.move", { key: original, status: "Done" });
    expect(warned.warnings?.join(" ")).toContain(copy);
    call(ws, "task.move", { key: original, status: "To do" });
    const closed = call<TaskChange>(ws, "task.move", { key: original, status: "Done", close_duplicates: true });
    expect(closed.duplicates_closed).toEqual([copy]);
    expect(call<TaskList>(ws, "task.get", { keys: [copy] }).items[0]?.status.name).toBe("Done");
  });
});
