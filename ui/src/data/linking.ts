// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What the editor completes in descriptions and comments: tasks after "#"
// (by a word of the title or by key), task keys typed in capitals, and wiki
// pages and their headings after "[[".

import { useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import type { CompletionProviders, PageSuggestion, TaskSuggestion } from "../editor/types";
import { type WikiSearchHit } from "../generated/ops";
import { useCall } from "./hooks";
import { useTaskCompletion } from "./markdown";
import { opKey } from "./queries";

/** How many suggestions a search offers. */
export const LINK_SUGGESTIONS = 20;

/** A key being typed: "TM-", "tm-4". */
const KEY_PREFIX = /^([A-Za-z][A-Za-z0-9]{0,9})-(\d*)$/u;

/**
 * The words of a search as the task query reads them: letters, digits, "-"
 * and "_" only, so that nothing typed after "#" turns into a condition.
 */
export function searchWords(text: string): string {
  return text
    .replace(/[^\p{L}\p{N}_\s-]/gu, " ")
    .split(/\s+/u)
    .filter((word) => /[\p{L}\p{N}]/u.test(word))
    .join(" ");
}

/** What goes between a wiki link's brackets for a search result. */
export function pageTarget(hit: WikiSearchHit): PageSuggestion {
  if (hit.heading) {
    return { target: `${hit.page.title}#${hit.heading.text}`, detail: hit.page.title };
  }
  return { target: hit.page.title, detail: hit.page.project };
}

/**
 * The editor's completion for the text of a task in `project`: task keys,
 * tasks by title or key after "#", and that project's wiki pages after "[[".
 */
export function useLinkCompletion(project: string | undefined): CompletionProviders {
  const call = useCall();
  const queryClient = useQueryClient();
  const keys = useTaskCompletion();
  return useMemo<CompletionProviders>(
    () => ({
      ...keys,
      findTasks: async (query): Promise<readonly TaskSuggestion[]> => {
        const key = KEY_PREFIX.exec(query.trim());
        if (key && keys.tasks) {
          const found = await keys.tasks((key[1] ?? "").toUpperCase(), key[2] ?? "");
          if (found.length > 0) {
            return found;
          }
        }
        const words = searchWords(query);
        const input = words === "" ? { query: "", sort: "-updated", limit: LINK_SUGGESTIONS } : { query: words, limit: LINK_SUGGESTIONS };
        try {
          const result = await queryClient.query({ queryKey: opKey("task.search", input), queryFn: () => call("task.search", input), staleTime: 10_000 });
          return result.items.map((task) => ({ key: task.key, title: task.title }));
        } catch {
          return [];
        }
      },
      pages: async (query): Promise<readonly PageSuggestion[]> => {
        const input = { text: query, limit: LINK_SUGGESTIONS, ...(project === undefined ? {} : { project }) };
        try {
          const result = await queryClient.query({ queryKey: opKey("wiki.search", input), queryFn: () => call("wiki.search", input), staleTime: 10_000 });
          // A heading that only repeats its page's title adds nothing.
          return result.items.filter((hit) => hit.heading === undefined || query.includes("#") || hit.heading.text.toLocaleLowerCase() !== hit.page.title.toLocaleLowerCase()).map(pageTarget);
        } catch {
          return [];
        }
      },
    }),
    [call, queryClient, keys, project],
  );
}
