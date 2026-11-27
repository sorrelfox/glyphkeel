// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The engine's friendly dates, read the same way in the page so that a date
// field can show which day the words mean while they are typed: `today`,
// `fri`, `next fri`, `in 3d`, `+2w`, `dec 1`, `end of month`, `2026-12-01`.
// Days are `YYYY-MM-DD`; arithmetic is done at midnight UTC.

import { type WeekStart } from "../generated/ops";
import { addDays, addMonths, weekday } from "../screens/calendar/model";

const WEEKDAY_NAMES = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];
const MONTHS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
const WEEK_STARTS: Record<WeekStart, number> = { sunday: 0, monday: 1, saturday: 6 };

const pad = (value: number, width = 2) => String(value).padStart(width, "0");

/** A real day from its parts, or null (`2026-02-30` is not one). */
function dayOf(year: number, month: number, date: number): string | null {
  if (!Number.isInteger(year) || year < 1 || year > 9999 || month < 1 || month > 12 || date < 1) {
    return null;
  }
  const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const length = month === 2 ? (leap ? 29 : 28) : [31, 0, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1] ?? 0;
  return date <= length ? `${pad(year, 4)}-${pad(month)}-${pad(date)}` : null;
}

function weekOf(day: string, start: number): { first: string; last: string } {
  const first = addDays(day, -((weekday(day) - start + 7) % 7));
  return { first, last: addDays(first, 6) };
}

function monthOf(day: string): { first: string; last: string } {
  const first = `${day.slice(0, 7)}-01`;
  return { first, last: addDays(addMonths(first, 1), -1) };
}

function yearOf(day: string): { first: string; last: string } {
  return { first: `${day.slice(0, 4)}-01-01`, last: `${day.slice(0, 4)}-12-31` };
}

/** A signed offset in calendar units: `-7d`, `+2w`, `-1m`, `+1y`. */
function offset(text: string, today: string): string | null {
  const match = /^([+-])(\d{1,6})([dwmy])$/.exec(text);
  if (!match) {
    return null;
  }
  const n = (match[1] === "-" ? -1 : 1) * Number(match[2]);
  switch (match[3]) {
    case "d":
      return addDays(today, n);
    case "w":
      return addDays(today, 7 * n);
    case "m":
      return addMonths(today, n);
    default:
      return addMonths(today, 12 * n);
  }
}

/** A date or period as the days it covers: `today`, `-7d`, `next-month`, `2026-12-01`. */
function queryDate(text: string, today: string, weekStart: number): { first: string; last: string } | null {
  const lower = text.toLowerCase();
  const one = (day: string | null) => (day === null ? null : { first: day, last: day });
  switch (lower) {
    case "today":
      return one(today);
    case "yesterday":
      return one(addDays(today, -1));
    case "tomorrow":
      return one(addDays(today, 1));
  }
  const period = /^(this|last|next)-(.*)$/.exec(lower);
  if (period) {
    const step = period[1] === "this" ? 0 : period[1] === "last" ? -1 : 1;
    switch (period[2]) {
      case "week":
        return weekOf(addDays(today, 7 * step), weekStart);
      case "month":
        return monthOf(addMonths(today, step));
      case "year":
        return yearOf(addMonths(today, 12 * step));
      default:
        return null;
    }
  }
  const shifted = offset(lower, today);
  if (shifted !== null) {
    return one(shifted);
  }
  const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(lower);
  return iso ? one(dayOf(Number(iso[1]), Number(iso[2]), Number(iso[3]))) : null;
}

/** A weekday from its name or its start from three letters on, 0 for Sunday. */
function weekdayOf(word: string): number | null {
  const index = WEEKDAY_NAMES.findIndex((name) => word.startsWith(name.slice(0, 3)) && name.startsWith(word));
  return index < 0 ? null : index;
}

/** A month from its name or its start from three letters on, 1 for January. */
function monthOfWord(word: string): number | null {
  const bare = word.replace(/\.+$/, "");
  if (bare.length < 3) {
    return null;
  }
  const index = MONTHS.findIndex((name) => name.startsWith(bare));
  return index < 0 ? null : index + 1;
}

function unitDays(n: number, unit: string, today: string): string | null {
  switch (unit.replace(/s+$/, "")) {
    case "day":
    case "d":
      return addDays(today, n);
    case "week":
    case "w":
      return addDays(today, 7 * n);
    case "month":
      return addMonths(today, n);
    case "year":
    case "y":
      return addMonths(today, 12 * n);
    default:
      return null;
  }
}

/** The next `month`/`date` on or after today, this year or a later one. */
function nextDate(today: string, month: number, date: number, year: number | null): string | null {
  if (year !== null) {
    return dayOf(year, month, date);
  }
  const thisYear = Number(today.slice(0, 4));
  const day = dayOf(thisYear, month, date);
  if (day !== null && day >= today) {
    return day;
  }
  for (let ahead = 1; ahead <= 4; ahead += 1) {
    const later = dayOf(thisYear + ahead, month, date);
    if (later !== null) {
      return later;
    }
  }
  return null;
}

const count = (text: string): number | null => (/^\d+$/.test(text) && Number(text) <= 10_000 ? Number(text) : null);

/**
 * The day that friendly words mean, read as the engine reads them, or null
 * when they are not a date. `today` is `YYYY-MM-DD` in the person's zone;
 * `weekStart` is the workspace's first day of the week.
 */
export function parseNaturalDay(text: string, today: string, weekStart: WeekStart = "monday"): string | null {
  const start = WEEK_STARTS[weekStart];
  const words = text
    .trim()
    .toLowerCase()
    .split(/[\s,]+/)
    .filter((word) => word !== "");
  const [a, b, c] = words;
  if (a === undefined) {
    return null;
  }
  if (words.length === 1) {
    const period = queryDate(a, today, start);
    if (period !== null) {
      return period.first;
    }
    const wanted = weekdayOf(a);
    if (wanted === null) {
      return null;
    }
    const ahead = (wanted - weekday(today) + 7) % 7;
    return addDays(today, ahead === 0 ? 7 : ahead);
  }
  if (words.length === 2 && b !== undefined && (a === "this" || a === "next")) {
    const wanted = weekdayOf(b);
    if (wanted !== null) {
      const week = weekOf(addDays(today, a === "next" ? 7 : 0), start);
      return addDays(week.first, (wanted - start + 7) % 7);
    }
    return queryDate(`${a}-${b}`, today, start)?.first ?? null;
  }
  if (a === "in" && words.length === 3 && b !== undefined && c !== undefined) {
    const n = b === "a" || b === "an" || b === "one" ? 1 : count(b);
    return n === null ? null : unitDays(n, c, today);
  }
  if (a === "in" && words.length === 2 && b !== undefined) {
    const compact = /^(\d+)(\D.*)$/.exec(b);
    const n = compact ? count(compact[1] ?? "") : null;
    if (!compact || n === null) {
      return null;
    }
    const unit = compact[2] ?? "";
    return unitDays(n, unit === "m" ? "month" : unit, today);
  }
  if (a === "end" && b === "of" && (words.length === 3 || (words.length === 4 && c === "the"))) {
    const unit = words[words.length - 1];
    if (unit === "week") {
      return weekOf(today, start).last;
    }
    if (unit === "month") {
      return monthOf(today).last;
    }
    return unit === "year" ? yearOf(today).last : null;
  }
  if ((words.length === 2 || words.length === 3) && b !== undefined) {
    let year: number | null = null;
    if (c !== undefined) {
      if (!/^\d+$/.test(c) || Number(c) < 1 || Number(c) > 65_535) {
        return null;
      }
      year = Number(c);
    }
    const first = monthOfWord(a);
    const second = monthOfWord(b);
    let month: number;
    let dateWord: string;
    if (first !== null && second === null) {
      month = first;
      dateWord = b;
    } else if (first === null && second !== null) {
      month = second;
      dateWord = a;
    } else {
      return null;
    }
    const digits = dateWord.replace(/[a-z.]+$/, "");
    if (!/^\d{1,3}$/.test(digits) || Number(digits) > 255) {
      return null;
    }
    return nextDate(today, month, Number(digits), year);
  }
  return null;
}
