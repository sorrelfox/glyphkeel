// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { keepMounted, step, type LaneOrder } from "./dnd";

const lanes: LaneOrder[] = [
  { id: "todo", cards: ["A", "B", "C"], closed: false },
  { id: "hidden", cards: ["X"], closed: true },
  { id: "doing", cards: ["D"], closed: false },
  { id: "empty", cards: [], closed: false },
];

describe("step", () => {
  it("steps one index up or down inside a lane", () => {
    expect(step(lanes, "B", "B", "ArrowDown")).toEqual({ lane: "todo", index: 2, target: "C" });
    expect(step(lanes, "B", "B", "ArrowUp")).toEqual({ lane: "todo", index: 0, target: "A" });
    expect(step(lanes, "C", "B", "ArrowDown")).toBeNull();
  });

  it("goes to the next open lane at the same index, or its end", () => {
    expect(step(lanes, "A", "A", "ArrowRight")).toEqual({ lane: "doing", index: 0, target: "D" });
    expect(step(lanes, "C", "C", "ArrowRight")).toEqual({ lane: "doing", index: 1, target: "doing" });
    expect(step(lanes, "D", "D", "ArrowRight")).toEqual({ lane: "empty", index: 0, target: "empty" });
    expect(step(lanes, "D", "D", "ArrowLeft")).toEqual({ lane: "todo", index: 0, target: "A" });
    expect(step(lanes, "A", "A", "ArrowLeft")).toBeNull();
  });

  it("steps from a lane the card is over", () => {
    expect(step(lanes, "empty", "B", "ArrowLeft")).toEqual({ lane: "doing", index: 0, target: "D" });
  });

  it("ignores the dragged card in the lane it goes to", () => {
    const moving: LaneOrder[] = [
      { id: "todo", cards: ["C"], closed: false },
      { id: "doing", cards: ["B", "A"], closed: false },
    ];
    expect(step(moving, "A", "A", "ArrowLeft")).toEqual({ lane: "todo", index: 1, target: "todo" });
    expect(step(moving, "B", "A", "ArrowLeft")).toEqual({ lane: "todo", index: 0, target: "C" });
  });
});

describe("keepMounted", () => {
  it("adds the dragged row to the visible range", () => {
    const range = { startIndex: 10, endIndex: 14, overscan: 2, count: 300 };
    expect(keepMounted(range, -1)).toEqual([8, 9, 10, 11, 12, 13, 14, 15, 16]);
    expect(keepMounted(range, 200)).toEqual([8, 9, 10, 11, 12, 13, 14, 15, 16, 200]);
    expect(keepMounted(range, 3)).toEqual([3, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  });
});
