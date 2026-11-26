// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type CreatedClosedReport, type CumulativeFlowReport, type CycleTimeReport, type TaskChange } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { flowStats } from "./reports";
import { seed } from "./seed";

/** Monday 2026-11-02, 09:00 local time. */
const START = new Date(2026, 10, 2, 9, 0);
const DAY = 86_400_000;

function workspace() {
  let now = START.getTime();
  const ws = new FakeWorkspace(seed(START), () => new Date(now));
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
  const call = <T,>(op: string, input: unknown = {}): T => ws.call(op, input).output as T;
  const at = (days: number) => {
    now = START.getTime() + days * DAY;
  };
  return { ws, call, at };
}

describe("the stand-in's flow reports", () => {
  it("replays moves and reopenings, by week", () => {
    const { ws, call, at } = workspace();
    call("project.create", { name: "Flow", key: "FL" });
    const make = (title: string) => call<TaskChange>("task.create", { project: "FL", title, status: "To do" }).task.key;
    const a = make("A");
    const b = make("B");
    at(1);
    call("task.move", { key: a, status: "In progress" });
    at(3);
    call("task.move", { key: a, status: "Done" });
    at(8);
    call("task.move", { key: b, status: "Done" });
    at(9);
    call("task.move", { key: b, status: "To do" });
    at(10);
    call("task.move", { key: b, status: "Done" });
    at(20);
    const range = { project: "FL", from: "2026-11-02", to: "2026-11-15", bucket: "week" };
    const created = call<CreatedClosedReport>("report.created_closed", { ...range, csv: true });
    expect(created.items.map((row) => [row.created, row.closed, row.reopened, row.open])).toEqual([
      [2, 1, 0, 1],
      [0, 2, 1, 0],
    ]);
    expect(created.csv?.split("\r\n")[0]).toBe("start,end,created,closed,reopened,open");

    const flow = call<CumulativeFlowReport>("report.cumulative_flow", range);
    const done = flow.bands.findIndex((band) => band.name === "Done");
    expect(flow.items.map((row) => row.counts[done])).toEqual([1, 2]);

    const cycle = call<CycleTimeReport>("report.cycle_time", { ...range, tasks: true });
    expect(cycle.cycle).toMatchObject({ count: 1, p50: 2 });
    expect(cycle.lead).toMatchObject({ count: 2, p50: 6.5 });
    expect(cycle.tasks?.map((task) => [task.key, task.reopened])).toEqual([
      [a, 0],
      [b, 1],
    ]);

    expect(() => ws.call("report.created_closed", { type: "Nope" })).toThrow(FakeError);
    expect(() => ws.call("report.cycle_time", { from: "2020-01-01", to: "2026-01-01", bucket: "day" })).toThrow(FakeError);
  });

  it("interpolates percentiles", () => {
    expect(flowStats([4, 1, 3, 2])).toMatchObject({ count: 4, p50: 2.5, p85: 3.55, min: 1, max: 4, mean: 2.5 });
    expect(flowStats([])).toEqual({ count: 0 });
  });
});
