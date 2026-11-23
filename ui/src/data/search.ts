// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The global search: ranked results across tasks, comments, wiki pages,
// files and projects, a page at a time.

import { useInfiniteQuery, type InfiniteData } from "@tanstack/react-query";

import { type SearchKind, type SearchResult, type TextRange } from "../generated/ops";
import { useCall } from "./hooks";
import { opPrefix } from "./queries";
import { type OpFailure } from "./transport";

/** Results per page. */
export const SEARCH_PAGE = 50;

/** Every kind of result, in the order the filters show them. */
export const SEARCH_KINDS: readonly SearchKind[] = ["task", "comment", "wiki", "file", "project"];

export interface SearchRequest {
  text: string;
  /** Only these kinds; empty for all. */
  kinds: readonly SearchKind[];
  /** Only this project, with its sub-projects. */
  project: string | undefined;
}

/** The results of a search, page after page; nothing is asked while the text is empty. */
export function useSearch(request: SearchRequest) {
  const call = useCall();
  const text = request.text.trim();
  const input = {
    text,
    limit: SEARCH_PAGE,
    ...(request.kinds.length === 0 ? {} : { kinds: [...request.kinds] }),
    ...(request.project === undefined ? {} : { project: request.project, include_subprojects: true }),
  };
  return useInfiniteQuery<SearchResult, OpFailure, InfiniteData<SearchResult>, readonly unknown[], string | undefined>({
    queryKey: [...opPrefix("search.query"), input],
    queryFn: ({ pageParam }) => call("search.query", { ...input, ...(pageParam === undefined ? {} : { cursor: pageParam }) }),
    initialPageParam: undefined,
    getNextPageParam: (page) => page.next_cursor,
    enabled: text !== "",
  });
}

/** A text cut into plain pieces and marked matches, for drawing highlights. */
export function highlightPieces(text: string, ranges: readonly TextRange[] | undefined): { text: string; hit: boolean }[] {
  const pieces: { text: string; hit: boolean }[] = [];
  let at = 0;
  for (const range of [...(ranges ?? [])].sort((a, b) => a.from - b.from)) {
    const from = Math.max(range.from, at);
    const to = Math.min(range.to, text.length);
    if (to <= from) {
      continue;
    }
    if (from > at) {
      pieces.push({ text: text.slice(at, from), hit: false });
    }
    pieces.push({ text: text.slice(from, to), hit: true });
    at = to;
  }
  if (at < text.length || pieces.length === 0) {
    pieces.push({ text: text.slice(at), hit: false });
  }
  return pieces;
}
