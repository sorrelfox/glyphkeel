// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { parseQuickAdd, projectFor } from "./parse";

const PROJECTS = [
  { key: "TM", name: "Task Manager" },
  { key: "PER", name: "Personal" },
  { key: "LAB", name: "Home lab" },
];
const options = { projects: PROJECTS, today: "2026-11-23" };

describe("the quick-add syntax", () => {
  it("takes the project, the priority and a trailing date out of the title", () => {
    expect(parseQuickAdd("Renew domain fri #Personal", options)).toEqual({ title: "Renew domain", project: "PER", due: "2026-11-27", dueText: "fri" });
    expect(parseQuickAdd("!high Fix the login #tm in 2 weeks", options)).toEqual({
      title: "Fix the login",
      project: "TM",
      priority: "high",
      due: "2026-12-07",
      dueText: "in 2 weeks",
    });
    expect(parseQuickAdd("Back up the NAS !1 #homelab", options)).toEqual({ title: "Back up the NAS", project: "LAB", priority: "urgent" });
  });

  it("leaves a plain title alone", () => {
    expect(parseQuickAdd("  Water the plants  ", options)).toEqual({ title: "Water the plants" });
    expect(parseQuickAdd("Email about #42 issue", options)).toEqual({ title: "Email about issue", unknownProject: "42" });
    expect(parseQuickAdd("Say hi!", options)).toEqual({ title: "Say hi!" });
  });

  it("keeps at least one word of title, and the date when told to", () => {
    expect(parseQuickAdd("tomorrow", options)).toEqual({ title: "tomorrow" });
    expect(parseQuickAdd("Meet Anna dec 1", options)).toMatchObject({ title: "Meet Anna", due: "2026-12-01" });
    expect(parseQuickAdd("Plan the week end of month", { ...options, keepDate: true })).toEqual({ title: "Plan the week end of month" });
  });

  it("finds a project by key, by its name without spaces, or by the start of one name", () => {
    expect(projectFor("per", PROJECTS)?.key).toBe("PER");
    expect(projectFor("HomeLab", PROJECTS)?.key).toBe("LAB");
    expect(projectFor("task", PROJECTS)?.key).toBe("TM");
    // "Personal" and nothing else starts with "pe"; "a" starts none.
    expect(projectFor("pe", PROJECTS)?.key).toBe("PER");
    expect(projectFor("x", PROJECTS)).toBeUndefined();
  });
});
