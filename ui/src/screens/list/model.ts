// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The list's rules without React: its columns, sorting by column headers,
// groups of rows, the query text edits that chips and headers make, and
// selections.

import { PRIORITIES } from "../../data/catalog";
import { type ListGrouping, type QueryToken, type SavedView, type TaskCreateInput, type TextRange } from "../../generated/ops";

/** Every column the list offers, in their default order. */
export const COLUMNS = [
  "key",
  "title",
  "project",
  "type",
  "status",
  "priority",
  "labels",
  "start",
  "due",
  "estimate",
  "spent",
  "remaining",
  "progress",
  "created",
  "updated",
] as const;
export type ColumnId = (typeof COLUMNS)[number];

/** The columns shown when a view names none. Title is always shown. */
export const DEFAULT_COLUMNS: readonly ColumnId[] = ["key", "title", "status", "priority", "labels", "due", "estimate", "spent", "updated"];

/** The workspace-wide list shows the project too. */
export const DEFAULT_COLUMNS_ALL: readonly ColumnId[] = ["key", "title", "project", "status", "priority", "labels", "due", "estimate", "spent", "updated"];

/** Columns that can't be hidden. */
export const FIXED_COLUMNS: readonly ColumnId[] = ["title"];

/** Widths in the grid; the title takes the rest. */
export const COLUMN_WIDTH: Record<ColumnId, string> = {
  key: "76px",
  title: "minmax(240px, 1fr)",
  project: "84px",
  type: "104px",
  status: "128px",
  priority: "108px",
  labels: "168px",
  start: "92px",
  due: "92px",
  estimate: "84px",
  spent: "84px",
  remaining: "88px",
  progress: "76px",
  created: "104px",
  updated: "104px",
};

/** Columns whose numbers align at the end. */
export const NUMERIC_COLUMNS: ReadonlySet<ColumnId> = new Set(["estimate", "spent", "remaining", "progress"]);

/** The query field a column sorts by; columns without one don't sort. */
export const SORT_KEY: Partial<Record<ColumnId, string>> = {
  key: "key",
  title: "title",
  project: "project",
  type: "type",
  status: "status",
  priority: "priority",
  start: "start",
  due: "due",
  estimate: "estimate",
  spent: "spent",
  progress: "progress",
  created: "created",
  updated: "updated",
};

export function isColumn(value: string): value is ColumnId {
  return (COLUMNS as readonly string[]).includes(value);
}

/**
 * A view's columns, known ones only, title always among them. A view that
 * names none shows the person's own columns for the list, if any, else the
 * defaults.
 */
export function viewColumns(view: Pick<SavedView, "columns"> | undefined, workspaceWide: boolean, own: readonly string[] = []): ColumnId[] {
  const named = (view?.columns ?? []).filter(isColumn);
  const mine = own.filter(isColumn);
  const columns = named.length > 0 ? named : mine.length > 0 ? mine : [...(workspaceWide ? DEFAULT_COLUMNS_ALL : DEFAULT_COLUMNS)];
  return columns.includes("title") ? columns : ["title", ...columns];
}

/** The narrowest and widest a column can be made, in pixels (the engine's limits). */
export const MIN_WIDTH = 40;
export const MAX_WIDTH = 1200;

/** A column's width in pixels: the person's own, else its default (the title's least). */
export function columnWidth(column: ColumnId, widths: Readonly<Record<string, number>>): number {
  return widths[column] ?? Number(/\d+/.exec(COLUMN_WIDTH[column])?.[0] ?? MIN_WIDTH);
}

/**
 * The grid's column template: the selection box, then the columns. A width
 * the person chose is kept exactly; the title otherwise takes the rest.
 */
export function gridTemplate(columns: readonly ColumnId[], widths: Readonly<Record<string, number>> = {}): string {
  return ["28px", ...columns.map((column) => (widths[column] === undefined ? COLUMN_WIDTH[column] : `${String(widths[column])}px`))].join(" ");
}

/** Moves a column to another place among the shown ones. */
export function moveColumn(columns: readonly ColumnId[], column: ColumnId, to: number): ColumnId[] {
  const rest = columns.filter((each) => each !== column);
  const at = Math.max(0, Math.min(rest.length, to));
  return [...rest.slice(0, at), column, ...rest.slice(at)];
}

/** How a column is sorted now, and its place among the sort keys (1 for the first). */
export function sortState(sort: readonly string[], column: ColumnId): { direction: "ascending" | "descending"; position: number } | null {
  const key = SORT_KEY[column];
  if (key === undefined) {
    return null;
  }
  const index = sort.findIndex((each) => each === key || each === `-${key}`);
  if (index < 0) {
    return null;
  }
  return { direction: sort[index]?.startsWith("-") === true ? "descending" : "ascending", position: index + 1 };
}

/**
 * The sort after a click on a header: ascending, then descending, then the
 * default order. With `add` (Shift held) the column is one more sort key
 * after the others, or its own key goes the same round without touching the
 * others. Returns the sort keys, empty for the default.
 */
export function nextSort(sort: readonly string[], column: ColumnId, add = false): string[] {
  const key = SORT_KEY[column];
  if (key === undefined) {
    return [...sort];
  }
  const state = sortState(sort, column);
  if (add) {
    if (state === null) {
      return [...sort, key];
    }
    const index = state.position - 1;
    return state.direction === "ascending" ? sort.map((each, at) => (at === index ? `-${key}` : each)) : sort.filter((_, at) => at !== index);
  }
  // A plain click makes the column the only key; on the first key it turns it round, then drops it.
  if (state?.position !== 1) {
    return [key];
  }
  return state.direction === "ascending" ? [`-${key}`] : [];
}

/** The groupings the list offers, in the menu's order; Project only where several projects show. */
export const GROUPINGS = ["status", "priority", "label", "parent", "due_week", "project"] as const satisfies readonly ListGrouping[];

export function isGrouping(value: string | undefined): value is ListGrouping {
  return (GROUPINGS as readonly string[]).includes(value ?? "");
}

/** One line of a grouped list: a group's header, a task row by its place among all rows, or the line that adds a task to a group. */
export type ListLine = { kind: "group"; group: number } | { kind: "row"; row: number } | { kind: "add"; group: number };

/** The lines of a list, with or without groups, and the way between lines and rows. */
export interface ListLines {
  /** How many lines show: rows and group headers, without the rows of collapsed groups. */
  count: number;
  at: (line: number) => ListLine | undefined;
  /** The line of a row; undefined when its group is collapsed. */
  lineOf: (row: number) => number | undefined;
  /** The rows the lines `first` to `last` cover, for loading their pages. */
  rowsIn: (first: number, last: number) => { first: number; last: number };
}

/**
 * Lines for rows in groups of the given sizes (in row order); collapsed
 * groups show only their header. With `adds`, an open group ends with a line
 * that adds a task to it. Without groups every line is a row.
 */
export function listLines(total: number, groups: readonly { count: number }[] | undefined, collapsed: (group: number) => boolean = () => false, adds = false): ListLines {
  if (groups === undefined || groups.length === 0) {
    return {
      count: total,
      at: (line) => (line >= 0 && line < total ? { kind: "row", row: line } : undefined),
      lineOf: (row) => row,
      rowsIn: (first, last) => ({ first, last }),
    };
  }
  // Per group: its first line (the header) and its first row.
  const lineStart: number[] = [];
  const rowStart: number[] = [];
  let line = 0;
  let row = 0;
  groups.forEach((group, index) => {
    lineStart.push(line);
    rowStart.push(row);
    line += 1 + (collapsed(index) ? 0 : group.count + (adds ? 1 : 0));
    row += group.count;
  });
  const count = line;
  /** The last group whose start (in `starts`) is at or before `value`. */
  const groupAt = (starts: readonly number[], value: number) => {
    let low = 0;
    let high = starts.length - 1;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if ((starts[middle] ?? 0) <= value) {
        low = middle;
      } else {
        high = middle - 1;
      }
    }
    return low;
  };
  const at = (index: number): ListLine | undefined => {
    if (index < 0 || index >= count) {
      return undefined;
    }
    const group = groupAt(lineStart, index);
    const offset = index - (lineStart[group] ?? 0);
    if (offset === 0) {
      return { kind: "group", group };
    }
    const rows = (rowStart[group + 1] ?? row) - (rowStart[group] ?? 0);
    return offset > rows ? { kind: "add", group } : { kind: "row", row: (rowStart[group] ?? 0) + offset - 1 };
  };
  return {
    count,
    at,
    lineOf: (target) => {
      const group = groupAt(rowStart, target);
      return collapsed(group) ? undefined : (lineStart[group] ?? 0) + 1 + target - (rowStart[group] ?? 0);
    },
    rowsIn: (first, last) => {
      const rowOf = (index: number, forward: boolean) => {
        const found = at(index);
        if (found === undefined) {
          return forward ? 0 : Math.max(0, row - 1);
        }
        if (found.kind === "row") {
          return found.row;
        }
        if (found.kind === "add") {
          // After the group's last row.
          const next = rowStart[found.group + 1] ?? row;
          return forward ? Math.min(next, Math.max(0, row - 1)) : Math.max(0, next - 1);
        }
        return forward ? (rowStart[found.group] ?? 0) : Math.max(0, (rowStart[found.group] ?? 0) - 1);
      };
      return { first: rowOf(first, true), last: Math.max(rowOf(first, true), rowOf(last, false)) };
    },
  };
}

/** Removes pieces of a query and tidies the spaces left behind. */
export function removeRanges(text: string, ranges: readonly TextRange[]): string {
  let result = text;
  for (const range of [...ranges].sort((a, b) => b.from - a.from)) {
    result = result.slice(0, range.from) + result.slice(range.to);
  }
  return result.replace(/\s{2,}/g, " ").trim();
}

/**
 * The query with its `sort:` replaced (or added, or removed for an empty
 * sort). The rest of the text stays exactly as typed.
 */
export function withSort(text: string, tokens: readonly QueryToken[], sort: readonly string[]): string {
  const token = tokens.find((candidate) => candidate.kind === "sort");
  const term = sort.length > 0 ? `sort:${sort.join(",")}` : "";
  if (!token) {
    return term === "" ? text : `${text.trimEnd()}${text.trim() === "" ? "" : " "}${term}`;
  }
  const replaced = text.slice(0, token.range.from) + term + text.slice(token.range.to);
  return term === "" ? replaced.replace(/\s{2,}/g, " ").trim() : replaced;
}

/** What the list shows, compared with the saved view it came from. */
export interface ListSetup {
  query: string;
  sort: string;
  columns: readonly string[];
  /** The grouping, "" for none. */
  groupBy: string;
}

/**
 * Whether the list differs from its saved view: "Changed since last save".
 * `columns` are the ones the view opens with.
 */
export function changedFrom(view: SavedView | undefined, setup: ListSetup, columns: readonly string[]): boolean {
  if (!view) {
    return setup.query.trim() !== "" || setup.sort !== "" || setup.groupBy !== "";
  }
  return (
    view.query.trim() !== setup.query.trim() ||
    (view.sort ?? "") !== setup.sort ||
    (view.group_by ?? "") !== setup.groupBy ||
    columns.join(",") !== setup.columns.join(",")
  );
}

/** A selection: tasks by key, with the row a Shift-click extends from. */
export interface Selection {
  keys: ReadonlySet<string>;
  anchor: number | null;
}

export const NO_SELECTION: Selection = { keys: new Set(), anchor: null };

/**
 * A click on a row: alone it selects the row, with Ctrl (or Cmd) it toggles
 * it, with Shift it selects from the anchor to it. `keyAt` names the task of
 * a loaded row.
 */
export function clickRow(selection: Selection, index: number, key: string, mods: { toggle: boolean; range: boolean }, keyAt: (index: number) => string | undefined): Selection {
  if (mods.range && selection.anchor !== null) {
    const keys = new Set(mods.toggle ? selection.keys : []);
    const [from, to] = selection.anchor < index ? [selection.anchor, index] : [index, selection.anchor];
    for (let at = from; at <= to; at += 1) {
      const each = keyAt(at);
      if (each !== undefined) {
        keys.add(each);
      }
    }
    return { keys, anchor: selection.anchor };
  }
  if (mods.toggle) {
    const keys = new Set(selection.keys);
    if (keys.has(key)) {
      keys.delete(key);
    } else {
      keys.add(key);
    }
    return { keys, anchor: index };
  }
  return { keys: new Set([key]), anchor: index };
}

/** A file name for an export: the view's name (or "tasks") and today's date. */
export function exportName(view: string | undefined, today: string, extension: "csv" | "json"): string {
  const base = (view ?? "tasks").replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "").toLowerCase() || "tasks";
  return `${base}-${today}.${extension}`;
}

/**
 * The new task the "+" line of a group adds: in the list's project (or the
 * group's, grouped by project; a parent's, grouped by parent), with the
 * group's status, priority, label, parent or due week (its first day)
 * filled in. Undefined where there is no project to put it in.
 */
export function groupTaskInput(grouping: ListGrouping, group: { value?: string }, project: string | undefined, title: string): TaskCreateInput | undefined {
  const value = group.value;
  const parentProject = grouping === "parent" && value !== undefined ? value.slice(0, Math.max(0, value.lastIndexOf("-"))) : "";
  const home = grouping === "project" ? value : parentProject !== "" ? parentProject : project;
  if (home === undefined || home === "") {
    return undefined;
  }
  const input: TaskCreateInput = { project: home, title };
  if (value === undefined) {
    return input;
  }
  switch (grouping) {
    case "status":
      return { ...input, status: value };
    case "priority": {
      const priority = PRIORITIES.find((candidate) => candidate === value);
      return priority === undefined ? input : { ...input, priority };
    }
    case "label":
      return { ...input, labels: [value] };
    case "parent":
      return { ...input, parent: value };
    case "due_week":
      return { ...input, due_date: value };
    case "project":
      return input;
  }
}
