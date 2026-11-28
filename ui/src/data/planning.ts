// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The planning reports' arithmetic apart from React, and the personal
// choices they keep: the measure each project's velocity was last read in,
// and whether the task panel hints at how estimates usually turn out.

import { type EstimateGroup, type EstimateTask, type PlanMeasure } from "../generated/ops";
import { readPref, writePref } from "../prefs/store";

/** The measure, and with `field` the number field, velocity is read in. */
export interface MeasureChoice {
  measure: PlanMeasure;
  field?: string;
}

const MEASURES: readonly PlanMeasure[] = ["tasks", "estimate", "field"];

/** The measure a project's velocity was last read in; tasks at first. */
export function loadMeasure(project: string): MeasureChoice {
  const stored = readPref(`velocity.measure.${project}`);
  if (typeof stored !== "object" || stored === null) {
    return { measure: "tasks" };
  }
  const value = stored as Record<string, unknown>;
  const measure = MEASURES.find((each) => each === value.measure) ?? "tasks";
  if (measure === "field") {
    return typeof value.field === "string" && value.field !== "" ? { measure, field: value.field } : { measure: "tasks" };
  }
  return { measure };
}

export function saveMeasure(project: string, choice: MeasureChoice): void {
  writePref(`velocity.measure.${project}`, choice);
}

/** Whether the task panel says how estimates of the task's type usually turn out (a personal choice, off at first). */
export function loadEstimateHint(): boolean {
  return readPref("task.estimateHint") === true;
}

export function saveEstimateHint(on: boolean): void {
  writePref("task.estimateHint", on);
}

/** The ranges of the ratio histogram: their upper bounds, the last open. */
export const RATIO_BINS = [0.5, 0.8, 1.2, 1.5, 2, Infinity] as const;

/** Which ranges are on target (within 20 %). */
export const ON_TARGET_BINS: readonly boolean[] = RATIO_BINS.map((upper) => upper === 1.2);

/** How many tasks fall in each range of the ratio of spent to estimated time. A bound belongs to the range below it, except 0.8, which is on target. */
export function ratioBins(tasks: readonly EstimateTask[]): number[] {
  const counts: number[] = RATIO_BINS.map(() => 0);
  for (const task of tasks) {
    const r = Math.round(task.ratio * 10_000) / 10_000;
    const i = r < 0.8 ? (r <= 0.5 ? 0 : 1) : RATIO_BINS.findIndex((upper, at) => at >= 2 && r <= upper);
    counts[i] = (counts[i] ?? 0) + 1;
  }
  return counts;
}

/** The fewest closed tasks of a type the hint speaks from. */
export const HINT_MIN_TASKS = 5;
/** How far from 1 the median ratio must be for a hint. */
export const HINT_MIN_OFF = 0.1;

/** What the hint says about a type's estimates: over or under, by the median ratio; none with too few tasks or estimates on target. */
export function estimateHint(groups: readonly EstimateGroup[], type: string): { way: "over" | "under"; ratio: number; count: number } | null {
  const fold = type.normalize("NFKC").toLowerCase().trim();
  const group = groups.find((each) => each.key === fold);
  const ratio = group?.median_ratio;
  if (group === undefined || ratio === undefined || group.count < HINT_MIN_TASKS || Math.abs(ratio - 1) < HINT_MIN_OFF) {
    return null;
  }
  return { way: ratio > 1 ? "over" : "under", ratio, count: group.count };
}

/** The points of a burn-up's forecast line: from the first day at the line's start to the forecast day at the scope. */
export function forecastLine(from: string, forecast: { date: string; start: number; per_day: number } | undefined, scope: number, days: number): readonly [readonly [number, number], readonly [number, number]] | undefined {
  if (forecast === undefined) {
    return undefined;
  }
  const reach = Math.round((Date.parse(`${forecast.date}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000);
  if (reach <= days - 1) {
    return [
      [0, forecast.start],
      [reach, scope],
    ];
  }
  const end = days - 1;
  return [
    [0, forecast.start],
    [end, forecast.start + forecast.per_day * end],
  ];
}

/** Who to tell when the hint is switched on or off, so open task panels follow at once. */
const hintListeners = new Set<(on: boolean) => void>();

/** Switches the estimate hint and tells every open task panel. */
export function setEstimateHint(on: boolean): void {
  saveEstimateHint(on);
  for (const listener of hintListeners) {
    listener(on);
  }
}

/** Follows the estimate hint choice: `listener` is told of every change; returns the way to stop. */
export function followEstimateHint(listener: (on: boolean) => void): () => void {
  hintListeners.add(listener);
  return () => {
    hintListeners.delete(listener);
  };
}
