// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The calendar's arithmetic, free of React: days as `YYYY-MM-DD`, the weeks
// of a month or one week from the locale's first day, where each task's bar
// goes in a week, how many don't fit in a day, and what a drag to another
// day changes.

/** A calendar day, `YYYY-MM-DD`. */
export type Day = string;

export type CalendarMode = "month" | "week";

const DAY_MS = 86_400_000;

/** A day's date at midnight UTC, so that day arithmetic never meets a clock change. */
function utc(day: Day): number {
  const [year, month, date] = day.split("-").map(Number);
  return Date.UTC(year ?? 1970, (month ?? 1) - 1, date ?? 1);
}

function fromUtc(ms: number): Day {
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${String(date.getUTCFullYear()).padStart(4, "0")}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

export function addDays(day: Day, days: number): Day {
  return fromUtc(utc(day) + days * DAY_MS);
}

/** Days from `from` to `to`; negative when `to` comes first. */
export function daysBetween(from: Day, to: Day): number {
  return Math.round((utc(to) - utc(from)) / DAY_MS);
}

/** The day of the week, 0 for Sunday to 6 for Saturday. */
export function weekday(day: Day): number {
  return new Date(utc(day)).getUTCDay();
}

/** The same day of the month `months` months on, clamped to the month's last day. */
export function addMonths(day: Day, months: number): Day {
  const [year = 1970, month = 1, date = 1] = day.split("-").map(Number);
  const first = new Date(Date.UTC(year, month - 1 + months, 1));
  const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)).getUTCDate();
  return fromUtc(Date.UTC(first.getUTCFullYear(), first.getUTCMonth(), Math.min(date, last)));
}

/** A day as a `Date` at noon UTC, for `Intl.DateTimeFormat` with `timeZone: "UTC"`. */
export function asDate(day: Day): Date {
  return new Date(utc(day) + DAY_MS / 2);
}

/** Regions whose week starts on Sunday, for engines without `Intl.Locale` week information. */
const SUNDAY_REGIONS = new Set(["US", "CA", "MX", "BR", "JP", "KR", "TW", "HK", "IL", "PH", "IN", "ZA", "AU", "SA", "PE", "CO", "VE", "GT", "DO", "PR"]);

interface WeekInfo {
  firstDay?: number;
}

/**
 * The locale's first day of the week, 0 for Sunday to 6 for Saturday: from
 * `Intl.Locale`'s week information where the engine has it, else from the
 * region, else Monday.
 */
export function firstDayOfWeek(locale: string): number {
  try {
    const intl = new Intl.Locale(locale) as Intl.Locale & { getWeekInfo?: () => WeekInfo; weekInfo?: WeekInfo };
    const info = intl.getWeekInfo?.() ?? intl.weekInfo;
    if (info?.firstDay !== undefined) {
      // Intl counts Monday as 1 and Sunday as 7.
      return info.firstDay % 7;
    }
    const region = intl.maximize().region;
    return region !== undefined && SUNDAY_REGIONS.has(region) ? 0 : 1;
  } catch {
    return 1;
  }
}

/** The first day of the week that holds `day`. */
export function startOfWeek(day: Day, weekStart: number): Day {
  return addDays(day, -((weekday(day) - weekStart + 7) % 7));
}

/** The weeks on screen: the month's weeks, whole, or the one week. */
export function weeksOf(anchor: Day, mode: CalendarMode, weekStart: number): Day[][] {
  const week = (first: Day) => Array.from({ length: 7 }, (_, index) => addDays(first, index));
  if (mode === "week") {
    return [week(startOfWeek(anchor, weekStart))];
  }
  const first = `${anchor.slice(0, 7)}-01`;
  const last = addDays(addMonths(first, 1), -1);
  const weeks: Day[][] = [];
  for (let start = startOfWeek(first, weekStart); start <= last; start = addDays(start, 7)) {
    weeks.push(week(start));
  }
  return weeks;
}

/** The anchor one page on or back: a month or a week. */
export function step(anchor: Day, mode: CalendarMode, by: number): Day {
  return mode === "month" ? addMonths(anchor, by) : addDays(anchor, 7 * by);
}

/** The dates a task has on the calendar. */
export interface Dated {
  key: string;
  start_date?: string | undefined;
  due_date?: string | undefined;
}

/** The days a task covers: from its start to its due date, or the one date it has. */
export interface Span {
  from: Day;
  to: Day;
}

export function spanOf(task: Dated): Span | undefined {
  const start = task.start_date;
  const due = task.due_date;
  if (start !== undefined && due !== undefined) {
    return start <= due ? { from: start, to: due } : { from: due, to: start };
  }
  const one = start ?? due;
  return one === undefined ? undefined : { from: one, to: one };
}

/** A task's bar in one week. */
export interface Bar<T extends Dated> {
  task: T;
  /** The first column, 0 to 6. */
  column: number;
  /** How many days it covers in this week. */
  length: number;
  /** Its row in the week, from 0. */
  lane: number;
  /** It began in an earlier week. */
  continuesBefore: boolean;
  /** It goes on in a later week. */
  continuesAfter: boolean;
}

/**
 * The bars of a week: each task covering one of its days, in the first lane
 * free over all of its days. Longer and earlier tasks take the upper lanes;
 * ties go by key, so the layout never shuffles between reads.
 */
export function layoutWeek<T extends Dated>(week: readonly Day[], tasks: readonly T[]): Bar<T>[] {
  const first = week[0];
  const last = week.at(-1);
  if (first === undefined || last === undefined) {
    return [];
  }
  const placed = tasks
    .flatMap((task) => {
      const span = spanOf(task);
      if (!span || span.to < first || span.from > last) {
        return [];
      }
      const from = span.from < first ? first : span.from;
      const to = span.to > last ? last : span.to;
      return [{ task, span, column: daysBetween(first, from), length: daysBetween(from, to) + 1 }];
    })
    .sort((a, b) => a.column - b.column || b.length - a.length || a.task.key.localeCompare(b.task.key, "en", { numeric: true }));
  const lanes: number[][] = [];
  return placed.map(({ task, span, column, length }) => {
    let lane = lanes.findIndex((taken) => taken.slice(column, column + length).every((used) => used === 0));
    if (lane === -1) {
      lane = lanes.length;
      lanes.push(Array.from({ length: week.length }, () => 0));
    }
    const row = lanes[lane];
    if (row) {
      row.fill(1, column, column + length);
    }
    return { task, column, length, lane, continuesBefore: span.from < first, continuesAfter: span.to > last };
  });
}

/** How many bars cover each day of a week from lane `limit` down: the "+N more" of each day. */
export function hiddenPerDay(bars: readonly Bar<Dated>[], limit: number): number[] {
  const hidden = Array.from({ length: 7 }, () => 0);
  for (const bar of bars) {
    if (bar.lane >= limit) {
      for (let column = bar.column; column < bar.column + bar.length; column += 1) {
        hidden[column] = (hidden[column] ?? 0) + 1;
      }
    }
  }
  return hidden;
}

/** The tasks covering one day, in the week's order of lanes. */
export function tasksOn<T extends Dated>(bars: readonly Bar<T>[], week: readonly Day[], day: Day): T[] {
  const column = week.indexOf(day);
  return bars
    .filter((bar) => column >= bar.column && column < bar.column + bar.length)
    .sort((a, b) => a.lane - b.lane)
    .map((bar) => bar.task);
}

/**
 * What moving a task by `days` days changes: both of its dates move
 * together, so a task keeps its length. Undefined for no move.
 */
export function shifted(task: Dated, days: number): { start_date?: Day; due_date?: Day } | undefined {
  if (days === 0 || spanOf(task) === undefined) {
    return undefined;
  }
  return {
    ...(task.start_date === undefined ? {} : { start_date: addDays(task.start_date, days) }),
    ...(task.due_date === undefined ? {} : { due_date: addDays(task.due_date, days) }),
  };
}

/**
 * The query's `sort:` clauses outside quotes and parentheses, apart from
 * the rest of it. Only there is a `sort:` allowed, so only those move.
 */
export function splitSort(query: string): { rest: string; sort: string[] } {
  const sort: string[] = [];
  let rest = "";
  let depth = 0;
  let i = 0;
  while (i < query.length) {
    const char = query.charAt(i);
    if (char === '"') {
      const close = query.indexOf('"', i + 1);
      const end = close === -1 ? query.length : close + 1;
      rest += query.slice(i, end);
      i = end;
      continue;
    }
    if (char === "(") {
      depth += 1;
    } else if (char === ")") {
      depth = Math.max(0, depth - 1);
    }
    const startsWord = i === 0 || /\s/.test(query.charAt(i - 1));
    if (depth === 0 && startsWord && query.slice(i, i + 5).toLowerCase() === "sort:") {
      const match = /^\S*/.exec(query.slice(i));
      const word = match?.[0] ?? "";
      sort.push(word);
      i += word.length;
      continue;
    }
    rest += char;
    i += 1;
  }
  return { rest: rest.replace(/\s+/g, " ").trim(), sort };
}

/**
 * The query that reads the tasks of the days on screen: the person's query,
 * as a group, and the tasks whose days, from start to due, overlap the
 * days on screen. The person's `sort:` stays, outside the group.
 */
export function rangeQuery(query: string, from: Day, to: Day): string {
  const { rest, sort } = splitSort(query);
  const within = `during:${from}..${to}`;
  return [rest === "" ? within : `(${rest}) ${within}`, ...sort].join(" ");
}
