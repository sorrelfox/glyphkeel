// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Schedule, type ScheduledTask } from "../../generated/ops";
import { chartSvg, exportSpan, pngScale, type ExportInput, type Palette } from "./export";
import { buildRows } from "./model";

const status = { id: "s1", name: "To do", category: "todo" as const };
const bar = (key: string, start: string, due: string, extra: Partial<ScheduledTask> = {}): ScheduledTask => ({
  blocked: false,
  closed: false,
  dependents: 0,
  depth: 0,
  id: `id-${key}`,
  key,
  overdue: false,
  progress_percent: 0,
  project: "TM",
  shape: "bar",
  start_date: start,
  due_date: due,
  status,
  title: `Title of ${key} <&>`,
  version: 1,
  ...extra,
});

const schedule: Schedule = {
  today: "2026-11-05",
  range: { start: "2026-11-01", end: "2026-11-20" },
  tasks: [bar("TM-1", "2026-11-01", "2026-11-10", { progress_percent: 10 }), bar("TM-2", "2026-11-12", "2026-11-20")],
  unscheduled: [],
  unscheduled_total: 0,
  dependencies: [{ id: "d1", from: "TM-1", to: "TM-2", violated: false }],
  problems: [],
};

const colours = Object.fromEntries(["surface", "sunken", "border", "fg", "muted", "accent", "bar", "progress", "today", "warning", "danger", "milestone", "overdueBar", "startBar"].map((name) => [name, `var-${name}`])) as unknown as Palette;

function input(rtl: boolean): ExportInput {
  return {
    schedule,
    rows: buildRows(schedule.tasks, schedule.unscheduled, new Set()).rows,
    zoom: "day",
    rtl,
    weekStart: 1,
    weekend: new Set([0, 6]),
    palette: colours,
    font: "sans-serif",
    text: {
      title: "Tasks: November",
      columns: { key: "Key", title: "Title", start: "Start", due: "Due", progress: "%" },
      cell: (cell) => cell.first.slice(8),
      day: (day) => day,
      percent: (share) => `${String(Math.round(share * 100))}%`,
      group: () => "group",
      note: (task) => (task.key === "TM-1" ? { text: "Behind", tone: "warning" } : undefined),
    },
  };
}

describe("the picture of the chart", () => {
  it("covers every task with a few days around, and today", () => {
    expect(exportSpan(schedule, "day")).toEqual({ from: "2026-10-29", to: "2026-11-23" });
    expect(exportSpan({ ...schedule, today: "2026-12-24" }, "week")).toEqual({ from: "2026-10-25", to: "2026-12-31" });
  });

  it("draws the rows, the bars, the dependency and today, with its text escaped", () => {
    const { svg, width, height } = chartSvg(input(false));
    // Table 626 px and 26 days at 32 px; title, header and two rows.
    expect(width).toBe(626 + 26 * 32);
    expect(height).toBe(40 + 48 + 2 * 32 + 8);
    expect(svg).toContain("Title of TM-1 &lt;&amp;&gt;");
    expect(svg).toContain("Behind");
    expect(svg).toContain(`stroke="var-today"`);
    expect(svg).toContain(`fill="var-progress"`);
    // The dashed outline of a bar behind schedule.
    expect(svg).toContain(`stroke-dasharray="4 3"`);
    expect(svg.match(/<path d="M/g)?.length).toBeGreaterThanOrEqual(2);
    expect(() => new DOMParser().parseFromString(svg, "image/svg+xml").documentElement.nodeName).not.toThrow();
    expect(new DOMParser().parseFromString(svg, "image/svg+xml").querySelector("parsererror")).toBeNull();
  });

  it("puts the table on the right and mirrors the timeline for right-to-left languages", () => {
    const { svg } = chartSvg(input(true));
    expect(svg).toContain(`direction="rtl"`);
    expect(svg).toContain(`matrix(-1 0 0 1 ${String(26 * 32)} 0)`);
  });

  it("scales a picture down to what a canvas holds, and up for sharpness", () => {
    expect(pngScale(1000, 800)).toBe(2);
    expect(pngScale(20_000, 1000)).toBeCloseTo(0.8);
    expect(pngScale(12_000, 12_000)).toBeLessThan(1);
  });
});
