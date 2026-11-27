// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Markdown from the engine: rendered texts for the reader, blocks for the
// editor's preview, checkbox clicks, attached images and task-key
// completion.

import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";

import type { BlockRenderer, CompletionProviders, TaskSuggestion } from "../editor/types";
import { type Call, type ChecklistChange, type ChecklistItem, type Rendered, type RenderSource } from "../generated/ops";
import { imageUrl, inApp } from "../ipc/files";
import { useCall, useOp } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** The engine renders at most this many texts per call. */
const RENDER_BATCH = 200;

/** A task's description as the reader shows it. */
export function useRenderedDescription(key: string) {
  return useOp("markdown.render", { documents: [{ task: key }] });
}

/**
 * A wiki page's text as the reader shows it, rendered as that page's so its
 * macros expand. The text is part of the question, so every change to it,
 * a tick included, asks again: meanwhile the page's previous answer stays,
 * and the reader keeps its place instead of emptying.
 */
export function useRenderedPage(page: { id: string; body: string } | null) {
  const call = useCall();
  const id = page?.id ?? "";
  const input = { documents: [{ source: page?.body ?? "", page: id, document_id: `wiki-${id}` }] };
  return useQuery({
    queryKey: opKey("markdown.render", input),
    queryFn: () => call("markdown.render", input),
    enabled: page !== null,
    placeholderData: (previous, previousQuery) => (previousQuery?.queryKey[2] === undefined || renderedPageOf(previousQuery.queryKey[2]) !== id ? undefined : previous),
  });
}

/** The page a wiki page's render question names. */
function renderedPageOf(input: unknown): string | undefined {
  const documents = (input as { documents?: readonly { page?: string }[] } | null)?.documents;
  return documents?.[0]?.page;
}

/** Several texts, rendered in as few calls as the engine allows; answers in the same order. */
export function useRendered(documents: readonly RenderSource[]): { items: (Rendered | undefined)[]; error: OpFailure | null } {
  const call = useCall();
  const batches: RenderSource[][] = [];
  for (let start = 0; start < documents.length; start += RENDER_BATCH) {
    batches.push(documents.slice(start, start + RENDER_BATCH));
  }
  const results = useQueries({
    queries: batches.map((batch) => {
      const input = { documents: batch };
      return { queryKey: opKey("markdown.render", input), queryFn: () => call("markdown.render", input) };
    }),
  });
  const items: (Rendered | undefined)[] = [];
  results.forEach((result, index) => {
    const batch = batches[index] ?? [];
    batch.forEach((_document, offset) => {
      items.push(result.data?.items[offset]);
    });
  });
  return { items, error: (results.find((result) => result.error)?.error as OpFailure | undefined) ?? null };
}

/** What a click on a checkbox in the reader asks for: the engine's number and the item's text as rendered. */
export interface CheckboxClick {
  number: number;
  checked: boolean;
}

/**
 * Ticks or clears one item through the engine, named by its number and
 * text as the reader showed them: if the text changed meanwhile, the engine
 * finds the item by its text, or refuses.
 */
export function toggleChecklist(
  call: Call,
  owner: { task: string } | { comment: string } | { page: string },
  items: readonly ChecklistItem[],
  click: CheckboxClick,
): Promise<ChecklistChange> {
  const item = items.find((candidate) => candidate.number === click.number);
  return call("checklist.update", {
    ...("task" in owner ? { key: owner.task } : "comment" in owner ? { comment: owner.comment } : { page: owner.page }),
    toggle: [{ number: click.number, text: item?.text ?? "", checked: click.checked }],
  });
}

/** Where the reader gets an attached image: from the app's `gk-file:` protocol, or in a browser from its content. */
export function useImageSource(): (attachment: string) => Promise<string | null> {
  const call = useCall();
  const queryClient = useQueryClient();
  return async (attachment) => {
    if (inApp()) {
      return imageUrl(attachment);
    }
    const input = { attachment };
    const [file, content] = await Promise.all([
      queryClient.query({ queryKey: opKey("file.get", input), queryFn: () => call("file.get", input) }),
      queryClient.query({ queryKey: opKey("file.read", input), queryFn: () => call("file.read", input) }),
    ]);
    if (file.kind !== "image" || content.data_base64 === undefined) {
      return null;
    }
    return `data:${file.mime};base64,${content.data_base64}`;
  };
}

/** The editor's tables and callouts, rendered by the engine like the reader's. */
export function useBlockRenderer(): BlockRenderer {
  const call = useCall();
  // One renderer for the editor's life: the editor caches blocks per renderer.
  return useCallback<BlockRenderer>(
    async (source) => {
      const answer = await call("markdown.render", { documents: [{ source, document_id: "preview" }] });
      return answer.items[0]?.html ?? "";
    },
    [call],
  );
}

const MAX_SUGGESTIONS = 20;

/** Task keys for the editor's completion, from the project's board: by key ("TM-4" → "TM-41") or by a word of the title. */
export function useTaskCompletion(): CompletionProviders {
  const call = useCall();
  const queryClient = useQueryClient();
  return useMemo<CompletionProviders>(() => ({
    tasks: async (projectKey, query) => {
      const input = { project: projectKey, show_older: true };
      let board;
      try {
        board = await queryClient.query({ queryKey: opKey("board.get", input), queryFn: () => call("board.get", input), staleTime: 30_000, gcTime: 30_000 });
      } catch {
        return [];
      }
      const word = query.toLowerCase();
      const found: TaskSuggestion[] = [];
      for (const column of board.columns) {
        for (const card of column.cards) {
          const number = card.key.slice(card.key.indexOf("-") + 1);
          if (word === "" || number.startsWith(word) || card.title.toLowerCase().includes(word)) {
            found.push({ key: card.key, title: card.title });
          }
        }
      }
      return found.sort((a, b) => a.key.length - b.key.length || a.key.localeCompare(b.key, "en", { numeric: true })).slice(0, MAX_SUGGESTIONS);
    },
  }), [call, queryClient]);
}
