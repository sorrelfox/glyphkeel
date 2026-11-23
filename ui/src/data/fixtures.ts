// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// For tests: answers shaped like the engine's.

import { type Settings, type WorkspaceInfo } from "../generated/ops";

export function settings(overrides: Partial<Settings> = {}): Settings {
  return {
    agent_max_profile: "standard",
    days_per_week: 5,
    file_max_mb: 100,
    hours_per_day: 8,
    reminder_time: "09:00",
    timer_reminder_hours: 8,
    trash_days: 30,
    week_start: "monday",
    ...overrides,
  };
}

export function workspaceInfo(overrides: Partial<WorkspaceInfo> = {}): WorkspaceInfo {
  return {
    caller: { actor: { kind: "user", name: "You" }, via: "app" },
    guide: "",
    id: "w-1",
    name: "Personal",
    program_version: __VERSION__,
    schema_version: 1,
    settings: settings(),
    ...overrides,
  };
}
