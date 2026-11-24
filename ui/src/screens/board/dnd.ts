// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Drag and drop on the board: which drop target is under the card, how the
// keyboard steps a card through the lanes, and which cards a virtual lane
// keeps mounted.

import { closestCenter, pointerWithin, type CollisionDetection, type KeyboardCoordinateGetter, type UniqueIdentifier } from "@dnd-kit/core";
import { defaultRangeExtractor, type Range } from "@tanstack/react-virtual";

/**
 * The target under the pointer, else the closest one. `closestCorners` alone
 * sometimes picked the lane next to the one under the pointer. The keyboard
 * has no pointer: its steps put the dragged card's top start corner just
 * inside the target (see `indexCoordinates`), so that corner stands in for
 * one. The closest centre alone picked the lane over a card in a long lane,
 * whose centre is near the cards in its middle.
 */
export const pointerThenCenter: CollisionDetection = (args) => {
  const { collisionRect } = args;
  const pointerCoordinates = args.pointerCoordinates ?? { x: collisionRect.left, y: collisionRect.top };
  const within = pointerWithin({ ...args, pointerCoordinates });
  if (within.length > 0) {
    // Prefer a card under the pointer to the lane that holds it.
    const card = within.find((collision) => args.droppableContainers.find((container) => container.id === collision.id)?.data.current?.kind === "card");
    return card ? [card] : within;
  }
  return closestCenter(args);
};

/** The lanes during a drag: their IDs and the card keys they show, in order. */
export interface LaneOrder {
  id: string;
  cards: readonly string[];
  /** Cards can't be dropped here: collapsed, or not a status of the dragged card's project. */
  closed: boolean;
}

/** Where a keyboard step goes: a card, or an empty lane. */
export interface Step {
  lane: string;
  index: number;
  target: UniqueIdentifier;
}

/**
 * One keyboard step from `current` (the card or lane the dragged card is
 * over). Up and down step one index in the lane; left and right go to the
 * next open lane at the same index, or its end. Steps by index, not by
 * coordinates, so a virtual lane whose neighbours aren't drawn yet still
 * moves one card per key press.
 */
export function step(lanes: readonly LaneOrder[], current: UniqueIdentifier, active: UniqueIdentifier, code: string): Step | null {
  const id = String(current);
  const laneIndex = lanes.findIndex((lane) => lane.id === id || lane.cards.includes(id));
  const lane = lanes[laneIndex];
  if (!lane) {
    return null;
  }
  const index = lane.id === id ? lane.cards.length : lane.cards.indexOf(id);
  if (code === "ArrowUp" || code === "ArrowDown") {
    const next = index + (code === "ArrowUp" ? -1 : 1);
    const target = lane.cards[next];
    return target === undefined ? null : { lane: lane.id, index: next, target };
  }
  if (code !== "ArrowLeft" && code !== "ArrowRight") {
    return null;
  }
  const direction = code === "ArrowLeft" ? -1 : 1;
  for (let other = laneIndex + direction; other >= 0 && other < lanes.length; other += direction) {
    const candidate = lanes[other];
    if (!candidate || candidate.closed) {
      continue;
    }
    const cards = candidate.cards.filter((key) => key !== String(active));
    const at = Math.min(index, cards.length);
    const target = cards[at];
    return { lane: candidate.id, index: at, target: target ?? candidate.id };
  }
  return null;
}

/** What a lane tells the keyboard sensor through its drop target's data. */
export interface LaneData {
  kind: "lane";
  cards: readonly string[];
  closed: boolean;
  /** Brings an index of a virtual lane into view. */
  scrollTo?: ((index: number) => void) | undefined;
}

function isLaneData(data: unknown): data is LaneData {
  return typeof data === "object" && data !== null && "kind" in data && data.kind === "lane";
}

/** How far inside its target a keyboard step puts the dragged card's corner, in pixels. */
const INSIDE = 8;

/**
 * dnd-kit's keyboard sensor with index steps. The lanes, left to right, come
 * from their drop targets' data. Before it answers, the target is scrolled
 * into view and measured there, so the sensor never spends a key press on
 * scrolling. A virtual lane is asked to draw the target first; if it isn't
 * drawn yet, that key press does nothing.
 */
export const indexCoordinates: KeyboardCoordinateGetter = (event, { active, currentCoordinates, context }) => {
  const { over, collisionRect, droppableContainers } = context;
  const lanes = droppableContainers
    .toArray()
    .filter((container) => isLaneData(container.data.current))
    .map((container) => ({ container, left: container.node.current?.getBoundingClientRect().left ?? 0 }))
    .sort((a, b) => a.left - b.left)
    .map(({ container }) => {
      const data = container.data.current as LaneData;
      return { id: String(container.id), cards: data.cards, closed: container.disabled || data.closed, scrollTo: data.scrollTo };
    });
  const next = step(lanes, over?.id ?? active, active, event.code);
  if (!next || !collisionRect) {
    return undefined;
  }
  event.preventDefault();
  lanes.find((lane) => lane.id === next.lane)?.scrollTo?.(next.index);
  const node = droppableContainers.get(next.target)?.node.current;
  if (!node) {
    return undefined;
  }
  node.scrollIntoView({ block: "nearest", inline: "nearest" });
  // Where the target is without the shift a drag gives the cards it passes:
  // drop targets are found by those places, not by where the cards are drawn.
  const drawn = node.getBoundingClientRect();
  const shift = new DOMMatrixReadOnly(getComputedStyle(node).transform === "none" ? undefined : getComputedStyle(node).transform);
  const rect = { left: drawn.left - shift.m41, top: drawn.top - shift.m42, width: drawn.width };
  // The keyboard's "pointer" is the dragged card's top start corner, and the
  // pointer picks the drop target first: put that corner a little inside the
  // target, not on its edge, where it could miss it by a pixel and land on
  // the lane (its end) instead. An empty lane: the card goes to its top.
  const isLane = String(next.target) === next.lane;
  return {
    x: currentCoordinates.x + (rect.left - collisionRect.left) + (isLane ? (rect.width - collisionRect.width) / 2 : INSIDE),
    y: currentCoordinates.y + (rect.top - collisionRect.top) + INSIDE,
  };
};

/** The rows a virtual lane draws: the visible range, plus the dragged card, so dnd-kit keeps its node. */
export function keepMounted(range: Range, dragged: number): number[] {
  const indexes = defaultRangeExtractor(range);
  if (dragged >= 0 && !indexes.includes(dragged)) {
    indexes.push(dragged);
    indexes.sort((a, b) => a - b);
  }
  return indexes;
}
