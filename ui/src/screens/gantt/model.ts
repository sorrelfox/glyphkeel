// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart's rows and shapes, free of React: the schedule's tree as
// rows (with folded parents left out and the Unscheduled group last), where
// each task's shape lies on the timeline, what a drag or a key press does to
// its dates, and the path of a dependency's arrow.

import { type ScheduleDependency, type ScheduleProblem, type ScheduledTask, type UnscheduledTask } from "../../generated/ops";
import { addDays, daysBetween, type Day } from "../calendar/model";

/** A row's height, in pixels. */
export const ROW = 32;

export type Row =
  | { kind: "task"; task: ScheduledTask; level: number; hasKids: boolean; open: boolean }
  | { kind: "group"; count: number }
  | { kind: "unscheduled"; task: UnscheduledTask };

export interface Rows {
  rows: Row[];
  /** Each task's row, by key. */
  index: Map<string, number>;
}

/**
 * The chart's rows: the schedule's tree in its order, without the subtasks
 * of folded tasks, then a group row and the tasks without dates.
 */
export function buildRows(tasks: readonly ScheduledTask[], unscheduled: readonly UnscheduledTask[], folded: ReadonlySet<string>): Rows {
  const rows: Row[] = [];
  let hideBelow: number | null = null;
  tasks.forEach((task, at) => {
    if (hideBelow !== null) {
      if (task.depth > hideBelow) {
        return;
      }
      hideBelow = null;
    }
    const hasKids = (tasks[at + 1]?.depth ?? -1) > task.depth;
    const open = !folded.has(task.key);
    rows.push({ kind: "task", task, level: task.depth + 1, hasKids, open });
    if (hasKids && !open) {
      hideBelow = task.depth;
    }
  });
  if (unscheduled.length > 0) {
    rows.push({ kind: "group", count: unscheduled.length });
    for (const task of unscheduled) {
      rows.push({ kind: "unscheduled", task });
    }
  }
  const index = new Map<string, number>();
  rows.forEach((row, at) => {
    if (row.kind !== "group") {
      index.set(row.task.key, at);
    }
  });
  return { rows, index };
}

/** A task's own dates. */
export interface Dates {
  start: Day | undefined;
  due: Day | undefined;
}

export function datesOf(task: ScheduledTask): Dates {
  return { start: task.start_date, due: task.due_date };
}

export type Shape = ScheduledTask["shape"];

export interface Geometry {
  shape: Shape;
  /** The first day drawn and the day after the last, as logical pixels. */
  x1: number;
  x2: number;
}

/** Where a task lies on the timeline, with `dates` in place of its own while it is dragged. */
export function geometry(task: ScheduledTask, origin: Day, ppd: number, dates: Dates = datesOf(task)): Geometry | undefined {
  let first: Day | undefined;
  let last: Day | undefined;
  if (task.shape === "summary") {
    first = task.summary?.start ?? dates.start ?? dates.due;
    last = task.summary?.end ?? dates.due ?? dates.start;
  } else if (task.shape === "milestone") {
    first = dates.due ?? dates.start;
    last = first;
  } else {
    first = dates.start ?? dates.due;
    last = dates.due ?? dates.start;
  }
  if (first === undefined || last === undefined) {
    return undefined;
  }
  const x1 = daysBetween(origin, first) * ppd;
  const x2 = Math.max(x1 + 2, (daysBetween(origin, last) + 1) * ppd);
  return { shape: task.shape, x1, x2 };
}

/** What a drag changes: both dates, the start, or the due date. */
export type DragMode = "move" | "start" | "end";

/**
 * A task's dates after a drag of `days` days. A start never passes the due
 * date and a due date never comes before the start. Moving a milestone or a
 * deadline moves its due date; dragging a deadline's left side gives it a
 * start; dragging a start-only bar's end gives it a due date.
 */
export function draggedDates(task: Dates, mode: DragMode, days: number): Dates {
  const { start, due } = task;
  switch (mode) {
    case "move":
      return { start: start && addDays(start, days), due: due && addDays(due, days) };
    case "start": {
      const from = start ?? due;
      if (from === undefined) {
        return task;
      }
      const moved = addDays(from, days);
      return { start: due !== undefined && moved > due ? due : moved, due };
    }
    case "end": {
      const from = due ?? start;
      if (from === undefined) {
        return task;
      }
      const moved = addDays(from, days);
      return { start, due: start !== undefined && moved < start ? start : moved };
    }
  }
}

/** Whether two sets of dates are the same. */
export function sameDates(a: Dates, b: Dates): boolean {
  return a.start === b.start && a.due === b.due;
}

/** The fields of `task.update` that set `next` in place of `before`; none when nothing changes. */
export function datesUpdate(before: Dates, next: Dates): { start_date?: string; due_date?: string; clear?: ("start_date" | "due_date")[] } | undefined {
  const update: { start_date?: string; due_date?: string; clear?: ("start_date" | "due_date")[] } = {};
  const clear: ("start_date" | "due_date")[] = [];
  if (next.start !== before.start) {
    if (next.start === undefined) {
      clear.push("start_date");
    } else {
      update.start_date = next.start;
    }
  }
  if (next.due !== before.due) {
    if (next.due === undefined) {
      clear.push("due_date");
    } else {
      update.due_date = next.due;
    }
  }
  if (clear.length > 0) {
    update.clear = clear;
  }
  return Object.keys(update).length === 0 ? undefined : update;
}

/** Whether a change moves the task's end later, which is when "also move dependents" can move others. */
export function endsLater(before: Dates, next: Dates): boolean {
  const was = before.due ?? before.start;
  const now = next.due ?? next.start;
  return was !== undefined && now !== undefined && now > was;
}

/** The schedule's problems, by the key of the task they are about. */
export function problemsByTask(problems: readonly ScheduleProblem[]): Map<string, ScheduleProblem[]> {
  const byTask = new Map<string, ScheduleProblem[]>();
  for (const problem of problems) {
    const list = byTask.get(problem.task);
    if (list) {
      list.push(problem);
    } else {
      byTask.set(problem.task, [problem]);
    }
  }
  return byTask;
}

/** The problems a warning sign marks in the table; overdue and no due date show as notes on the bar. */
export function isWarning(problem: ScheduleProblem): boolean {
  return problem.kind === "starts_before_blocker_ends" || problem.kind === "dependency_loop" || problem.kind === "outside_parent";
}

/** The dependencies with at least one end among the rows `[r0, r1)`, or crossing them. */
export function visibleDependencies(dependencies: readonly ScheduleDependency[], index: ReadonlyMap<string, number>, r0: number, r1: number): ScheduleDependency[] {
  return dependencies.filter((dependency) => {
    const a = index.get(dependency.from);
    const b = index.get(dependency.to);
    return a !== undefined && b !== undefined && Math.max(a, b) >= r0 && Math.min(a, b) < r1;
  });
}

/**
 * The elbow line of a finish-to-start dependency, from the end of one bar
 * (`xa`, `ya`) to the start of another (`xb`, `yb`). When the second bar
 * starts too soon, the line goes round through the gap between the rows.
 */
export function elbow(xa: number, ya: number, xb: number, yb: number): string {
  if (xb - xa >= 16) {
    const turn = xa + 8;
    return `M${String(xa)},${String(ya)} H${String(turn)} V${String(yb)} H${String(xb)}`;
  }
  const gap = ya + (yb > ya ? ROW / 2 : -ROW / 2);
  return `M${String(xa)},${String(ya)} h8 V${String(gap)} H${String(xb - 8)} V${String(yb)} H${String(xb)}`;
}

/** Whether a dependency already runs from `from` to `to`. */
export function linkBetween(dependencies: readonly ScheduleDependency[], from: string, to: string): ScheduleDependency | undefined {
  return dependencies.find((dependency) => dependency.from === from && dependency.to === to);
}

/** The rows a window of `height` pixels scrolled to `top` shows, in chunks of `chunk` rows with `overscan` rows around. */
export function rowWindow(top: number, height: number, count: number, overscan = 8, chunk = 8): [number, number] {
  const r0 = Math.max(0, Math.floor((top / ROW - overscan) / chunk) * chunk);
  const r1 = Math.min(count, Math.ceil(((top + height) / ROW + overscan) / chunk) * chunk);
  return [r0, r1];
}

/** The logical pixels of timeline to draw for a view scrolled to `scroll`, in chunks of `chunk` pixels. */
export function columnWindow(scroll: number, width: number, chunk = 512): [number, number] {
  const x0 = Math.max(0, Math.floor((scroll - chunk) / chunk) * chunk);
  const x1 = Math.ceil((scroll + width + chunk) / chunk) * chunk;
  return [x0, x1];
}
