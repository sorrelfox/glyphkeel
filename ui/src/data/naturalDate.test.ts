// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { addMonths, parseNaturalDate } from "./naturalDate";

// Monday 23 November 2026, as in the engine's tests.
const TODAY = "2026-11-23";
const n = (text: string) => parseNaturalDate(text, TODAY, "monday");

describe("friendly dates", () => {
  it("reads the same forms as the engine", () => {
    expect(n("fri")).toBe("2026-11-27");
    expect(n("Friday")).toBe("2026-11-27");
    expect(n("mon")).toBe("2026-11-30");
    expect(n("next fri")).toBe("2026-12-04");
    expect(n("this sun")).toBe("2026-11-29");
    expect(n("in 2 weeks")).toBe("2026-12-07");
    expect(n("in a month")).toBe("2026-12-23");
    expect(n("in 3 days")).toBe("2026-11-26");
    expect(n("dec 1")).toBe("2026-12-01");
    expect(n("1 Dec")).toBe("2026-12-01");
    expect(n("nov 1")).toBe("2027-11-01");
    expect(n("december 1 2028")).toBe("2028-12-01");
    expect(n("feb 29")).toBe("2028-02-29");
    expect(n("end of month")).toBe("2026-11-30");
    expect(n("end of week")).toBe("2026-11-29");
    expect(n("next week")).toBe("2026-11-30");
    expect(n("today")).toBe("2026-11-23");
    expect(n("tomorrow")).toBe("2026-11-24");
    expect(n("+3d")).toBe("2026-11-26");
    expect(n("2026-12-24")).toBe("2026-12-24");
    expect(n("someday")).toBeNull();
    expect(n("fr")).toBeNull();
    expect(n("")).toBeNull();
    expect(n("2026-02-30")).toBeNull();
    expect(n("buy milk")).toBeNull();
  });

  it("follows the first day of the week", () => {
    expect(parseNaturalDate("end of week", TODAY, "sunday")).toBe("2026-11-28");
    expect(parseNaturalDate("next week", TODAY, "sunday")).toBe("2026-11-29");
    expect(parseNaturalDate("this mon", TODAY, "saturday")).toBe("2026-11-23");
  });

  it("keeps month ends and leap days", () => {
    expect(addMonths("2026-01-31", 1)).toBe("2026-02-28");
    expect(addMonths("2028-01-31", 1)).toBe("2028-02-29");
    expect(addMonths("2026-03-31", -1)).toBe("2026-02-28");
    expect(parseNaturalDate("+1m", "2026-01-31")).toBe("2026-02-28");
  });
});
