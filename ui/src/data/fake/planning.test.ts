// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type BurnupReport, type DashboardChange, type DashboardShow, type EstimatesReport, type TaskChange, type VelocityReport } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { forecastOf } from "./planning";
import { seed } from "./seed";

const START = new Date(2026, 10, 23, 9, 0);
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

function workspace() {
  let now = START.getTime();
  const ws = new FakeWorkspace(seed(START), () => new Date(now));
  ws.call("project.create", { name: "Yard", key: "ZZ" });
  return {
    ws,
    at: (ms: number) => {
      now = START.getTime() + ms;
    },
  };
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function code(run: () => unknown): string {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error.code;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

function task(ws: FakeWorkspace, title: string, extra: Record<string, unknown> = {}): string {
  return call<TaskChange>(ws, "task.create", { project: "ZZ", title, ...extra }).task.key;
}

function done(ws: FakeWorkspace, key: string) {
  ws.call("task.move", { key, status: "Done" });
}

describe("the fake's planning reports", () => {
  it("gives velocity per completed sprint with the rolling average", () => {
    const { ws, at } = workspace();
    const a = task(ws, "A", { estimate: "2h" });
    const b = task(ws, "B", { estimate: "1h" });
    const c = task(ws, "C", { estimate: "4h" });
    ws.call("sprint.create", { project: "ZZ", name: "S1", tasks: [a, b, c] });
    ws.call("sprint.create", { project: "ZZ", name: "S2" });
    ws.call("sprint.start", { sprint: "S1", project: "ZZ" });
    at(HOUR);
    done(ws, a);
    done(ws, b);
    at(DAY);
    ws.call("sprint.complete", { sprint: "active", project: "ZZ", move_open_tasks_to: "S2" });
    at(DAY + HOUR);
    ws.call("sprint.start", { sprint: "S2", project: "ZZ" });
    at(DAY + 2 * HOUR);
    done(ws, c);
    at(2 * DAY);
    ws.call("sprint.complete", { sprint: "active", project: "ZZ" });
    const d = task(ws, "D", { estimate: "3h" });
    ws.call("sprint.create", { project: "ZZ", name: "S3", tasks: [d] });

    const tasks = call<VelocityReport>(ws, "report.velocity", { project: "ZZ" });
    expect(tasks.sprints.map((s) => [s.sprint.name, s.committed, s.completed, s.average])).toEqual([
      ["S1", 3, 2, 2],
      ["S2", 1, 1, 1.5],
    ]);
    expect(tasks.average).toBe(1.5);
    expect(tasks.average_of).toBe(2);
    expect(tasks.upcoming).toEqual([expect.objectContaining({ planned: 1, tasks: 1 })]);
    const hours = call<VelocityReport>(ws, "report.velocity", { project: "ZZ", measure: "estimate", csv: true });
    expect(hours.sprints.map((s) => [s.committed, s.completed])).toEqual([
      [7, 3],
      [4, 4],
    ]);
    expect(hours.average).toBe(3.5);
    expect(hours.csv?.split("\r\n")[0]).toBe("sprint,start_date,end_date,completed_on,committed,completed,committed_tasks,completed_tasks,average");
    expect(code(() => ws.call("report.velocity", { project: "ZZ", measure: "field" }))).toBe("invalid");
  });

  it("gives a burn-up whose scope steps up and whose forecast meets it", () => {
    const { ws, at } = workspace();
    ws.call("version.create", { project: "ZZ", name: "1.0", start_date: "2026-11-23", due_date: "2026-12-04" });
    const a = task(ws, "A", { fix_version: "1.0" });
    const b = task(ws, "B", { fix_version: "1.0" });
    at(DAY);
    done(ws, a);
    at(2 * DAY);
    task(ws, "C", { fix_version: "1.0" });
    done(ws, b);
    at(3 * DAY);
    const report = call<BurnupReport>(ws, "report.burnup", { version: "1.0", project: "ZZ" });
    expect(report.days.map((d) => [d.date, d.scope, d.completed])).toEqual([
      ["2026-11-23", 2, 0],
      ["2026-11-24", 2, 1],
      ["2026-11-25", 3, 2],
      ["2026-11-26", 3, 2],
    ]);
    expect(report.end).toBe("2026-12-04");
    expect(report.forecast).toEqual({ date: "2026-11-27", start: 0.2, per_day: 0.7 });
    expect(code(() => ws.call("report.burnup", { project: "ZZ" }))).toBe("invalid");
  });

  it("hides the forecast with fewer than three days, no progress or all done", () => {
    const day = (scope: number, completed: number) => ({ date: "", scope, completed, scope_tasks: 0, completed_tasks: 0 });
    expect(forecastOf("2026-11-23", [day(2, 0), day(2, 1)])).toBeUndefined();
    expect(forecastOf("2026-11-23", [day(2, 1), day(2, 1), day(2, 1)])).toBeUndefined();
    expect(forecastOf("2026-11-23", [day(2, 0), day(2, 1), day(2, 2)])).toBeUndefined();
    expect(forecastOf("2026-11-23", [day(4, 0), day(4, 1), day(4, 2)])?.date).toBe("2026-11-27");
  });

  it("compares spent with estimated time, by type, leaving out tasks without either", () => {
    const { ws } = workspace();
    const closed = (title: string, extra: Record<string, unknown>, logged: string | null) => {
      const key = task(ws, title, extra);
      if (logged !== null) {
        ws.call("time.log", { key, duration: logged });
      }
      done(ws, key);
      return key;
    };
    closed("One", { estimate: "1h" }, "60m");
    const two = closed("Two", { estimate: "1h" }, "90m");
    closed("Three", { estimate: "2h" }, "96m");
    const four = closed("Four", { estimate: "1h", type: "Bug" }, "30m");
    closed("Five", { estimate: "30m", type: "Bug" }, "36m");
    closed("No estimate", {}, "1h");
    closed("No time", { estimate: "1h" }, null);
    const report = call<EstimatesReport>(ws, "report.estimates", { project: "ZZ", group_by: "type" });
    expect(report.total).toMatchObject({ count: 5, estimate_minutes: 330, spent_minutes: 312, median_ratio: 1, within_share: 0.6, over: 1, under: 1 });
    expect(report.total.outliers.slice(0, 2).map((t) => t.key)).toEqual([four, two]);
    expect(report.without_estimate).toBe(1);
    expect(report.without_time).toBe(1);
    expect(report.groups.map((g) => [g.name, g.count, g.median_ratio, g.within_share])).toEqual([
      ["Task", 3, 1, 0.6667],
      ["Bug", 2, 0.85, 0.5],
    ]);
  });

  it("shows the planning widgets with the reports' figures", () => {
    const { ws } = workspace();
    ws.call("version.create", { project: "ZZ", name: "1.0" });
    task(ws, "A", { fix_version: "1.0" });
    const made = call<DashboardChange>(ws, "dashboard.create", {
      name: "Plan",
      project: "ZZ",
      widgets: [{ kind: "velocity" }, { kind: "burnup", settings: { version: "1.0" } }, { kind: "estimate_accuracy" }],
    });
    expect(made.dashboard.widgets.map((w) => w.width)).toEqual([2, 2, 2]);
    const shown = call<DashboardShow>(ws, "dashboard.show", { dashboard: "Plan", project: "ZZ" });
    expect(shown.widgets[0]?.velocity).toEqual(call<VelocityReport>(ws, "report.velocity", { project: "ZZ" }));
    expect(shown.widgets[1]?.burnup?.days.at(-1)?.scope).toBe(1);
    expect(shown.widgets[2]?.estimates?.group_by).toBe("type");
  });
});
