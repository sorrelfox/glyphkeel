// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type RulePreviewInput } from "../../generated/ops";
import { afterCompletion, buildRule, firstDay, nextAfter, parseRule, previewDates, RuleProblem, summaryOf, toCore, upcoming, type Core } from "./recurrenceRules";

// 2026-11-26 is a Thursday.
const START = "2026-11-26";
const read = (input: RulePreviewInput, start = START) => buildRule(input, start, null, (_field, text) => text);
const dates = (input: RulePreviewInput, limit = 4, start = START) => previewDates(read(input, start), limit);

describe("counting a rule's dates", () => {
  it("repeats every N days from the start", () => {
    expect(dates({ every: "day", interval: 3 })).toEqual(["2026-11-26", "2026-11-29", "2026-12-02", "2026-12-05"]);
  });

  it("falls on the chosen weekdays, every N weeks counted from the start's week", () => {
    expect(dates({ text: "every monday and thursday" })).toEqual(["2026-11-26", "2026-11-30", "2026-12-03", "2026-12-07"]);
    expect(dates({ every: "week", interval: 2, weekdays: ["mon", "fri"], count: 3 })).toEqual(["2026-11-27", "2026-12-07", "2026-12-11"]);
    // Without weekdays, the start's.
    expect(read({ every: "week" }).weekdays).toEqual(["thu"]);
  });

  it("keeps the day of the month, the 31st being the last day of shorter months", () => {
    expect(dates({ text: "the 31st of each month" }, 3, "2027-01-01")).toEqual(["2027-01-31", "2027-02-28", "2027-03-31"]);
    expect(dates({ every: "month", month_day: -1 }, 3, "2028-01-15")).toEqual(["2028-01-31", "2028-02-29", "2028-03-31"]);
  });

  it("finds the nth and the last weekday of a month, passing over months without a fifth", () => {
    expect(dates({ text: "monthly on the last fri" }, 3)).toEqual(["2026-11-27", "2026-12-25", "2027-01-29"]);
    expect(dates({ every: "month", week_of_month: 2, weekday: "tue" }, 2)).toEqual(["2026-12-08", "2027-01-12"]);
    expect(dates({ every: "month", week_of_month: 5, weekday: "mon" }, 2, "2026-12-01")).toEqual(["2027-03-29", "2027-05-31"]);
  });

  it("repeats yearly in a month, with 29 February as the 28th in other years", () => {
    expect(dates({ text: "yearly in march" }, 2)).toEqual(["2027-03-26", "2028-03-26"]);
    expect(dates({ every: "year" }, 3, "2028-02-29")).toEqual(["2028-02-29", "2029-02-28", "2030-02-28"]);
  });

  it("stops at its end day or after its count", () => {
    expect(dates({ text: "daily until 2026-11-28" }, 5)).toEqual(["2026-11-26", "2026-11-27", "2026-11-28"]);
    expect(dates({ text: "every monday, 2 times" }, 5)).toEqual(["2026-11-30", "2026-12-07"]);
  });

  it("counts from completion in whole units, months keeping the day", () => {
    const rule: Core = toCore(read({ text: "1 month after done" }));
    expect(rule.from).toBe("completion");
    expect(afterCompletion(rule, "2027-01-31")).toBe("2027-02-28");
    expect(upcoming(toCore(read({ text: "3 days after done" })), "2026-11-26", 2, 1)).toEqual(["2026-11-29", "2026-12-02"]);
  });

  it("starts no earlier than its start", () => {
    const rule = toCore(read({ text: "every monday" }));
    expect(firstDay(rule)).toBe("2026-11-30");
    expect(nextAfter(rule, "2026-01-01")).toBe("2026-11-30");
  });
});

describe("reading a rule", () => {
  it("reads the engine's forms", () => {
    expect(parseRule("every 2 weeks on mon and thu", START)).toMatchObject({ every: "week", interval: 2, weekdays: ["mon", "thu"] });
    expect(parseRule("weekdays", START)).toMatchObject({ every: "week", weekdays: ["mon", "tue", "wed", "thu", "fri"] });
    expect(parseRule("monthly on the 15th", START)).toMatchObject({ every: "month", monthDay: 15 });
    expect(parseRule("every other month", START)).toMatchObject({ every: "month", interval: 2 });
    expect(parseRule("every sunday, 10 times", START)).toMatchObject({ every: "week", weekdays: ["sun"], count: 10 });
  });

  it("names what it can't read", () => {
    const problem = (() => {
      try {
        read({ text: "now and then" });
      } catch (error) {
        return error;
      }
      return null;
    })();
    expect(problem).toBeInstanceOf(RuleProblem);
    expect((problem as RuleProblem).messageId).toBe("value-recurrence-text");
    expect(() => read({ text: "3 days after done", create: "ahead" })).toThrow(/completion/);
    expect(() => read({ every: "month", weekdays: ["mon"] })).toThrow(RuleProblem);
  });

  it("lets fields override the text", () => {
    expect(read({ text: "every monday", interval: 3 })).toMatchObject({ every: "week", interval: 3, weekdays: ["mon"] });
  });
});

describe("the rule in words", () => {
  it("writes what the engine writes", () => {
    expect(summaryOf(read({ every: "week", interval: 2, weekdays: ["mon", "fri"], count: 3 }))).toBe("Every 2 weeks on Monday and Friday, 3 times");
    expect(summaryOf(read({ text: "monthly on the last fri" }))).toBe("Every month on the last Friday");
    expect(summaryOf(read({ text: "3 days after done, 1 times" }))).toBe("3 days after it is done, once");
    expect(summaryOf(read({ every: "year", until: "2030-01-01" }))).toBe("Every year on November 26, until 2030-01-01");
  });
});
