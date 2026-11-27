// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Dashboard, type Widget } from "../generated/ops";
import { chosenDashboard, clampWidth, defaultWidth, moved, stepPlace, widthAfterDrag } from "./dashboards";

const widget = (id: string): Widget => ({ id, kind: "note", width: 1, settings: {} });
const board = (id: string): Dashboard => ({ id, name: id, widgets: [], rank: "a", created_at: "", updated_at: "", created_by: { kind: "user", name: "You" } });

describe("arranging widgets", () => {
  const widgets = ["a", "b", "c"].map(widget);

  it("moves a widget to a place, as the engine does", () => {
    expect(moved(widgets, "c", 1).map((each) => each.id)).toEqual(["c", "a", "b"]);
    expect(moved(widgets, "a", 3).map((each) => each.id)).toEqual(["b", "c", "a"]);
    expect(moved(widgets, "a", 9).map((each) => each.id)).toEqual(["b", "c", "a"]);
    expect(moved(widgets, "x", 1).map((each) => each.id)).toEqual(["a", "b", "c"]);
  });

  it("steps a place earlier or later, not past the ends", () => {
    expect(stepPlace(widgets, "b", -1)).toBe(1);
    expect(stepPlace(widgets, "b", 1)).toBe(3);
    expect(stepPlace(widgets, "a", -1)).toBeNull();
    expect(stepPlace(widgets, "c", 1)).toBeNull();
  });

  it("snaps a dragged width to whole columns between one and three", () => {
    expect(widthAfterDrag(1, 250, 300, 16)).toBe(2);
    expect(widthAfterDrag(1, 100, 300, 16)).toBe(1);
    expect(widthAfterDrag(2, -400, 300, 16)).toBe(1);
    expect(widthAfterDrag(2, 2000, 300, 16)).toBe(3);
    expect(clampWidth(0)).toBe(1);
    expect(defaultWidth("cumulative_flow")).toBe(2);
    expect(defaultWidth("note")).toBe(1);
  });

  it("shows the dashboard asked for, else the first", () => {
    const boards = [board("x"), board("y")];
    expect(chosenDashboard(boards, "y")?.id).toBe("y");
    expect(chosenDashboard(boards, "gone")?.id).toBe("x");
    expect(chosenDashboard([], undefined)).toBeUndefined();
  });
});
