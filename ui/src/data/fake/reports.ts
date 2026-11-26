// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The flow reports of the in-memory engine: the same rules as the real one,
// on the fake's records, in the computer's time zone.

import {
  type Actor,
  type Category,
  type CreatedClosedReport,
  type CumulativeFlowReport,
  type CycleTimeReport,
  type CycleTimeTask,
  type FlowBand,
  type FlowBucket,
  type FlowGroupBy,
  type FlowStats,
  type Hue,
  type Settings,
} from "../../generated/ops";
import { addDays, csvOf, localDay, weekStart } from "./time";

export interface FlowStatus {
  id: string;
  name: string;
  category: Category;
  hue: Hue;
  /** Its place on its project's board. */
  rank: number;
}

/** One event of a task's history: a move into a status, or into or out of the trash. */
export interface FlowEvent {
  at: number;
  status?: string;
  trashed?: boolean;
  actor?: Actor;
}

/** A task in scope, with what the history knows of it. */
export interface FlowTask {
  key: string;
  title: string;
  created: number;
  createdBy: Actor;
  /** Its status now, and since when. */
  status: string;
  entered: number;
  /** The first open status of its project, for tasks without history. */
  firstOpen: string | undefined;
  events: FlowEvent[];
}

export interface FlowInput {
  from: string;
  to: string;
  bucket: FlowBucket;
  actor?: string | undefined;
}

interface State {
  at: number;
  status: string | undefined;
  live: boolean;
}

interface Replayed {
  task: FlowTask;
  states: State[];
  closes: { at: number; actor: Actor | undefined }[];
  reopens: { at: number; actor: Actor | undefined }[];
  firstProgress: number | undefined;
}

const DAY_MS = 86_400_000;
const CATEGORIES: readonly Category[] = ["todo", "in_progress", "done", "cancelled"];

const closedCategory = (category: Category | undefined) => category === "done" || category === "cancelled";

/** Local midnight of `day`. */
function midnight(day: string): number {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1).getTime();
}

/** The periods from `from` to `to`, the first and last cut to the range. */
export function flowPeriods(from: string, to: string, bucket: FlowBucket, first: Settings["week_start"]): { start: string; end: string }[] {
  const out: { start: string; end: string }[] = [];
  let start = bucket === "week" ? weekStart(from, first) : bucket === "month" ? `${from.slice(0, 7)}-01` : from;
  while (start <= to && out.length <= 1000) {
    let next: string;
    if (bucket === "day") {
      next = addDays(start, 1);
    } else if (bucket === "week") {
      next = addDays(start, 7);
    } else {
      const [y, m] = start.split("-").map(Number);
      next = localDay(new Date(y ?? 1970, m ?? 1, 1));
    }
    const end = addDays(next, -1);
    out.push({ start: start < from ? from : start, end: end > to ? to : end });
    start = next;
  }
  return out;
}

/** The default first day: twelve periods back from `to`. */
export function defaultFrom(to: string, bucket: FlowBucket, first: Settings["week_start"]): string {
  if (bucket === "day") {
    return addDays(to, -29);
  }
  if (bucket === "week") {
    return addDays(weekStart(to, first), -77);
  }
  const [y, m] = to.split("-").map(Number);
  return localDay(new Date(y ?? 1970, (m ?? 1) - 12, 1));
}

function replay(task: FlowTask, category: (status: string) => Category | undefined): Replayed {
  const events = [...task.events];
  const lastStatus = [...events].reverse().find((event) => event.status !== undefined)?.status;
  if (lastStatus === undefined && task.entered > task.created + 1000 && task.firstOpen !== undefined && task.firstOpen !== task.status) {
    events.unshift({ at: task.created, status: task.firstOpen });
  }
  if (lastStatus !== task.status) {
    events.push({ at: Math.max(task.entered, events.at(-1)?.at ?? task.created), status: task.status });
  }
  const out: Replayed = { task, states: [{ at: task.created, status: undefined, live: true }], closes: [], reopens: [], firstProgress: undefined };
  let current: State = { at: task.created, status: undefined, live: true };
  for (const event of events) {
    const at = Math.max(event.at, task.created, current.at);
    const next: State = { ...current, at };
    if (event.status !== undefined) {
      const was = current.status === undefined ? undefined : category(current.status);
      const now = category(event.status);
      if (current.live && closedCategory(now) && !closedCategory(was)) {
        out.closes.push({ at, actor: event.actor });
      } else if (current.live && !closedCategory(now) && closedCategory(was)) {
        out.reopens.push({ at, actor: event.actor });
      }
      if (now === "in_progress" && out.firstProgress === undefined) {
        out.firstProgress = at;
      }
      next.status = event.status;
    }
    if (event.trashed !== undefined) {
      next.live = !event.trashed;
    }
    const last = out.states.at(-1);
    if (last && last.at === at) {
      out.states[out.states.length - 1] = next;
    } else {
      out.states.push(next);
    }
    current = next;
  }
  return out;
}

function stateAt(r: Replayed, at: number): State | undefined {
  let found: State | undefined;
  for (const state of r.states) {
    if (state.at > at) {
      break;
    }
    found = state;
  }
  return found;
}

function by(who: string | undefined, actor: Actor | undefined): boolean {
  const w = who?.trim().toLowerCase();
  if (w === undefined) {
    return true;
  }
  if (actor === undefined) {
    return false;
  }
  return w === "me" ? actor.kind === "user" : w === "agent" ? actor.kind === "agent" : actor.name.toLowerCase() === w;
}

const round2 = (x: number) => Math.round(x * 100) / 100;

function percentile(sorted: readonly number[], p: number): number | undefined {
  if (sorted.length === 0) {
    return undefined;
  }
  const rank = (p / 100) * (sorted.length - 1);
  const lo = sorted[Math.floor(rank)] ?? 0;
  const hi = sorted[Math.ceil(rank)] ?? lo;
  return lo + (hi - lo) * (rank - Math.floor(rank));
}

export function flowStats(values: readonly number[]): FlowStats {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) {
    return { count: 0 };
  }
  const pick = (p: number) => round2(percentile(sorted, p) ?? 0);
  return {
    count: sorted.length,
    mean: round2(sorted.reduce((sum, x) => sum + x, 0) / sorted.length),
    p50: pick(50),
    p85: pick(85),
    p95: pick(95),
    min: round2(sorted[0] ?? 0),
    max: round2(sorted.at(-1) ?? 0),
  };
}

/** Everything the three reports share. */
function prepare(tasks: readonly FlowTask[], statuses: ReadonlyMap<string, FlowStatus>, input: FlowInput, first: Settings["week_start"]) {
  const periods = flowPeriods(input.from, input.to, input.bucket, first);
  const bounds = periods.map((p) => [midnight(p.start), midnight(addDays(p.end, 1))] as const);
  const category = (status: string) => statuses.get(status)?.category;
  const replays = tasks.map((task) => replay(task, category));
  const start = bounds[0]?.[0] ?? 0;
  const end = bounds.at(-1)?.[1] ?? 0;
  const periodOf = (at: number) => (at < start || at >= end ? -1 : bounds.findIndex(([, e]) => at < e));
  const openAt = (r: Replayed, at: number) => {
    const s = stateAt(r, at);
    return s !== undefined && s.live && s.status !== undefined && !closedCategory(category(s.status));
  };
  return { periods, bounds, replays, periodOf, openAt, start, category };
}

function bump(row: Record<"created" | "closed" | "reopened" | "open", number> | undefined, field: "created" | "closed" | "reopened" | "open"): void {
  if (row) {
    row[field] += 1;
  }
}

export function createdClosed(tasks: readonly FlowTask[], statuses: ReadonlyMap<string, FlowStatus>, input: FlowInput, first: Settings["week_start"], csv: boolean): CreatedClosedReport {
  const { periods, bounds, replays, periodOf, openAt, start } = prepare(tasks, statuses, input, first);
  const rows = periods.map((p) => ({ start: p.start, end: p.end, created: 0, closed: 0, reopened: 0, open: 0 }));
  let openBefore = 0;
  for (const r of replays) {
    const made = periodOf(r.task.created);
    if (made >= 0 && by(input.actor, r.task.createdBy)) {
      bump(rows[made], "created");
    }
    for (const close of r.closes) {
      const p = periodOf(close.at);
      if (p >= 0 && by(input.actor, close.actor)) {
        bump(rows[p], "closed");
      }
    }
    for (const reopen of r.reopens) {
      const p = periodOf(reopen.at);
      if (p >= 0 && by(input.actor, reopen.actor)) {
        bump(rows[p], "reopened");
      }
    }
    if (openAt(r, start - 1)) {
      openBefore += 1;
    }
    bounds.forEach(([, end], i) => {
      if (openAt(r, end - 1)) {
        bump(rows[i], "open");
      }
    });
  }
  const sum = (field: "created" | "closed" | "reopened") => rows.reduce((total, row) => total + row[field], 0);
  return {
    bucket: input.bucket,
    closed: sum("closed"),
    created: sum("created"),
    ...(csv
      ? { csv: csvOf(["start", "end", "created", "closed", "reopened", "open"], rows.map((row) => [row.start, row.end, String(row.created), String(row.closed), String(row.reopened), String(row.open)])) }
      : {}),
    from: input.from,
    items: rows,
    open_before: openBefore,
    reopened: sum("reopened"),
    to: input.to,
  };
}

export function cumulativeFlow(
  tasks: readonly FlowTask[],
  statuses: ReadonlyMap<string, FlowStatus>,
  scoped: ReadonlySet<string>,
  input: FlowInput,
  groupBy: FlowGroupBy,
  first: Settings["week_start"],
  csv: boolean,
): CumulativeFlowReport {
  const { periods, bounds, replays } = prepare(tasks, statuses, input, first);
  const bands: FlowBand[] = [];
  const bandOf = new Map<string, number>();
  const keys = new Map<string, number>();
  if (groupBy === "category") {
    for (const c of CATEGORIES) {
      keys.set(c, bands.length);
      bands.push({ key: c, name: c, category: c });
    }
  }
  const sorted = [...statuses.values()].sort((a, b) => CATEGORIES.indexOf(a.category) - CATEGORIES.indexOf(b.category) || a.rank - b.rank || a.name.localeCompare(b.name));
  for (const s of sorted) {
    const key = groupBy === "category" ? s.category : s.name.toLowerCase();
    let band = keys.get(key);
    if (band === undefined) {
      band = bands.length;
      keys.set(key, band);
      bands.push({ key, name: s.name, category: s.category, hue: s.hue });
    }
    bandOf.set(s.id, band);
  }
  const counts = periods.map(() => bands.map(() => 0));
  for (const r of replays) {
    bounds.forEach(([, end], i) => {
      const s = stateAt(r, end - 1);
      const band = s?.status === undefined ? undefined : bandOf.get(s.status);
      if (s?.live === true && band !== undefined) {
        const row = counts[i];
        if (row) {
          row[band] = (row[band] ?? 0) + 1;
        }
      }
    });
  }
  const keep = bands.map((_band, b) => groupBy === "category" || counts.some((row) => (row[b] ?? 0) > 0) || [...scoped].some((id) => bandOf.get(id) === b));
  const kept = keep.flatMap((kept, b) => (kept ? [b] : []));
  const items = periods.map((p, i) => ({ start: p.start, end: p.end, counts: kept.map((b) => counts[i]?.[b] ?? 0) }));
  const keptBands = kept.flatMap((b) => (bands[b] ? [bands[b]] : []));
  return {
    bands: keptBands,
    bucket: input.bucket,
    ...(csv ? { csv: csvOf(["start", "end", ...keptBands.map((b) => b.name)], items.map((row) => [row.start, row.end, ...row.counts.map(String)])) } : {}),
    from: input.from,
    group_by: groupBy,
    items,
    to: input.to,
  };
}

export function cycleTime(tasks: readonly FlowTask[], statuses: ReadonlyMap<string, FlowStatus>, input: FlowInput, first: Settings["week_start"], withTasks: boolean, csv: boolean): CycleTimeReport {
  const { periods, replays, periodOf, category } = prepare(tasks, statuses, input, first);
  const cycle = periods.map((): number[] => []);
  const lead = periods.map((): number[] => []);
  const listed: CycleTimeTask[] = [];
  for (const r of replays) {
    const last = r.states.at(-1);
    const close = r.closes.at(-1);
    if (!last?.live || last.status === undefined || !closedCategory(category(last.status)) || close === undefined || !by(input.actor, close.actor)) {
      continue;
    }
    const p = periodOf(close.at);
    if (p < 0) {
      continue;
    }
    const leadDays = (close.at - r.task.created) / DAY_MS;
    const cycleDays = r.firstProgress !== undefined && r.firstProgress <= close.at ? (close.at - r.firstProgress) / DAY_MS : undefined;
    lead[p]?.push(leadDays);
    if (cycleDays !== undefined) {
      cycle[p]?.push(cycleDays);
    }
    listed.push({
      key: r.task.key,
      title: r.task.title,
      closed_at: new Date(close.at).toISOString(),
      closed_day: localDay(new Date(close.at)),
      ...(cycleDays === undefined ? {} : { cycle_days: round2(cycleDays) }),
      lead_days: round2(leadDays),
      reopened: r.reopens.filter((reopen) => reopen.at <= close.at).length,
    });
  }
  listed.sort((a, b) => a.closed_at.localeCompare(b.closed_at) || a.key.localeCompare(b.key));
  return {
    bucket: input.bucket,
    ...(csv
      ? {
          csv: csvOf(
            ["key", "title", "closed_at", "closed_day", "cycle_days", "lead_days", "reopened"],
            listed.map((t) => [t.key, t.title, t.closed_at, t.closed_day, t.cycle_days === undefined ? "" : String(t.cycle_days), String(t.lead_days), String(t.reopened)]),
          ),
        }
      : {}),
    cycle: flowStats(cycle.flat()),
    from: input.from,
    items: periods.map((p, i) => ({ start: p.start, end: p.end, cycle: flowStats(cycle[i] ?? []), lead: flowStats(lead[i] ?? []) })),
    lead: flowStats(lead.flat()),
    ...(withTasks ? { tasks: listed } : {}),
    to: input.to,
  };
}
