// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ListedTask, type Schedule, type TaskSearchResult, type BaselineChange, type BaselineList } from "../../generated/ops";
import { criticalPath, workWeek } from "./critical";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

function listed(key: string, extra: Partial<ListedTask> = {}): ListedTask {
  return {
    id: `id-${key}`,
    key,
    title: key,
    project: "TM",
    progress_percent: 0,
    status: { id: "s", name: "To do", category: "todo", hue: "grey" },
    type: { id: "task", name: "Task", glyph: "task" },
    labels: [],
    version: 1,
    ...extra,
  } as ListedTask;
}

const link = (from: string, to: string, lagDays = 0) => ({ id: `${from}>${to}`, from: `id-${from}`, to: `id-${to}`, lagDays });

describe("the sample critical path", () => {
  it("counts working days from Monday", () => {
    const week = workWeek(5);
    expect(week.begin("2026-11-28")).toBe(week.begin("2026-11-30"));
    expect(week.end("2026-11-29")).toBe(week.end("2026-11-27"));
    expect(week.day(week.begin("2026-11-27") + 1)).toBe("2026-11-30");
  });

  it("gives the textbook network its times, slack and critical tasks", () => {
    const days = [3, 4, 2, 5, 3, 2, 1];
    const tasks = ["A", "B", "C", "D", "E", "F", "G"].map((key, at) => listed(key, { estimate_minutes: (days[at] ?? 1) * 480 }));
    const links = [link("A", "B"), link("A", "C"), link("B", "D"), link("C", "E"), link("D", "F"), link("E", "F"), link("C", "G")];
    const plan = criticalPath(tasks, links, 5, 8, "2026-11-23");
    expect(tasks.map((task) => plan.timing.get(task.key)?.slack_days)).toEqual([0, 0, 4, 0, 4, 0, 8]);
    expect(plan.timing.get("D")).toMatchObject({ earliest_start: "2026-12-02", earliest_finish: "2026-12-08", critical: true });
    expect(plan.path?.finish).toBe("2026-12-10");
    // A lag of five days between C and E makes the lower chain the longer one.
    const lagged = criticalPath(tasks, [...links.filter((each) => each.id !== "C>E"), link("C", "E", 5)], 5, 8, "2026-11-23");
    expect(["B", "C", "D", "E"].map((key) => lagged.timing.get(key)?.slack_days)).toEqual([1, 0, 1, 0]);
  });

  it("rolls parents up and lists tasks without dates or estimate", () => {
    const tasks = [listed("P"), listed("A", { parent: "P", start_date: "2026-11-23", due_date: "2026-11-25" }), listed("B", { parent: "P", start_date: "2026-11-30", due_date: "2026-12-04" }), listed("N")];
    const plan = criticalPath(tasks, [link("A", "B")], 5, 8, "2026-11-23");
    expect(plan.timing.get("A")?.slack_days).toBe(2);
    expect(plan.timing.get("P")).toMatchObject({ critical: true, earliest_start: "2026-11-23", latest_finish: "2026-12-04" });
    expect(plan.path?.left_out).toEqual([{ key: "N", reason: "no_dates_or_estimate" }]);
    expect(criticalPath(tasks, [link("A", "B")], 5, 8, "2026-11-23", "2026-12-11").timing.get("B")?.slack_days).toBe(5);
  });
});

const TODAY = new Date(2026, 10, 23, 9, 0);

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

describe("the sample baselines", () => {
  it("save the plan, show slip, new and removed tasks, answer queries and undo", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const project = "TM";
    const before = call<Schedule>(ws, "schedule.get", { project });
    const dated = before.tasks.filter((task) => task.shape === "bar" && !task.closed);
    const [moved, gone] = dated;
    expect(moved).toBeDefined();
    expect(gone).toBeDefined();
    if (!moved || !gone) {
      return;
    }
    const made = call<BaselineChange>(ws, "baseline.create", { project, name: "Plan A" });
    expect(made.baseline.tasks).toBeGreaterThan(1);
    expect(() => call(ws, "baseline.create", { project, name: "plan a" })).toThrow(FakeError);
    const due = moved.due_date ?? "";
    const later = new Date(Date.parse(`${due}T00:00:00Z`) + 4 * 86_400_000).toISOString().slice(0, 10);
    call(ws, "task.update", { key: moved.key, due_date: later });
    call(ws, "task.delete", { keys: [gone.key] });
    const added = call<{ task: { key: string } }>(ws, "task.create", { project, title: "Late addition", due_date: later }).task.key;
    const after = call<Schedule>(ws, "schedule.get", { project, baseline: made.baseline.id });
    const row = (key: string) => after.tasks.find((task) => task.key === key);
    expect(row(moved.key)?.baseline?.slip_days).toBe(4);
    expect(row(added)?.new_since_baseline).toBe(true);
    expect(after.baseline?.removed?.map((task) => [task.key, task.deleted])).toEqual([[gone.key, true]]);
    expect(call<TaskSearchResult>(ws, "task.search", { query: "slip>3d" }).items.map((task) => task.key)).toEqual([moved.key]);
    expect(call<TaskSearchResult>(ws, "task.search", { query: 'baseline:"Plan A" slip>3d' }).items.map((task) => task.key)).toEqual([moved.key]);
    const critical = call<TaskSearchResult>(ws, "task.search", { query: "is:critical project:TM" }).items.map((task) => task.key);
    expect(critical.length).toBeGreaterThan(0);
    const slack = call<TaskSearchResult>(ws, "task.search", { query: "slack<1d project:TM" }).items.map((task) => task.key);
    expect(slack.sort()).toEqual([...critical].sort());
    call(ws, "history.undo");
    call(ws, "history.undo");
    call(ws, "history.undo");
    call(ws, "history.undo");
    expect(call<BaselineList>(ws, "baseline.list", { project }).total).toBe(0);
  });
});
