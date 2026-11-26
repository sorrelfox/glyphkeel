// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The person's layout of a project's Gantt chart: the zoom level, the
// table's width and its columns' widths, the grouping, and whether moving a
// task also moves the tasks that depend on it. The engine keeps it per
// project with the person's other view settings (`gantt.layout.*`), apart
// from the workspace's settings.

import { type GanttGrouping, type GanttLayout as EngineLayout, type GanttLayoutUpdateInput } from "../../generated/ops";
import { ZOOMS, type Zoom } from "./scale";

/** The table's columns, in order. */
export const COLUMNS = ["key", "title", "claim", "start", "due", "progress"] as const;
export type Column = (typeof COLUMNS)[number];

/** The columns whose width the person sets; the title takes the rest. */
export const SIZED: readonly Column[] = ["key", "claim", "start", "due", "progress"];

/** The ways to group the rows, as the toolbar offers them. */
export const GROUPINGS: readonly GanttGrouping[] = ["parent", "label", "status", "claim", "none"];

/** The narrowest and widest table and column, in pixels. */
export const TABLE_MIN = 240;
export const TABLE_MAX = 900;
export const COLUMN_MIN = 32;
export const COLUMN_MAX = 400;

export interface GanttLayout {
  zoom: Zoom;
  /** The table's width in pixels; none: the default. */
  table: number | undefined;
  /** Column widths in pixels; the columns left out keep their default. */
  widths: Partial<Record<Column, number>>;
  groupBy: GanttGrouping;
  /** Whether a change of dates also moves the tasks that depend on the task. */
  moveDependents: boolean;
}

export const DEFAULT_LAYOUT: GanttLayout = { zoom: "day", table: undefined, widths: {}, groupBy: "parent", moveDependents: false };

const clamp = (value: number, min: number, max: number) => Math.round(Math.min(max, Math.max(min, value)));

/** The engine's layout, checked: anything missing or out of the chart's range takes its default or nearest value. */
export function layoutFrom(stored: Partial<EngineLayout> | undefined): GanttLayout {
  const value: Partial<EngineLayout> = stored ?? {};
  const zoom = ZOOMS.find((candidate) => candidate === value.zoom) ?? DEFAULT_LAYOUT.zoom;
  const table = typeof value.table === "number" && Number.isFinite(value.table) ? clamp(value.table, TABLE_MIN, TABLE_MAX) : undefined;
  const widths: Partial<Record<Column, number>> = {};
  for (const column of SIZED) {
    const width: unknown = value.widths?.[column];
    if (typeof width === "number" && Number.isFinite(width)) {
      widths[column] = clamp(width, COLUMN_MIN, COLUMN_MAX);
    }
  }
  const groupBy = GROUPINGS.find((candidate) => candidate === value.group_by) ?? DEFAULT_LAYOUT.groupBy;
  return { zoom, table, widths, groupBy, moveDependents: value.move_dependents === true };
}

/** The `gantt.layout.update` input that makes `change` to project `project`'s layout. */
export function layoutUpdate(project: string, change: Partial<GanttLayout>): GanttLayoutUpdateInput {
  return {
    project,
    ...(change.zoom === undefined ? {} : { zoom: change.zoom }),
    ...(change.table === undefined ? {} : { table: change.table }),
    ...(change.widths === undefined ? {} : { widths: Object.fromEntries(Object.entries(change.widths)) }),
    ...(change.groupBy === undefined ? {} : { group_by: change.groupBy }),
    ...(change.moveDependents === undefined ? {} : { move_dependents: change.moveDependents }),
  };
}
