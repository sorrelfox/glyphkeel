// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Wiki pages, read only: the wiki's own screens come later. A link names a
// page by its title (and maybe a heading); search results by its ID.

import { useQuery } from "@tanstack/react-query";

import { type WikiPage } from "../generated/ops";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { OpFailure } from "./transport";

/** A page as a link or a search result names it. */
export type WikiPageName = { id: string } | { title: string; project?: string | undefined };

/** Splits a link's target, `Page#Heading`, at its first "#". */
export function splitTarget(target: string): { title: string; heading: string | undefined } {
  const at = target.indexOf("#");
  return at < 0 ? { title: target.trim(), heading: undefined } : { title: target.slice(0, at).trim(), heading: target.slice(at + 1).trim() || undefined };
}

const fold = (text: string) => text.toLocaleLowerCase();

/**
 * One page with its text. A title without a project is looked up in every
 * active project, through the wiki search; the first page of exactly that
 * title wins. Answers null when there is no such page.
 */
export function useWikiPage(name: WikiPageName) {
  const call = useCall();
  return useQuery<WikiPage | null, OpFailure>({
    queryKey: [...opKey("wiki.get", "id" in name ? { page: name.id } : { page: name.title, project: name.project ?? null })],
    queryFn: async () => {
      if ("id" in name) {
        return call("wiki.get", { page: name.id });
      }
      if (name.project !== undefined) {
        try {
          return await call("wiki.get", { page: name.title, project: name.project });
        } catch (error) {
          if (error instanceof OpFailure && error.error.code === "not_found") {
            return null;
          }
          throw error;
        }
      }
      const found = await call("wiki.search", { text: name.title, limit: 50 });
      const hit = found.items.find((item) => item.heading === undefined && fold(item.page.title) === fold(name.title));
      return hit ? call("wiki.get", { page: hit.page.id }) : null;
    },
  });
}
