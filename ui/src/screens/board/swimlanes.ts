// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The board's swimlanes, worked out from the engine's lanes and the board
// view: which cards each lane holds in each column, the drop targets (one
// cell per lane and column), and what a card dropped into another lane
// changes. Pure functions, so the rules are tested without a page.

import { type Card, type Priority } from "../../data/catalog";
import { type LaneBy, type MoveChanges, type Swimlane } from "../../generated/ops";
import { type BoardView } from "./model";

/** The lane of the cards without a value. */
export const EMPTY_LANE = "none";

/** The groupings the toolbar offers, custom fields aside. */
export const LANE_KINDS = ["none", "parent", "priority", "label", "sprint", "type"] as const satisfies readonly LaneBy[];

/** The groupings the board across projects offers: by project first, its default. */
export const ACROSS_LANE_KINDS = ["project", "none", "parent", "priority", "label", "sprint", "type"] as const satisfies readonly LaneBy[];

/** Between a lane's ID and a column's in a cell's ID: no ID holds it. */
const SEPARATOR = "\u001f";

/** The drop target of one lane in one column. */
export function cellId(lane: string, column: string): string {
  return `${lane}${SEPARATOR}${column}`;
}

/** The lane and column of a cell's ID; undefined for any other ID. */
export function parseCell(id: string): { lane: string; column: string } | undefined {
  const at = id.indexOf(SEPARATOR);
  return at < 0 ? undefined : { lane: id.slice(0, at), column: id.slice(at + 1) };
}

export interface SwimlaneView {
  lane: Swimlane;
  /** Collapsed by the person. */
  collapsed: boolean;
  /** The cards shown in each column (filters, sort and older cards applied), by column ID. */
  cells: Record<string, Card[]>;
  /** Its cards in each column, filters aside, by column ID. */
  counts: Record<string, number>;
  /** Its cards on the board, filters aside. */
  total: number;
}

/** The lane a card is in: the one it names, else the lane without a value. */
export const laneOfCard = (card: Card): string => card.lane ?? EMPTY_LANE;

/**
 * The swimlanes as the board draws them, in the engine's order. Counts come
 * from the cards, so a move shows in them at once.
 */
export function swimlaneView(lanes: readonly Swimlane[], view: BoardView, collapsed: readonly string[]): SwimlaneView[] {
  const known = new Set(lanes.map((lane) => lane.id));
  const place = (card: Card) => (known.has(laneOfCard(card)) ? laneOfCard(card) : EMPTY_LANE);
  return lanes.map((lane) => {
    const cells: Record<string, Card[]> = {};
    const counts: Record<string, number> = {};
    let total = 0;
    for (const column of view.lanes) {
      cells[column.column.id] = column.cards.filter((card) => place(card) === lane.id);
      const all = column.column.cards.filter((card) => place(card) === lane.id).length;
      counts[column.column.id] = all;
      total += all;
    }
    return { lane, collapsed: collapsed.includes(lane.id), cells, counts, total };
  });
}

/** Why a card can't go into a lane, as a message ID with its arguments; undefined when it can. */
export interface Refusal {
  id: string;
  args: Record<string, string>;
}

/**
 * What a card moving from lane `from` into lane `to` changes, or why it
 * can't. Labels: the card leaves its lane's label for the target's; into the
 * lane without a value it loses every label, so it lands there.
 */
export function laneChanges(by: LaneBy, laneField: string, card: Card, from: Swimlane | undefined, to: Swimlane, name: string): { changes: MoveChanges } | { refused: Refusal } {
  if (by === "project") {
    // A card changes project by a move of its own, never by a drop.
    return { refused: { id: "swimlane-refused-project", args: { key: card.key, lane: name } } };
  }
  if (!to.settable) {
    return { refused: { id: "swimlane-refused-closed", args: { lane: name } } };
  }
  const value = to.empty ? undefined : to.value;
  switch (by) {
    case "none":
      return { changes: {} };
    case "priority":
      return { changes: { priority: value === undefined ? "none" : (value as Priority) } };
    case "type":
      return value === undefined ? { refused: { id: "swimlane-refused-closed", args: { lane: name } } } : { changes: { type: value } };
    case "label": {
      if (value === undefined) {
        return { changes: { clear: ["labels"] } };
      }
      const leaving = from && !from.empty && from.value !== undefined ? [from.value] : [];
      return { changes: { add_labels: [value], ...(leaving.length > 0 ? { remove_labels: leaving } : {}) } };
    }
    case "sprint":
      return { changes: value === undefined ? { clear: ["sprint"] } : { sprint: value } };
    case "parent": {
      if (value === undefined) {
        return { changes: { clear: ["parent"] } };
      }
      if (value === card.id) {
        return { refused: { id: "swimlane-refused-own-parent", args: { key: card.key } } };
      }
      const project = to.key?.slice(0, to.key.lastIndexOf("-"));
      if (project !== undefined && project !== card.project) {
        return { refused: { id: "swimlane-refused-other-project", args: { key: card.key, lane: name } } };
      }
      return { changes: { parent: value } };
    }
    case "field":
      return { changes: { fields: { [laneField]: value ?? null } } };
  }
}

/** The card as it will look once a lane's changes are made: it shows in the target lane at once. */
export function withLane(card: Card, to: Swimlane, by: LaneBy): Card {
  const moved: Card = { ...card, lane: to.id };
  if (by === "priority") {
    moved.priority = to.empty ? "none" : (to.value as Priority);
  } else if (by === "label") {
    const others = card.labels.filter((label) => label.id !== card.lane);
    moved.labels = to.empty ? [] : [{ id: to.id, name: to.name, hue: to.hue ?? "grey" }, ...others.filter((label) => label.name !== to.name)];
  }
  return moved;
}
