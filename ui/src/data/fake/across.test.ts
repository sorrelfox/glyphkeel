// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type BoardAcross, type ViewChange } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

const across = (ws: FakeWorkspace, input: Record<string, unknown> = {}) => call<BoardAcross>(ws, "board.get_across", input);
const keysIn = (board: BoardAcross, column: string) => board.columns.find((each) => each.id === column)?.cards.map((card) => card.key) ?? [];

describe("the board across projects in the sample workspace", () => {
  it("puts the cards of every active project in their category's column, archived ones aside", () => {
    const board = across(new FakeWorkspace(seed(TODAY), () => TODAY));
    expect(board.columns.map((each) => each.id)).toEqual(["todo", "in_progress", "done"]);
    const all = board.columns.flatMap((each) => each.cards);
    expect(new Set(all.map((card) => card.project))).toEqual(new Set(["TM", "CLI", "DOC", "PER", "LAB"]));
    expect(all.every((card) => card.status !== undefined)).toBe(true);
    expect(board.lanes_by).toBe("project");
    expect(board.lanes?.map((lane) => lane.id)).toEqual(["CLI", "DOC", "LAB", "PER", "TM"]);
    expect(board.lanes?.every((lane) => !lane.settable)).toBe(true);
    // Each column names every project's first status of its category.
    const tm = board.projects.find((each) => each.key === "TM");
    expect(board.columns[1]?.statuses.TM).toBe(tm?.statuses.find((status) => status.category === "in_progress")?.id);
  });

  it("narrows to a query and keeps saved views with their folder and lanes", () => {
    const ws = workspace();
    const board = across(ws, { query: "project:PER", lanes: "none" });
    expect(board.columns.flatMap((each) => each.cards).every((card) => card.project === "PER")).toBe(true);
    expect(board.lanes).toBeUndefined();
    call(ws, "folder.create", { name: "Home" });
    call(ws, "folder.move", { project: "PER", folder: "Home" });
    const { view } = call<ViewChange>(ws, "view.create", { name: "Home", kind: "board_across", query: "", folder: "home", group_by: "priority" });
    expect(view.folder).toBe("Home");
    const saved = across(ws, { view: view.id });
    expect(saved.folder).toBe("Home");
    expect(saved.lanes_by).toBe("priority");
    expect(new Set(saved.columns.flatMap((each) => each.cards).map((card) => card.project))).toEqual(new Set(["PER"]));
    expect(() => across(ws, { lanes: "field" })).toThrow(FakeError);
    expect(() => call(ws, "view.create", { name: "X", kind: "board_across", query: "", project: "TM" })).toThrow(FakeError);
  });

  it("moves a card to the status a column names", () => {
    const ws = workspace();
    const board = across(ws, { lanes: "none" });
    const key = keysIn(board, "todo").find((each) => each.startsWith("PER-"));
    expect(key).toBeDefined();
    const status = board.columns[1]?.statuses.PER;
    call(ws, "task.move", { key, status });
    expect(keysIn(across(ws, { lanes: "none" }), "in_progress")).toContain(key);
  });
});
