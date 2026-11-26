// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The calendar's tasks: every task of a project with a date in the days on
// screen, read page by page with the list's search.

import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { type ListedTask, type TaskSearchInput, type TaskSearchResult } from "../generated/ops";
import { rangeQuery, type Day } from "../screens/calendar/model";
import { useCall } from "./hooks";
import { opPrefix } from "./queries";
import { type OpFailure } from "./transport";

/** The calendar reads at most this many tasks for the days on screen. */
export const CALENDAR_LIMIT = 1000;

const PAGE = 200;

export interface CalendarScope {
  project: string;
  includeSubprojects: boolean;
  /** The person's query; empty for every task. */
  query: string;
  from: Day;
  to: Day;
}

export interface CalendarTasks {
  tasks: ListedTask[];
  /** How many match in all; more than `tasks` holds when the limit cut them. */
  total: number;
  error: OpFailure | null;
  pending: boolean;
  loaded: boolean;
}

/** The engine's input for the first page of a calendar's days. */
export function calendarInput(scope: CalendarScope): TaskSearchInput {
  return {
    query: rangeQuery(scope.query, scope.from, scope.to),
    project: scope.project,
    include_subprojects: scope.includeSubprojects,
    sort: "start,due",
    limit: PAGE,
  };
}

/**
 * The tasks with a date in the days on screen. The last answer stays while
 * the next is read, so paging between months doesn't flash empty; change
 * events refresh it like every search.
 */
export function useCalendarTasks(scope: CalendarScope): CalendarTasks {
  const call = useCall();
  const input = calendarInput(scope);
  const result = useQuery<{ items: ListedTask[]; total: number }, OpFailure>({
    queryKey: [...opPrefix("task.search"), { calendar: input }],
    queryFn: async () => {
      const items: ListedTask[] = [];
      let page: TaskSearchResult = await call("task.search", input);
      items.push(...page.items);
      while (page.next_cursor !== undefined && items.length < CALENDAR_LIMIT) {
        page = await call("task.search", { ...input, cursor: page.next_cursor });
        items.push(...page.items);
      }
      return { items, total: page.total };
    },
    placeholderData: keepPreviousData,
  });
  return {
    tasks: result.data?.items ?? [],
    total: result.data?.total ?? 0,
    error: result.error,
    pending: result.isPlaceholderData || result.isFetching,
    loaded: result.data !== undefined,
  };
}
