// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ScheduleProblem, type ScheduledTask, type UnscheduledTask } from "../../generated/ops";
import { layoutFrom } from "./layout";
import { buildRows, columnWindow, datesUpdate, draggedDates, elbow, endsLater, geometry, isWarning, problemsByTask, ROW, rowWindow, visibleDependencies } from "./model";

const status = { id: "s1", name: "To do", category: "todo" as const, hue: "grey" as const };

function task(key: string, depth: number, extra: { [K in keyof ScheduledTask]?: ScheduledTask[K] | undefined } = {}): ScheduledTask {
  const all = {
    blocked: false,
    closed: false,
    dependents: 0,
    depth,
    id: `id-${key}`,
    key,
    overdue: false,
    progress_percent: 0,
    project: "TM",
    shape: "bar",
    start_date: "2026-11-10",
    due_date: "2026-11-12",
    status,
    title: key,
    version: 1,
    ...extra,
  };
  // Fields given as undefined are left out, as the engine leaves them out.
  return Object.fromEntries(Object.entries(all).filter(([, value]) => value !== undefined)) as ScheduledTask;
}

const loose: UnscheduledTask = { closed: false, id: "id-TM-9", key: "TM-9", milestone: false, project: "TM", status, title: "Loose", version: 1 };

describe("rows", () => {
  const tasks = [task("TM-1", 0, { shape: "summary" }), task("TM-2", 1), task("TM-3", 1, { shape: "summary" }), task("TM-4", 2), task("TM-5", 0)];

  it("keep the tree's order, mark parents, and end with the Unscheduled group", () => {
    const { rows, index } = buildRows(tasks, [loose], new Set());
    expect(rows.map((row) => (row.kind === "group" ? "group" : row.task.key))).toEqual(["TM-1", "TM-2", "TM-3", "TM-4", "TM-5", "group", "TM-9"]);
    expect(rows[0]).toMatchObject({ kind: "task", level: 1, hasKids: true, open: true });
    expect(rows[1]).toMatchObject({ level: 2, hasKids: false });
    expect(rows[5]).toEqual({ kind: "group", count: 1 });
    expect(index.get("TM-9")).toBe(6);
  });

  it("leave out the subtasks of a folded task, at every level below it", () => {
    const { rows } = buildRows(tasks, [], new Set(["TM-1"]));
    expect(rows.map((row) => (row.kind === "group" ? "group" : row.task.key))).toEqual(["TM-1", "TM-5"]);
    expect(rows[0]).toMatchObject({ open: false });
    const inner = buildRows(tasks, [], new Set(["TM-3"]));
    expect(inner.rows.map((row) => (row.kind === "group" ? "group" : row.task.key))).toEqual(["TM-1", "TM-2", "TM-3", "TM-5"]);
  });
});

describe("shapes", () => {
  it("run from the first day to the end of the last", () => {
    expect(geometry(task("TM-1", 0), "2026-11-01", 10)).toEqual({ shape: "bar", x1: 90, x2: 120 });
  });

  it("draw a deadline at its due date, a start-only task over one day, a milestone on its day", () => {
    expect(geometry(task("TM-1", 0, { shape: "deadline", start_date: undefined }), "2026-11-01", 10)).toEqual({ shape: "deadline", x1: 110, x2: 120 });
    expect(geometry(task("TM-1", 0, { shape: "no_deadline", due_date: undefined }), "2026-11-01", 10)).toEqual({ shape: "no_deadline", x1: 90, x2: 100 });
    expect(geometry(task("TM-1", 0, { shape: "milestone", start_date: undefined }), "2026-11-01", 10)).toEqual({ shape: "milestone", x1: 110, x2: 120 });
  });

  it("span a parent's summary range, and are at least two pixels wide", () => {
    expect(geometry(task("TM-1", 0, { shape: "summary", summary: { start: "2026-11-02", end: "2026-11-30" } }), "2026-11-01", 1)).toEqual({ shape: "summary", x1: 1, x2: 30 });
    expect(geometry(task("TM-1", 0, { start_date: "2026-11-02", due_date: "2026-11-02" }), "2026-11-01", 1.2)?.x2).toBeCloseTo(3.2, 6);
  });

  it("use the dragged dates in place of the task's own", () => {
    expect(geometry(task("TM-1", 0), "2026-11-01", 10, { start: "2026-11-11", due: "2026-11-13" })).toEqual({ shape: "bar", x1: 100, x2: 130 });
  });
});

describe("dragging dates", () => {
  const both = { start: "2026-11-10", due: "2026-11-12" };

  it("moves both dates, or one", () => {
    expect(draggedDates(both, "move", 3)).toEqual({ start: "2026-11-13", due: "2026-11-15" });
    expect(draggedDates(both, "start", -2)).toEqual({ start: "2026-11-08", due: "2026-11-12" });
    expect(draggedDates(both, "end", 5)).toEqual({ start: "2026-11-10", due: "2026-11-17" });
  });

  it("never lets the start pass the due date", () => {
    expect(draggedDates(both, "start", 9)).toEqual({ start: "2026-11-12", due: "2026-11-12" });
    expect(draggedDates(both, "end", -9)).toEqual({ start: "2026-11-10", due: "2026-11-10" });
  });

  it("gives a deadline a start, and a start-only task a due date", () => {
    expect(draggedDates({ start: undefined, due: "2026-11-12" }, "start", -4)).toEqual({ start: "2026-11-08", due: "2026-11-12" });
    expect(draggedDates({ start: "2026-11-10", due: undefined }, "end", 2)).toEqual({ start: "2026-11-10", due: "2026-11-12" });
    expect(draggedDates({ start: undefined, due: "2026-11-12" }, "move", 1)).toEqual({ start: undefined, due: "2026-11-13" });
  });

  it("asks the engine for only what changed", () => {
    expect(datesUpdate(both, both)).toBeUndefined();
    expect(datesUpdate(both, { start: "2026-11-11", due: "2026-11-12" })).toEqual({ start_date: "2026-11-11" });
    expect(datesUpdate(both, { start: undefined, due: "2026-11-14" })).toEqual({ due_date: "2026-11-14", clear: ["start_date"] });
  });

  it("knows when the end moves later, when dependents may move", () => {
    expect(endsLater(both, { start: "2026-11-11", due: "2026-11-13" })).toBe(true);
    expect(endsLater(both, { start: "2026-11-09", due: "2026-11-11" })).toBe(false);
    expect(endsLater({ start: undefined, due: undefined }, both)).toBe(false);
  });
});

describe("problems and dependencies", () => {
  const problems: ScheduleProblem[] = [
    { kind: "starts_before_blocker_ends", task: "TM-2", other: "TM-1", message: "", message_id: "schedule-starts-before-blocker" },
    { kind: "overdue", task: "TM-2", message: "", message_id: "schedule-overdue" },
    { kind: "no_due_date", task: "TM-3", message: "", message_id: "schedule-no-due-date" },
  ];

  it("are grouped by task, and only some get the warning sign", () => {
    const byTask = problemsByTask(problems);
    expect(byTask.get("TM-2")?.length).toBe(2);
    expect(byTask.get("TM-2")?.filter(isWarning).map((p) => p.kind)).toEqual(["starts_before_blocker_ends"]);
    expect(byTask.get("TM-3")?.filter(isWarning)).toEqual([]);
  });

  it("draw only the dependencies that touch or cross the rows on screen", () => {
    const index = new Map([["A", 0], ["B", 50], ["C", 100]]);
    const deps = [
      { id: "1", from: "A", to: "C", violated: false },
      { id: "2", from: "A", to: "B", violated: false },
      { id: "3", from: "X", to: "A", violated: false },
    ];
    expect(visibleDependencies(deps, index, 60, 80).map((d) => d.id)).toEqual(["1"]);
    expect(visibleDependencies(deps, index, 40, 56).map((d) => d.id)).toEqual(["1", "2"]);
  });

  it("go straight when there is room, round the rows when not", () => {
    expect(elbow(100, 16, 140, 48)).toBe("M100,16 H108 V48 H140");
    expect(elbow(100, 16, 100, 48)).toBe(`M100,16 h8 V${String(16 + ROW / 2)} H92 V48 H100`);
  });
});

describe("windows", () => {
  it("move in chunks with rows to spare", () => {
    expect(rowWindow(0, 600, 500)).toEqual([0, 32]);
    expect(rowWindow(ROW * 100, 600, 500)).toEqual([88, 128]);
    expect(rowWindow(ROW * 495, 600, 500)).toEqual([480, 500]);
    expect(columnWindow(0, 1200)).toEqual([0, 2048]);
    expect(columnWindow(5000, 1200)).toEqual([4096, 7168]);
  });
});

describe("the person's layout", () => {
  it("takes what is stored and sound, and the defaults for the rest", () => {
    expect(layoutFrom(undefined)).toEqual({ zoom: "day", table: undefined, widths: {}, moveDependents: false });
    expect(layoutFrom({ zoom: "month", table: 5000, widths: { key: 12, due: 90, title: 300, start: "wide" }, moveDependents: true })).toEqual({
      zoom: "month",
      table: 900,
      widths: { key: 32, due: 90 },
      moveDependents: true,
    });
    expect(layoutFrom({ zoom: "decade" }).zoom).toBe("day");
  });
});
