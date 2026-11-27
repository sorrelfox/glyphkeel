// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type DashboardChange, type DashboardList } from "../../generated/ops";
import { chosenDashboard, pinnedTo } from "../dashboards";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

describe("pinning a dashboard to a project", () => {
  it("pins, moves back to the workspace, and refuses a pin both given and cleared", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const made = call<DashboardChange>(ws, "dashboard.create", { name: "Team" });
    expect(made.dashboard.project).toBeUndefined();
    const pinned = call<DashboardChange>(ws, "dashboard.update", { dashboard: "Team", pin_to: "TM" });
    expect(pinned.dashboard.project).toBe("TM");
    const list = call<DashboardList>(ws, "dashboard.list", { all: true });
    expect(pinnedTo(list.items, "TM").map((each) => each.name)).toEqual(["Team"]);
    expect(chosenDashboard(list.items, undefined, "TM")?.id).toBe(made.dashboard.id);
    const back = call<DashboardChange>(ws, "dashboard.update", { dashboard: made.dashboard.id, clear: ["pin_to"] });
    expect(back.dashboard.project).toBeUndefined();
    expect(() => ws.call("dashboard.update", { dashboard: made.dashboard.id, pin_to: "TM", clear: ["pin_to"] })).toThrow(FakeError);
  });
});
