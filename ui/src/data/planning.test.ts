// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { afterEach, describe, expect, it } from "vitest";

import { type EstimateGroup, type EstimateTask } from "../generated/ops";
import { estimateHint, followEstimateHint, forecastLine, loadEstimateHint, loadMeasure, ratioBins, saveMeasure, setEstimateHint } from "./planning";

afterEach(() => {
  localStorage.clear();
});

const task = (ratio: number): EstimateTask => ({ key: "TM-1", title: "", type: "Task", closed_on: "2026-11-23", estimate_minutes: 60, spent_minutes: 60 * ratio, ratio });

const group = (key: string, count: number, median: number): EstimateGroup => ({ key, name: key, count, estimate_minutes: 0, spent_minutes: 0, median_ratio: median, within_share: 0, over: 0, under: 0, outliers: [] });

describe("the planning reports' arithmetic", () => {
  it("puts ratios in the histogram's ranges, the bounds of 20 % on target", () => {
    expect(ratioBins([0.5, 0.6, 0.8, 1, 1.2, 1.21, 1.5, 2, 2.5].map(task))).toEqual([1, 1, 3, 2, 1, 1]);
  });

  it("hints only from enough tasks whose estimates are off", () => {
    const groups = [group("bug", 6, 1.4), group("task", 6, 1.05), group("story", 4, 0.5), group("chore", 5, 0.7)];
    expect(estimateHint(groups, "Bug")).toEqual({ way: "over", ratio: 1.4, count: 6 });
    expect(estimateHint(groups, "Task")).toBeNull();
    expect(estimateHint(groups, "Story")).toBeNull();
    expect(estimateHint(groups, "Chore")).toEqual({ way: "under", ratio: 0.7, count: 5 });
    expect(estimateHint(groups, "Epic")).toBeNull();
  });

  it("draws the forecast to the scope, or to the chart's end when that comes first", () => {
    const forecast = { date: "2026-11-27", start: 0.2, per_day: 0.7 };
    expect(forecastLine("2026-11-23", forecast, 3, 10)).toEqual([
      [0, 0.2],
      [4, 3],
    ]);
    const end = forecastLine("2026-11-23", forecast, 3, 3);
    expect(end?.[1][0]).toBe(2);
    expect(end?.[1][1]).toBeCloseTo(1.6);
    expect(forecastLine("2026-11-23", undefined, 3, 10)).toBeUndefined();
  });

  it("keeps the measure per project and the hint per person", () => {
    expect(loadMeasure("TM")).toEqual({ measure: "tasks" });
    saveMeasure("TM", { measure: "field", field: "cf.points" });
    expect(loadMeasure("TM")).toEqual({ measure: "field", field: "cf.points" });
    expect(loadMeasure("ZZ")).toEqual({ measure: "tasks" });
    expect(loadEstimateHint()).toBe(false);
    const heard: boolean[] = [];
    const stop = followEstimateHint((on) => heard.push(on));
    setEstimateHint(true);
    stop();
    setEstimateHint(false);
    expect(heard).toEqual([true]);
    expect(loadEstimateHint()).toBe(false);
  });
});
