// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Today: what needs attention across the projects, and the sidebar's counts.

import { useQuery } from "@tanstack/react-query";

import { type Today } from "../generated/ops";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** The input of every Today read, so that the screen and the sidebar share one answer. */
const TODAY_INPUT = {};

/**
 * The lists of Today and their counts. Read again every few minutes as well
 * as on changes, so that a new day moves tasks into "Overdue".
 */
export function useTodayOverview() {
  const call = useCall();
  return useQuery<Today, OpFailure>({
    queryKey: opKey("today.get", TODAY_INPUT),
    queryFn: () => call("today.get", TODAY_INPUT),
    refetchInterval: 5 * 60_000,
  });
}

/** The sidebar's counts: tasks on Today, and every open task (All tasks). */
export function useSidebarCounts(): { today: number; open: number } | undefined {
  const today = useTodayOverview();
  return today.data ? { today: today.data.counts.today, open: today.data.counts.open } : undefined;
}
