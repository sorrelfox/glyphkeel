// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { NATURAL_DATES } from "../../generated/naturalDates";
import { FakeWorkspace } from "./engine";
import { parseNaturalDay } from "./naturalDate";
import { seed } from "./seed";

// The engine's worked examples, with the day its parser gives for each.
describe("the test engine's friendly dates", () => {
  it("has the engine's examples", () => {
    expect(NATURAL_DATES.length).toBeGreaterThan(50);
  });

  it.each(NATURAL_DATES.map((each) => [each.text, each.today, each.week_start, each.day] as const))("reads %j on %s (weeks from %s) as %s", (text, today, start, day) => {
    expect(parseNaturalDay(text, today, start)).toBe(day);
  });

  it("answers date.parse as the engine does", () => {
    const now = new Date(2026, 10, 26, 12, 0);
    const ws = new FakeWorkspace(seed(now), () => now);
    for (const each of NATURAL_DATES) {
      ws.call("settings.update", { week_start: each.week_start });
      const call = () => ws.call("date.parse", { text: each.text, today: each.today }).output;
      if (each.day === null) {
        expect(call, each.text).toThrow();
      } else {
        expect(call(), each.text).toEqual({ date: each.day });
      }
    }
    expect(ws.call("date.parse", { text: "tomorrow" }).output).toEqual({ date: "2026-11-27" });
    expect(() => ws.call("date.parse", { text: "fri", today: "monday" })).toThrow();
  });
});
