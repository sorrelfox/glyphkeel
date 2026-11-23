// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Times as the person sees them: in the time zone chosen in Preferences
// (the system's by default). Stored times stay in UTC; only showing them,
// and reading what the person types, uses the zone.

import { useAppSettings, zoneOffsetSeconds } from "../ipc/appSettings";

/**
 * The zone times are shown in, by its IANA name, for `Intl.DateTimeFormat`'s
 * `timeZone`; undefined means the system's.
 */
export function useTimeZone(): string | undefined {
  return useAppSettings().data?.zone_in_use;
}

const pad = (value: number) => String(value).padStart(2, "0");

/** The calendar fields of a moment in a zone (the system's when undefined). */
function fieldsIn(at: Date | number, zone: string | undefined) {
  const parts = new Intl.DateTimeFormat("en-US", {
    ...(zone === undefined ? {} : { timeZone: zone }),
    hourCycle: "h23",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
  }).formatToParts(at);
  const part = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((each) => each.type === type)?.value ?? 0);
  return { year: part("year"), month: part("month"), day: part("day"), hour: part("hour"), minute: part("minute") };
}

/** The day of a moment in a zone, as `YYYY-MM-DD`. */
export function dayIn(at: Date | number, zone: string | undefined): string {
  const fields = fieldsIn(at, zone);
  return `${String(fields.year).padStart(4, "0")}-${pad(fields.month)}-${pad(fields.day)}`;
}

/** A moment as `YYYY-MM-DDTHH:MM` in a zone, for a date-and-time field. */
export function momentIn(at: Date | number, zone: string | undefined): string {
  const fields = fieldsIn(at, zone);
  return `${dayIn(at, zone)}T${pad(fields.hour)}:${pad(fields.minute)}`;
}

/**
 * The moment a local `YYYY-MM-DDTHH:MM` names in a zone, in milliseconds
 * since the epoch; NaN when the text isn't one. A time skipped by a clock
 * change lands just after it.
 */
export function momentFrom(local: string, zone: string | undefined): number {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/u.exec(local);
  if (!match) {
    return Number.NaN;
  }
  const [, year, month, day, hour, minute] = match.map(Number) as [number, number, number, number, number, number];
  if (zone === undefined) {
    return new Date(year, month - 1, day, hour, minute).getTime();
  }
  const wall = Date.UTC(year, month - 1, day, hour, minute);
  // The offset at the wall time read as UTC, then again at the first guess: right on both sides of a change.
  let at = wall - zoneOffsetSeconds(zone, wall) * 1000;
  at = wall - zoneOffsetSeconds(zone, at) * 1000;
  return at;
}

/** Today in the person's zone, as `YYYY-MM-DD`. */
export function useToday(now: () => Date = () => new Date()): string {
  return dayIn(now(), useTimeZone());
}
