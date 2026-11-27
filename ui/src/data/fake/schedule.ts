// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The sample workspace's schedule, as the engine works it out: the tasks
// with dates as a tree with summary ranges, milestones, the tasks without
// dates, `blocks` links as finish-to-start dependencies with their
// problems, and "also move dependents".

import {
  type ListedTask,
  type Schedule,
  type ScheduleDependency,
  type ScheduleProblem,
  type ScheduleShape,
  type ScheduledTask,
  type UnscheduledTask,
} from "../../generated/ops";

/** The most tasks without dates listed, as the engine lists them; the rest are only counted. */
export const MAX_UNSCHEDULED = 200;

/** A `blocks` link, by task ID. */
export interface BlocksLink {
  id: string;
  from: string;
  to: string;
  /** Days that must pass after the blocker ends; none when left out. */
  lagDays?: number | undefined;
}

const DAY_MS = 86_400_000;
const utc = (day: string) => Date.parse(`${day}T00:00:00Z`);
const addDays = (day: string, days: number) => new Date(utc(day) + days * DAY_MS).toISOString().slice(0, 10);

interface Dates {
  start: string | undefined;
  due: string | undefined;
}
const begin = (d: Dates) => d.start ?? d.due;
const end = (d: Dates) => d.due ?? d.start;

/** The day after the blocker ends (and its lag), when the blocked task starts too soon. */
function tooEarly(blocker: Dates, blocked: Dates, lag = 0): { begin: string; end: string; earliest: string } | undefined {
  const e = end(blocker);
  const b = begin(blocked);
  if (e === undefined || b === undefined) {
    return undefined;
  }
  const earliest = addDays(e, 1 + Math.max(0, lag));
  return b < earliest ? { begin: b, end: e, earliest } : undefined;
}

function problem(kind: ScheduleProblem["kind"], task: string, id: string, message: string, args: Record<string, string>, other?: string): ScheduleProblem {
  return { kind, task, message_id: id, message, args, ...(other === undefined ? {} : { other }) };
}

/** The tasks that depend on `id` through `blocks` links, directly or through others. */
function dependentsOf(id: string, links: readonly BlocksLink[]): Set<string> {
  const seen = new Set<string>();
  const queue = [id];
  while (queue.length > 0) {
    const at = queue.shift() ?? "";
    for (const link of links) {
      if (link.from === at && !seen.has(link.to) && link.to !== id) {
        seen.add(link.to);
        queue.push(link.to);
      }
    }
  }
  return seen;
}

/**
 * The schedule of `tasks` (already narrowed to the project and the query).
 * `milestone` tells the tasks of a milestone type.
 */
export function scheduleOf(tasks: readonly ListedTask[], links: readonly BlocksLink[], milestone: (task: ListedTask) => boolean, today: string, archived: ReadonlySet<string> = new Set()): Schedule {
  const readOnly = (task: ListedTask) => (archived.has(task.project) ? { read_only: "archived_project" as const } : {});
  const byKey = new Map(tasks.map((task) => [task.key, task]));
  const kids = new Map<string, ListedTask[]>();
  for (const task of tasks) {
    if (task.parent !== undefined && byKey.has(task.parent)) {
      const list = kids.get(task.parent) ?? [];
      list.push(task);
      kids.set(task.parent, list);
    }
  }
  const dated = (task: ListedTask): boolean => task.start_date !== undefined || task.due_date !== undefined || (kids.get(task.key) ?? []).some(dated);
  // By start date, then due date; a parent without dates of its own by its summary's.
  const order = (a: ListedTask, b: ListedTask) => {
    const span = (t: ListedTask): Dates => {
      if (t.start_date !== undefined || t.due_date !== undefined) {
        return { start: t.start_date, due: t.due_date };
      }
      const range = summaryOf(t);
      return { start: range?.start, due: range?.end };
    };
    const da = span(a);
    const db = span(b);
    return (begin(da) ?? "9999").localeCompare(begin(db) ?? "9999") || (end(da) ?? "9999").localeCompare(end(db) ?? "9999") || a.key.localeCompare(b.key, "en", { numeric: true });
  };
  const summaryOf = (task: ListedTask): { start: string; end: string } | undefined => {
    let lo: string | undefined;
    let hi: string | undefined;
    const walk = (t: ListedTask) => {
      const d: Dates = { start: t.start_date, due: t.due_date };
      const b = begin(d);
      const e = end(d);
      if (b !== undefined && e !== undefined) {
        lo = lo === undefined || b < lo ? b : lo;
        hi = hi === undefined || e > hi ? e : hi;
      }
      for (const kid of kids.get(t.key) ?? []) {
        walk(kid);
      }
    };
    walk(task);
    return lo !== undefined && hi !== undefined ? { start: lo, end: hi } : undefined;
  };
  const idOf = new Map(tasks.map((task) => [task.id, task]));
  const rows: ScheduledTask[] = [];
  const add = (task: ListedTask, depth: number) => {
    const children = (kids.get(task.key) ?? []).filter(dated).sort(order);
    const closed = task.closed_at !== undefined;
    let shape: ScheduleShape;
    if (milestone(task)) {
      shape = "milestone";
    } else if (children.length > 0) {
      shape = "summary";
    } else if (task.start_date !== undefined && task.due_date !== undefined) {
      shape = "bar";
    } else if (task.due_date !== undefined) {
      shape = "deadline";
    } else {
      shape = "no_deadline";
    }
    const summary = children.length > 0 ? summaryOf(task) : undefined;
    rows.push({
      blocked: links.some((link) => link.to === task.id && idOf.get(link.from)?.closed_at === undefined && idOf.has(link.from)),
      closed,
      dependents: dependentsOf(task.id, links).size,
      depth,
      ...(task.due_date === undefined ? {} : { due_date: task.due_date }),
      id: task.id,
      key: task.key,
      ...(task.labels.length > 0 ? { labels: task.labels } : {}),
      ...readOnly(task),
      ...(task.claim ? { claim: task.claim } : {}),
      overdue: !closed && task.due_date !== undefined && task.due_date < today,
      ...(task.parent === undefined || !byKey.has(task.parent) ? {} : { parent: task.parent }),
      progress_percent: task.progress_percent,
      project: task.project,
      shape,
      ...(task.start_date === undefined ? {} : { start_date: task.start_date }),
      status: task.status,
      ...(summary === undefined ? {} : { summary }),
      ...(task.recurring === true ? { recurring: true } : {}),
      title: task.title,
      version: task.version,
      ...(task.fields && Object.keys(task.fields).length > 0 ? { fields: task.fields } : {}),
    });
    for (const child of children) {
      add(child, depth + 1);
    }
  };
  const tops = tasks.filter((task) => (task.parent === undefined || !byKey.has(task.parent) || !dated(byKey.get(task.parent) as ListedTask)) && dated(task)).sort(order);
  for (const task of tops) {
    add(task, 0);
  }

  const shownIds = new Map(rows.map((row) => [row.id, row]));
  const dependencies: ScheduleDependency[] = [];
  const problems: ScheduleProblem[] = [];
  for (const link of links) {
    const from = shownIds.get(link.from);
    const to = shownIds.get(link.to);
    if (!from || !to) {
      continue;
    }
    const early = tooEarly({ start: from.start_date, due: from.due_date }, { start: to.start_date, due: to.due_date }, link.lagDays);
    dependencies.push({ id: link.id, from: from.key, to: to.key, ...(link.lagDays ? { lag_days: link.lagDays } : {}), violated: early !== undefined });
    if (early) {
      problems.push(
        problem("starts_before_blocker_ends", to.key, "schedule-starts-before-blocker", `${to.key} starts on ${early.begin}, before ${from.key} ends on ${early.end}.`, { task: to.key, start: early.begin, blocker: from.key, end: early.end }, from.key),
      );
    }
  }
  for (const row of rows) {
    if (row.closed) {
      continue;
    }
    if (row.due_date !== undefined && row.due_date < today) {
      problems.push(problem("overdue", row.key, "schedule-overdue", `${row.key} was due on ${row.due_date}.`, { task: row.key, due: row.due_date }));
    }
    if (row.start_date !== undefined && row.due_date === undefined && row.shape !== "milestone") {
      problems.push(problem("no_due_date", row.key, "schedule-no-due-date", `${row.key} starts on ${row.start_date} but has no due date.`, { task: row.key, start: row.start_date }));
    }
  }

  const undated = tasks.filter((task) => !dated(task));
  const unscheduled: UnscheduledTask[] = undated
    .slice(0, MAX_UNSCHEDULED)
    .map((task) => ({
      closed: task.closed_at !== undefined,
      id: task.id,
      key: task.key,
      ...(task.labels.length > 0 ? { labels: task.labels } : {}),
      ...readOnly(task),
      ...(task.claim ? { claim: task.claim } : {}),
      milestone: milestone(task),
      ...(task.parent === undefined ? {} : { parent: task.parent }),
      project: task.project,
      status: task.status,
      title: task.title,
      version: task.version,
      ...(task.fields && Object.keys(task.fields).length > 0 ? { fields: task.fields } : {}),
    }));

  let lo: string | undefined;
  let hi: string | undefined;
  for (const row of rows) {
    const d: Dates = { start: row.summary?.start ?? row.start_date, due: row.summary?.end ?? row.due_date };
    const b = begin(d);
    const e = end(d);
    if (b !== undefined && (lo === undefined || b < lo)) {
      lo = b;
    }
    if (e !== undefined && (hi === undefined || e > hi)) {
      hi = e;
    }
  }
  return { today, ...(lo !== undefined && hi !== undefined ? { range: { start: lo, end: hi } } : {}), tasks: rows, unscheduled, unscheduled_total: undated.length, dependencies, problems };
}

/** A task's dates, as "also move dependents" reads and writes them. */
export interface Movable {
  start: string | null;
  due: string | null;
  closed: boolean;
}

/**
 * Moves the tasks that depend on `id` later, as far as each must to begin
 * after its blockers end, keeping their lengths; closed tasks stay. Returns
 * the IDs moved, blockers first.
 */
export function moveDependents(id: string, tasks: ReadonlyMap<string, Movable>, links: readonly BlocksLink[]): string[] {
  const moved: string[] = [];
  const queue = [id];
  const guard = new Map<string, number>();
  while (queue.length > 0) {
    const at = queue.shift() ?? "";
    const blocker = tasks.get(at);
    if (!blocker) {
      continue;
    }
    for (const link of links) {
      if (link.from !== at) {
        continue;
      }
      const task = tasks.get(link.to);
      if (!task || task.closed || link.to === id) {
        continue;
      }
      const early = tooEarly({ start: blocker.start ?? undefined, due: blocker.due ?? undefined }, { start: task.start ?? undefined, due: task.due ?? undefined }, link.lagDays);
      if (!early) {
        continue;
      }
      const days = Math.round((utc(early.earliest) - utc(early.begin)) / DAY_MS);
      task.start = task.start === null ? null : addDays(task.start, days);
      task.due = task.due === null ? null : addDays(task.due, days);
      if (!moved.includes(link.to)) {
        moved.push(link.to);
      }
      const seen = (guard.get(link.to) ?? 0) + 1;
      guard.set(link.to, seen);
      if (seen < 50) {
        queue.push(link.to);
      }
    }
  }
  return moved;
}
