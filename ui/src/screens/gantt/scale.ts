// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart's time scale, free of React: days as `YYYY-MM-DD` placed
// on a line of pixels from an origin day, the cells of the two header rows
// at each zoom level, the days the chart covers, and where to scroll so that
// a zoom keeps the day in the middle where it was.

import { addDays, addMonths, daysBetween, weekday, type Day } from "../calendar/model";

export type Zoom = "day" | "week" | "month" | "quarter";

/** The zoom levels, from the closest to the farthest. */
export const ZOOMS: readonly Zoom[] = ["day", "week", "month", "quarter"];

/** Pixels per day at each zoom level. */
export const PX_PER_DAY: Readonly<Record<Zoom, number>> = { day: 32, week: 12, month: 3.2, quarter: 1.2 };

export type Unit = "day" | "week" | "month" | "quarter" | "year";

/** The units of the upper and the lower header row at each zoom level. */
export const UNITS: Readonly<Record<Zoom, readonly [Unit, Unit]>> = {
  day: ["month", "day"],
  week: ["month", "week"],
  month: ["year", "month"],
  quarter: ["year", "quarter"],
};

/** The next zoom level in (`-1`) or out (`1`), or none at either end. */
export function zoomStep(zoom: Zoom, by: -1 | 1): Zoom | undefined {
  return ZOOMS[ZOOMS.indexOf(zoom) + by];
}

/** The logical position of a day's start, in pixels from the origin. */
export function dayX(origin: Day, day: Day, ppd: number): number {
  return daysBetween(origin, day) * ppd;
}

/** The day at a logical position. */
export function dayAt(origin: Day, x: number, ppd: number): Day {
  return addDays(origin, Math.floor(x / ppd));
}

/** The first day of the unit that holds `day`. */
export function floorUnit(day: Day, unit: Unit, weekStart: number): Day {
  switch (unit) {
    case "day":
      return day;
    case "week":
      return addDays(day, -((weekday(day) - weekStart + 7) % 7));
    case "month":
      return `${day.slice(0, 7)}-01`;
    case "quarter": {
      const month = Number(day.slice(5, 7));
      const first = month - ((month - 1) % 3);
      return `${day.slice(0, 4)}-${String(first).padStart(2, "0")}-01`;
    }
    case "year":
      return `${day.slice(0, 4)}-01-01`;
  }
}

/** The first day of the next unit after the one starting on `day`. */
export function nextUnit(day: Day, unit: Unit): Day {
  switch (unit) {
    case "day":
      return addDays(day, 1);
    case "week":
      return addDays(day, 7);
    case "month":
      return addMonths(day, 1);
    case "quarter":
      return addMonths(day, 3);
    case "year":
      return addMonths(day, 12);
  }
}

export interface HeaderCell {
  /** The unit's first day (clipped to the origin). */
  day: Day;
  /** The unit's own first day, which may lie before the origin. */
  first: Day;
  unit: Unit;
  /** Logical position and width, in pixels. */
  x: number;
  width: number;
}

/**
 * The header cells of one unit that meet the logical pixels `[x0, x1]`,
 * clipped to the chart's `[0, maxX]`.
 */
export function headerCells(origin: Day, ppd: number, unit: Unit, x0: number, x1: number, maxX: number, weekStart: number): HeaderCell[] {
  const cells: HeaderCell[] = [];
  let first = floorUnit(dayAt(origin, Math.max(0, x0), ppd), unit, weekStart);
  for (let guard = 0; guard < 5000; guard += 1) {
    const next = nextUnit(first, unit);
    const x = Math.max(0, dayX(origin, first, ppd));
    const end = Math.min(maxX, dayX(origin, next, ppd));
    if (x > x1 || x >= maxX) {
      break;
    }
    cells.push({ day: first < origin ? origin : first, first, unit, x, width: end - x });
    first = next;
  }
  return cells;
}

/**
 * How many days the chart keeps before the first and after the last date at
 * each zoom level: at least a few screens' worth, so that zooming out can
 * keep the middle day in the middle.
 */
export const LEAD_DAYS: Readonly<Record<Zoom, number>> = { day: 60, week: 120, month: 365, quarter: 730 };
export const TRAIL_DAYS: Readonly<Record<Zoom, number>> = { day: 120, week: 240, month: 545, quarter: 1095 };
/** The shortest chart, in days. */
export const MIN_DAYS = 365;

export interface Span {
  /** The first day drawn: the first of a month. */
  origin: Day;
  /** How many days are drawn. */
  days: number;
}

/**
 * The days the chart covers: from a while before the earliest date (or
 * today) to a while after the latest, at least a year, starting on the
 * first of a year so that every header begins with a whole unit.
 */
export function chartSpan(range: { start: Day; end: Day } | undefined, today: Day, zoom: Zoom = "day"): Span {
  const from = range && range.start < today ? range.start : today;
  const to = range && range.end > today ? range.end : today;
  const origin = floorUnit(addDays(from, -LEAD_DAYS[zoom]), "year", 1);
  const days = Math.max(MIN_DAYS, daysBetween(origin, to) + TRAIL_DAYS[zoom]);
  return { origin, days };
}

/**
 * The scroll offset (logical, from the chart's start) that puts the day
 * `centre` (a fractional day count from the origin) in the middle of
 * `visible` pixels of timeline; never before the start.
 */
export function scrollFor(centre: number, ppd: number, visible: number): number {
  return Math.max(0, centre * ppd - visible / 2);
}

/** The day (as a fractional day count from the origin) in the middle of the visible timeline. */
export function centreOf(scroll: number, ppd: number, visible: number): number {
  return (scroll + visible / 2) / ppd;
}

/** The days on screen: from the first to the last day the visible pixels touch. */
export function visibleDays(origin: Day, scroll: number, ppd: number, visible: number, days: number): { from: Day; to: Day } {
  const first = Math.max(0, Math.floor(scroll / ppd));
  const last = Math.min(days - 1, Math.max(first, Math.ceil((scroll + visible) / ppd) - 1));
  return { from: addDays(origin, first), to: addDays(origin, last) };
}

/** Regions whose weekend is Friday and Saturday, for engines without `Intl.Locale` week information. */
const FRIDAY_SATURDAY = new Set(["AE", "BH", "DZ", "EG", "IL", "IQ", "JO", "KW", "LY", "OM", "QA", "SA", "SD", "SY", "YE"]);

/**
 * The locale's weekend days, 0 for Sunday to 6 for Saturday: from
 * `Intl.Locale`'s week information where the engine has it, else from the
 * region, else Saturday and Sunday.
 */
export function weekendDays(locale: string): ReadonlySet<number> {
  try {
    const intl = new Intl.Locale(locale) as Intl.Locale & { getWeekInfo?: () => { weekend?: number[] }; weekInfo?: { weekend?: number[] } };
    const info = intl.getWeekInfo?.() ?? intl.weekInfo;
    if (info?.weekend !== undefined) {
      // Intl counts Monday as 1 and Sunday as 7.
      return new Set(info.weekend.map((day) => day % 7));
    }
    const region = intl.maximize().region;
    return new Set(region !== undefined && FRIDAY_SATURDAY.has(region) ? [5, 6] : [6, 0]);
  } catch {
    return new Set([6, 0]);
  }
}

/** The weekend days among the days `[first, last]`, as day counts from the origin. */
export function weekendsBetween(origin: Day, first: number, last: number, weekend: ReadonlySet<number>): number[] {
  const out: number[] = [];
  let day = weekday(addDays(origin, first));
  for (let at = first; at <= last; at += 1) {
    if (weekend.has(day)) {
      out.push(at);
    }
    day = (day + 1) % 7;
  }
  return out;
}
