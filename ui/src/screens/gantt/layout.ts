// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The person's layout of a project's Gantt chart: the zoom level, the
// table's width and its columns' widths, and whether moving a task also
// moves the tasks that depend on it. Kept with the person's other settings
// of this app (like the board's card size), per project ID.

import { readPref, writePref } from "../../prefs/store";
import { ZOOMS, type Zoom } from "./scale";

/** The table's columns, in order. */
export const COLUMNS = ["key", "title", "start", "due", "progress"] as const;
export type Column = (typeof COLUMNS)[number];

/** The columns whose width the person sets; the title takes the rest. */
export const SIZED: readonly Column[] = ["key", "start", "due", "progress"];

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
  /** Whether a change of dates also moves the tasks that depend on the task. */
  moveDependents: boolean;
}

export const DEFAULT_LAYOUT: GanttLayout = { zoom: "day", table: undefined, widths: {}, moveDependents: false };

const clamp = (value: number, min: number, max: number) => Math.round(Math.min(max, Math.max(min, value)));

/** A layout as stored, checked: anything missing or out of range takes its default. */
export function layoutFrom(stored: unknown): GanttLayout {
  const value = typeof stored === "object" && stored !== null ? (stored as Record<string, unknown>) : {};
  const zoom = ZOOMS.find((candidate) => candidate === value.zoom) ?? DEFAULT_LAYOUT.zoom;
  const table = typeof value.table === "number" && Number.isFinite(value.table) ? clamp(value.table, TABLE_MIN, TABLE_MAX) : undefined;
  const widths: Partial<Record<Column, number>> = {};
  if (typeof value.widths === "object" && value.widths !== null) {
    for (const column of SIZED) {
      const width = (value.widths as Record<string, unknown>)[column];
      if (typeof width === "number" && Number.isFinite(width)) {
        widths[column] = clamp(width, COLUMN_MIN, COLUMN_MAX);
      }
    }
  }
  return { zoom, table, widths, moveDependents: value.moveDependents === true };
}

const prefName = (project: string) => `gantt.layout.${project}`;

export function loadLayout(project: string): GanttLayout {
  return layoutFrom(readPref(prefName(project)));
}

export function saveLayout(project: string, layout: GanttLayout): void {
  writePref(prefName(project), layout);
}
