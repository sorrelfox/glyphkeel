// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart's data: the engine's schedule of a project, with its
// sub-projects when asked and narrowed by a query.

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useEffect } from "react";

import { type Baseline, type GanttLayoutChange, type MovedTask, type Schedule, type ScheduleInput, type SchedulePreview, type SchedulePreviewInput } from "../generated/ops";
import { useCall, useOp } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

export interface ScheduleScope {
  project: string;
  includeSubprojects: boolean;
  /** The person's query; empty for every task. */
  query: string;
  /** The baseline to compare with, by ID; none: no comparison. */
  baseline?: string | undefined;
}

export function scheduleInput(scope: ScheduleScope): ScheduleInput {
  return {
    project: scope.project,
    include_subprojects: scope.includeSubprojects,
    ...(scope.query.trim() === "" ? {} : { query: scope.query }),
    ...(scope.baseline === undefined ? {} : { baseline: scope.baseline }),
  };
}

/** A project's schedule baselines, newest first. */
export function useBaselines(project: string): { baselines: Baseline[] | undefined; error: OpFailure | null } {
  const result = useOp("baseline.list", { project });
  return { baselines: result.data?.items, error: result.error };
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
  // A claim ends without a change event: read the schedule again when the
  // first one shown ends, so the agent's name doesn't stay until a reload.
  const { data, refetch } = result;
  useEffect(() => {
    const now = Date.now();
    const ends = firstClaimEnd(data, now);
    if (ends === undefined) {
      return undefined;
    }
    const timer = window.setTimeout(() => void refetch(), Math.min(MAX_TIMER, ends - now + CLAIM_GRACE));
    return () => {
      window.clearTimeout(timer);
    };
  }, [data, refetch]);
  return { schedule: result.data, error: result.error, pending: result.isPlaceholderData || result.isFetching };
}

/** The longest wait a browser timer takes (claims last hours at most). */
const MAX_TIMER = 2_147_483_647;
/** A moment after a claim ends, so the engine sees it ended too. */
const CLAIM_GRACE = 1_000;

/**
 * When the first claim in `schedule` that is still on at `now` ends
 * (milliseconds since the epoch); none: `undefined`.
 */
export function firstClaimEnd(schedule: Schedule | undefined, now: number): number | undefined {
  let first: number | undefined;
  for (const task of [...(schedule?.tasks ?? []), ...(schedule?.unscheduled ?? [])]) {
    const until = task.claim === undefined ? Number.NaN : Date.parse(task.claim.until);
    if (until > now && (first === undefined || until < first)) {
      first = until;
    }
  }
  return first;
}

/** The person's layout of a project's Gantt chart, as the engine keeps it. */
export function useGanttLayout(project: string): { layout: GanttLayoutChange | undefined; error: OpFailure | null } {
  const result = useOp("gantt.layout.get", { project });
  return { layout: result.data, error: result.error };
}

/**
 * The tasks "also move dependents" would move if a task got the dates in
 * `input`, from the engine without changing anything; none asked: nothing.
 * The answer for other dates stays until the new one comes, marked stale.
 */
export function useMovePreview(input: SchedulePreviewInput | null): { moved: MovedTask[] | undefined; stale: boolean } {
  const call = useCall();
  const result = useQuery<SchedulePreview, OpFailure>({
    queryKey: opKey("schedule.preview_move", input ?? { key: "" }),
    queryFn: () => call("schedule.preview_move", input ?? { key: "" }),
    enabled: input !== null,
    placeholderData: keepPreviousData,
    staleTime: 5_000,
  });
  if (input === null) {
    return { moved: undefined, stale: false };
  }
  return { moved: result.data?.moved, stale: result.isPlaceholderData };
}
