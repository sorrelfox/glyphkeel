// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Dashboards: the list of them, each widget's value (one engine call per
// widget, cached and refreshed by change events), and the arithmetic of
// arranging widgets in the grid.

import { type Dashboard, type FlowBucket, type PlanMeasure, type TimeGroupBy, type Widget, type WidgetKind, type WidgetSetting } from "../generated/ops";
import { useOp } from "./hooks";

/** The grid's columns; a widget spans one to all of them. */
export const COLUMNS = 3;

/** The kinds in the order the app offers them. */
export const WIDGET_KINDS = [
  "query_count",
  "query_list",
  "tasks_by_field",
  "due_soon",
  "overdue",
  "created_closed",
  "cumulative_flow",
  "cycle_time",
  "time_report",
  "burndown",
  "version_progress",
  "burnup",
  "velocity",
  "estimate_accuracy",
  "activity",
  "automation_failures",
  "note",
] as const satisfies readonly WidgetKind[];

/** The settings each kind takes, as the engine checks them. */
export const KIND_SETTINGS: Readonly<Record<WidgetKind, readonly WidgetSetting[]>> = {
  query_count: ["query", "project"],
  query_list: ["query", "project", "limit"],
  created_closed: ["query", "project", "bucket"],
  cumulative_flow: ["query", "project", "bucket"],
  cycle_time: ["query", "project", "bucket"],
  time_report: ["project", "days", "group_by"],
  burndown: ["project"],
  version_progress: ["project"],
  burnup: ["project", "version", "measure", "field"],
  velocity: ["project", "measure", "field"],
  estimate_accuracy: ["query", "project", "days"],
  tasks_by_field: ["field", "query", "project"],
  due_soon: ["query", "project", "days", "limit"],
  overdue: ["query", "project", "limit"],
  activity: ["project", "actor_kind", "days", "limit"],
  automation_failures: ["project", "limit"],
  note: ["text"],
};

/** The kinds drawn as a chart, which also read as a table. */
export const CHART_KINDS: readonly WidgetKind[] = ["created_closed", "cumulative_flow", "cycle_time", "burndown", "burnup", "velocity", "estimate_accuracy"];

/** The kinds whose figure is a query's tasks, so that the list can open with it. */
export const QUERY_KINDS: readonly WidgetKind[] = ["query_count", "query_list", "due_soon", "overdue", "tasks_by_field"];

/** The built-in fields tasks are counted by; custom fields come as `cf.<name>`. */
export const COUNT_FIELDS = ["status", "priority", "type", "label"] as const;

export const BUCKETS = ["day", "week", "month"] as const satisfies readonly FlowBucket[];
export const TIME_GROUPS = ["project", "task", "person", "work_kind", "day", "week", "month"] as const satisfies readonly TimeGroupBy[];

/** The kinds that need a project when the dashboard has none. */
export const NEEDS_PROJECT: readonly WidgetKind[] = ["burndown", "version_progress", "burnup", "velocity"];

/** How velocity and burn-up widgets measure work. */
export const MEASURES = ["tasks", "estimate", "field"] as const satisfies readonly PlanMeasure[];

/** How wide a widget of `kind` starts: charts take two columns. */
export function defaultWidth(kind: WidgetKind): number {
  return CHART_KINDS.includes(kind) ? 2 : 1;
}

/** Every dashboard: the workspace's, then each project's. */
export function useDashboards() {
  return useOp("dashboard.list", { all: true });
}

/** What one widget shows now: one engine call, cached, re-read after every change. */
export function useWidgetValue(dashboard: string, widget: string) {
  return useOp("dashboard.widget", { dashboard, widget });
}

/** The dashboard to show: the one asked for, else the first pinned to the project it was opened from, else the first. */
export function chosenDashboard(dashboards: readonly Dashboard[], id: string | undefined, from?: string): Dashboard | undefined {
  return dashboards.find((each) => each.id === id) ?? (from === undefined ? undefined : pinnedTo(dashboards, from)[0]) ?? dashboards[0];
}

/** The dashboards pinned to the project with key `project`, in order. */
export function pinnedTo(dashboards: readonly Dashboard[], project: string): Dashboard[] {
  return dashboards.filter((each) => each.project === project);
}

/** The place (1 is the first) a widget moves to one step `step` away, or null at an end. */
export function stepPlace(widgets: readonly Widget[], id: string, step: number): number | null {
  const at = widgets.findIndex((widget) => widget.id === id);
  const to = at + step;
  return at < 0 || to < 0 || to >= widgets.length ? null : to + 1;
}

/** The widgets with the one of `id` moved to `place` (1 is the first), as the engine arranges them. */
export function moved(widgets: readonly Widget[], id: string, place: number): Widget[] {
  const at = widgets.findIndex((widget) => widget.id === id);
  const moving = widgets[at];
  if (at < 0 || moving === undefined) {
    return [...widgets];
  }
  const rest = widgets.filter((widget) => widget.id !== id);
  rest.splice(Math.max(0, Math.min(rest.length, place - 1)), 0, moving);
  return rest;
}

/** A width kept within the grid. */
export function clampWidth(width: number): number {
  return Math.max(1, Math.min(COLUMNS, Math.round(width)));
}

/**
 * The width a widget gets when its end edge is dragged `delta` pixels in
 * the reading direction, in a grid whose columns are `column` pixels wide
 * with `gap` between them.
 */
export function widthAfterDrag(start: number, delta: number, column: number, gap: number): number {
  const step = column + gap;
  if (step <= 0) {
    return clampWidth(start);
  }
  return clampWidth(start + Math.round(delta / step));
}
