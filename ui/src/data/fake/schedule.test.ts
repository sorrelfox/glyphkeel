// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { fakeTransport } from "./transport";
import { moveDependents, scheduleOf } from "./schedule";
import { type GanttLayoutChange, type ListedTask, type Schedule, type SchedulePreview, type TaskChange } from "../../generated/ops";

const TODAY = new Date("2026-11-11T10:00:00");

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

describe("the sample schedule", () => {
  it("lists the first 200 tasks without dates and counts them all", () => {
    const tasks = [listed("TM-1", { due_date: "2026-11-20" }), ...Array.from({ length: 250 }, (_, at) => listed(`TM-${String(at + 2)}`))];
    const schedule = scheduleOf(tasks, [], () => false, "2026-11-11");
    expect(schedule.tasks.map((task) => task.key)).toEqual(["TM-1"]);
    expect(schedule.unscheduled).toHaveLength(200);
    expect(schedule.unscheduled_total).toBe(250);
  });

  it("draws parents as summaries over their subtasks and keeps tasks without dates apart", () => {
    const tasks = [
      listed("TM-1"),
      listed("TM-2", { parent: "TM-1", start_date: "2026-11-02", due_date: "2026-11-05" }),
      listed("TM-3", { parent: "TM-1", due_date: "2026-11-20" }),
      listed("TM-4", { start_date: "2026-11-01" }),
      listed("TM-5"),
      listed("TM-6", { due_date: "2026-11-30", type: { id: "m", name: "Milestone", glyph: "milestone" } }),
    ];
    const schedule = scheduleOf(tasks, [{ id: "r1", from: "id-TM-2", to: "id-TM-4" }], (task) => task.type.glyph === "milestone", "2026-11-11");
    expect(schedule.tasks.map((task) => [task.key, task.depth, task.shape])).toEqual([
      ["TM-4", 0, "no_deadline"],
      ["TM-1", 0, "summary"],
      ["TM-2", 1, "bar"],
      ["TM-3", 1, "deadline"],
      ["TM-6", 0, "milestone"],
    ]);
    expect(schedule.tasks.find((task) => task.key === "TM-1")?.summary).toEqual({ start: "2026-11-02", end: "2026-11-20" });
    expect(schedule.unscheduled.map((task) => task.key)).toEqual(["TM-5"]);
    expect(schedule.dependencies).toEqual([{ id: "r1", from: "TM-2", to: "TM-4", violated: true }]);
    expect(schedule.tasks.find((task) => task.key === "TM-2")?.dependents).toBe(1);
    expect(schedule.problems.map((problem) => [problem.kind, problem.task])).toEqual([
      ["starts_before_blocker_ends", "TM-4"],
      ["no_due_date", "TM-4"],
      ["overdue", "TM-2"],
    ]);
    expect(schedule.range).toEqual({ start: "2026-11-01", end: "2026-11-30" });
  });

  it("moves dependents later as far as they must, and leaves closed ones", () => {
    const tasks = new Map([
      ["a", { start: "2026-11-01", due: "2026-11-10", closed: false }],
      ["b", { start: "2026-11-05", due: "2026-11-06", closed: false }],
      ["c", { start: "2026-11-07", due: "2026-11-08", closed: false }],
      ["d", { start: "2026-11-01", due: "2026-11-02", closed: true }],
    ]);
    const links = [
      { id: "1", from: "a", to: "b" },
      { id: "2", from: "b", to: "c" },
      { id: "3", from: "a", to: "d" },
    ];
    expect(moveDependents("a", tasks, links)).toEqual(["b", "c"]);
    expect(tasks.get("b")).toMatchObject({ start: "2026-11-11", due: "2026-11-12" });
    expect(tasks.get("c")).toMatchObject({ start: "2026-11-13", due: "2026-11-14" });
    expect(tasks.get("d")).toMatchObject({ start: "2026-11-01" });
  });

  it("answers schedule.get and moves dependents through task.update as one change", async () => {
    const transport = fakeTransport({ today: () => TODAY });
    const schedule = (await transport.call("schedule.get", { project: "TM" })) as Schedule;
    expect(schedule.today).toBe("2026-11-11");
    expect(schedule.tasks.length + schedule.unscheduled.length).toBeGreaterThan(0);
    const blocker = schedule.tasks.find((task) => task.dependents > 0 && task.due_date !== undefined && !task.closed);
    if (!blocker?.due_date) {
      return;
    }
    const later = new Date(Date.parse(`${blocker.due_date}T00:00:00Z`) + 60 * 86_400_000).toISOString().slice(0, 10);
    // The preview names exactly what the change then moves, and changes nothing.
    const preview = (await transport.call("schedule.preview_move", { key: blocker.key, ...(blocker.start_date === undefined ? {} : { start_date: blocker.start_date }), due_date: later })) as SchedulePreview;
    const unchanged = (await transport.call("schedule.get", { project: "TM" })) as Schedule;
    expect(unchanged.tasks.find((task) => task.key === blocker.key)?.due_date).toBe(blocker.due_date);
    const change = (await transport.call("task.update", { key: blocker.key, due_date: later, move_dependents: true })) as TaskChange;
    expect(change.moved?.length ?? 0).toBeGreaterThan(0);
    expect(preview.moved.map((task) => [task.key, task.due_date])).toEqual((change.moved ?? []).map((task) => [task.key, task.due_date]));
    await transport.call("history.undo", {});
    const again = (await transport.call("schedule.get", { project: "TM" })) as Schedule;
    expect(again.tasks.find((task) => task.key === blocker.key)?.due_date).toBe(blocker.due_date);
  });

  it("keeps the person's Gantt layout per project, as one change Undo takes back", async () => {
    const transport = fakeTransport({ today: () => TODAY });
    const empty = (await transport.call("gantt.layout.get", { project: "TM" })) as GanttLayoutChange;
    expect(empty.layout).toEqual({ zoom: "day", widths: {}, group_by: "parent", move_dependents: false });
    await transport.call("gantt.layout.update", { project: "TM", zoom: "week", widths: { key: 90 }, group_by: "label" });
    const kept = (await transport.call("gantt.layout.get", { project: "TM" })) as GanttLayoutChange;
    expect(kept.layout).toEqual({ zoom: "week", widths: { key: 90 }, group_by: "label", move_dependents: false });
    expect(((await transport.call("gantt.layout.get", { project: "PER" })) as GanttLayoutChange).layout.zoom).toBe("day");
    await expect(transport.call("gantt.layout.update", { project: "TM", widths: { key: 5 } })).rejects.toThrow();
    await transport.call("history.undo", {});
    expect(((await transport.call("gantt.layout.get", { project: "TM" })) as GanttLayoutChange).layout.zoom).toBe("day");
  });
});
