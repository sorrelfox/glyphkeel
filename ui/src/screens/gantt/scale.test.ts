// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { centreOf, chartSpan, dayAt, dayX, floorUnit, headerCells, MIN_DAYS, nextUnit, PX_PER_DAY, scrollFor, visibleDays, weekendDays, weekendsBetween, zoomStep } from "./scale";

describe("days on the line", () => {
  it("places a day by its distance from the origin, across a clock change", () => {
    expect(dayX("2026-10-01", "2026-10-01", 32)).toBe(0);
    expect(dayX("2026-10-01", "2026-11-01", 32)).toBe(31 * 32);
    // Summer time ends on 25 October in Europe; days stay whole.
    expect(dayX("2026-10-24", "2026-10-26", 10)).toBe(20);
    expect(dayAt("2026-10-01", 31 * 32, 32)).toBe("2026-11-01");
    expect(dayAt("2026-10-01", 31 * 32 - 1, 32)).toBe("2026-10-31");
  });

  it("finds the first day of a unit", () => {
    expect(floorUnit("2026-11-11", "week", 1)).toBe("2026-11-09");
    expect(floorUnit("2026-11-11", "week", 0)).toBe("2026-11-08");
    expect(floorUnit("2026-11-11", "month", 1)).toBe("2026-11-01");
    expect(floorUnit("2026-11-11", "quarter", 1)).toBe("2026-10-01");
    expect(floorUnit("2026-02-28", "quarter", 1)).toBe("2026-01-01");
    expect(floorUnit("2026-11-11", "year", 1)).toBe("2026-01-01");
    expect(nextUnit("2026-12-01", "month")).toBe("2027-01-01");
    expect(nextUnit("2026-10-01", "quarter")).toBe("2027-01-01");
    expect(nextUnit("2026-01-31", "day")).toBe("2026-02-01");
  });

  it("steps the zoom in and out, and stops at either end", () => {
    expect(zoomStep("day", 1)).toBe("week");
    expect(zoomStep("week", -1)).toBe("day");
    expect(zoomStep("day", -1)).toBeUndefined();
    expect(zoomStep("quarter", 1)).toBeUndefined();
  });
});

describe("header cells", () => {
  it("cuts months into cells of their own width, the first clipped to the origin", () => {
    const cells = headerCells("2026-10-15", 10, "month", 0, 600, 10_000, 1);
    expect(cells.map((cell) => cell.first)).toEqual(["2026-10-01", "2026-11-01", "2026-12-01"]);
    expect(cells[0]).toMatchObject({ day: "2026-10-15", x: 0, width: 170 });
    expect(cells[1]).toMatchObject({ x: 170, width: 300 });
  });

  it("only makes the cells in view, and none past the end", () => {
    const cells = headerCells("2026-01-01", 32, "day", 32 * 100, 32 * 110, 32 * 105, 1);
    expect(cells[0]?.first).toBe("2026-04-11");
    expect(cells.at(-1)?.first).toBe("2026-04-15");
    expect(cells.every((cell) => cell.x + cell.width <= 32 * 105)).toBe(true);
  });

  it("starts weeks on the locale's first day", () => {
    const monday = headerCells("2026-11-01", PX_PER_DAY.week, "week", 0, 200, 1000, 1);
    const sunday = headerCells("2026-11-01", PX_PER_DAY.week, "week", 0, 200, 1000, 0);
    expect(monday[1]?.first).toBe("2026-11-02");
    expect(sunday[1]?.first).toBe("2026-11-08");
  });
});

describe("the chart's days", () => {
  it("runs from before the first date to after the last, from the first of a month, a year at least", () => {
    const span = chartSpan({ start: "2026-11-20", end: "2026-12-10" }, "2026-11-11");
    expect(span.origin).toBe("2026-01-01");
    expect(span.days).toBeGreaterThanOrEqual(MIN_DAYS);
    const long = chartSpan({ start: "2026-01-05", end: "2028-06-30" }, "2026-11-11");
    expect(long.origin).toBe("2025-01-01");
    expect(dayAt(long.origin, long.days - 1, 1) > "2028-06-30").toBe(true);
    expect(chartSpan(undefined, "2026-11-11").origin).toBe("2026-01-01");
    // Farther out, more room on either side.
    const far = chartSpan({ start: "2026-11-20", end: "2026-12-10" }, "2026-11-11", "quarter");
    expect(far.origin).toBe("2024-01-01");
    expect(far.days).toBeGreaterThan(3 * 365);
  });

  it("keeps the day in the middle when the zoom changes", () => {
    const visible = 900;
    const before = PX_PER_DAY.day;
    const scroll = 30_000;
    const middle = centreOf(scroll, before, visible);
    const after = scrollFor(middle, PX_PER_DAY.month, visible);
    expect(centreOf(after, PX_PER_DAY.month, visible)).toBeCloseTo(middle, 6);
    // Never before the start.
    expect(scrollFor(2, PX_PER_DAY.day, visible)).toBe(0);
  });

  it("says which days are on screen", () => {
    expect(visibleDays("2026-11-01", 0, 32, 320, 365)).toEqual({ from: "2026-11-01", to: "2026-11-10" });
    expect(visibleDays("2026-11-01", 16, 32, 320, 365)).toEqual({ from: "2026-11-01", to: "2026-11-11" });
    expect(visibleDays("2026-11-01", 0, 32, 32_000, 30)).toEqual({ from: "2026-11-01", to: "2026-11-30" });
  });
});

describe("weekends", () => {
  it("are Saturday and Sunday by default, Friday and Saturday where the locale says", () => {
    expect([...weekendDays("en-GB")].sort()).toEqual([0, 6]);
    expect([...weekendDays("ar-SA")].sort()).toEqual([5, 6]);
    expect([...weekendDays("not a locale")].sort()).toEqual([0, 6]);
  });

  it("are found among a stretch of days", () => {
    // 2026-11-01 is a Sunday.
    expect(weekendsBetween("2026-11-01", 0, 13, new Set([0, 6]))).toEqual([0, 6, 7, 13]);
  });
});
