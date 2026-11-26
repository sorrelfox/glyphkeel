// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { addDays, addMonths, daysBetween, firstDayOfWeek, hiddenPerDay, layoutWeek, rangeQuery, shifted, spanOf, splitSort, startOfWeek, step, tasksOn, weeksOf, weekday } from "./model";

describe("days", () => {
  it("adds days across months, years and clock changes", () => {
    expect(addDays("2026-10-31", 1)).toBe("2026-11-01");
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
    expect(addDays("2026-03-29", 1)).toBe("2026-03-30");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
    expect(daysBetween("2026-11-01", "2026-12-01")).toBe(30);
    expect(daysBetween("2026-12-01", "2026-11-29")).toBe(-2);
  });

  it("adds months, keeping to the month's last day", () => {
    expect(addMonths("2026-01-31", 1)).toBe("2026-02-28");
    expect(addMonths("2028-01-31", 1)).toBe("2028-02-29");
    expect(addMonths("2026-12-15", 1)).toBe("2027-01-15");
    expect(addMonths("2026-01-10", -1)).toBe("2025-12-10");
  });

  it("knows the day of the week", () => {
    expect(weekday("2026-11-26")).toBe(4);
    expect(startOfWeek("2026-11-26", 1)).toBe("2026-11-23");
    expect(startOfWeek("2026-11-26", 0)).toBe("2026-11-22");
    expect(startOfWeek("2026-11-22", 1)).toBe("2026-11-16");
    expect(startOfWeek("2026-11-28", 6)).toBe("2026-11-28");
  });
});

describe("the first day of the week", () => {
  it("comes from the locale", () => {
    expect(firstDayOfWeek("en-US")).toBe(0);
    expect(firstDayOfWeek("en-GB")).toBe(1);
    expect(firstDayOfWeek("de-DE")).toBe(1);
  });

  it("falls back to Monday for a tag it can't read", () => {
    expect(firstDayOfWeek("not a locale")).toBe(1);
  });
});

describe("the weeks on screen", () => {
  it("covers the whole month in whole weeks", () => {
    const weeks = weeksOf("2026-11-26", "month", 1);
    expect(weeks).toHaveLength(6);
    expect(weeks[0]?.[0]).toBe("2026-10-26");
    expect(weeks.at(-1)?.at(-1)).toBe("2026-12-06");
    expect(weeks.every((week) => week.length === 7)).toBe(true);
  });

  it("starts the weeks on the locale's day", () => {
    const weeks = weeksOf("2026-11-26", "month", 0);
    expect(weeks[0]?.[0]).toBe("2026-11-01");
    expect(weeks).toHaveLength(5);
    // February 2026 starts on a Sunday and has exactly four weeks.
    expect(weeksOf("2026-02-10", "month", 0)).toHaveLength(4);
  });

  it("shows one week in the week view", () => {
    expect(weeksOf("2026-11-26", "week", 1)).toEqual([["2026-11-23", "2026-11-24", "2026-11-25", "2026-11-26", "2026-11-27", "2026-11-28", "2026-11-29"]]);
  });

  it("pages by a month or a week", () => {
    expect(step("2026-11-26", "month", 1)).toBe("2026-12-26");
    expect(step("2026-11-26", "week", -1)).toBe("2026-11-19");
  });
});

describe("a task's days", () => {
  it("runs from start to due, or is the one date it has", () => {
    expect(spanOf({ key: "A-1", start_date: "2026-11-02", due_date: "2026-11-05" })).toEqual({ from: "2026-11-02", to: "2026-11-05" });
    expect(spanOf({ key: "A-1", due_date: "2026-11-05" })).toEqual({ from: "2026-11-05", to: "2026-11-05" });
    expect(spanOf({ key: "A-1", start_date: "2026-11-02" })).toEqual({ from: "2026-11-02", to: "2026-11-02" });
    expect(spanOf({ key: "A-1", start_date: "2026-11-09", due_date: "2026-11-05" })).toEqual({ from: "2026-11-05", to: "2026-11-09" });
    expect(spanOf({ key: "A-1" })).toBeUndefined();
  });

  it("moves both dates together", () => {
    expect(shifted({ key: "A-1", start_date: "2026-11-02", due_date: "2026-11-05" }, 3)).toEqual({ start_date: "2026-11-05", due_date: "2026-11-08" });
    expect(shifted({ key: "A-1", due_date: "2026-11-30" }, 2)).toEqual({ due_date: "2026-12-02" });
    expect(shifted({ key: "A-1", due_date: "2026-11-30" }, 0)).toBeUndefined();
    expect(shifted({ key: "A-1" }, 1)).toBeUndefined();
  });
});

describe("the bars of a week", () => {
  const week = weeksOf("2026-11-23", "week", 1)[0] ?? [];

  it("spans multi-day tasks and clips them at the week's edges", () => {
    const bars = layoutWeek(week, [
      { key: "A-1", start_date: "2026-11-24", due_date: "2026-11-26" },
      { key: "A-2", start_date: "2026-11-20", due_date: "2026-11-24" },
      { key: "A-3", start_date: "2026-11-28", due_date: "2026-12-02" },
      { key: "A-4", due_date: "2026-12-10" },
    ]);
    const byKey = new Map(bars.map((bar) => [bar.task.key, bar]));
    expect(bars).toHaveLength(3);
    expect(byKey.get("A-2")).toMatchObject({ column: 0, length: 2, continuesBefore: true, continuesAfter: false });
    expect(byKey.get("A-1")).toMatchObject({ column: 1, length: 3, continuesBefore: false, continuesAfter: false });
    expect(byKey.get("A-3")).toMatchObject({ column: 5, length: 2, continuesBefore: false, continuesAfter: true });
  });

  it("stacks overlapping tasks in lanes and reuses free lanes", () => {
    const bars = layoutWeek(week, [
      { key: "A-1", start_date: "2026-11-23", due_date: "2026-11-25" },
      { key: "A-2", due_date: "2026-11-24" },
      { key: "A-3", due_date: "2026-11-27" },
      { key: "A-4", start_date: "2026-11-24", due_date: "2026-11-27" },
    ]);
    const lane = (key: string) => bars.find((bar) => bar.task.key === key)?.lane;
    expect(lane("A-1")).toBe(0);
    expect(lane("A-4")).toBe(1);
    expect(lane("A-2")).toBe(2);
    // Friday is free in the top lane.
    expect(lane("A-3")).toBe(0);
    expect(tasksOn(bars, week, "2026-11-24").map((task) => task.key)).toEqual(["A-1", "A-4", "A-2"]);
  });

  it("counts what doesn't fit per day", () => {
    const tasks = ["A-1", "A-2", "A-3", "A-4", "A-5"].map((key) => ({ key, due_date: "2026-11-25" }));
    tasks.push({ key: "A-6", due_date: "2026-11-26" });
    const bars = layoutWeek(week, [...tasks, { key: "A-7", start_date: "2026-11-24", due_date: "2026-11-26" }]);
    // The span takes the top lane; five single days under it on Wednesday, three below the limit.
    expect(hiddenPerDay(bars, 3)).toEqual([0, 0, 3, 0, 0, 0, 0]);
    expect(hiddenPerDay(bars, 10)).toEqual([0, 0, 0, 0, 0, 0, 0]);
  });

  it("orders keys by number, so the layout is stable", () => {
    const bars = layoutWeek(week, [
      { key: "A-10", due_date: "2026-11-25" },
      { key: "A-9", due_date: "2026-11-25" },
    ]);
    expect(bars.map((bar) => [bar.task.key, bar.lane])).toEqual([
      ["A-9", 0],
      ["A-10", 1],
    ]);
  });
});

describe("the query for the days on screen", () => {
  it("adds the days to the person's query as a group, keeping its sort outside", () => {
    expect(rangeQuery("", "2026-11-01", "2026-11-30")).toBe("during:2026-11-01..2026-11-30");
    expect(rangeQuery("label:bug OR is:blocked sort:due", "2026-11-01", "2026-11-30")).toBe("(label:bug OR is:blocked) during:2026-11-01..2026-11-30 sort:due");
    expect(rangeQuery("sort:-priority", "2026-11-01", "2026-11-07")).toBe("during:2026-11-01..2026-11-07 sort:-priority");
  });

  it("moves only a sort outside quotes and parentheses", () => {
    expect(splitSort('"sort:me" is:open SORT:due')).toEqual({ rest: '"sort:me" is:open', sort: ["SORT:due"] });
    expect(splitSort("(a sort:due) b")).toEqual({ rest: "(a sort:due) b", sort: [] });
    expect(splitSort("resort:x")).toEqual({ rest: "resort:x", sort: [] });
  });
});
