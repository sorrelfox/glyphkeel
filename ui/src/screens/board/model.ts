// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What the board shows, worked out from the engine's board, the personal
// layout and the quick filters. Pure functions, so the rules are tested
// without a page.

import { PRIORITIES, type Board, type BoardColumn, type BoardLayout, type Card, type Priority, type SortMode } from "../../data/catalog";

export const DUE_FILTERS = ["overdue", "today", "week", "none"] as const;
export type DueFilter = (typeof DUE_FILTERS)[number];

/** The board's quick filters. */
export interface Filters {
  text: string;
  labels: string[];
  priorities: Priority[];
  due: DueFilter[];
  changedByAgents: boolean;
  recentlyUpdated: boolean;
}

export const NO_FILTERS: Readonly<Filters> = { text: "", labels: [], priorities: [], due: [], changedByAgents: false, recentlyUpdated: false };

/** "Recently updated" means within this many days. */
export const RECENT_DAYS = 7;

export function isFiltered(filters: Filters): boolean {
  return (
    filters.text.trim() !== "" ||
    filters.labels.length > 0 ||
    filters.priorities.length > 0 ||
    filters.due.length > 0 ||
    filters.changedByAgents ||
    filters.recentlyUpdated
  );
}

/** A day as `YYYY-MM-DD` in local time. */
export function isoDay(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${String(date.getFullYear())}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function addDays(day: string, days: number): string {
  const [year, month, date] = day.split("-").map(Number);
  return isoDay(new Date(year ?? 1970, (month ?? 1) - 1, (date ?? 1) + days));
}

/** How a due date reads: overdue, today, soon (within three days), or plain. Closed tasks are never overdue. */
export function dueState(due: string | null, closed: boolean, today: string): "overdue" | "today" | "soon" | null {
  if (due === null || closed) {
    return null;
  }
  if (due < today) {
    return "overdue";
  }
  if (due === today) {
    return "today";
  }
  return due <= addDays(today, 3) ? "soon" : null;
}

export function matches(card: Card, filters: Filters, now: Date): boolean {
  const text = filters.text.trim().toLowerCase();
  if (text !== "" && !card.title.toLowerCase().includes(text) && !card.key.toLowerCase().includes(text)) {
    return false;
  }
  if (filters.labels.length > 0 && !card.labels.some((label) => filters.labels.includes(label.name))) {
    return false;
  }
  if (filters.priorities.length > 0 && !filters.priorities.includes(card.priority)) {
    return false;
  }
  if (filters.due.length > 0) {
    const today = isoDay(now);
    const weekEnd = addDays(today, 7);
    const closed = card.closed_at !== null;
    const due = card.due_date;
    const hit = filters.due.some((kind) => {
      switch (kind) {
        case "overdue":
          return due !== null && !closed && due < today;
        case "today":
          return due === today;
        case "week":
          return due !== null && due >= today && due <= weekEnd;
        case "none":
          return due === null;
      }
    });
    if (!hit) {
      return false;
    }
  }
  if (filters.changedByAgents && card.updated.actor.kind !== "agent") {
    return false;
  }
  if (filters.recentlyUpdated && now.getTime() - Date.parse(card.updated.at) > RECENT_DAYS * 86_400_000) {
    return false;
  }
  return true;
}

const priorityRank = (priority: Priority) => PRIORITIES.indexOf(priority);

/** Cards in a sort mode. Manual keeps the shared order; the others break ties by it. */
export function sortCards(cards: readonly Card[], mode: SortMode): Card[] {
  const order = new Map(cards.map((card, index) => [card.key, index]));
  const manual = (a: Card, b: Card) => (order.get(a.key) ?? 0) - (order.get(b.key) ?? 0);
  const sorted = [...cards];
  switch (mode) {
    case "manual":
      return sorted;
    case "priority":
      return sorted.sort((a, b) => priorityRank(b.priority) - priorityRank(a.priority) || manual(a, b));
    case "due":
      return sorted.sort((a, b) => {
        if (a.due_date === b.due_date) {
          return manual(a, b);
        }
        if (a.due_date === null) {
          return 1;
        }
        if (b.due_date === null) {
          return -1;
        }
        return a.due_date < b.due_date ? -1 : 1;
      });
    case "updated":
      return sorted.sort((a, b) => Date.parse(b.updated.at) - Date.parse(a.updated.at) || manual(a, b));
  }
}

/**
 * Columns in the personal order. A column the layout doesn't know yet, such as
 * a status added since, goes in at its project position, after the column
 * that comes before it in the project's order.
 */
export function orderColumns(columns: readonly BoardColumn[], order: readonly string[]): BoardColumn[] {
  const byId = new Map(columns.map((column) => [column.id, column]));
  const result = order.filter((id) => byId.has(id));
  columns.forEach((column, index) => {
    if (result.includes(column.id)) {
      return;
    }
    const previous = columns[index - 1];
    const at = previous ? result.indexOf(previous.id) + 1 : 0;
    result.splice(at, 0, column.id);
  });
  return result.map((id) => byId.get(id)).filter((column): column is BoardColumn => column !== undefined);
}

export interface Lane {
  column: BoardColumn;
  collapsed: boolean;
  sort: SortMode;
  /** The cards shown, in order. */
  cards: Card[];
  /** All cards in the column, filters aside: what the WIP count reads. */
  total: number;
  /** Matching cards hidden because they closed long ago. */
  older: number;
}

export interface BoardView {
  lanes: Lane[];
  hidden: { column: BoardColumn; count: number }[];
  /** Cards on the board, and how many the filters let through. */
  total: number;
  shown: number;
  open: number;
}

/** Everything the board draws. */
export function boardView(board: Board, layout: BoardLayout, filters: Filters, now: Date, showOlder: ReadonlySet<string>): BoardView {
  const byColumn = new Map<string, Card[]>();
  for (const card of board.cards) {
    const column = board.column_of[card.key];
    if (column !== undefined) {
      byColumn.set(column, [...(byColumn.get(column) ?? []), card]);
    }
  }
  const cutoff = now.getTime() - layout.hide_done_after_days * 86_400_000;
  const lanes: Lane[] = [];
  const hidden: BoardView["hidden"] = [];
  let shown = 0;
  for (const column of orderColumns(board.columns, layout.order)) {
    const all = byColumn.get(column.id) ?? [];
    if (layout.hidden.includes(column.id)) {
      hidden.push({ column, count: all.length });
      continue;
    }
    const matching = all.filter((card) => matches(card, filters, now));
    const closedColumn = column.category === "done" || column.category === "cancelled";
    const hideOld = closedColumn && layout.hide_done_after_days > 0 && !showOlder.has(column.id);
    const recent = hideOld ? matching.filter((card) => card.closed_at === null || Date.parse(card.closed_at) >= cutoff) : matching;
    const sort = layout.column_sort[column.id] ?? layout.sort_mode;
    lanes.push({
      column,
      collapsed: layout.collapsed.includes(column.id),
      sort,
      cards: sortCards(recent, sort),
      total: all.length,
      older: matching.length - recent.length,
    });
    shown += recent.length;
  }
  return {
    lanes,
    hidden,
    total: board.cards.length,
    shown,
    open: board.cards.filter((card) => card.closed_at === null).length,
  };
}

/** The new order after moving one column a step left or right; the order lists every column. */
export function moveColumn(columns: readonly BoardColumn[], order: readonly string[], id: string, step: -1 | 1): string[] {
  const ids = orderColumns(columns, order).map((column) => column.id);
  const at = ids.indexOf(id);
  const to = at + step;
  if (at < 0 || to < 0 || to >= ids.length) {
    return ids;
  }
  const other = ids[to];
  if (other === undefined) {
    return ids;
  }
  ids[to] = id;
  ids[at] = other;
  return ids;
}

/** Where a card goes, given the visible order it should land in. */
export function positionIn(ids: readonly string[], key: string): { before: string } | { after: string } | "bottom" {
  const at = ids.indexOf(key);
  const next = ids[at + 1];
  if (next !== undefined) {
    return { before: next };
  }
  const previous = ids[at - 1];
  return previous !== undefined ? { after: previous } : "bottom";
}

/** Minutes as people read them: "1h 30m", "45m", "2h". */
export function formatMinutes(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) {
    return `${String(rest)}m`;
  }
  return rest === 0 ? `${String(hours)}h` : `${String(hours)}h ${String(rest)}m`;
}

/**
 * A duration as typed: "90m", "1.5h", "1:30", "2d", "1w", "1h 30m". Days and
 * weeks use the workspace's working hours and days. Null when it doesn't parse.
 */
export function parseDuration(text: string, hoursPerDay = 8, daysPerWeek = 5): number | null {
  const trimmed = text.trim().toLowerCase();
  if (trimmed === "") {
    return null;
  }
  const clock = /^(\d+):([0-5]\d)$/.exec(trimmed);
  if (clock) {
    return Number(clock[1]) * 60 + Number(clock[2]);
  }
  const unit: Record<string, number> = { m: 1, h: 60, d: 60 * hoursPerDay, w: 60 * hoursPerDay * daysPerWeek };
  let total = 0;
  let rest = trimmed;
  const part = /^\s*(\d+(?:\.\d+)?)\s*([mhdw])/;
  while (rest.trim() !== "") {
    const found = part.exec(rest);
    if (!found) {
      return null;
    }
    total += Number(found[1]) * (unit[found[2] ?? "m"] ?? 1);
    rest = rest.slice(found[0].length);
  }
  return Math.round(total);
}
