// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Recurrence rules as the engine reads and counts them: when the next
// occurrence of a recurring task falls, a rule written in words, and the
// rule in words for people. Everything works on days, `YYYY-MM-DD`.

import { type CreateWhen, type Frequency, type RecurrenceRule, type RepeatFrom, type RulePreviewInput, type Weekday } from "../../generated/ops";

/** The weekdays, Monday first. */
export const WEEKDAYS: readonly Weekday[] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
const INTERVAL_MAX = 999;
const COUNT_MAX = 10_000;
const DAYS_AHEAD_MAX = 365;
const DAY_MS = 86_400_000;

/** A rule that can't be read or doesn't hold together: the field, the message and its arguments. */
export class RuleProblem extends Error {
  constructor(
    readonly field: string,
    readonly messageId: string,
    message: string,
    readonly args: Record<string, string | number> = {},
  ) {
    super(message);
  }
}

/** A rule while it is read: the stored rule's calendar parts. */
export interface Core {
  every: Frequency;
  interval: number;
  weekdays: Weekday[];
  monthDay: number | null;
  nth: { nth: number; weekday: Weekday } | null;
  month: number | null;
  start: string;
  until: string | null;
  count: number | null;
  from: RepeatFrom;
}

// ---------- days ----------

const parts = (day: string): [number, number, number] => {
  const [y, m, d] = day.split("-").map(Number);
  return [y ?? 1970, m ?? 1, d ?? 1];
};

/** Days since 1970-01-01. */
export function dayNumber(day: string): number {
  const [y, m, d] = parts(day);
  return Math.round(Date.UTC(y, m - 1, d) / DAY_MS);
}

export function dayOf(n: number): string {
  return new Date(n * DAY_MS).toISOString().slice(0, 10);
}

export function plusDays(day: string, n: number): string {
  return dayOf(dayNumber(day) + n);
}

/** Monday is 0, Sunday 6. */
export function weekdayIndex(day: string): number {
  return (new Date(dayNumber(day) * DAY_MS).getUTCDay() + 6) % 7;
}

export function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

/** Whether `text` is a real day written `YYYY-MM-DD`. */
export function isDay(text: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(text) && dayOf(dayNumber(text)) === text;
}

const monthIndex = (day: string) => {
  const [y, m] = parts(day);
  return y * 12 + m - 1;
};

const monthOf = (index: number): [number, number] => [Math.floor(index / 12), (((index % 12) + 12) % 12) + 1];

const day = (y: number, m: number, d: number) => `${String(y).padStart(4, "0")}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;

/** Day `d` of a month (-1: the last); past the month's end, its last day. */
function dayInMonth(y: number, m: number, d: number): string {
  const last = daysInMonth(y, m);
  return day(y, m, d < 0 ? last : Math.min(d, last));
}

/** The nth (1–5, or -1 for the last) weekday of a month, if it has one. */
function nthInMonth(y: number, m: number, nth: number, weekday: Weekday): string | null {
  const target = WEEKDAYS.indexOf(weekday);
  if (nth < 0) {
    const last = day(y, m, daysInMonth(y, m));
    return plusDays(last, -((((weekdayIndex(last) - target) % 7) + 7) % 7));
  }
  const first = day(y, m, 1);
  const found = plusDays(first, ((((target - weekdayIndex(first)) % 7) + 7) % 7) + 7 * (nth - 1));
  return parts(found)[1] === m ? found : null;
}

// ---------- counting ----------

/** The weekdays of a weekly rule, Monday first, never empty. */
export function daysOfWeek(rule: Core): Weekday[] {
  if (rule.weekdays.length === 0) {
    return [WEEKDAYS[weekdayIndex(rule.start)] ?? "mon"];
  }
  return WEEKDAYS.filter((each) => rule.weekdays.includes(each));
}

const within = (rule: Core, d: string | null) => (d === null || (rule.until !== null && d > rule.until) ? null : d);

/** The first day of the pattern after `after`, not before the start nor after `until`. */
export function nextAfter(rule: Core, after: string): string | null {
  const fromDay = plusDays(after, 1) > rule.start ? plusDays(after, 1) : rule.start;
  const from = dayNumber(fromDay);
  const interval = Math.max(1, rule.interval);
  const start = dayNumber(rule.start);
  if (rule.every === "day") {
    return within(rule, dayOf(start + Math.ceil((from - start) / interval) * interval));
  }
  if (rule.every === "week") {
    const days = daysOfWeek(rule).map((each) => WEEKDAYS.indexOf(each));
    const monday0 = start - weekdayIndex(rule.start);
    let d = from;
    for (let i = 0; i < 64; i += 1) {
      const week = Math.floor((d - monday0) / 7);
      if (week % interval !== 0) {
        d = monday0 + Math.ceil(week / interval) * interval * 7;
        continue;
      }
      if (days.includes(weekdayIndex(dayOf(d)))) {
        return within(rule, dayOf(d));
      }
      d += 1;
    }
    return null;
  }
  const step = rule.every === "year" ? interval * 12 : interval;
  const [sy, sm, sd] = parts(rule.start);
  const first = rule.every === "year" ? sy * 12 + (rule.month ?? sm) - 1 : monthIndex(rule.start);
  const gap = monthIndex(fromDay) - first;
  let k = gap < 0 ? first : first + Math.ceil(gap / step) * step;
  if (k - step >= first) {
    k -= step;
  }
  for (let i = 0; i < 200; i += 1) {
    const [y, m] = monthOf(k);
    const candidate = rule.nth !== null ? nthInMonth(y, m, rule.nth.nth, rule.nth.weekday) : dayInMonth(y, m, rule.monthDay ?? sd);
    if (candidate !== null && candidate >= fromDay) {
      return within(rule, candidate);
    }
    k += step;
  }
  return null;
}

/** The first occurrence: the first day of the pattern from the start. */
export function firstDay(rule: Core): string | null {
  return nextAfter(rule, plusDays(rule.start, -1));
}

/** From completion: `interval` units after `done`; months keep the day, or the month's last. */
export function afterCompletion(rule: Core, done: string): string | null {
  const n = Math.max(1, rule.interval);
  if (rule.every === "day" || rule.every === "week") {
    return within(rule, plusDays(done, rule.every === "day" ? n : 7 * n));
  }
  const [y, m] = monthOf(monthIndex(done) + (rule.every === "year" ? n * 12 : n));
  return within(rule, dayInMonth(y, m, parts(done)[2]));
}

/** The next occurrence after one on `last` that was done on `done`. */
export function nextOccurrence(rule: Core, last: string, done: string): string | null {
  return rule.from === "schedule" ? nextAfter(rule, last) : afterCompletion(rule, done);
}

/** Up to `n` days of the pattern after `after`, with `made` occurrences already counted. */
export function upcoming(rule: Core, after: string, n: number, made: number): string[] {
  const out: string[] = [];
  let last = after;
  let count = made;
  while (out.length < n) {
    if (rule.count !== null && count >= rule.count) {
      break;
    }
    const next = rule.from === "schedule" ? nextAfter(rule, last) : afterCompletion(rule, last);
    if (next === null) {
      break;
    }
    out.push(next);
    count += 1;
    last = next;
  }
  return out;
}

// ---------- checking ----------

/** Checks the parts of a rule against each other. */
export function check(rule: Core): void {
  if (rule.interval < 1 || rule.interval > INTERVAL_MAX) {
    throw new RuleProblem("interval", "value-recurrence-interval", `The interval must be 1 to ${String(INTERVAL_MAX)}.`, { max: INTERVAL_MAX });
  }
  if (rule.monthDay !== null && !(rule.monthDay === -1 || (rule.monthDay >= 1 && rule.monthDay <= 31))) {
    throw new RuleProblem("month_day", "value-recurrence-month-day", "The day of the month must be 1 to 31, or -1 for the last day.");
  }
  if (rule.nth !== null && !(rule.nth.nth === -1 || (rule.nth.nth >= 1 && rule.nth.nth <= 5))) {
    throw new RuleProblem("week_of_month", "value-recurrence-week-of-month", "The week of the month must be 1 to 5, or -1 for the last.");
  }
  if (rule.month !== null && !(rule.month >= 1 && rule.month <= 12)) {
    throw new RuleProblem("month", "value-recurrence-month", "The month must be 1 to 12.");
  }
  if (rule.monthDay !== null && rule.nth !== null) {
    throw new RuleProblem("month_day", "value-recurrence-day-and-weekday", "Give a day of the month or a weekday of the month, not both.");
  }
  const pattern = rule.weekdays.length > 0 || rule.monthDay !== null || rule.nth !== null || rule.month !== null;
  const misfit =
    rule.every === "day"
      ? pattern
      : rule.every === "week"
        ? rule.monthDay !== null || rule.nth !== null || rule.month !== null
        : rule.every === "month"
          ? rule.weekdays.length > 0 || rule.month !== null
          : rule.weekdays.length > 0;
  if (misfit) {
    throw new RuleProblem("every", "value-recurrence-parts", `These parts don't fit a rule that repeats every ${rule.every}.`, { unit: rule.every });
  }
  if (rule.from === "completion" && pattern) {
    throw new RuleProblem("from", "value-recurrence-completion-pattern", "A rule that repeats from completion can't fall on chosen days.");
  }
  if (rule.until !== null && rule.until < rule.start) {
    throw new RuleProblem("until", "value-recurrence-until-before-start", `The rule ends on ${rule.until}, before it starts on ${rule.start}.`, { until: rule.until, start: rule.start });
  }
  if (rule.count !== null && (rule.count < 1 || rule.count > COUNT_MAX)) {
    throw new RuleProblem("count", "value-recurrence-count", `The number of occurrences must be 1 to ${String(COUNT_MAX)}.`, { max: COUNT_MAX });
  }
}

// ---------- reading words ----------

const MONTHS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

function parseWeekday(word: string): Weekday | null {
  const w = word.trim().toLowerCase().replace(/s+$/, "");
  const names: Record<string, Weekday> = { mon: "mon", monday: "mon", tue: "tue", tuesday: "tue", wed: "wed", wednesday: "wed", thu: "thu", thur: "thu", thursday: "thu", fri: "fri", friday: "fri", sat: "sat", saturday: "sat", sun: "sun", sunday: "sun" };
  return names[w] ?? null;
}

function parseMonth(word: string): number | null {
  if (word.length < 3) {
    return null;
  }
  const i = MONTHS.findIndex((month) => word.startsWith(month.slice(0, 3)));
  return i >= 0 && ((MONTHS[i] ?? "").startsWith(word) || word === "sept") ? i + 1 : null;
}

function parseUnit(word: string): Frequency | null {
  const units: Record<string, Frequency> = { day: "day", d: "day", week: "week", w: "week", wk: "week", month: "month", mo: "month", year: "year", y: "year", yr: "year" };
  return units[word.replace(/s+$/, "")] ?? null;
}

function parseOrdinal(word: string): number | null {
  if (word === "last") {
    return -1;
  }
  const words = ["first", "second", "third", "fourth", "fifth"];
  if (words.includes(word)) {
    return words.indexOf(word) + 1;
  }
  const found = /^(\d+)(st|nd|rd|th)?$/.exec(word);
  return found ? Number(found[1]) : null;
}

const UNIT_LESS: readonly string[] = ["every", "each", "repeat", "repeats", "the", "of", "on", "in", "for", "x", "times", "time", "occurrences"];

/**
 * Reads a rule in words with `start` as its first day: `daily`, `weekdays`,
 * `every 2 weeks on mon and thu`, `the 1st of each month`, `monthly on the
 * last fri`, `yearly in march`, `3 days after done`, `until 2027-06-30`,
 * `10 times`.
 */
export function parseRule(text: string, start: string): Core {
  const bad = () => new RuleProblem("text", "value-recurrence-text", `"${text.trim()}" is not a rule: write, for example, every monday, every 2 weeks on mon and thu, monthly on the last fri, or 3 days after done.`, { text: text.trim() });
  const words = text
    .toLowerCase()
    .replace(/[,;/+&]/g, " ")
    .split(/\s+/)
    .filter((word) => word !== "" && word !== "and" && word !== "a" && word !== "an");
  if (words.length === 0) {
    throw bad();
  }
  const rule: Core = { every: "day", interval: 1, weekdays: [], monthDay: null, nth: null, month: null, start, until: null, count: null, from: "schedule" };
  let unit: Frequency | null = null;
  const weekdays: Weekday[] = [];
  let ordinal: number | null = null;
  let ordinalWeekday: Weekday | null = null;
  let i = 0;
  while (i < words.length) {
    const w = words[i] ?? "";
    i += 1;
    if (w === "daily") unit = "day";
    else if (w === "weekly") unit = "week";
    else if (w === "biweekly" || w === "fortnightly") {
      unit = "week";
      rule.interval = 2;
    } else if (w === "monthly") unit = "month";
    else if (w === "yearly" || w === "annually") unit = "year";
    else if (["weekday", "weekdays", "workday", "workdays"].includes(w)) {
      unit = "week";
      weekdays.push(...WEEKDAYS.slice(0, 5));
    } else if (UNIT_LESS.includes(w)) {
      // Words that only read well.
    } else if (w === "other") rule.interval = 2;
    else if (w === "after") {
      if (["done", "completion", "completed", "closing", "close", "finish"].includes(words[i] ?? "")) {
        i += 1;
      }
      rule.from = "completion";
    } else if (["until", "till", "ending", "ends"].includes(w)) {
      const d = words[i];
      i += 1;
      if (d === undefined || !isDay(d)) {
        throw bad();
      }
      rule.until = d;
    } else {
      const weekday = parseWeekday(w);
      if (weekday !== null) {
        if (ordinal !== null && ordinalWeekday === null) {
          ordinalWeekday = weekday;
        } else {
          weekdays.push(weekday);
        }
        continue;
      }
      const u = parseUnit(w);
      if (u !== null) {
        unit = u;
        continue;
      }
      const month = parseMonth(w);
      if (month !== null) {
        rule.month = month;
        unit ??= "year";
        continue;
      }
      if (/^\d+$/.test(w)) {
        const n = Number(w);
        const next = words[i];
        if (next !== undefined && ["times", "time", "x", "occurrences"].includes(next)) {
          rule.count = n;
          i += 1;
        } else if (next !== undefined && parseUnit(next) !== null) {
          rule.interval = n;
        } else if (words[i - 2] === "for") {
          rule.count = n;
        } else if (ordinal === null && n <= 31 && unit !== null) {
          ordinal = n;
        } else {
          rule.interval = n;
        }
        continue;
      }
      const o = parseOrdinal(w);
      if (o !== null) {
        ordinal = o;
        continue;
      }
      throw bad();
    }
  }
  const every = unit ?? (weekdays.length > 0 ? "week" : ordinal !== null ? "month" : null);
  if (every === null) {
    throw bad();
  }
  rule.every = every;
  if (ordinal !== null && ordinalWeekday !== null) {
    rule.nth = { nth: ordinal, weekday: ordinalWeekday };
  } else if (ordinal !== null) {
    rule.monthDay = ordinal;
  }
  if (weekdays.length > 0) {
    if (every !== "week") {
      throw new RuleProblem("every", "value-recurrence-parts", `These parts don't fit a rule that repeats every ${every}.`, { unit: every });
    }
    rule.weekdays = WEEKDAYS.filter((each) => weekdays.includes(each));
  }
  check(rule);
  return rule;
}

// ---------- the API's shape ----------

export function toCore(rule: RecurrenceRule): Core {
  return {
    every: rule.every,
    interval: rule.interval,
    weekdays: [...(rule.weekdays ?? [])],
    monthDay: rule.month_day ?? null,
    nth: rule.week_of_month !== undefined && rule.weekday !== undefined ? { nth: rule.week_of_month, weekday: rule.weekday } : null,
    month: rule.month ?? null,
    start: rule.starts,
    until: rule.until ?? null,
    count: rule.count ?? null,
    from: rule.from,
  };
}

export function fromCore(rule: Core, create: CreateWhen, daysAhead: number): RecurrenceRule {
  const weekdays = rule.every === "week" ? daysOfWeek(rule) : [];
  const ahead = create === "ahead" ? daysAhead : 0;
  return {
    every: rule.every,
    interval: rule.interval,
    ...(weekdays.length > 0 ? { weekdays } : {}),
    ...(rule.monthDay === null ? {} : { month_day: rule.monthDay }),
    ...(rule.nth === null ? {} : { week_of_month: rule.nth.nth, weekday: rule.nth.weekday }),
    ...(rule.month === null ? {} : { month: rule.month }),
    starts: rule.start,
    ...(rule.until === null ? {} : { until: rule.until }),
    ...(rule.count === null ? {} : { count: rule.count }),
    from: rule.from,
    create,
    ...(ahead > 0 ? { days_ahead: ahead } : {}),
  };
}

const given = <T>(value: T | null | undefined): value is T => value !== null && value !== undefined;

/**
 * Reads a rule given in words or field by field: fields given next to the
 * text override it; `existing` is the rule it changes, kept where the input
 * says nothing. `date` reads a day as typed (`today`, `next mon`).
 */
export function buildRule(input: RulePreviewInput, defaultStart: string, existing: RecurrenceRule | null, date: (field: string, text: string) => string): RecurrenceRule {
  const start = given(input.starts) ? date("starts", input.starts) : (existing?.starts ?? defaultStart);
  const fromText = given(input.text);
  let rule: Core;
  if (given(input.text)) {
    try {
      rule = parseRule(input.text, start);
    } catch (problem) {
      if (problem instanceof RuleProblem && problem.field !== "until") {
        throw new RuleProblem("text", problem.messageId, problem.message, problem.args);
      }
      throw problem;
    }
  } else if (given(input.every)) {
    rule = { every: input.every, interval: 1, weekdays: [], monthDay: null, nth: null, month: null, start, until: null, count: null, from: "schedule" };
  } else if (existing !== null) {
    rule = toCore(existing);
  } else {
    throw new RuleProblem("every", "recurrence-rule-missing", "Say how the task repeats: give `text` (every monday) or `every`.");
  }
  rule.start = start;
  if (given(input.every) && fromText) rule.every = input.every;
  if (given(input.interval)) rule.interval = input.interval;
  if (input.weekdays !== undefined && input.weekdays.length > 0) rule.weekdays = WEEKDAYS.filter((each) => input.weekdays?.includes(each));
  if (given(input.month_day)) {
    rule.monthDay = input.month_day;
    rule.nth = null;
  }
  if (given(input.week_of_month)) {
    if (!given(input.weekday)) {
      throw new RuleProblem("weekday", "recurrence-weekday-missing", "Give the weekday for the week of the month.");
    }
    rule.nth = { nth: input.week_of_month, weekday: input.weekday };
    if (!given(input.month_day)) rule.monthDay = null;
  }
  if (given(input.month)) rule.month = input.month;
  if (given(input.until)) rule.until = input.until.trim() === "" ? null : date("until", input.until);
  if (given(input.count)) rule.count = input.count;
  if (given(input.from)) rule.from = input.from;
  if (rule.from === "completion" && !fromText) {
    // From completion only the unit and the interval count.
    rule.weekdays = [];
    rule.monthDay = null;
    rule.nth = null;
    rule.month = null;
  }
  try {
    check(rule);
  } catch (problem) {
    if (problem instanceof RuleProblem && fromText && problem.field !== "until") {
      throw new RuleProblem("text", problem.messageId, problem.message, problem.args);
    }
    throw problem;
  }
  const create = input.create ?? existing?.create ?? "on_close";
  const daysAhead = input.days_ahead ?? existing?.days_ahead ?? 0;
  if (create === "ahead" && rule.from === "completion") {
    throw new RuleProblem("create", "recurrence-ahead-from-completion", "Occurrences counted from completion can only be made when the last one is closed.");
  }
  if (daysAhead > DAYS_AHEAD_MAX) {
    throw new RuleProblem("days_ahead", "recurrence-days-ahead", `Make occurrences at most ${String(DAYS_AHEAD_MAX)} days ahead.`, { max: DAYS_AHEAD_MAX });
  }
  return fromCore(rule, create, daysAhead);
}

/** The first dates of a rule from its start. */
export function previewDates(rule: RecurrenceRule, limit: number): string[] {
  const core = toCore(rule);
  const first = firstDay(core);
  return first === null ? [] : [first, ...upcoming(core, first, limit - 1, 1)].slice(0, limit);
}

// ---------- in words ----------

const WEEKDAY_NAMES: Record<Weekday, string> = { mon: "Monday", tue: "Tuesday", wed: "Wednesday", thu: "Thursday", fri: "Friday", sat: "Saturday", sun: "Sunday" };
const ORDINALS: Record<string, string> = { "1": "first", "2": "second", "3": "third", "4": "fourth", "5": "fifth" };

const list = (items: readonly string[]) => (items.length < 2 ? (items[0] ?? "") : `${items.slice(0, -1).join(", ")} and ${items[items.length - 1] ?? ""}`);
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many.replace("#", String(n)));

/** The rule in English, as the engine's English messages write it. */
export function summaryOf(rule: RecurrenceRule): string {
  const n = rule.interval;
  const capital = (unit: string) => plural(n, `Every ${unit}`, `Every # ${unit}s`);
  const nth = (k: number, w: Weekday) => `${ORDINALS[String(k)] ?? "last"} ${WEEKDAY_NAMES[w]}`;
  const [, startMonth, startDay] = parts(rule.starts);
  let base: string;
  if (rule.from === "completion") {
    base = plural(n, `1 ${rule.every} after it is done`, `# ${rule.every}s after it is done`);
  } else if (rule.every === "day") {
    base = capital("day");
  } else if (rule.every === "week") {
    base = `${capital("week")} on ${list((rule.weekdays ?? []).map((w) => WEEKDAY_NAMES[w]))}`;
  } else if (rule.every === "month") {
    base = rule.week_of_month !== undefined && rule.weekday !== undefined ? `${capital("month")} on the ${nth(rule.week_of_month, rule.weekday)}` : rule.month_day === -1 ? `${capital("month")} on the last day` : `${capital("month")} on day ${String(rule.month_day ?? startDay)}`;
  } else {
    const month = MONTHS[(rule.month ?? startMonth) - 1] ?? "january";
    const name = `${month.charAt(0).toUpperCase()}${month.slice(1)}`;
    base = rule.week_of_month !== undefined && rule.weekday !== undefined ? `${capital("year")} on the ${nth(rule.week_of_month, rule.weekday)} of ${name}` : rule.month_day === -1 ? `${capital("year")} on the last day of ${name}` : `${capital("year")} on ${name} ${String(rule.month_day ?? startDay)}`;
  }
  if (rule.until !== undefined) {
    base = `${base}, until ${rule.until}`;
  }
  if (rule.count !== undefined) {
    base = rule.count === 1 ? `${base}, once` : `${base}, ${String(rule.count)} times`;
  }
  return base;
}
