// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What the board shows, worked out from the engine's board, the personal
// layout and the quick filters. Pure functions, so the rules are tested
// without a page.

import { parseDuration as parseTyped } from "../../data/duration";
import { PRIORITIES, type Board, type BoardLayout, type Card, type Column, type Place, type Priority, type SortMode } from "../../data/catalog";

export const DUE_FILTERS = ["overdue", "today", "week", "none"] as const;
export type DueFilter = (typeof DUE_FILTERS)[number];

/** The board's quick filters. */
export interface Filters {
  text: string;
  labels: string[];
  priorities: Priority[];
  due: DueFilter[];
  /** Sprint IDs, and {@link BACKLOG} for cards in no sprint. */
  sprints: string[];
  recentlyUpdated: boolean;
}

/** The sprint filter's choice for cards in no sprint. */
export const BACKLOG = "*backlog";

export const NO_FILTERS: Readonly<Filters> = { text: "", labels: [], priorities: [], due: [], sprints: [], recentlyUpdated: false };

/** "Recently updated" means within this many days. */
export const RECENT_DAYS = 7;

export function isFiltered(filters: Filters): boolean {
  return (
    filters.text.trim() !== "" ||
    filters.labels.length > 0 ||
    filters.priorities.length > 0 ||
    filters.due.length > 0 ||
    filters.sprints.length > 0 ||
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
export function dueState(due: string | null | undefined, closed: boolean, today: string): "overdue" | "today" | "soon" | null {
  if (due === null || due === undefined || closed) {
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
    const closed = card.closed_at !== undefined;
    const due = card.due_date ?? null;
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
  if (filters.sprints.length > 0 && !filters.sprints.includes(card.sprint?.id ?? BACKLOG)) {
    return false;
  }
  if (filters.recentlyUpdated && now.getTime() - Date.parse(card.updated_at) > RECENT_DAYS * 86_400_000) {
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
        if (a.due_date === undefined) {
          return 1;
        }
        if (b.due_date === undefined) {
          return -1;
        }
        return a.due_date < b.due_date ? -1 : 1;
      });
    case "updated":
      return sorted.sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at) || manual(a, b));
  }
}

/**
 * Columns in the personal order. A column the layout doesn't know yet, such as
 * a status added since, goes in at its project position, after the column
 * that comes before it in the project's order.
 */
export function orderColumns(columns: readonly Column[], order: readonly string[]): Column[] {
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
  return result.map((id) => byId.get(id)).filter((column): column is Column => column !== undefined);
}

export interface Lane {
  column: Column;
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
  hidden: { column: Column; count: number }[];
  /** Cards on the board, and how many the filters let through. */
  total: number;
  shown: number;
  open: number;
}

/** Everything the board draws. */
export function boardView(board: Board, layout: BoardLayout, filters: Filters, now: Date, showOlder: ReadonlySet<string>): BoardView {
  const cutoff = now.getTime() - layout.hide_done_after_days * 86_400_000;
  const lanes: Lane[] = [];
  const hidden: BoardView["hidden"] = [];
  let shown = 0;
  for (const column of orderColumns(board.columns, layout.order)) {
    if (layout.hidden.includes(column.id)) {
      hidden.push({ column, count: column.count });
      continue;
    }
    const all = column.cards;
    const matching = all.filter((card) => matches(card, filters, now));
    const closedColumn = isClosedCategory(column.category);
    const hideOld = closedColumn && layout.hide_done_after_days > 0 && !showOlder.has(column.id);
    const recent = hideOld ? matching.filter((card) => card.closed_at === undefined || Date.parse(card.closed_at) >= cutoff) : matching;
    const sort = layout.column_sort[column.id] ?? layout.sort_mode;
    lanes.push({
      column,
      collapsed: layout.collapsed.includes(column.id),
      sort,
      cards: sortCards(recent, sort),
      total: column.count,
      // Older cards the engine kept back, and older cards here that the column doesn't show yet.
      older: (showOlder.has(column.id) ? 0 : column.older) + matching.length - recent.length,
    });
    shown += recent.length;
  }
  const total = board.columns.reduce((sum, column) => sum + column.count, 0);
  // A task is closed exactly when its status is.
  const open = board.columns.reduce((sum, column) => sum + (isClosedCategory(column.category) ? 0 : column.count), 0);
  return { lanes, hidden, total, shown, open };
}

const isClosedCategory = (category: Column["category"]) => category === "done" || category === "cancelled";

/** The new order after moving one column a step left or right; the order lists every column. */
export function moveColumn(columns: readonly Column[], order: readonly string[], id: string, step: -1 | 1): string[] {
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
export function positionIn(ids: readonly string[], key: string): Place {
  const at = ids.indexOf(key);
  const next = ids[at + 1];
  if (next !== undefined) {
    return { before: next };
  }
  const previous = ids[at - 1];
  return previous !== undefined ? { after: previous } : { at: "bottom" };
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
 * A duration as typed: "90m", "1.5h", "1,5h", "1:30", "2d", "1w", "1h 30m".
 * Days and weeks use the workspace's working hours and days. Null when it
 * doesn't parse.
 */
export function parseDuration(text: string, hoursPerDay = 8, daysPerWeek = 5): number | null {
  return parseTyped(text, { hoursPerDay, daysPerWeek });
}
