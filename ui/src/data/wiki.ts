// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Wiki pages: one page by a link's title or by its ID, a project's tree,
// and the moves that dragging or the keyboard make in it. A link names a
// page by its title (and maybe a heading); search results by its ID.

import { useQuery } from "@tanstack/react-query";

import { type Backlink, type Reference, type WikiMoveInput, type WikiPage, type WikiPageSummary } from "../generated/ops";
import { useCall, useOp } from "./hooks";
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

/** A project's pages in tree order. */
export function useWikiTree(project: string) {
  return useOp("wiki.list", { project });
}

/** One page of the tree with the pages it holds. */
export interface WikiNode {
  page: WikiPageSummary;
  children: WikiNode[];
}

/** The pages of `wiki.list`, in tree order, as nested nodes. */
export function buildTree(items: readonly WikiPageSummary[]): WikiNode[] {
  const nodes = new Map<string, WikiNode>(items.map((page) => [page.id, { page, children: [] }]));
  const roots: WikiNode[] = [];
  for (const page of items) {
    const node = nodes.get(page.id);
    const parent = page.parent === undefined ? undefined : nodes.get(page.parent);
    if (!node) {
      continue;
    }
    if (parent) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  }
  return roots;
}

/** Whether `id` is `ancestor` or lies inside it. */
export function isInside(items: readonly WikiPageSummary[], id: string, ancestor: string): boolean {
  const byId = new Map(items.map((page) => [page.id, page]));
  for (let at: string | undefined = id; at !== undefined; at = byId.get(at)?.parent) {
    if (at === ancestor) {
      return true;
    }
  }
  return false;
}

/** Where a dragged page is dropped: before or after another page, or inside it. */
export type DropPosition = "before" | "after" | "on";

/**
 * The move that puts `dragged` at a drop target, or null when it goes
 * nowhere: onto itself or into a page inside it.
 */
export function moveFor(items: readonly WikiPageSummary[], dragged: string, target: string, position: DropPosition): WikiMoveInput | null {
  const moving = items.find((page) => page.id === dragged);
  const onto = items.find((page) => page.id === target);
  if (!moving || !onto || isInside(items, target, dragged)) {
    return null;
  }
  if (position === "on") {
    return { page: dragged, parent: target, at: "bottom" };
  }
  const parent = onto.parent;
  const place = position === "before" ? { before: target } : { after: target };
  if (parent === moving.parent) {
    return { page: dragged, ...place };
  }
  return parent === undefined ? { page: dragged, top_level: true, ...place } : { page: dragged, parent, ...place };
}

/** A move made from the keyboard: up or down among its siblings, into the page above it, or out of its parent. */
export type KeyboardMove = "up" | "down" | "in" | "out";

/** The move a key asks for, or null when the page can't go that way. */
export function keyboardMove(items: readonly WikiPageSummary[], id: string, direction: KeyboardMove): WikiMoveInput | null {
  const page = items.find((candidate) => candidate.id === id);
  if (!page) {
    return null;
  }
  const siblings = items.filter((candidate) => candidate.parent === page.parent);
  const index = siblings.indexOf(page);
  const previous = siblings[index - 1];
  const next = siblings[index + 1];
  switch (direction) {
    case "up":
      return previous ? { page: id, before: previous.id } : null;
    case "down":
      return next ? { page: id, after: next.id } : null;
    case "in":
      return previous ? { page: id, parent: previous.id, at: "bottom" } : null;
    case "out": {
      if (page.parent === undefined) {
        return null;
      }
      const holder = items.find((candidate) => candidate.id === page.parent);
      return holder?.parent === undefined ? { page: id, top_level: true, after: page.parent } : { page: id, parent: holder.parent, after: page.parent };
    }
  }
}

/** What a page's text links to, by the target as written: found or not, and where. */
export function referenceIndex(references: readonly Reference[] | undefined): Map<string, Reference> {
  const index = new Map<string, Reference>();
  for (const reference of references ?? []) {
    if (reference.kind === "page") {
      index.set(fold(reference.target.trim()), reference);
    }
  }
  return index;
}

/** The pages and tasks that link to a page, split for the aside: pages, and tasks (descriptions and comments). */
export function splitBacklinks(items: readonly Backlink[]): { pages: Backlink[]; tasks: Backlink[] } {
  const pages = items.filter((item) => item.page !== undefined);
  // A task's description and its comments name the same task: listed once.
  const tasks: Backlink[] = [];
  for (const item of items) {
    if (item.task !== undefined && !tasks.some((other) => other.task === item.task)) {
      tasks.push(item);
    }
  }
  return { pages, tasks };
}
