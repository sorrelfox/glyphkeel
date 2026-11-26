// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Schedule } from "../generated/ops";
import { firstClaimEnd } from "./schedule";

const NOW = Date.parse("2026-11-11T10:00:00Z");

function schedule(tasks: (string | undefined)[], unscheduled: (string | undefined)[] = []): Schedule {
  const claimed = (until: string | undefined) => (until === undefined ? {} : { claim: { agent: "Build agent", session: "s", until } });
  return { tasks: tasks.map(claimed), unscheduled: unscheduled.map(claimed) } as unknown as Schedule;
}

describe("when the chart reads its schedule again for a claim that ends", () => {
  it("is the first claim to end, scheduled or not", () => {
    expect(firstClaimEnd(schedule(["2026-11-11T10:45:00Z", undefined, "2026-11-11T10:30:00Z"]), NOW)).toBe(Date.parse("2026-11-11T10:30:00Z"));
    expect(firstClaimEnd(schedule([], ["2026-11-11T10:05:00Z"]), NOW)).toBe(Date.parse("2026-11-11T10:05:00Z"));
  });

  it("is never, without claims or with only ended ones", () => {
    expect(firstClaimEnd(undefined, NOW)).toBeUndefined();
    expect(firstClaimEnd(schedule([undefined]), NOW)).toBeUndefined();
    expect(firstClaimEnd(schedule(["2026-11-11T09:59:00Z", "not a time"]), NOW)).toBeUndefined();
  });
});
