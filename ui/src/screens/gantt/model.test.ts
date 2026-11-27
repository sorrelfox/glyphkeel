// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ScheduleProblem, type ScheduledTask, type UnscheduledTask } from "../../generated/ops";
import { layoutFrom, layoutUpdate } from "./layout";
import { behindSchedule, buildRows, milestonesOf, columnWindow, firstLabel, datesUpdate, draggedDates, elbow, endsLater, geometry, isWarning, problemsByTask, ROW, rowWindow, undatedQuery, visibleDependencies } from "./model";

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
    expect(rows[5]).toEqual({ kind: "group", id: "unscheduled", head: { by: "unscheduled" }, count: 1 });
    expect(index.get("TM-9")).toBe(6);
  });

  it("count every task without dates in the Unscheduled group, also those not listed", () => {
    const { rows } = buildRows(tasks, [loose], new Set(), "parent", undefined, 19_441);
    expect(rows.find((row) => row.kind === "group")).toMatchObject({ id: "unscheduled", count: 19_441 });
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
  it("takes what the engine keeps and is sound, and the defaults for the rest", () => {
    expect(layoutFrom(undefined)).toEqual({ zoom: "day", table: undefined, widths: {}, groupBy: "parent", moveDependents: false });
    const stored = { zoom: "month", table: 5000, widths: { key: 12, due: 90, title: 300 }, group_by: "label", move_dependents: true } as const;
    expect(layoutFrom(stored)).toEqual({ zoom: "month", table: 900, widths: { key: 32, due: 90 }, groupBy: "label", moveDependents: true });
    // Damaged values from an older or newer program.
    expect(layoutFrom(JSON.parse('{"zoom":"decade","group_by":"colour","widths":{"start":"wide"}}') as Parameters<typeof layoutFrom>[0])).toEqual(layoutFrom(undefined));
  });

  it("sends the engine only what changed", () => {
    expect(layoutUpdate("TM", { zoom: "week" })).toEqual({ project: "TM", zoom: "week" });
    expect(layoutUpdate("TM", { widths: { key: 80 }, groupBy: "none", moveDependents: false, table: 400 })).toEqual({
      project: "TM",
      widths: { key: 80 },
      group_by: "none",
      move_dependents: false,
      table: 400,
    });
  });
});

describe("grouping", () => {
  const label = (name: string, hue: "red" | "blue" = "red") => ({ id: `l-${name}`, name, hue });
  const doing = { id: "s2", name: "Doing", category: "in_progress" as const };
  const tasks = [
    task("TM-1", 0, { shape: "summary", labels: [label("ui"), label("api", "blue")] }),
    task("TM-2", 1, { status: doing }),
    task("TM-3", 0, { labels: [label("ui")], status: doing }),
  ];
  const order = { statuses: new Map([["s2", 0], ["s1", 1]]), hues: new Map(), compare: (a: string, b: string) => a.localeCompare(b) };

  it("by label: the first label in alphabetical order, the tasks without one last, flat", () => {
    const { rows } = buildRows(tasks, [loose], new Set(), "label", order);
    expect(rows.map((row) => (row.kind === "group" ? row.id : row.task.key))).toEqual(["label:api", "TM-1", "label:ui", "TM-3", "label:", "TM-2", "unscheduled", "TM-9"]);
    expect(rows.every((row) => row.kind !== "task" || (row.level === 1 && !row.hasKids))).toBe(true);
    expect(rows[0]).toMatchObject({ head: { by: "label", name: "api", hue: "blue" }, count: 1 });
    expect(rows[4]).toMatchObject({ head: { by: "label", name: undefined }, count: 1 });
  });

  it("by status: in board order", () => {
    const { rows, index } = buildRows(tasks, [], new Set(), "status", order);
    expect(rows.map((row) => (row.kind === "group" ? row.id : row.task.key))).toEqual(["status:s2", "TM-2", "TM-3", "status:s1", "TM-1"]);
    expect(index.get("TM-1")).toBe(4);
  });

  it("none: every task flat, in the schedule's order; folds don't matter", () => {
    const { rows } = buildRows(tasks, [], new Set(["TM-1"]), "none");
    expect(rows.map((row) => (row.kind === "group" ? row.id : row.task.key))).toEqual(["TM-1", "TM-2", "TM-3"]);
  });

  it("by claim: one group per agent session, the unclaimed tasks last", () => {
    const claim = (agent: string, session: string) => ({ agent, session, until: "2026-11-26T12:00:00Z" });
    const claimed = [
      task("TM-1", 0, { claim: claim("Helper", "s-2") }),
      task("TM-2", 0),
      task("TM-3", 0, { claim: claim("Builder", "s-1") }),
      task("TM-4", 0, { claim: claim("Helper", "s-2") }),
    ];
    const { rows } = buildRows(claimed, [], new Set(), "claim", order);
    expect(rows.map((row) => (row.kind === "group" ? row.id : row.task.key))).toEqual(["claim:s-1", "TM-3", "claim:s-2", "TM-1", "TM-4", "claim:", "TM-2"]);
    expect(rows[2]).toMatchObject({ head: { by: "claim", agent: "Helper", session: "s-2" }, count: 2 });
    expect(rows[5]).toMatchObject({ head: { by: "claim", agent: undefined }, count: 1 });
  });

  it("finds the first label in the person's order", () => {
    expect(firstLabel({ labels: [label("b"), label("A")] }, (a, b) => a.localeCompare(b))?.name).toBe("A");
    expect(firstLabel({}, (a, b) => a.localeCompare(b))).toBeUndefined();
  });
});

describe("behind schedule", () => {
  const bar = (extra: Parameters<typeof task>[2]) => task("TM-1", 0, { start_date: "2026-11-01", due_date: "2026-11-10", ...extra });

  it("when the time gone runs more than ten points ahead of the progress", () => {
    // The 5th day of 10, to its middle: 45 % of the time.
    expect(behindSchedule(bar({ progress_percent: 30 }), "2026-11-05")).toEqual({ done: 0.3, expected: 0.45 });
    // 10 points or less behind is within the tolerance.
    expect(behindSchedule(bar({ progress_percent: 35 }), "2026-11-05")).toBeUndefined();
    expect(behindSchedule(bar({ progress_percent: 40 }), "2026-11-05")).toBeUndefined();
    expect(behindSchedule(bar({ progress_percent: 34 }), "2026-11-05")).toEqual({ done: 0.34, expected: 0.45 });
  });

  it("not before a whole day of the bar has passed", () => {
    // The first day: 5 % gone, so nothing yet; a one-day bar is never behind.
    expect(behindSchedule(bar({}), "2026-11-01")).toBeUndefined();
    expect(behindSchedule(bar({ start_date: "2026-11-05", due_date: "2026-11-05" }), "2026-11-05")).toBeUndefined();
    // The second day: 15 % gone, more than 10 points ahead of nothing done.
    expect(behindSchedule(bar({}), "2026-11-02")).toEqual({ done: 0, expected: 0.15 });
    // A two-day bar on its second day: 75 % gone.
    expect(behindSchedule(bar({ start_date: "2026-11-01", due_date: "2026-11-02", progress_percent: 60 }), "2026-11-02")).toEqual({ done: 0.6, expected: 0.75 });
  });

  it("only for open bars that have begun and aren't overdue", () => {
    expect(behindSchedule(bar({}), "2026-10-31")).toBeUndefined();
    expect(behindSchedule(bar({ closed: true }), "2026-11-05")).toBeUndefined();
    expect(behindSchedule(bar({ overdue: true }), "2026-11-11")).toBeUndefined();
    expect(behindSchedule(bar({ shape: "summary" }), "2026-11-05")).toBeUndefined();
    expect(behindSchedule(bar({ shape: "deadline", start_date: undefined }), "2026-11-05")).toBeUndefined();
  });
});

describe("milestones", () => {
  it("lists the milestones by day, those without one last, with how each stands", () => {
    const done = { id: "s3", name: "Done", category: "done" as const };
    const list = milestonesOf({
      today: "2026-11-20",
      tasks: [
        task("TM-10", 0, { shape: "milestone", start_date: undefined, due_date: "2026-12-01" }),
        task("TM-2", 0, { shape: "milestone", start_date: undefined, due_date: "2026-11-15" }),
        task("TM-3", 0, { shape: "milestone", start_date: undefined, due_date: "2026-11-10", closed: true, status: done }),
        task("TM-4", 0),
      ],
      unscheduled: [{ ...loose, key: "TM-12", milestone: true }, loose, { ...loose, key: "TM-11", milestone: true }],
    });
    expect(list.map((milestone) => [milestone.key, milestone.date, milestone.state, milestone.late])).toEqual([
      ["TM-3", "2026-11-10", "done", 0],
      ["TM-2", "2026-11-15", "overdue", 5],
      ["TM-10", "2026-12-01", "open", 0],
      ["TM-11", undefined, "open", 0],
      ["TM-12", undefined, "open", 0],
    ]);
    expect(list[0]?.status).toBe("Done");
  });
});

describe("the list query for the tasks without dates", () => {
  it("keeps the chart's query without its sort clause", () => {
    expect(undatedQuery("")).toBe("is:unscheduled");
    expect(undatedQuery("  sort:due ")).toBe("is:unscheduled");
    expect(undatedQuery("label:bug OR label:crash sort:-due")).toBe("is:unscheduled (label:bug OR label:crash)");
  });
});
