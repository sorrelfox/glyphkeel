// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Board, type BoardLayout, type Card, type Column } from "../../data/catalog";
import {
  boardView,
  dueState,
  formatMinutes,
  matches,
  moveColumn,
  BACKLOG,
  NO_FILTERS,
  orderColumns,
  parseDuration,
  positionIn,
  sortCards,
} from "./model";

const NOW = new Date(2026, 10, 23, 12, 0);

const LAYOUT: BoardLayout = {
  order: [],
  hidden: [],
  collapsed: [],
  sort_mode: "manual",
  column_sort: {},
  hide_done_after_days: 14,
  card_fields: ["type", "priority", "due", "labels", "checklist"],
  include_subprojects: false,
  colour_by: "none",
  colour_rules: [],
  covers: "off",
};

function card(key: string, overrides: Partial<Card> = {}): Card {
  return {
    id: key,
    key,
    project: "TM",
    title: `Task ${key}`,
    type: { id: "ty-task", name: "Task", glyph: "task" },
    priority: "none",
    labels: [],
    blocked: false,
    comment_count: 0,
    file_count: 0,
    spent_minutes: 0,
    checklist: { done: 0, total: 0 },
    progress_percent: 0,
    subtasks: { done: 0, total: 0 },
    rank: "a",
    status_entered_at: NOW.toISOString(),
    updated_at: NOW.toISOString(),
    version: 1,
    ...overrides,
  };
}

function column(id: string, cards: Card[] = [], overrides: Partial<Column> = {}): Column {
  return {
    id,
    name: id,
    category: "todo",
    hue: "grey",
    statuses: { TM: id },
    cards,
    count: cards.length,
    older: 0,
    over_limit: false,
    under_minimum: false,
    hidden: false,
    collapsed: false,
    sort_mode: "manual",
    ...overrides,
  };
}

describe("dueState", () => {
  it("reads overdue, today and soon, and never overdue once closed", () => {
    expect(dueState("2026-11-22", false, "2026-11-23")).toBe("overdue");
    expect(dueState("2026-11-22", true, "2026-11-23")).toBeNull();
    expect(dueState("2026-11-23", false, "2026-11-23")).toBe("today");
    expect(dueState("2026-11-26", false, "2026-11-23")).toBe("soon");
    expect(dueState("2026-11-27", false, "2026-11-23")).toBeNull();
    expect(dueState(null, false, "2026-11-23")).toBeNull();
  });
});

describe("matches", () => {
  it("applies every quick filter", () => {
    const recent = card("TM-1");
    const old = card("TM-2", { updated_at: new Date(2026, 9, 1).toISOString() });
    expect(matches(old, { ...NO_FILTERS, recentlyUpdated: true }, NOW)).toBe(false);
    expect(matches(recent, { ...NO_FILTERS, recentlyUpdated: true }, NOW)).toBe(true);
    expect(matches(card("TM-3", { priority: "high" }), { ...NO_FILTERS, priorities: ["high", "urgent"] }, NOW)).toBe(true);
    expect(matches(card("TM-3"), { ...NO_FILTERS, priorities: ["high"] }, NOW)).toBe(false);
    expect(matches(card("TM-4", { labels: [{ id: "l", name: "board", hue: "blue" }] }), { ...NO_FILTERS, labels: ["board"] }, NOW)).toBe(true);
    expect(matches(card("TM-5", { title: "Fix the parser" }), { ...NO_FILTERS, text: "PARSER" }, NOW)).toBe(true);
    expect(matches(card("TM-5"), { ...NO_FILTERS, text: "tm-5" }, NOW)).toBe(true);
    expect(matches(card("TM-6", { due_date: "2026-11-20" }), { ...NO_FILTERS, due: ["overdue"] }, NOW)).toBe(true);
    expect(matches(card("TM-6", { due_date: "2026-11-20", closed_at: NOW.toISOString() }), { ...NO_FILTERS, due: ["overdue"] }, NOW)).toBe(false);
    expect(matches(card("TM-7"), { ...NO_FILTERS, due: ["none"] }, NOW)).toBe(true);
    const inSprint = card("TM-8", { sprint: { id: "s-1", name: "Sprint 1", state: "active" } });
    expect(matches(inSprint, { ...NO_FILTERS, sprints: ["s-1"] }, NOW)).toBe(true);
    expect(matches(inSprint, { ...NO_FILTERS, sprints: [BACKLOG] }, NOW)).toBe(false);
    expect(matches(card("TM-9"), { ...NO_FILTERS, sprints: [BACKLOG] }, NOW)).toBe(true);
  });
});

describe("sortCards", () => {
  const cards = [
    card("A", { priority: "low", due_date: "2026-12-01" }),
    card("B", { priority: "urgent" }),
    card("C", { priority: "high", due_date: "2026-11-24", updated_at: new Date(2026, 10, 1).toISOString() }),
  ];
  it("keeps the shared order in manual mode and sorts the others", () => {
    expect(sortCards(cards, "manual").map((c) => c.key)).toEqual(["A", "B", "C"]);
    expect(sortCards(cards, "priority").map((c) => c.key)).toEqual(["B", "C", "A"]);
    expect(sortCards(cards, "due").map((c) => c.key)).toEqual(["C", "A", "B"]);
    expect(sortCards(cards, "updated").map((c) => c.key)).toEqual(["A", "B", "C"]);
  });
});

describe("orderColumns", () => {
  const columns = ["backlog", "todo", "doing", "done"].map((id) => column(id));
  it("puts a column the layout doesn't know at its project position", () => {
    expect(orderColumns(columns, ["done", "todo", "backlog"]).map((c) => c.id)).toEqual(["done", "todo", "doing", "backlog"]);
    expect(orderColumns(columns, []).map((c) => c.id)).toEqual(["backlog", "todo", "doing", "done"]);
  });
  it("forgets columns that no longer exist", () => {
    expect(orderColumns(columns, ["gone", "done"]).map((c) => c.id)).toEqual(["backlog", "todo", "doing", "done"]);
  });
  it("moves a column one step and lists every column", () => {
    expect(moveColumn(columns, [], "doing", -1)).toEqual(["backlog", "doing", "todo", "done"]);
    expect(moveColumn(columns, [], "backlog", -1)).toEqual(["backlog", "todo", "doing", "done"]);
  });
});

describe("boardView", () => {
  const board: Board = {
    project: "TM",
    layout: LAYOUT,
    hidden_columns: 0,
    hidden_tasks: 0,
    columns: [
      column("todo", [card("TM-1")]),
      column("doing", [card("TM-2"), card("TM-3", { priority: "high" })], { wip_limit: 1 }),
      column(
        "done",
        [card("TM-4", { closed_at: new Date(2026, 10, 20).toISOString() }), card("TM-5", { closed_at: new Date(2026, 8, 1).toISOString() })],
        { category: "done" },
      ),
      column("review", [card("TM-6")]),
    ],
  };
  it("hides columns, counts what they hold, and keeps old closed cards behind Show older", () => {
    const view = boardView(board, { ...LAYOUT, hidden: ["review"], collapsed: ["todo"] }, NO_FILTERS, NOW, new Set());
    expect(view.lanes.map((lane) => lane.column.id)).toEqual(["todo", "doing", "done"]);
    expect(view.lanes[0]?.collapsed).toBe(true);
    expect(view.hidden).toEqual([{ column: board.columns[3], count: 1 }]);
    const done = view.lanes[2];
    expect(done?.cards.map((c) => c.key)).toEqual(["TM-4"]);
    expect(done?.older).toBe(1);
    expect(view.lanes[1]?.total).toBe(2);
    expect(view.open).toBe(4);
    const older = boardView(board, LAYOUT, NO_FILTERS, NOW, new Set(["done"]));
    expect(older.lanes[2]?.cards.map((c) => c.key)).toEqual(["TM-4", "TM-5"]);
  });
  it("counts the older cards the engine kept back until the column shows them", () => {
    const kept = { ...board, columns: board.columns.map((each) => (each.id === "done" ? { ...each, cards: each.cards.slice(0, 1), older: 7 } : each)) };
    expect(boardView(kept, LAYOUT, NO_FILTERS, NOW, new Set()).lanes[2]?.older).toBe(7);
    expect(boardView(kept, LAYOUT, NO_FILTERS, NOW, new Set(["done"])).lanes[2]?.older).toBe(0);
  });
  it("sorts a column by its own mode and counts what the filters let through", () => {
    const view = boardView(board, { ...LAYOUT, column_sort: { doing: "priority" } }, { ...NO_FILTERS, text: "TM-" }, NOW, new Set());
    expect(view.lanes[1]?.cards.map((c) => c.key)).toEqual(["TM-3", "TM-2"]);
    expect(view.lanes[1]?.sort).toBe("priority");
    const filtered = boardView(board, LAYOUT, { ...NO_FILTERS, priorities: ["high"] }, NOW, new Set());
    expect(filtered.shown).toBe(1);
    expect(filtered.total).toBe(6);
  });
});

describe("positions and durations", () => {
  it("places a card before its new neighbour, else after the one above", () => {
    expect(positionIn(["A", "B", "C"], "A")).toEqual({ before: "B" });
    expect(positionIn(["A", "B", "C"], "C")).toEqual({ after: "B" });
    expect(positionIn(["A"], "A")).toEqual({ at: "bottom" });
  });
  it("reads durations the way people type them", () => {
    expect(parseDuration("90m")).toBe(90);
    expect(parseDuration("1.5h")).toBe(90);
    expect(parseDuration("1:30")).toBe(90);
    expect(parseDuration("1h 30m")).toBe(90);
    expect(parseDuration("2d")).toBe(960);
    expect(parseDuration("1w", 8, 5)).toBe(2400);
    expect(parseDuration("soon")).toBeNull();
    expect(formatMinutes(90)).toBe("1h 30m");
    expect(formatMinutes(45)).toBe("45m");
    expect(formatMinutes(120)).toBe("2h");
  });
});
