// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Dates written the friendly way, as the engine reads them in date fields
// and agents' date inputs: `fri`, `next fri`, `in 2 weeks`, `dec 1`,
// `end of month`, and the query language's `today`, `+3d`, `2026-12-24`.
// The quick-add field needs the answer while the person types, before
// anything reaches the engine, so the same rules are kept here; the tests
// share the engine's cases. Words are English for now, as in the engine.

import { type WeekStart } from "../generated/ops";

/** Days from the first to the last, both included, as `YYYY-MM-DD`. */
interface Days {
  first: string;
  last: string;
}

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/;

/** The day's date at midnight UTC, or null for a day that doesn't exist. */
function toDate(day: string): Date | null {
  const match = DAY.exec(day);
  if (!match) {
    return null;
  }
  const [year, month, date] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const value = new Date(Date.UTC(year, month - 1, date));
  value.setUTCFullYear(year);
  if (value.getUTCMonth() !== month - 1 || value.getUTCDate() !== date) {
    return null;
  }
  return value;
}

function format(value: Date): string {
  const year = String(value.getUTCFullYear()).padStart(4, "0");
  const month = String(value.getUTCMonth() + 1).padStart(2, "0");
  const date = String(value.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${date}`;
}

function makeDay(year: number, month: number, date: number): string | null {
  if (year < 1 || year > 9999) {
    return null;
  }
  const text = `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(date).padStart(2, "0")}`;
  return toDate(text) ? text : null;
}

function shift(day: string, days: number): string {
  const value = toDate(day) ?? new Date(0);
  value.setUTCDate(value.getUTCDate() + days);
  return format(value);
}

/** `day` moved by calendar months; the day of the month is kept, or the month's last day when it has fewer. */
export function addMonths(day: string, months: number): string | null {
  const value = toDate(day);
  if (!value) {
    return null;
  }
  const index = value.getUTCFullYear() * 12 + value.getUTCMonth() + months;
  const year = Math.floor(index / 12);
  const month = (((index % 12) + 12) % 12) + 1;
  for (let date = value.getUTCDate(); date >= 1; date -= 1) {
    const candidate = makeDay(year, month, date);
    if (candidate) {
      return candidate;
    }
  }
  return null;
}

/** Monday 0 … Sunday 6. */
function weekdayIndex(day: string): number {
  return ((toDate(day)?.getUTCDay() ?? 1) + 6) % 7;
}

const WEEK_START_INDEX: Readonly<Record<WeekStart, number>> = { monday: 0, saturday: 5, sunday: 6 };

function weekOf(day: string, start: WeekStart): Days {
  const back = (((weekdayIndex(day) - WEEK_START_INDEX[start]) % 7) + 7) % 7;
  const first = shift(day, -back);
  return { first, last: shift(first, 6) };
}

function monthOf(day: string): Days {
  const first = `${day.slice(0, 8)}01`;
  const next = addMonths(first, 1);
  return { first, last: next ? shift(next, -1) : first };
}

function yearOf(day: string): Days {
  return { first: `${day.slice(0, 4)}-01-01`, last: `${day.slice(0, 4)}-12-31` };
}

/** A signed offset in calendar units: `-7d`, `+2w`, `-1m`, `+1y`. */
function offset(text: string, today: string): string | null {
  const match = /^([+-])(\d{1,6})([dwmy])$/i.exec(text);
  if (!match) {
    return null;
  }
  const n = (match[1] === "-" ? -1 : 1) * Number(match[2]);
  switch (match[3]?.toLowerCase()) {
    case "d":
      return shift(today, n);
    case "w":
      return shift(today, n * 7);
    case "m":
      return addMonths(today, n);
    default:
      return addMonths(today, n * 12);
  }
}

/** A date or period of the query language, as the days it covers. */
function parseQueryDate(text: string, today: string, weekStart: WeekStart): Days | null {
  const lower = text.toLowerCase();
  const one = (day: string | null): Days | null => (day === null ? null : { first: day, last: day });
  switch (lower) {
    case "today":
      return one(today);
    case "yesterday":
      return one(shift(today, -1));
    case "tomorrow":
      return one(shift(today, 1));
    default:
      break;
  }
  const dash = lower.indexOf("-");
  if (dash > 0) {
    const step = { this: 0, last: -1, next: 1 }[lower.slice(0, dash)];
    if (step !== undefined) {
      const unit = lower.slice(dash + 1);
      if (unit === "week") {
        return weekOf(shift(today, step * 7), weekStart);
      }
      if (unit === "month") {
        const day = addMonths(today, step);
        return day ? monthOf(day) : null;
      }
      if (unit === "year") {
        const day = addMonths(today, step * 12);
        return day ? yearOf(day) : null;
      }
      return null;
    }
  }
  const moved = offset(lower, today);
  if (moved) {
    return one(moved);
  }
  return toDate(lower) ? one(lower) : null;
}

const WEEKDAYS = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"] as const;

const MONTHS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"] as const;

/** `fri`, `frid`, `friday`: Monday 0 … Sunday 6. */
function weekday(word: string): number | null {
  const index = WEEKDAYS.findIndex((name) => word.startsWith(name.slice(0, 3)));
  if (index < 0) {
    return null;
  }
  return WEEKDAYS[index]?.startsWith(word) ? index : null;
}

/** `dec`, `dec.`, `december`: 1 … 12. */
function month(word: string): number | null {
  const stem = word.replace(/\.+$/, "");
  if (stem.length < 3) {
    return null;
  }
  const index = MONTHS.findIndex((name) => name.startsWith(stem));
  return index < 0 ? null : index + 1;
}

function unitDays(n: number, unit: string, today: string): string | null {
  switch (unit.replace(/s+$/, "")) {
    case "day":
    case "d":
      return shift(today, n);
    case "week":
    case "w":
      return shift(today, n * 7);
    case "month":
      return addMonths(today, n);
    case "year":
    case "y":
      return addMonths(today, n * 12);
    default:
      return null;
  }
}

/** The next `month`/`date` on or after `today` (this year or a later one). */
function nextDate(today: string, monthNumber: number, date: number, year: number | null): string | null {
  if (year !== null) {
    return makeDay(year, monthNumber, date);
  }
  const thisYear = Number(today.slice(0, 4));
  const candidate = makeDay(thisYear, monthNumber, date);
  if (candidate !== null && candidate >= today) {
    return candidate;
  }
  for (let ahead = 1; ahead <= 4; ahead += 1) {
    const later = makeDay(thisYear + ahead, monthNumber, date);
    if (later) {
      return later;
    }
  }
  return null;
}

/**
 * A date written the friendly way, as `YYYY-MM-DD`, or null. `today` is
 * today's date in the person's time zone.
 */
export function parseNaturalDate(text: string, today: string, weekStart: WeekStart = "monday"): string | null {
  const words = text
    .trim()
    .toLowerCase()
    .split(/[\s,]+/)
    .filter((word) => word !== "");
  const [a, b, c] = words;
  if (words.length === 0 || a === undefined) {
    return null;
  }
  if (words.length === 1) {
    const days = parseQueryDate(a, today, weekStart);
    if (days) {
      return days.first;
    }
    const target = weekday(a);
    if (target === null) {
      return null;
    }
    const ahead = (((target - weekdayIndex(today)) % 7) + 7) % 7;
    return shift(today, ahead === 0 ? 7 : ahead);
  }
  if (words.length === 2 && b !== undefined && (a === "this" || a === "next")) {
    const step = a === "next" ? 1 : 0;
    const target = weekday(b);
    if (target !== null) {
      const week = weekOf(shift(today, step * 7), weekStart);
      const into = (((target - WEEK_START_INDEX[weekStart]) % 7) + 7) % 7;
      return shift(week.first, into);
    }
    return parseQueryDate(`${a}-${b}`, today, weekStart)?.first ?? null;
  }
  if (words.length === 3 && a === "in" && b !== undefined && c !== undefined) {
    const n = ["a", "an", "one"].includes(b) ? 1 : /^\d+$/.test(b) && Number(b) <= 10_000 ? Number(b) : null;
    return n === null ? null : unitDays(n, c, today);
  }
  const endOf = words.length === 3 && a === "end" && b === "of" ? c : words.length === 4 && a === "end" && b === "of" && c === "the" ? words[3] : undefined;
  if (endOf !== undefined) {
    switch (endOf) {
      case "week":
        return weekOf(today, weekStart).last;
      case "month":
        return monthOf(today).last;
      case "year":
        return yearOf(today).last;
      default:
        return null;
    }
  }
  if ((words.length === 2 || words.length === 3) && b !== undefined) {
    let year: number | null = null;
    if (c !== undefined) {
      if (!/^\d+$/.test(c) || Number(c) < 1 || Number(c) > 65_535) {
        return null;
      }
      year = Number(c);
    }
    const [monthA, monthB] = [month(a), month(b)];
    let monthNumber: number;
    let dateWord: string;
    if (monthA !== null && monthB === null) {
      [monthNumber, dateWord] = [monthA, b];
    } else if (monthA === null && monthB !== null) {
      [monthNumber, dateWord] = [monthB, a];
    } else {
      return null;
    }
    const digits = dateWord.replace(/[a-z.]+$/i, "");
    if (!/^\d{1,3}$/.test(digits) || Number(digits) > 255) {
      return null;
    }
    return nextDate(today, monthNumber, Number(digits), year);
  }
  return null;
}
