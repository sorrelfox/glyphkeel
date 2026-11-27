// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { parseNaturalDay } from "./naturalDate";

// Monday 23 November 2026, as in the engine's own tests.
const TODAY = "2026-11-23";
const read = (text: string, start: "monday" | "sunday" | "saturday" = "monday") => parseNaturalDay(text, TODAY, start);

describe("parseNaturalDay", () => {
  it("reads days, offsets and periods", () => {
    expect(read("today")).toBe("2026-11-23");
    expect(read("Tomorrow")).toBe("2026-11-24");
    expect(read("yesterday")).toBe("2026-11-22");
    expect(read("+3d")).toBe("2026-11-26");
    expect(read("+2w")).toBe("2026-12-07");
    expect(read("-1m")).toBe("2026-10-23");
    expect(read("+1y")).toBe("2027-11-23");
    expect(read("2026-12-01")).toBe("2026-12-01");
    expect(read("2026-2-30")).toBeNull();
    expect(read("next-month")).toBe("2026-12-01");
  });

  it("reads weekdays as the engine does", () => {
    expect(read("mon")).toBe("2026-11-30");
    expect(read("fri")).toBe("2026-11-27");
    expect(read("friday")).toBe("2026-11-27");
    expect(read("fr")).toBeNull();
    expect(read("this fri")).toBe("2026-11-27");
    expect(read("next fri")).toBe("2026-12-04");
    expect(read("next fri", "sunday")).toBe("2026-12-04");
    expect(read("next week")).toBe("2026-11-30");
    expect(read("next week", "sunday")).toBe("2026-11-29");
  });

  it("reads spans, month names and ends", () => {
    expect(read("in 3 days")).toBe("2026-11-26");
    expect(read("in a week")).toBe("2026-11-30");
    expect(read("in 3d")).toBe("2026-11-26");
    expect(read("in 1m")).toBe("2026-12-23");
    expect(read("dec 1")).toBe("2026-12-01");
    expect(read("1 dec")).toBe("2026-12-01");
    expect(read("nov 1")).toBe("2027-11-01");
    expect(read("december 1, 2027")).toBe("2027-12-01");
    expect(read("feb 29")).toBe("2028-02-29");
    expect(read("end of month")).toBe("2026-11-30");
    expect(read("end of the year")).toBe("2026-12-31");
    expect(read("end of week")).toBe("2026-11-29");
  });

  it("answers null for anything else", () => {
    expect(read("")).toBeNull();
    expect(read("soon")).toBeNull();
    expect(read("next lunch")).toBeNull();
    expect(read("11/27/2026")).toBeNull();
  });
});
