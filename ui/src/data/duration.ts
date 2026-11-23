// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Durations as people type and read them, with the engine's rules: weeks,
// days, hours and minutes of working time, where a day and a week have the
// workspace's lengths.

/** A working day's hours and a working week's days. */
export interface DurationUnits {
  hoursPerDay: number;
  daysPerWeek: number;
}

export const DEFAULT_UNITS: DurationUnits = { hoursPerDay: 8, daysPerWeek: 5 };

const UNIT_WORDS: Record<string, "m" | "h" | "d" | "w"> = {
  m: "m",
  min: "m",
  mins: "m",
  h: "h",
  hr: "h",
  hrs: "h",
  d: "d",
  w: "w",
};

function minutesPer(unit: "m" | "h" | "d" | "w", units: DurationUnits): number {
  switch (unit) {
    case "m":
      return 1;
    case "h":
      return 60;
    case "d":
      return 60 * units.hoursPerDay;
    case "w":
      return 60 * units.hoursPerDay * units.daysPerWeek;
  }
}

/**
 * A duration as typed: "90m", "1.5h", "1,5h", "1:30", "2d", "1w", "1h 30m",
 * with an optional leading "+" ("+30m"). Days and weeks use the workspace's
 * working hours and days. Null when it doesn't parse; each unit may appear
 * once.
 */
export function parseDuration(text: string, units: DurationUnits = DEFAULT_UNITS): number | null {
  let rest = text.trim().toLowerCase().replace(/^\+\s*/, "");
  if (rest === "") {
    return null;
  }
  const clock = /^(\d+):([0-5]\d)$/.exec(rest);
  if (clock) {
    return Number(clock[1]) * 60 + Number(clock[2]);
  }
  const seen = new Set<string>();
  let total = 0;
  const part = /^(\d*(?:[.,]\d+)?|\d+[.,]?)\s*([a-z]+)\s*/;
  while (rest !== "") {
    const found = part.exec(rest);
    const number = found?.[1] ?? "";
    const unit = UNIT_WORDS[found?.[2] ?? ""];
    if (!found || number === "" || unit === undefined || seen.has(unit)) {
      return null;
    }
    seen.add(unit);
    total += Number(number.replace(",", ".")) * minutesPer(unit, units);
    rest = rest.slice(found[0].length);
  }
  return Math.round(total);
}

/**
 * Minutes in weeks, days, hours and minutes of working time: "1w 2d 4h 30m",
 * "3d", "45m", "0m". Negative values get a minus sign.
 */
export function formatDuration(minutes: number, units: DurationUnits = DEFAULT_UNITS): string {
  const sign = minutes < 0 ? "-" : "";
  let rest = Math.abs(Math.round(minutes));
  const parts: string[] = [];
  for (const unit of ["w", "d", "h", "m"] as const) {
    const size = minutesPer(unit, units);
    const count = Math.floor(rest / size);
    rest -= count * size;
    if (count > 0) {
      parts.push(`${String(count)}${unit}`);
    }
  }
  return parts.length === 0 ? "0m" : `${sign}${parts.join(" ")}`;
}

/** Seconds as a running clock: "0:42:10". */
export function formatClock(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const secs = whole % 60;
  return `${String(hours)}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}
