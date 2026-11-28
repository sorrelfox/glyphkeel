// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Card } from "../../data/catalog";
import { type Swimlane } from "../../generated/ops";
import { cellId, laneChanges, parseCell, withLane } from "./swimlanes";

const lane = (id: string, more: Partial<Swimlane> = {}): Swimlane => ({ id, name: id, value: id, empty: false, settable: true, count: 0, counts: {}, ...more });
const EMPTY: Swimlane = { id: "none", name: "", empty: true, settable: true, count: 0, counts: {} };

const card = (more: Partial<Card> = {}) =>
  ({ id: "t1", key: "TM-1", project: "TM", priority: "low", labels: [{ id: "bug", name: "bug", hue: "red" }], lane: "bug", ...more }) as Card;

describe("swimlane cells", () => {
  it("joins and splits a lane and a column", () => {
    const id = cellId("Front end", "col-1");
    expect(parseCell(id)).toEqual({ lane: "Front end", column: "col-1" });
    expect(parseCell("col-1")).toBeUndefined();
  });
});

describe("what a move into another lane changes", () => {
  it("sets the priority, or none in the lane without one", () => {
    expect(laneChanges("priority", "", card(), lane("low"), lane("urgent"), "Urgent")).toEqual({ changes: { priority: "urgent" } });
    expect(laneChanges("priority", "", card(), lane("low"), EMPTY, "No priority")).toEqual({ changes: { priority: "none" } });
  });

  it("swaps the lane's label for the target's, and clears labels in the empty lane", () => {
    expect(laneChanges("label", "", card(), lane("bug"), lane("ui"), "ui")).toEqual({ changes: { add_labels: ["ui"], remove_labels: ["bug"] } });
    expect(laneChanges("label", "", card(), EMPTY, lane("ui"), "ui")).toEqual({ changes: { add_labels: ["ui"] } });
    expect(laneChanges("label", "", card(), lane("bug"), EMPTY, "No label")).toEqual({ changes: { clear: ["labels"] } });
  });

  it("sets or clears the sprint, and refuses a completed one", () => {
    expect(laneChanges("sprint", "", card(), EMPTY, lane("s1"), "One")).toEqual({ changes: { sprint: "s1" } });
    expect(laneChanges("sprint", "", card(), lane("s1"), EMPTY, "Backlog")).toEqual({ changes: { clear: ["sprint"] } });
    expect(laneChanges("sprint", "", card(), EMPTY, lane("s0", { settable: false }), "Zero")).toEqual({ refused: { id: "swimlane-refused-closed", args: { lane: "Zero" } } });
  });

  it("sets the parent, refusing the card's own lane and another project's parent", () => {
    expect(laneChanges("parent", "", card(), EMPTY, lane("p1", { key: "TM-9" }), "TM-9 Epic")).toEqual({ changes: { parent: "p1" } });
    expect(laneChanges("parent", "", card(), EMPTY, lane("t1", { key: "TM-1" }), "TM-1")).toEqual({ refused: { id: "swimlane-refused-own-parent", args: { key: "TM-1" } } });
    expect(laneChanges("parent", "", card(), EMPTY, lane("p2", { key: "ZZ-2" }), "ZZ-2 Other")).toEqual({ refused: { id: "swimlane-refused-other-project", args: { key: "TM-1", lane: "ZZ-2 Other" } } });
    expect(laneChanges("parent", "", card(), lane("p1"), EMPTY, "No parent")).toEqual({ changes: { clear: ["parent"] } });
  });

  it("sets the type by name and a select field's option", () => {
    expect(laneChanges("type", "", card(), lane("x"), lane("bug-type", { value: "Bug" }), "Bug")).toEqual({ changes: { type: "Bug" } });
    expect(laneChanges("field", "cf.area", card(), EMPTY, lane("Web"), "Web")).toEqual({ changes: { fields: { "cf.area": "Web" } } });
    expect(laneChanges("field", "cf.area", card(), lane("Web"), EMPTY, "No value")).toEqual({ changes: { fields: { "cf.area": null } } });
  });

  it("shows the card in the target lane at once", () => {
    expect(withLane(card(), lane("urgent"), "priority")).toMatchObject({ lane: "urgent", priority: "urgent" });
    expect(withLane(card(), lane("ui", { hue: "blue" }), "label").labels).toEqual([{ id: "ui", name: "ui", hue: "blue" }]);
    expect(withLane(card(), EMPTY, "label").labels).toEqual([]);
  });
});

describe("collapsed lanes", () => {
  it("are the person's own, per project and grouping", async () => {
    const { loadCollapsedLanes, saveCollapsedLanes } = await import("../../prefs/store");
    saveCollapsedLanes("p1", "priority", ["low"]);
    expect(loadCollapsedLanes("p1", "priority")).toEqual(["low"]);
    expect(loadCollapsedLanes("p1", "label")).toEqual([]);
    expect(loadCollapsedLanes("p2", "priority")).toEqual([]);
  });
});
