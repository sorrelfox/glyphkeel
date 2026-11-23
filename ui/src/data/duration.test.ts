// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { formatClock, formatDuration, parseDuration } from "./duration";

describe("durations", () => {
  it("reads what people type, with a decimal comma, a leading plus and unit words", () => {
    expect(parseDuration("+30m")).toBe(30);
    expect(parseDuration("1,5h")).toBe(90);
    expect(parseDuration("1.5h")).toBe(90);
    expect(parseDuration("1:30")).toBe(90);
    expect(parseDuration("1h 30m")).toBe(90);
    expect(parseDuration("2 hrs 15 min")).toBe(135);
    expect(parseDuration("1w 2d")).toBe(2400 + 960);
    expect(parseDuration("1d", { hoursPerDay: 6, daysPerWeek: 4 })).toBe(360);
    expect(parseDuration(",5h")).toBe(30);
  });

  it("refuses what isn't a duration", () => {
    expect(parseDuration("")).toBeNull();
    expect(parseDuration("soon")).toBeNull();
    expect(parseDuration("30")).toBeNull();
    expect(parseDuration("1h 2h")).toBeNull();
    expect(parseDuration("h")).toBeNull();
  });

  it("tells minutes in weeks, days, hours and minutes of working time", () => {
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(45)).toBe("45m");
    expect(formatDuration(90)).toBe("1h 30m");
    expect(formatDuration(600)).toBe("1d 2h");
    expect(formatDuration(2400 + 480 + 30)).toBe("1w 1d 30m");
    expect(formatDuration(-90)).toBe("-1h 30m");
    expect(formatDuration(600, { hoursPerDay: 10, daysPerWeek: 5 })).toBe("1d");
    // What it writes reads back the same.
    for (const minutes of [1, 59, 61, 479, 481, 2399, 2401, 9999]) {
      expect(parseDuration(formatDuration(minutes))).toBe(minutes);
    }
  });

  it("shows a running timer as a clock", () => {
    expect(formatClock(0)).toBe("0:00:00");
    expect(formatClock(42 * 60 + 10)).toBe("0:42:10");
    expect(formatClock(9 * 3600 + 5)).toBe("9:00:05");
  });
});
