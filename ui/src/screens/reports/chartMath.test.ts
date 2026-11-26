// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { bandPath, labelStep, niceAxis, slots, stack, stepIndex, yScale } from "./chartMath";

describe("the report charts' geometry", () => {
  it("rounds the value axis", () => {
    expect(niceAxis(7)).toEqual({ max: 8, ticks: [0, 2, 4, 6, 8] });
    expect(niceAxis(0)).toEqual({ max: 1, ticks: [0, 1] });
    expect(niceAxis(1234).max).toBe(1500);
  });

  it("runs time from the start edge, mirrored right to left", () => {
    const ltr = slots(4, 400, false, { start: 40, end: 0, top: 0, bottom: 0 });
    const rtl = slots(4, 400, true, { start: 40, end: 0, top: 0, bottom: 0 });
    expect(ltr.centre(0)).toBe(85);
    expect(rtl.centre(0)).toBe(315);
    expect(ltr.left(0)).toBe(40);
    expect(rtl.left(0)).toBe(270);
    expect(rtl.startEdge).toBe(360);
    expect(rtl.endEdge).toBe(0);
  });

  it("moves through the periods in the reading direction", () => {
    expect(stepIndex("ArrowRight", 1, 5, false)).toBe(2);
    expect(stepIndex("ArrowRight", 1, 5, true)).toBe(0);
    expect(stepIndex("ArrowLeft", 1, 5, true)).toBe(2);
    expect(stepIndex("End", 1, 5, false)).toBe(4);
    expect(stepIndex("ArrowRight", 4, 5, false)).toBe(4);
    expect(stepIndex("Enter", 1, 5, false)).toBeNull();
  });

  it("stacks bands from the bottom up and draws them", () => {
    const stacked = stack([[1, 2, 3]], [2, 1, 0]);
    expect(stacked[0]).toEqual([
      { low: 5, high: 6 },
      { low: 3, high: 5 },
      { low: 0, high: 3 },
    ]);
    expect(bandPath([0, 10], [5, 5], [1, 2])).toBe("M0,1L10,2L10,5L0,5Z");
    const y = yScale(10, 100, { start: 0, end: 0, top: 0, bottom: 0 });
    expect([y(0), y(10), y(5)]).toEqual([100, 0, 50]);
  });

  it("thins out labels that would overlap", () => {
    expect(labelStep(100, 8, 64)).toBe(8);
    expect(labelStep(5, 100, 64)).toBe(1);
  });
});
