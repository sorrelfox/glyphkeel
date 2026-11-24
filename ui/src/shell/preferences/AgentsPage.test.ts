// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type WorkspaceInfo } from "../../generated/ops";
import { agentLimits } from "./AgentsPage";

const DEFAULTS = { writes_per_minute: 120, new_tasks_per_day: 500, trash_items_per_call: 20, trash_items_per_hour: 50 };

function infoWith(limits: unknown): WorkspaceInfo {
  return { agent_limits: limits } as unknown as WorkspaceInfo;
}

describe("agentLimits", () => {
  it("takes the limits the engine reports", () => {
    const limits = { writes_per_minute: 60, new_tasks_per_day: 200, trash_items_per_call: 10, trash_items_per_hour: 25 };
    expect(agentLimits(infoWith(limits))).toEqual(limits);
  });

  it("falls back to the usual limits when the engine reports none, or some are missing", () => {
    expect(agentLimits(undefined)).toEqual(DEFAULTS);
    expect(agentLimits(infoWith(undefined))).toEqual(DEFAULTS);
    expect(agentLimits(infoWith({ writes_per_minute: 30, new_tasks_per_day: "many" }))).toEqual({ ...DEFAULTS, writes_per_minute: 30 });
  });
});
