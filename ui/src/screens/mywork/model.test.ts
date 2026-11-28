// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type AcrossProject, type BoardAcross, type Card } from "../../generated/ops";
import { joinQuery, landing, lanesOf, moveAcross, splitToggles, toBoard } from "./model";

const project: AcrossProject = {
  key: "TM",
  name: "Team",
  hue: "blue",
  statuses: [
    { id: "s1", name: "Backlog", category: "todo", hue: "grey" },
    { id: "s2", name: "To do", category: "todo", hue: "grey" },
    { id: "s3", name: "In progress", category: "in_progress", hue: "blue" },
    { id: "s4", name: "Done", category: "done", hue: "green" },
  ],
};

function card(key: string, status: string): Card {
  return {
    key,
    title: key,
    project: "TM",
    type: { id: "ty", name: "Task", glyph: "task" },
    priority: "none",
    labels: [],
    checklist: { done: 0, total: 0 },
    progress_percent: 0,
    subtasks: { done: 0, total: 0 },
    blocked: false,
    comment_count: 0,
    file_count: 0,
    spent_minutes: 0,
    rank: "a",
    status_entered_at: "2026-11-23T10:00:00Z",
    updated_at: "2026-11-23T10:00:00Z",
    version: 1,
    id: key,
    status: { id: status, name: status, hue: "grey" },
  };
}

const column = (id: "todo" | "in_progress" | "done", cards: Card[]) => ({
  id,
  name: id,
  category: id,
  hue: "grey" as const,
  statuses: {},
  over_limit: false,
  under_minimum: false,
  count: cards.length,
  older: 0,
  hidden: false,
  collapsed: false,
  sort_mode: "manual" as const,
  cards,
});

const across: BoardAcross = {
  query: "",
  columns: [column("todo", [card("TM-1", "s1")]), column("in_progress", []), column("done", [])],
  lanes_by: "project",
  projects: [project],
  hide_done_after_days: 14,
};

describe("the quick toggles", () => {
  it("are conditions added after what was typed, and read back from a saved query", () => {
    expect(joinQuery("label:bug", ["high", "week"])).toBe("label:bug due:this-week priority>=high");
    expect(joinQuery("", ["sprint"])).toBe("sprint:active");
    expect(joinQuery("label:a OR label:b", ["high"])).toBe("(label:a OR label:b) priority>=high");
    expect(splitToggles("label:bug due:this-week priority>=high")).toEqual({ typed: "label:bug", toggles: ["week", "high"] });
    expect(splitToggles("sprint:active")).toEqual({ typed: "", toggles: ["sprint"] });
    // Inside an OR a condition is not a toggle.
    expect(splitToggles("label:a OR sprint:active")).toEqual({ typed: "label:a OR sprint:active", toggles: [] });
  });
});

describe("a drop into a column", () => {
  it("goes to the one status of the category, asks among several, and nowhere within its column", () => {
    const todo = card("TM-1", "s1");
    expect(landing(todo, project, "todo", "in_progress")).toEqual({ kind: "one", status: project.statuses[2] });
    expect(landing(card("TM-2", "s3"), project, "in_progress", "todo")).toEqual({ kind: "choose", statuses: [project.statuses[0], project.statuses[1]] });
    expect(landing(todo, project, "todo", "todo")).toEqual({ kind: "none" });
    expect(landing(todo, { ...project, statuses: project.statuses.filter((each) => each.category !== "done") }, "todo", "done")).toEqual({ kind: "none" });
  });

  it("shows the card in its new column with its new status at once", () => {
    const doing = project.statuses[2];
    if (doing === undefined) {
      throw new Error("no in-progress status");
    }
    const moved = moveAcross(across, "TM-1", "in_progress", doing);
    expect(moved.columns[0]?.cards).toEqual([]);
    expect(moved.columns[0]?.count).toBe(0);
    expect(moved.columns[1]?.cards.map((each) => [each.key, each.status?.name])).toEqual([["TM-1", "In progress"]]);
    expect(moved.columns[1]?.count).toBe(1);
  });
});

describe("the board's shape", () => {
  it("is a project board's, sorted by the engine, with the lanes it was read with", () => {
    const board = toBoard(across);
    expect(board.layout.sort_mode).toBe("priority");
    expect(board.layout.lanes).toBe("project");
    expect(board.columns).toBe(across.columns);
    expect(lanesOf("priority")).toBe("priority");
    expect(lanesOf(undefined)).toBe("project");
    expect(lanesOf("field")).toBe("project");
  });
});
