// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { settings } from "../../data/fixtures";
import { LIMIT_ROWS, limitPatch } from "./AgentsPage";

describe("limits on agent sessions", () => {
  it("sets a number, or lifts the limit", () => {
    expect(limitPatch("writes_per_minute", 30)).toEqual({ writes_per_minute: 30 });
    expect(limitPatch("new_tasks_per_day", null)).toEqual({ no_limit: ["new_tasks_per_day"] });
  });

  it("shows every limit, with the engine's defaults", () => {
    const defaults = settings().agent_limits;
    expect(LIMIT_ROWS.map((row) => row.limit).sort()).toEqual(["new_tasks_per_day", "trash_items_per_call", "trash_items_per_hour", "writes_per_minute"]);
    for (const row of LIMIT_ROWS) {
      expect(defaults[row.limit]).toBe(row.byDefault);
    }
  });
});
