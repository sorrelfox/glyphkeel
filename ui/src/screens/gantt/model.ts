// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart's rows and shapes, free of React: the schedule's tree as
// rows (with folded parents left out and the Unscheduled group last), where
// each task's shape lies on the timeline, what a drag or a key press does to
// its dates, and the path of a dependency's arrow.

import { type GanttGrouping, type Hue, type LabelRef, type Schedule, type ScheduleDependency, type ScheduleProblem, type ScheduledTask, type UnscheduledTask } from "../../generated/ops";
import { addDays, daysBetween, type Day } from "../calendar/model";

/** A row's height, in pixels. */
export const ROW = 32;

/** What a group row heads: the tasks without dates, or the tasks sharing a label, a status or an agent's claim. */
export type GroupHead =
  | { by: "unscheduled" }
  | { by: "label"; name: string | undefined; hue: Hue | undefined }
  | { by: "status"; name: string; hue: Hue | undefined }
  | { by: "claim"; agent: string | undefined; session: string | undefined };

export type Row =
  | { kind: "task"; task: ScheduledTask; level: number; hasKids: boolean; open: boolean }
  | { kind: "group"; id: string; head: GroupHead; count: number }
  | { kind: "unscheduled"; task: UnscheduledTask };

export interface Rows {
  rows: Row[];
  /** Each task's row, by key. */
  index: Map<string, number>;
}

/** What grouping by status or label needs to know beyond the tasks. */
export interface GroupOrder {
  /** The statuses' places on the board, by status ID; the others follow by category, then name. */
  statuses: ReadonlyMap<string, number>;
  /** Each status's colour, by ID, where known. */
  hues: ReadonlyMap<string, Hue>;
  /** Compares names in the person's language. */
  compare: (a: string, b: string) => number;
}

const CATEGORY_ORDER: Readonly<Record<string, number>> = { todo: 0, in_progress: 1, done: 2, cancelled: 3 };

/** A task's first label in alphabetical order, as the list groups by label. */
export function firstLabel(task: Pick<ScheduledTask, "labels">, compare: (a: string, b: string) => number): LabelRef | undefined {
  let first: LabelRef | undefined;
  for (const label of task.labels ?? []) {
    if (first === undefined || compare(label.name, first.name) < 0) {
      first = label;
    }
  }
  return first;
}

/** The dated tasks as one level, each a row of its own: for flat and grouped charts. */
function flat(tasks: readonly ScheduledTask[]): Row[] {
  return tasks.map((task) => ({ kind: "task", task, level: 1, hasKids: false, open: true }));
}

/** The dated tasks in groups by label, status or claim, each group headed by a row, in the group's order. */
function grouped(tasks: readonly ScheduledTask[], by: "label" | "status" | "claim", order: GroupOrder): Row[] {
  const groups = new Map<string, { head: GroupHead; tasks: ScheduledTask[]; rank: [number, number, string] }>();
  for (const task of tasks) {
    let id: string;
    let head: GroupHead;
    let rank: [number, number, string];
    if (by === "label") {
      const label = firstLabel(task, order.compare);
      id = label === undefined ? "" : label.name;
      head = { by: "label", name: label?.name, hue: label?.hue };
      rank = [label === undefined ? 1 : 0, 0, label?.name ?? ""];
    } else if (by === "claim") {
      // One group per agent session: two sessions of one agent are two groups.
      const claim = task.claim;
      id = claim?.session ?? "";
      head = { by: "claim", agent: claim?.agent, session: claim?.session };
      rank = [claim === undefined ? 1 : 0, 0, claim ? `${claim.agent}\u0000${claim.session}` : ""];
    } else {
      id = task.status.id;
      head = { by: "status", name: task.status.name, hue: order.hues.get(task.status.id) };
      rank = [order.statuses.get(task.status.id) ?? Number.MAX_SAFE_INTEGER, CATEGORY_ORDER[task.status.category] ?? 4, task.status.name];
    }
    const group = groups.get(id);
    if (group) {
      group.tasks.push(task);
    } else {
      groups.set(id, { head, tasks: [task], rank });
    }
  }
  const sorted = [...groups.entries()].sort(([, a], [, b]) => a.rank[0] - b.rank[0] || a.rank[1] - b.rank[1] || order.compare(a.rank[2], b.rank[2]));
  return sorted.flatMap(([id, group]) => [{ kind: "group", id: `${by}:${id}`, head: group.head, count: group.tasks.length } satisfies Row, ...flat(group.tasks)]);
}

/** The default order of groups: names in code-point order, statuses by category. */
export const PLAIN_ORDER: GroupOrder = { statuses: new Map(), hues: new Map(), compare: (a, b) => (a < b ? -1 : a > b ? 1 : 0) };

/**
 * The chart's rows: the schedule's tree in its order, without the subtasks
 * of folded tasks (grouped by parent, the default); or one flat list; or
 * groups by label, status or the agent session that claimed the task.
 * Then a group row and the tasks without dates.
 */
export function buildRows(
  tasks: readonly ScheduledTask[],
  unscheduled: readonly UnscheduledTask[],
  folded: ReadonlySet<string>,
  grouping: GanttGrouping = "parent",
  order: GroupOrder = PLAIN_ORDER,
): Rows {
  let rows: Row[] = [];
  if (grouping === "none") {
    rows = flat(tasks);
  } else if (grouping === "label" || grouping === "status" || grouping === "claim") {
    rows = grouped(tasks, grouping, order);
  } else {
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
  }
  if (unscheduled.length > 0) {
    rows.push({ kind: "group", id: "unscheduled", head: { by: "unscheduled" }, count: unscheduled.length });
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

/** How many percentage points expected progress may run ahead of actual progress before a bar is behind. */
export const BEHIND_TOLERANCE = 0.1;

/**
 * How far behind a bar is: the share done and the share of its time gone
 * by (to the middle of today, where the today line is), when the second
 * runs ahead of the first by more than ten percentage points and at least
 * one whole day of the bar has passed. Only open bars with both dates that
 * aren't overdue yet; an overdue bar has its own note.
 */
export function behindSchedule(task: ScheduledTask, today: Day): { done: number; expected: number } | undefined {
  const { start_date: start, due_date: due } = task;
  if (task.shape !== "bar" || task.closed || task.overdue || start === undefined || due === undefined || today > due) {
    return undefined;
  }
  // The days of the bar wholly gone: none on its first day.
  const gone = daysBetween(start, today);
  if (gone < 1) {
    return undefined;
  }
  const expected = (gone + 0.5) / (daysBetween(start, due) + 1);
  const done = task.progress_percent / 100;
  // Rounded to whole hundredths of a point, so a share such as 0.3 - 0.2 isn't counted as more than 0.1.
  return Math.round((expected - done) * 10_000) > BEHIND_TOLERANCE * 10_000 ? { done, expected } : undefined;
}

/** A milestone in the project's list of them. */
export interface Milestone {
  key: string;
  title: string;
  /** Its day: the due date, else the start; none when it has no date yet. */
  date: Day | undefined;
  /** Its status's name. */
  status: string;
  /** Closed; open and past its day; or open. */
  state: "done" | "overdue" | "open";
  /** For an overdue milestone, the days since its day. */
  late: number;
}

/**
 * A schedule's milestones, by their day, those without a day last; ties by
 * key in its numeric order.
 */
export function milestonesOf(schedule: Pick<Schedule, "tasks" | "unscheduled" | "today">): Milestone[] {
  const dated = schedule.tasks
    .filter((task) => task.shape === "milestone")
    .map((task): Milestone => {
      const date = task.due_date ?? task.start_date;
      const overdue = !task.closed && date !== undefined && date < schedule.today;
      return {
        key: task.key,
        title: task.title,
        date,
        status: task.status.name,
        state: task.closed ? "done" : overdue ? "overdue" : "open",
        late: overdue ? daysBetween(date, schedule.today) : 0,
      };
    });
  const undated = schedule.unscheduled
    .filter((task) => task.milestone)
    .map((task): Milestone => ({ key: task.key, title: task.title, date: undefined, status: task.status.name, state: task.closed ? "done" : "open", late: 0 }));
  const byKey = (a: Milestone, b: Milestone) => a.key.localeCompare(b.key, "en", { numeric: true });
  dated.sort((a, b) => (a.date ?? "").localeCompare(b.date ?? "") || byKey(a, b));
  undated.sort(byKey);
  return [...dated, ...undated];
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
