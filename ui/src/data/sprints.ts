// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Sprints of a project: the sprints page's data, the task panel's choice of
// sprint, the board's sprint filter and the burndown.

import { type BurndownDay, type Sprint, type SprintState } from "../generated/ops";
import { useOp } from "./hooks";

/** The states, in the order the sprints page shows them. */
export const SPRINT_STATES = ["active", "planned", "closed"] as const satisfies readonly SprintState[];

/** How long a new sprint runs by default, in days (two weeks, both ends counted). */
export const SPRINT_DAYS = 14;

/** A project's sprints with their tasks and the backlog, for the sprints page; refreshed by change events. */
export function useSprintBoard(project: string) {
  return useOp("sprint.list", { project, include_tasks: true });
}

/** A project's sprints, for pickers and filters. */
export function useSprints(project: string, options: { enabled?: boolean } = {}) {
  return useOp("sprint.list", { project }, options);
}

/** A started sprint's burndown. */
export function useBurndown(sprint: string, options: { enabled?: boolean } = {}) {
  return useOp("sprint.burndown", { sprint }, options);
}

/** The sprints a task can go in: planned and active ones. */
export function joinable(sprints: readonly Sprint[]): Sprint[] {
  return sprints.filter((sprint) => sprint.state !== "closed");
}

/** The sprints the page shows: the active one first, then planned ones, then (when asked for) closed ones, newest first. */
export function shownSprints(sprints: readonly Sprint[], showClosed: boolean): Sprint[] {
  const active = sprints.filter((sprint) => sprint.state === "active");
  const planned = sprints.filter((sprint) => sprint.state === "planned");
  const closed = showClosed ? sprints.filter((sprint) => sprint.state === "closed").reverse() : [];
  return [...active, ...planned, ...closed];
}

/** A day `days` after `day` (both `YYYY-MM-DD`). */
export function addDays(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

/** The name the next sprint is offered: one more than the highest "<word> <number>" so far. */
export function nextNumber(sprints: readonly Sprint[]): number {
  const numbers = sprints.map((sprint) => /(\d+)\s*$/.exec(sprint.name)?.[1]).flatMap((n) => (n === undefined ? [] : [Number(n)]));
  return Math.max(0, ...numbers) + 1;
}

/** What the burndown measures: tasks, or their estimates. */
export type BurndownUnit = "tasks" | "estimate";

/** One point of the burndown: the day and the work still open. */
export function remaining(day: BurndownDay, unit: BurndownUnit): number {
  return unit === "tasks" ? day.open_tasks : day.open_estimate_minutes;
}

/**
 * The days the chart spans: from the sprint's first day to its last (or
 * the last day with data, when it ran over), each with its data if it has
 * happened yet.
 */
export function chartDays(days: readonly BurndownDay[], start: string | undefined, end: string | undefined): { date: string; day: BurndownDay | undefined }[] {
  const first = start ?? days[0]?.date;
  const lastData = days[days.length - 1]?.date;
  const last = end !== undefined && (lastData === undefined || end >= lastData) ? end : lastData;
  if (first === undefined || last === undefined) {
    return [];
  }
  const byDate = new Map(days.map((day) => [day.date, day]));
  const out: { date: string; day: BurndownDay | undefined }[] = [];
  for (let date = first; date <= last && out.length < 366; date = addDays(date, 1)) {
    out.push({ date, day: byDate.get(date) });
  }
  return out;
}

/** The ideal line: from the work at the start down to none on the last day, one value per day. */
export function idealLine(start: number, count: number): number[] {
  if (count <= 1) {
    return [start];
  }
  return Array.from({ length: count }, (_x, i) => start - (start * i) / (count - 1));
}
