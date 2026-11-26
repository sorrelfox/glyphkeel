// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The hooks of the list: a query's matching tasks, loaded in pages as the
// rows come into view; the query's reading for the query field; saved views;
// bulk changes; and the export of every matching task.

import { keepPreviousData, useQueries, useQuery, type UseQueryResult } from "@tanstack/react-query";

import {
  type BulkChanges,
  type BulkResult,
  type Call,
  type Hue,
  type ListedTask,
  type ListGroup,
  type ListGrouping,
  type ListLayoutChange,
  type ListTotals,
  type OpMap,
  type ParsedQuery,
  type Project,
  type SavedView,
  type TaskSearchInput,
  type TaskSearchResult,
  type ViewKind,
} from "../generated/ops";
import { formatDuration, type DurationUnits } from "./duration";
import { useCall, useOp } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** Rows per page: the list never holds much more than the rows near the screen. */
export const LIST_PAGE = 200;

/** Pages out of view are dropped from memory this long after they leave it. */
const PAGE_KEEP_MS = 20_000;

/** What a list shows: a query, in a project or the whole workspace. */
export interface ListScope {
  /** The project's key; none for every project. */
  project?: string | undefined;
  includeSubprojects: boolean;
  query: string;
  /** The order when the query has no `sort:` (`due,-priority`). */
  sort?: string | undefined;
  /** What the rows are grouped by; none for one plain list. */
  groupBy?: ListGrouping | undefined;
}

/** The engine's input for one page of a list. */
export function searchInput(scope: ListScope, page: number, limit = LIST_PAGE): TaskSearchInput {
  return {
    query: scope.query,
    limit,
    ...(page > 0 ? { cursor: String(page * limit) } : {}),
    ...(scope.project !== undefined ? { project: scope.project, include_subprojects: scope.includeSubprojects } : {}),
    ...(scope.sort !== undefined && scope.sort !== "" ? { sort: scope.sort } : {}),
    ...(scope.groupBy !== undefined ? { group_by: scope.groupBy } : {}),
  };
}

/** The pages that hold rows `first` to `last`. */
export function pagesFor(first: number, last: number, total: number): number[] {
  if (total <= 0 || last < first) {
    return [0];
  }
  const from = Math.max(0, Math.floor(first / LIST_PAGE));
  const to = Math.min(Math.floor((total - 1) / LIST_PAGE), Math.floor(Math.max(first, last) / LIST_PAGE));
  const pages = [0];
  for (let page = Math.max(from, 1); page <= to; page += 1) {
    pages.push(page);
  }
  return pages;
}

export interface ListRows {
  /** How many tasks match; undefined until the first page arrives. */
  total: number | undefined;
  totals: ListTotals | undefined;
  /** The order the engine used, as sort keys. */
  sort: string[];
  /** With a grouping: the groups, in the order the rows come. */
  groups: ListGroup[] | undefined;
  /** The task at a row, once its page is loaded. */
  rowAt: (index: number) => ListedTask | undefined;
  /** The rows loaded so far, in order: for selections by range and Select all. */
  loaded: () => ListedTask[];
  /** The query doesn't run: the engine's error. */
  error: OpFailure | null;
  /** The first page of a new query is on its way; the old rows still show. */
  pending: boolean;
}

/**
 * A query's tasks, read in pages of 200 rows: the first page always (it
 * carries the total and the totals), and the pages under the rows in view.
 * The first page keeps showing the last answer while a new query runs, so
 * the list doesn't flash empty between keystrokes.
 */
export function useListRows(
  scope: ListScope,
  inView: { first: number; last: number } | ((groups: readonly ListGroup[] | undefined, total: number) => { first: number; last: number }),
  enabled = true,
): ListRows {
  const call = useCall();
  const first = useQuery<TaskSearchResult, OpFailure>({
    queryKey: opKey("task.search", searchInput(scope, 0)),
    queryFn: () => call("task.search", searchInput(scope, 0)),
    placeholderData: keepPreviousData,
    enabled,
  });
  const total = first.data?.total;
  // Grouped lists know which rows are in view once they know the groups.
  const visible = typeof inView === "function" ? inView(scope.groupBy === undefined ? undefined : first.data?.groups, total ?? 0) : inView;
  const more = pagesFor(visible.first, visible.last, total ?? 0).filter((page) => page > 0);
  const pages: UseQueryResult<TaskSearchResult, OpFailure>[] = useQueries({
    queries: more.map((page) => ({
      queryKey: opKey("task.search", searchInput(scope, page)),
      queryFn: () => call("task.search", searchInput(scope, page)),
      gcTime: PAGE_KEEP_MS,
      enabled: enabled && !first.isPlaceholderData && total !== undefined,
    })),
  });
  const byPage = new Map<number, ListedTask[]>();
  if (first.data) {
    byPage.set(0, first.data.items);
  }
  more.forEach((page, index) => {
    const items = pages[index]?.data?.items;
    if (items) {
      byPage.set(page, items);
    }
  });
  return {
    total,
    totals: first.data?.totals,
    sort: first.data?.sort ?? [],
    groups: scope.groupBy === undefined ? undefined : first.data?.groups,
    rowAt: (index) => byPage.get(Math.floor(index / LIST_PAGE))?.[index % LIST_PAGE],
    loaded: () => [...byPage.entries()].sort(([a], [b]) => a - b).flatMap(([, items]) => items),
    error: first.error,
    pending: first.isPlaceholderData || first.isFetching,
  };
}

/** How many tasks the scope holds without a query: the "of N" of the filter banner. */
export function useUnfilteredCount(scope: Omit<ListScope, "query" | "sort">): number | undefined {
  return useOp("task.search", searchInput({ ...scope, query: "" }, 0, 1)).data?.total;
}

/**
 * The query's reading, for the query field: tokens, chips and the first
 * mistake. Read again on every keystroke; the last answer stays while the
 * next is on its way, so the colours don't flicker.
 */
export function useParsedQuery(query: string): UseQueryResult<ParsedQuery, OpFailure> {
  const call = useCall();
  return useQuery<ParsedQuery, OpFailure>({
    queryKey: opKey("query.parse", { query }),
    queryFn: () => call("query.parse", { query }),
    placeholderData: keepPreviousData,
  });
}

/** The fields queries may use, for completion. */
export function useQueryFields() {
  return useOp("query.parse", { query: "", fields: true }).data?.fields;
}

/** A screen's saved views (the list's unless said): a project's own first, then the workspace's. */
export function useSavedViews(project: string | undefined, kind: ViewKind = "list"): { views: SavedView[] | undefined; error: OpFailure | null } {
  const own = useOp("view.list", { project: project ?? null, kind }, { enabled: project !== undefined });
  const shared = useOp("view.list", { kind });
  if (shared.error || own.error) {
    return { views: [], error: shared.error ?? own.error };
  }
  if (!shared.data || (project !== undefined && !own.data)) {
    return { views: undefined, error: null };
  }
  return { views: [...(own.data?.items ?? []), ...shared.data.items], error: null };
}

/** The person's layout of a list (column order and widths), per project and once for the whole workspace. */
export function useListLayout(project: string | undefined): { layout: ListLayoutChange | undefined; error: OpFailure | null } {
  const result = useOp("list.layout.get", project === undefined ? {} : { project });
  return { layout: result.data, error: result.error };
}

/** The view a list opens with: the project's default, else the workspace's, else none. */
export function defaultView(views: readonly SavedView[], project: string | undefined): SavedView | undefined {
  return (
    views.find((view) => view.is_default && project !== undefined && view.project === project) ??
    views.find((view) => view.is_default && view.project === undefined)
  );
}

/** The engine's input for a bulk change: a preview, or the change itself for the previewed count. */
export function bulkInput(keys: readonly string[], changes: BulkChanges, run: { count: number } | null): OpMap["task.bulk_update"]["input"] {
  return { keys: [...keys], changes, ...(run === null ? { dry_run: true } : { dry_run: false, expected_count: run.count }) };
}

/** Previews a bulk change: what would change, nothing changed. */
export function previewBulk(call: Call, keys: readonly string[], changes: BulkChanges): Promise<BulkResult> {
  return call("task.bulk_update", bulkInput(keys, changes, null));
}

/** Makes a bulk change after its preview; one Undo step takes it back. */
export function applyBulk(call: Call, keys: readonly string[], changes: BulkChanges, count: number): Promise<BulkResult> {
  return call("task.bulk_update", bulkInput(keys, changes, { count }));
}

/** Every task a list's query matches, page by page, for export. */
export async function allRows(call: Call, scope: ListScope): Promise<ListedTask[]> {
  const rows: ListedTask[] = [];
  let cursor: string | undefined;
  do {
    const page: TaskSearchResult = await call("task.search", { ...searchInput(scope, 0), ...(cursor === undefined ? {} : { cursor }) });
    rows.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor !== undefined);
  return rows;
}

/** The columns of an export, in order: stable, English machine names, as the engine's fields. */
export const EXPORT_COLUMNS = [
  "key",
  "title",
  "project",
  "status",
  "type",
  "priority",
  "labels",
  "parent",
  "start",
  "due",
  "estimate",
  "spent",
  "remaining",
  "estimate_minutes",
  "spent_minutes",
  "progress",
  "created",
  "updated",
  "closed",
] as const;

function exportValue(task: ListedTask, column: (typeof EXPORT_COLUMNS)[number], units: DurationUnits): string {
  const duration = (minutes: number | undefined) => (minutes === undefined ? "" : formatDuration(minutes, units));
  switch (column) {
    case "key":
      return task.key;
    case "title":
      return task.title;
    case "project":
      return task.project;
    case "status":
      return task.status.name;
    case "type":
      return task.type.name;
    case "priority":
      return task.priority;
    case "labels":
      return task.labels.map((label) => label.name).join(", ");
    case "parent":
      return task.parent ?? "";
    case "start":
      return task.start_date ?? "";
    case "due":
      return task.due_date ?? "";
    case "estimate":
      return duration(task.estimate_minutes);
    case "spent":
      return duration(task.spent_minutes);
    case "remaining":
      return duration(task.remaining_minutes);
    case "estimate_minutes":
      return task.estimate_minutes === undefined ? "" : String(task.estimate_minutes);
    case "spent_minutes":
      return String(task.spent_minutes);
    case "progress":
      return String(task.progress_percent);
    case "created":
      return task.created_at;
    case "updated":
      return task.updated_at;
    case "closed":
      return task.closed_at ?? "";
  }
}

/** A CSV field: quoted when it holds a separator, a quote or a line break. Formula starts are defused. */
function csvField(value: string): string {
  const safe = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value;
  return /[",\r\n]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

/**
 * Tasks as CSV (RFC 4180, CRLF line ends, a header row). Values stay
 * locale-neutral: ISO dates, durations in the workspace's units and in
 * minutes, priorities and fields by their machine names.
 */
export function toCsv(tasks: readonly ListedTask[], units: DurationUnits): string {
  const lines = [EXPORT_COLUMNS.join(",")];
  for (const task of tasks) {
    lines.push(EXPORT_COLUMNS.map((column) => csvField(exportValue(task, column, units))).join(","));
  }
  return `${lines.join("\r\n")}\r\n`;
}

/** Tasks as JSON: the engine's own records, with the query that chose them. */
export function toJson(tasks: readonly ListedTask[], scope: ListScope): string {
  return `${JSON.stringify({ query: scope.query, ...(scope.project !== undefined ? { project: scope.project } : {}), total: tasks.length, items: tasks }, null, 2)}\n`;
}

/** The names a query may use, for completion, and each status's colour. */
export interface ListNames {
  statuses: string[];
  types: string[];
  labels: string[];
  projects: string[];
  /** Status colours by status ID. */
  hues: ReadonlyMap<string, Hue>;
}

/**
 * The statuses, types and labels of the projects a list covers (one project
 * and its sub-projects, or every active project), and the project keys.
 */
export function useListNames(project: string | undefined): ListNames {
  const call = useCall();
  const list = useOp("project.list", { include_archived: true });
  const all = list.data?.items ?? [];
  const parentOf = new Map(all.map((each) => [each.key, each.parent]));
  const under = (key: string | undefined): boolean => key !== undefined && (key === project || under(parentOf.get(key)));
  const covered = project === undefined ? all.filter((each) => each.state === "active") : all.filter((each) => under(each.key));
  const details = useQueries({
    queries: covered.map((each) => ({
      queryKey: opKey("project.get", { project: each.key }),
      queryFn: () => call("project.get", { project: each.key }),
    })),
  });
  const unique = (names: string[]) => [...new Set(names)];
  const projects: Project[] = details.flatMap((result) => (result.data ? [result.data] : []));
  return {
    statuses: unique(projects.flatMap((each) => each.statuses.map((status) => status.name))),
    types: unique(projects.flatMap((each) => each.types.map((type) => type.name))),
    labels: unique(projects.flatMap((each) => each.labels.map((label) => label.name))),
    projects: all.map((each) => each.key),
    hues: new Map(projects.flatMap((each) => each.statuses.map((status) => [status.id, status.hue] as const))),
  };
}
