// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The list's rules without React: its columns, sorting by a column header,
// the query text edits that chips and headers make, and selections.

import { type QueryToken, type SavedView, type TextRange } from "../../generated/ops";

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

/** A view's columns, known ones only, title always among them. */
export function viewColumns(view: Pick<SavedView, "columns"> | undefined, workspaceWide: boolean): ColumnId[] {
  const named = (view?.columns ?? []).filter(isColumn);
  const columns = named.length > 0 ? named : [...(workspaceWide ? DEFAULT_COLUMNS_ALL : DEFAULT_COLUMNS)];
  return columns.includes("title") ? columns : ["title", ...columns];
}

/** The grid's column template: the selection box, then the columns. */
export function gridTemplate(columns: readonly ColumnId[]): string {
  return ["28px", ...columns.map((column) => COLUMN_WIDTH[column])].join(" ");
}

/** How a column is sorted now: by the first sort key only, as the header shows it. */
export function sortState(sort: readonly string[], column: ColumnId): "ascending" | "descending" | null {
  const key = SORT_KEY[column];
  const first = sort[0];
  if (key === undefined || first === undefined) {
    return null;
  }
  if (first === key) {
    return "ascending";
  }
  return first === `-${key}` ? "descending" : null;
}

/**
 * The sort after a click on a header: ascending, then descending, then the
 * default order. Returns the sort keys, empty for the default.
 */
export function nextSort(sort: readonly string[], column: ColumnId): string[] {
  const key = SORT_KEY[column];
  if (key === undefined) {
    return [...sort];
  }
  switch (sortState(sort, column)) {
    case "ascending":
      return [`-${key}`];
    case "descending":
      return [];
    default:
      return [key];
  }
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
}

/** Whether the list differs from its saved view: "Changed since last save". */
export function changedFrom(view: SavedView | undefined, setup: ListSetup, workspaceWide: boolean): boolean {
  if (!view) {
    return setup.query.trim() !== "" || setup.sort !== "";
  }
  return (
    view.query.trim() !== setup.query.trim() ||
    (view.sort ?? "") !== setup.sort ||
    viewColumns(view, workspaceWide).join(",") !== setup.columns.join(",")
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
