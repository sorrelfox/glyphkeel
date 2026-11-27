// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type BurndownDay, type Sprint } from "../generated/ops";
import { chartDays, idealLine, joinable, nextNumber, remaining, shownSprints } from "./sprints";

const sprint = (name: string, state: Sprint["state"]): Sprint => ({
  id: name,
  name,
  project: "TM",
  state,
  progress: { total: 0, closed: 0, estimate_minutes: 0, closed_estimate_minutes: 0 },
  rank: "1",
  created_at: "",
  updated_at: "",
});

const day = (date: string, open: number): BurndownDay => ({ date, open_tasks: open, total_tasks: 4, open_estimate_minutes: open * 60, total_estimate_minutes: 240 });

describe("sprints", () => {
  it("shows the active sprint first, then planned ones, closed ones on request", () => {
    const all = [sprint("S1", "closed"), sprint("S2", "closed"), sprint("S3", "active"), sprint("S4", "planned")];
    expect(shownSprints(all, false).map((s) => s.name)).toEqual(["S3", "S4"]);
    expect(shownSprints(all, true).map((s) => s.name)).toEqual(["S3", "S4", "S2", "S1"]);
    expect(joinable(all).map((s) => s.name)).toEqual(["S3", "S4"]);
  });

  it("offers the next number", () => {
    expect(nextNumber([])).toBe(1);
    expect(nextNumber([sprint("Sprint 3", "closed"), sprint("Launch", "planned"), sprint("Sprint 7", "planned")])).toBe(8);
  });

  it("spans the sprint's days and draws the ideal line to zero", () => {
    const days = chartDays([day("2026-11-23", 4), day("2026-11-24", 3)], "2026-11-23", "2026-11-26");
    expect(days.map((d) => d.date)).toEqual(["2026-11-23", "2026-11-24", "2026-11-25", "2026-11-26"]);
    expect(days[2]?.day).toBeUndefined();
    expect(idealLine(6, 4)).toEqual([6, 4, 2, 0]);
    expect(remaining(day("2026-11-23", 2), "estimate")).toBe(120);
    // A sprint that ran over goes on to its last day of data.
    expect(chartDays([day("2026-11-23", 4), day("2026-11-25", 1)], "2026-11-23", "2026-11-24")).toHaveLength(3);
  });
});
