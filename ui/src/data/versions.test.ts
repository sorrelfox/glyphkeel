// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Version } from "../generated/ops";
import { isLate, markersOf, plannable, shownVersions } from "./versions";

function version(name: string, status: Version["status"], due?: string): Version {
  return {
    id: name,
    name,
    project: "TM",
    status,
    sharing: "none",
    progress: { total: 0, closed: 0, percent: 0, estimate_minutes: 0, spent_minutes: 0 },
    rank: "1",
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-01T00:00:00Z",
    ...(due === undefined ? {} : { due_date: due }),
  };
}

describe("versions", () => {
  const all = [version("0.9", "closed", "2026-11-01"), version("1.0", "open", "2026-12-01"), version("1.1", "locked")];

  it("hides closed versions unless asked, and offers only open ones for tasks", () => {
    expect(shownVersions(all, false).map((v) => v.name)).toEqual(["1.0", "1.1"]);
    expect(shownVersions(all, true)).toHaveLength(3);
    expect(plannable(all).map((v) => v.name)).toEqual(["1.0"]);
  });

  it("marks the due versions on the chart and tells late ones", () => {
    expect(markersOf(all).map((m) => [m.name, m.day])).toEqual([
      ["0.9", "2026-11-01"],
      ["1.0", "2026-12-01"],
    ]);
    expect(markersOf(undefined)).toEqual([]);
    expect(isLate(version("1.0", "open", "2026-11-20"), "2026-11-23")).toBe(true);
    expect(isLate(version("0.9", "closed", "2026-11-20"), "2026-11-23")).toBe(false);
    expect(isLate(version("1.1", "open"), "2026-11-23")).toBe(false);
  });
});
