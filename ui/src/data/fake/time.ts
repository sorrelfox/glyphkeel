// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The stand-in engine's time entries, timer and reminders: pure helpers on
// its records, with the engine's rules where the screens can see them.

import { type DueReminder, type Reminder, type Settings, type TimeEntry, type Timer } from "../../generated/ops";
import { formatDuration, type DurationUnits } from "../duration";
import { type FakeReminder, type FakeTimeEntry, type FakeTimer } from "./state";

/** The most one entry may hold: 24 hours. */
export const MAX_ENTRY_MINUTES = 24 * 60;

export function unitsOf(settings: Settings): DurationUnits {
  return { hoursPerDay: settings.hours_per_day, daysPerWeek: settings.days_per_week };
}

/** The local day of a moment, `YYYY-MM-DD`. */
export function localDay(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${String(date.getFullYear())}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** `YYYY-MM-DD` plus `days`. */
export function addDays(day: string, days: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return localDay(new Date(y ?? 1970, (m ?? 1) - 1, (d ?? 1) + days));
}

/** A work day as typed: `YYYY-MM-DD`, `today` or `yesterday`; null when it isn't one. */
export function workDay(text: string | null | undefined, now: Date): string | null {
  const value = (text ?? "today").trim().toLowerCase();
  if (value === "today") {
    return localDay(now);
  }
  if (value === "yesterday") {
    return addDays(localDay(now), -1);
  }
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(Date.parse(`${value}T00:00:00Z`)) ? value : null;
}

/** The first day of the week `day` falls in. */
export function weekStart(day: string, start: Settings["week_start"]): string {
  const [y, m, d] = day.split("-").map(Number);
  const date = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1);
  const first = start === "sunday" ? 0 : start === "saturday" ? 6 : 1;
  const back = (date.getDay() - first + 7) % 7;
  return addDays(day, -back);
}

export function timeEntryOut(entry: FakeTimeEntry, taskKey: string, units: DurationUnits): TimeEntry {
  return {
    author: entry.author,
    created_at: entry.createdAt,
    date: entry.date,
    duration: formatDuration(entry.minutes, units),
    ...(entry.editedAt === null ? {} : { edited_at: entry.editedAt }),
    id: entry.id,
    minutes: entry.minutes,
    ...(entry.note === "" ? {} : { note: entry.note }),
    task: taskKey,
    version: entry.version,
  };
}

/** The time a timer has run so far, in milliseconds. */
export function timerElapsedMs(timer: FakeTimer, now: Date): number {
  return timer.elapsedMs + (timer.runningSince === null ? 0 : Math.max(0, now.getTime() - Date.parse(timer.runningSince)));
}

export function timerOut(timer: FakeTimer, task: Timer["task"], now: Date, units: DurationUnits): Timer {
  const seconds = Math.floor(timerElapsedMs(timer, now) / 1000);
  return {
    elapsed: formatDuration(Math.round(seconds / 60), units),
    elapsed_seconds: seconds,
    ...(timer.note === "" ? {} : { note: timer.note }),
    ...(timer.runningSince === null ? {} : { running_since: timer.runningSince }),
    started_at: timer.startedAt,
    started_by: { kind: "user", name: "You" },
    state: timer.runningSince === null ? "paused" : "running",
    task,
  };
}

/** When a reminder fires: its moment, or the due date less its days at its time of day. */
export function reminderAt(reminder: FakeReminder, dueDate: string | null, reminderTime: string): string | null {
  if (reminder.at !== null) {
    return reminder.at;
  }
  if (dueDate === null || reminder.daysBeforeDue === null) {
    return null;
  }
  const day = addDays(dueDate, -reminder.daysBeforeDue);
  const [hours, minutes] = (reminder.time ?? reminderTime).split(":").map(Number);
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1, hours ?? 9, minutes ?? 0).toISOString();
}

export function reminderOut(reminder: FakeReminder, taskKey: string, at: string | null): Reminder {
  return {
    acknowledged: reminder.acknowledged,
    ...(at === null ? {} : { at }),
    created_at: reminder.createdAt,
    ...(reminder.daysBeforeDue === null ? {} : { days_before_due: reminder.daysBeforeDue }),
    id: reminder.id,
    ...(reminder.note === "" ? {} : { note: reminder.note }),
    set_by: { kind: "user", name: "You" },
    task: taskKey,
    ...(reminder.time === null ? {} : { time: reminder.time }),
  };
}

/** A moment as typed for a reminder: RFC 3339, or local `YYYY-MM-DD HH:MM`. */
export function momentOf(text: string): string | null {
  const local = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})$/.exec(text.trim());
  if (local) {
    const [, y, m, d, hh, mm] = local.map(Number);
    return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1, hh ?? 0, mm ?? 0).toISOString();
  }
  const parsed = Date.parse(text);
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString();
}

/** Due reminders, oldest first. */
export function sortDue(items: DueReminder[]): DueReminder[] {
  return items.sort((a, b) => a.at.localeCompare(b.at));
}

/** One CSV cell: a text a spreadsheet would run as a formula gets a leading apostrophe; quoted when needed. */
function csvCell(cell: string): string {
  const guarded = !/^\d+$/.test(cell) && /^[=+\-@\t\r]/.test(cell) ? `'${cell}` : cell;
  return /[",\n\r]/.test(guarded) ? `"${guarded.replace(/"/g, '""')}"` : guarded;
}

/** CSV text as the engine writes it: a header line, CRLF line ends. */
export function csvOf(header: readonly string[], rows: readonly (readonly string[])[]): string {
  return [header, ...rows].map((cells) => `${cells.map(csvCell).join(",")}\r\n`).join("");
}
