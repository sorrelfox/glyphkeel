// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type LayoutChange, type TaskChange } from "../../generated/ops";
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

function failure(run: () => unknown) {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

const board = (ws: FakeWorkspace, input: Record<string, unknown> = {}) => call<Board>(ws, "board.get", { project: "TM", ...input });
const cards = (b: Board) => b.columns.flatMap((column) => column.cards);

describe("swimlanes in the sample workspace", () => {
  it("groups the cards by priority with counts per column, the empty lane last", () => {
    const ws = workspace();
    const b = board(ws, { lanes: "priority" });
    const lanes = b.lanes ?? [];
    expect(lanes.map((lane) => lane.id)).toEqual(["urgent", "high", "normal", "low", "none"]);
    for (const lane of lanes) {
      const mine = cards(b).filter((card) => card.lane === lane.id);
      expect(lane.count).toBe(mine.length);
      for (const column of b.columns.filter((each) => !each.hidden)) {
        expect(lane.counts[column.id]).toBe(column.cards.filter((card) => card.lane === lane.id).length);
      }
    }
    expect(cards(b).every((card) => card.lane === (card.priority === "none" ? "none" : card.priority))).toBe(true);
    expect(board(ws).lanes).toBeUndefined();
  });

  it("keeps the grouping in the layout, undone like the rest", () => {
    const ws = workspace();
    call<LayoutChange>(ws, "layout.update", { project: "TM", lanes: "label" });
    expect(board(ws).lanes?.at(-1)?.id).toBe("none");
    call(ws, "layout.update", { project: "TM", lanes: "priority" });
    ws.call("history.undo", {});
    expect(call<LayoutChange>(ws, "layout.get", { project: "TM" }).layout).toMatchObject({ lanes: "label" });
    expect(failure(() => call(ws, "layout.update", { project: "TM", lanes: "field" })).details?.message_id).toBe("board-lane-field-needed");
  });

  it("moves a card into another lane and column in one change, one undo taking both back", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Crash", priority: "low" }).task.key;
    const before = call<TaskChange>(ws, "task.get", { keys: [key] }) as unknown as { items: { status: { name: string }; priority: string }[] };
    const status = before.items[0]?.status.name;
    const target = board(ws).columns.find((column) => column.name !== status)?.name;
    call(ws, "task.move", { key, status: target, changes: { priority: "urgent" } });
    const moved = cards(board(ws, { lanes: "priority" })).find((card) => card.key === key);
    expect(moved?.lane).toBe("urgent");
    ws.call("history.undo", {});
    const back = (call<unknown>(ws, "task.get", { keys: [key] }) as { items: { status: { name: string }; priority: string }[] }).items[0];
    expect(back).toMatchObject({ priority: "low", status: { name: status } });
  });
});
