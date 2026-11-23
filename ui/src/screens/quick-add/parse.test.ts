// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { looksLikeDate, parseQuickAdd, projectFor } from "./parse";

const PROJECTS = [
  { key: "TM", name: "Task Manager" },
  { key: "PER", name: "Personal" },
  { key: "LAB", name: "Home lab" },
];
const options = { projects: PROJECTS };

describe("the quick-add syntax", () => {
  it("takes the project, the priority and a trailing date out of the title", () => {
    expect(parseQuickAdd("Renew domain fri #Personal", options)).toEqual({ title: "Renew domain", project: "PER", dueText: "fri" });
    expect(parseQuickAdd("!high Fix the login #tm in 2 weeks", options)).toEqual({
      title: "Fix the login",
      project: "TM",
      priority: "high",
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
    expect(parseQuickAdd("Meet Anna dec 1", options)).toMatchObject({ title: "Meet Anna", dueText: "dec 1" });
    expect(parseQuickAdd("Plan the week end of month", { ...options, keepDate: true })).toEqual({ title: "Plan the week end of month" });
  });

  it("offers shorter dates for the engine to try, and leaves words that only look a little like dates", () => {
    expect(parseQuickAdd("Plan the week end of month", options)).toEqual({
      title: "Plan the week",
      dueText: "end of month",
    });
    expect(parseQuickAdd("Call the shop next friday", options)).toEqual({
      title: "Call the shop",
      dueText: "next friday",
      shorterDates: [{ title: "Call the shop next", dueText: "friday" }],
    });
    expect(parseQuickAdd("Feed the monkey", options)).toEqual({ title: "Feed the monkey" });
    expect(parseQuickAdd("Read chapter 12", options)).toEqual({ title: "Read chapter 12" });
  });

  it("knows the shapes of the engine's dates", () => {
    for (const phrase of ["today", "Tomorrow", "fri", "thurs", "wednesday", "2026-12-24", "+3d", "in 3d", "in 2 weeks", "in a month", "next mon", "this week", "next-month", "end of month", "end of the year", "dec 1", "1 Dec", "march 3rd 2027", "dec 41"]) {
      expect(looksLikeDate(phrase), phrase).toBe(true);
    }
    for (const phrase of ["monkey", "sunny", "fr", "may", "in", "next", "12", "the end", "marble 3"]) {
      expect(looksLikeDate(phrase), phrase).toBe(false);
    }
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
