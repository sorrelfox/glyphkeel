// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { findMatch, matchActions, matchProjects, nextScope, recentFrom, remember, RECENT_KEPT, typedKey } from "./model";

const ACTIONS = [
  { id: "palette.open", name: "Search or run a command" },
  { id: "app.backup", name: "Back up now" },
  { id: "nav.today", name: "Go to Today" },
  { id: "task.new", name: "New task" },
  { id: "sidebar.toggle", name: "Show or hide the sidebar" },
  { id: "app.preferences", name: "Preferences" },
];

describe("the palette's results", () => {
  it("lists the common actions for an empty field, never itself", () => {
    expect(matchActions(ACTIONS, "").map((action) => action.id)).toEqual(["task.new", "nav.today", "sidebar.toggle", "app.preferences"]);
    expect(matchActions(ACTIONS, "command")).toEqual([]);
  });

  it("puts names that start with the text first", () => {
    expect(matchActions(ACTIONS, "s").map((action) => action.id)).toEqual(["sidebar.toggle", "task.new", "app.preferences"]);
    expect(matchActions(ACTIONS, "to").map((action) => action.id)).toEqual(["nav.today"]);
    expect(matchActions(ACTIONS, "BACK").map((action) => action.id)).toEqual(["app.backup"]);
    expect(matchActions(ACTIONS, "p").map((action) => action.id)).toEqual(["app.preferences", "app.backup"]);
  });

  it("finds projects by key first, then by name or path", () => {
    const projects = [
      { key: "TM", name: "Task Manager", path: "Task Manager" },
      { key: "CLI", name: "CLI and MCP", path: "Task Manager › CLI and MCP" },
      { key: "ATM", name: "Cash", path: "Cash" },
    ];
    expect(matchProjects(projects, "tm").map((project) => project.key)).toEqual(["TM"]);
    expect(matchProjects(projects, "manager").map((project) => project.key)).toEqual(["TM", "CLI"]);
    expect(matchProjects(projects, "")).toEqual([]);
  });

  it("recognises a task key in any case", () => {
    expect(typedKey(" tm-41 ")).toBe("TM-41");
    expect(typedKey("TM-")).toBeNull();
    expect(typedKey("fix TM-41")).toBeNull();
  });

  it("marks where the text is found", () => {
    expect(findMatch("Back up now", "up")).toEqual({ start: 5, end: 7 });
    expect(findMatch("Back up now", " ")).toBeNull();
    expect(findMatch("Back up now", "zz")).toBeNull();
  });

  it("cycles the scopes with Tab and Shift+Tab", () => {
    expect(nextScope("all")).toBe("tasks");
    expect(nextScope("actions")).toBe("all");
    expect(nextScope("all", true)).toBe("actions");
  });

  it("keeps the latest recent items, each once", () => {
    let recent = remember([], { kind: "task", key: "TM-1", title: "One" });
    recent = remember(recent, { kind: "project", key: "TM", title: "Task Manager" });
    recent = remember(recent, { kind: "task", key: "TM-1", title: "One, renamed" });
    expect(recent.map((item) => item.key)).toEqual(["TM-1", "TM"]);
    expect(recent[0]?.title).toBe("One, renamed");
    for (let n = 2; n < 20; n += 1) {
      recent = remember(recent, { kind: "task", key: `TM-${String(n)}`, title: "" });
    }
    expect(recent).toHaveLength(RECENT_KEPT);
    expect(recentFrom([{ kind: "task", key: "TM-1", title: "x" }, { kind: "wiki", key: "a", title: "b" }, "junk", null])).toEqual([{ kind: "task", key: "TM-1", title: "x" }]);
    expect(recentFrom("junk")).toEqual([]);
  });
});
