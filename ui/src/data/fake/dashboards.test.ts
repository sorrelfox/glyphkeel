// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type DashboardChange, type DashboardList, type DashboardShow, type WidgetValue } from "../../generated/ops";
import { combined, quote } from "./dashboards";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function failure(run: () => unknown): { code: string; path: unknown } {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return { code: error.error.code, path: error.error.details?.path };
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

describe("queries of widgets", () => {
  it("combine the project, the query, extra conditions and an order as the engine does", () => {
    expect(combined("TM", "label:bug", ["is:open"], "due")).toBe("project:TM/* label:bug is:open sort:due");
    expect(combined("TM", "a OR b", [])).toBe("project:TM/* (a OR b)");
    expect(combined(undefined, "is:open sort:-priority", ["due<today"], "due")).toBe("is:open sort:-priority due<today");
    expect(quote("In progress")).toBe('"In progress"');
    expect(quote("Done")).toBe("Done");
  });
});

describe("the fake's dashboards", () => {
  it("makes the starter dashboard and shows every widget from the other operations", () => {
    const ws = workspace();
    const made = call<DashboardChange>(ws, "dashboard.create", { name: "Overview", starter: true });
    expect(made.dashboard.widgets.map((widget) => widget.kind)).toEqual(["query_count", "overdue", "due_soon", "tasks_by_field", "created_closed", "activity"]);
    const shown = call<DashboardShow>(ws, "dashboard.show", { dashboard: "overview" });
    const [count, overdue, dueSoon, byStatus, flow, activity] = shown.widgets;
    const open = call<{ total: number }>(ws, "task.search", { query: "is:open", limit: 1 }).total;
    expect(count?.count).toBe(open);
    expect(count?.tasks).toBeUndefined();
    expect(overdue?.error).toBeUndefined();
    expect(overdue?.query).toContain("due<today");
    expect(dueSoon?.error).toBeUndefined();
    expect(byStatus?.bars?.reduce((sum, bar) => sum + bar.count, 0)).toBe(open);
    expect(byStatus?.bars?.[0]?.query).toMatch(/^is:open status:/);
    expect(flow?.created_closed?.items.length).toBeGreaterThan(0);
    expect(activity?.activity?.length).toBeGreaterThan(0);
  });

  it("adds, moves, resizes, edits and removes widgets, each one Undo step", () => {
    const ws = workspace();
    call(ws, "dashboard.create", { name: "Mine", project: "TM" });
    call(ws, "dashboard.add_widget", { dashboard: "Mine", project: "TM", widget: { kind: "note", settings: { text: "Hi" } } });
    call(ws, "dashboard.add_widget", { dashboard: "Mine", project: "TM", widget: { kind: "burndown" } });
    const burndown = call<WidgetValue>(ws, "dashboard.widget", { dashboard: "Mine", project: "TM", widget: "2" });
    expect(burndown.burndown?.sprint.state).toBe("active");
    expect(burndown.widget.width).toBe(2);
    const moved = call<DashboardChange>(ws, "dashboard.update_widget", { dashboard: "Mine", project: "TM", widget: "2", position: 1, width: 3 });
    expect(moved.dashboard.widgets.map((widget) => [widget.kind, widget.width])).toEqual([
      ["burndown", 3],
      ["note", 1],
    ]);
    ws.call("history.undo", {});
    const list = call<DashboardList>(ws, "dashboard.list", { project: "TM" });
    expect(list.items[0]?.widgets.map((widget) => widget.kind)).toEqual(["note", "burndown"]);
    call(ws, "dashboard.remove_widget", { dashboard: "Mine", project: "TM", widget: "1" });
    expect(call<DashboardList>(ws, "dashboard.list", { all: true }).items[0]?.widgets).toHaveLength(1);
  });

  it("refuses what the engine refuses", () => {
    const ws = workspace();
    call(ws, "dashboard.create", { name: "Mine" });
    expect(failure(() => ws.call("dashboard.create", { name: "MINE" })).code).toBe("conflict");
    expect(failure(() => ws.call("dashboard.add_widget", { dashboard: "Mine", widget: { kind: "note", settings: { query: "x" } } }))).toEqual({ code: "invalid", path: "widget.settings.query" });
    expect(failure(() => ws.call("dashboard.add_widget", { dashboard: "Mine", widget: { kind: "burndown" } })).path).toBe("widget.settings.project");
    expect(failure(() => ws.call("dashboard.widget", { dashboard: "Mine", widget: "1" })).code).toBe("not_found");
  });
});
