// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The critical path as the engine works it out, for the sample workspace:
// finish-to-start `blocks` links with lags, on working days (the first
// days of the week from Monday, as many as the workspace's days per week).
// Each task's length comes from its dates, else from its estimate; tasks
// with neither are left out. A parent rolls up from its subtasks.

import { type CriticalPath, type LeftOutTask, type ListedTask, type TaskTiming } from "../../generated/ops";
import { type BlocksLink } from "./schedule";

const DAY_MS = 86_400_000;
/** Days since 1970 of Monday 5 January 1970, which weeks count from. */
const MONDAY = 4;
const epochDays = (day: string) => Math.round(Date.parse(`${day}T00:00:00Z`) / DAY_MS);
const mod = (a: number, b: number) => ((a % b) + b) % b;

/** Working day numbers for a week of `perWeek` working days. */
export function workWeek(perWeek: number) {
  const d = Math.min(7, Math.max(1, Math.round(perWeek)));
  const begin = (day: string) => {
    const n = epochDays(day) - MONDAY;
    return Math.floor(n / 7) * d + Math.min(mod(n, 7), d);
  };
  const end = (day: string) => {
    const n = epochDays(day) - MONDAY;
    return Math.floor(n / 7) * d + Math.min(mod(n, 7), d - 1);
  };
  const day = (index: number) => {
    const days = MONDAY + Math.floor(index / d) * 7 + mod(index, d);
    return new Date(days * DAY_MS).toISOString().slice(0, 10);
  };
  return { perWeek: d, begin, end, day };
}

interface Times {
  es: number;
  ef: number;
  ls: number;
  lf: number;
}

export interface CriticalResult {
  timing: Map<string, TaskTiming>;
  path: CriticalPath | undefined;
}

/**
 * The critical path of `tasks` over the links between them; `target` is
 * the day to count back from (`YYYY-MM-DD`), else the plan's finish.
 */
export function criticalPath(tasks: readonly ListedTask[], links: readonly BlocksLink[], perWeek: number, hoursPerDay: number, today: string, target?: string): CriticalResult {
  const week = workWeek(perWeek);
  const byId = new Map(tasks.map((task) => [task.id, task]));
  const byKey = new Map(tasks.map((task) => [task.key, task]));
  const kids = new Map<string, ListedTask[]>();
  for (const task of tasks) {
    const parent = task.parent === undefined ? undefined : byKey.get(task.parent);
    if (parent) {
      kids.set(parent.id, [...(kids.get(parent.id) ?? []), task]);
    }
  }
  const leftOut: LeftOutTask[] = [];
  const work = new Map<string, { days: number; notBefore?: number }>();
  for (const task of tasks) {
    const first = task.start_date ?? task.due_date;
    const last = task.due_date ?? task.start_date;
    if (first !== undefined && last !== undefined) {
      work.set(task.id, { days: Math.max(1, week.end(last) - week.begin(first) + 1), notBefore: week.begin(first) });
    } else if ((task.estimate_minutes ?? 0) > 0) {
      const perDay = Math.max(1, hoursPerDay) * 60;
      work.set(task.id, { days: Math.max(1, Math.ceil((task.estimate_minutes ?? 0) / perDay)) });
    } else if (!kids.has(task.id) && task.closed_at === undefined) {
      leftOut.push({ key: task.key, reason: "no_dates_or_estimate" });
    }
  }
  const inside = links.filter((link) => work.has(link.from) && work.has(link.to));
  const out = new Map<string, BlocksLink[]>();
  const into = new Map<string, BlocksLink[]>();
  for (const link of inside) {
    out.set(link.from, [...(out.get(link.from) ?? []), link]);
    into.set(link.to, [...(into.get(link.to) ?? []), link]);
  }
  // Kahn's order.
  const waiting = new Map<string, number>([...work.keys()].map((id) => [id, 0]));
  for (const link of inside) {
    waiting.set(link.to, (waiting.get(link.to) ?? 0) + 1);
  }
  const ready = [...work.keys()].filter((id) => waiting.get(id) === 0);
  const order: string[] = [];
  for (let next = 0; next < ready.length; next += 1) {
    const at = ready[next] ?? "";
    order.push(at);
    for (const link of out.get(at) ?? []) {
      const left = (waiting.get(link.to) ?? 0) - 1;
      waiting.set(link.to, left);
      if (left === 0) {
        ready.push(link.to);
      }
    }
  }
  const placed = new Set(order);
  for (const id of work.keys()) {
    if (!placed.has(id)) {
      leftOut.push({ key: byId.get(id)?.key ?? id, reason: "dependency_loop" });
    }
  }
  const dated = tasks.map((task) => task.start_date ?? task.due_date).filter((day): day is string => day !== undefined).sort();
  const begin = week.begin(dated[0] ?? today);
  const times = new Map<string, Times>();
  for (const id of order) {
    const w = work.get(id) ?? { days: 1 };
    let es = w.notBefore ?? begin;
    for (const link of into.get(id) ?? []) {
      const before = times.get(link.from);
      if (before) {
        es = Math.max(es, before.ef + 1 + Math.max(0, link.lagDays ?? 0));
      }
    }
    times.set(id, { es, ef: es + w.days - 1, ls: 0, lf: 0 });
  }
  const finish = order.length === 0 ? undefined : Math.max(...order.map((id) => times.get(id)?.ef ?? 0));
  const end = target !== undefined ? week.end(target) : finish;
  if (end === undefined || finish === undefined) {
    return { timing: new Map(), path: undefined };
  }
  for (const id of [...order].reverse()) {
    const t = times.get(id);
    if (!t) {
      continue;
    }
    let lf = end;
    for (const link of out.get(id) ?? []) {
      const after = times.get(link.to);
      if (after) {
        lf = Math.min(lf, after.ls - 1 - Math.max(0, link.lagDays ?? 0));
      }
    }
    t.lf = lf;
    t.ls = lf - (t.ef - t.es);
  }
  const timingOf = (t: Times, slack = t.ls - t.es): TaskTiming => ({
    earliest_start: week.day(t.es),
    earliest_finish: week.day(t.ef),
    latest_start: week.day(t.ls),
    latest_finish: week.day(t.lf),
    slack_days: slack,
    critical: slack <= 0,
  });
  const timing = new Map<string, TaskTiming>();
  const below = (task: ListedTask): ListedTask[] => [task, ...(kids.get(task.id) ?? []).flatMap(below)];
  for (const task of tasks) {
    if (kids.has(task.id)) {
      const all = below(task)
        .map((each) => times.get(each.id))
        .filter((t): t is Times => t !== undefined);
      if (all.length > 0) {
        const slack = Math.min(...all.map((t) => t.ls - t.es));
        const rolled: Times = { es: Math.min(...all.map((t) => t.es)), ef: Math.max(...all.map((t) => t.ef)), ls: Math.min(...all.map((t) => t.ls)), lf: Math.max(...all.map((t) => t.lf)) };
        timing.set(task.key, timingOf(rolled, slack));
      }
    } else {
      const t = times.get(task.id);
      if (t) {
        timing.set(task.key, timingOf(t));
      }
    }
  }
  return {
    timing,
    path: { end: week.day(end), finish: week.day(finish), working_days_per_week: week.perWeek, tasks: [], ...(leftOut.length > 0 ? { left_out: leftOut.slice(0, 200) } : {}) },
  };
}
