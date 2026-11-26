// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart's data: the engine's schedule of a project, with its
// sub-projects when asked and narrowed by a query.

import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { type Schedule, type ScheduleInput } from "../generated/ops";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

export interface ScheduleScope {
  project: string;
  includeSubprojects: boolean;
  /** The person's query; empty for every task. */
  query: string;
}

export function scheduleInput(scope: ScheduleScope): ScheduleInput {
  return {
    project: scope.project,
    include_subprojects: scope.includeSubprojects,
    ...(scope.query.trim() === "" ? {} : { query: scope.query }),
  };
}

export interface ScheduleData {
  schedule: Schedule | undefined;
  error: OpFailure | null;
  pending: boolean;
}

/**
 * The schedule on screen. The last answer stays while the next is read, so
 * changing the query or the scope doesn't flash empty; change events
 * refresh it.
 */
export function useSchedule(scope: ScheduleScope): ScheduleData {
  const call = useCall();
  const input = scheduleInput(scope);
  const result = useQuery<Schedule, OpFailure>({
    queryKey: opKey("schedule.get", input),
    queryFn: () => call("schedule.get", input),
    placeholderData: keepPreviousData,
  });
  return { schedule: result.data, error: result.error, pending: result.isPlaceholderData || result.isFetching };
}
