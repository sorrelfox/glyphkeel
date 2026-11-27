// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's global search, Today lists and wiki pages: plainer
// than the engine's, enough for the screens and tests.

import {
  type ListedTask,
  type SearchHit,
  type SearchInput,
  type SearchKind,
  type SearchResult,
  type TextRange,
  type WikiHeading,
  type WikiPageSummary,
  type WikiSearchHit,
  type WikiSearchInput,
} from "../../generated/ops";
import { addDays } from "./time";
import { type FakeWikiPage } from "./state";

/** Where each word of `words` occurs in `text`, ignoring case. */
export function matchRanges(text: string, words: readonly string[]): TextRange[] {
  const lower = text.toLowerCase();
  const ranges: TextRange[] = [];
  for (const word of words) {
    for (let at = lower.indexOf(word); at >= 0; at = lower.indexOf(word, at + word.length)) {
      ranges.push({ from: at, to: at + word.length });
    }
  }
  return ranges.sort((a, b) => a.from - b.from);
}

/** A piece of one line around the first match, with the matches in it. */
export function snippetOf(text: string, words: readonly string[]): { snippet: string; snippet_matches: TextRange[] } | null {
  const flat = text.replace(/\s+/g, " ");
  const first = matchRanges(flat, words)[0];
  if (!first) {
    return null;
  }
  const start = Math.max(0, first.from - 40);
  const snippet = flat.slice(start, start + 120);
  return { snippet, snippet_matches: matchRanges(snippet, words) };
}

const holdsAll = (text: string, words: readonly string[]) => words.every((word) => text.toLowerCase().includes(word));

/** What a search looks through, as the engine's index holds it. */
export interface SearchCorpus {
  tasks: readonly { task: ListedTask; description: string; projectActive: boolean }[];
  comments: readonly { id: string; task: ListedTask; body: string; updatedAt: string }[];
  files: readonly { id: string; task: ListedTask; name: string; createdAt: string }[];
  pages: readonly { page: FakeWikiPage; project: string }[];
  projects: readonly { id: string; key: string; name: string; active: boolean }[];
  /** Whether a project key is the one asked for (or below it). */
  inScope: (project: string) => boolean;
}

/** `search.query`: every word must occur; title matches first, then text, then comments and files. */
export function searchAll(corpus: SearchCorpus, input: SearchInput): SearchResult | "empty" {
  const words = input.text
    .toLowerCase()
    .split(/\s+/)
    .map((word) => word.replace(/^"|"$/g, ""))
    .filter((word) => word !== "");
  if (words.length === 0) {
    return "empty";
  }
  const kinds = new Set<SearchKind>(input.kinds && input.kinds.length > 0 ? input.kinds : ["project", "task", "comment", "wiki", "file"]);
  const found: { group: number; hit: SearchHit }[] = [];
  const taskFields = (task: ListedTask) => ({ project: task.project, task: task.key, status: task.status.name, category: task.status.category });
  const typedKey = input.text.trim().toUpperCase();
  for (const { task, description, projectActive } of corpus.tasks) {
    if (!kinds.has("task") || !projectActive || !corpus.inScope(task.project)) {
      continue;
    }
    if (task.key === typedKey) {
      found.push({ group: -1, hit: { kind: "task", id: task.id, matched_in: "key", title: task.title, updated_at: task.updated_at, ...taskFields(task) } });
    } else if (holdsAll(task.title, words)) {
      found.push({ group: 0, hit: { kind: "task", id: task.id, matched_in: "title", title: task.title, title_matches: matchRanges(task.title, words), updated_at: task.updated_at, ...taskFields(task) } });
    } else if (holdsAll(`${task.title} ${description}`, words)) {
      found.push({ group: 1, hit: { kind: "task", id: task.id, matched_in: "text", title: task.title, title_matches: matchRanges(task.title, words), ...snippetOf(description, words), updated_at: task.updated_at, ...taskFields(task) } });
    }
  }
  for (const { page, project } of corpus.pages) {
    if (!kinds.has("wiki") || !corpus.inScope(project) || (input.query ?? "") !== "") {
      continue;
    }
    const title = holdsAll(page.title, words);
    if (title || holdsAll(`${page.title} ${page.body}`, words)) {
      found.push({
        group: title ? 0 : 1,
        hit: { kind: "wiki", id: page.id, matched_in: title ? "title" : "text", project, slug: page.slug, title: page.title, title_matches: matchRanges(page.title, words), ...(title ? {} : snippetOf(page.body, words)), updated_at: page.updatedAt },
      });
    }
  }
  for (const comment of corpus.comments) {
    if (kinds.has("comment") && corpus.inScope(comment.task.project) && holdsAll(comment.body, words)) {
      found.push({ group: 2, hit: { kind: "comment", id: comment.id, matched_in: "text", title: comment.task.title, ...snippetOf(comment.body, words), updated_at: comment.updatedAt, ...taskFields(comment.task) } });
    }
  }
  for (const file of corpus.files) {
    if (kinds.has("file") && corpus.inScope(file.task.project) && holdsAll(file.name, words)) {
      found.push({ group: 2, hit: { kind: "file", id: file.id, matched_in: "title", title: file.name, title_matches: matchRanges(file.name, words), updated_at: file.createdAt, ...taskFields(file.task) } });
    }
  }
  for (const project of corpus.projects) {
    if (kinds.has("project") && project.active && corpus.inScope(project.key) && (project.key === typedKey || holdsAll(project.name, words))) {
      found.push({ group: 0, hit: { kind: "project", id: project.id, matched_in: project.key === typedKey ? "key" : "title", project: project.key, title: project.name, title_matches: matchRanges(project.name, words), updated_at: "" } });
    }
  }
  found.sort((a, b) => a.group - b.group || b.hit.updated_at.localeCompare(a.hit.updated_at));
  const offset = input.cursor ? Number(input.cursor) : 0;
  const limit = Math.min(Math.max(input.limit ?? 50, 1), 200);
  const items = found.slice(offset, offset + limit).map((entry) => entry.hit);
  const end = offset + items.length;
  return { items, total: found.length, ...(end < found.length ? { next_cursor: String(end) } : {}) };
}

/** A page's headings, from its Markdown. */
export function headingsOf(body: string): WikiHeading[] {
  const headings: WikiHeading[] = [];
  body.split(/\r\n|\r|\n/).forEach((line, index) => {
    const match = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (match) {
      headings.push({ level: (match[1] ?? "").length, line: index + 1, text: match[2] ?? "" });
    }
  });
  return headings;
}

/** `wiki.search`: titles (beginnings first), headings, text; `Page#Heading` lists headings; empty text lists recent pages. */
export function searchWiki(pages: readonly { page: FakeWikiPage; summary: WikiPageSummary }[], input: WikiSearchInput): WikiSearchHit[] {
  const text = (input.text ?? "").trim();
  const hits: WikiSearchHit[] = [];
  if (text === "") {
    return [...pages].sort((a, b) => b.page.updatedAt.localeCompare(a.page.updatedAt)).map(({ summary }) => ({ matched_in: "recent", page: summary }));
  }
  const hash = text.indexOf("#");
  if (hash >= 0) {
    const title = text.slice(0, hash).trim().toLowerCase();
    const heading = text.slice(hash + 1).trim().toLowerCase();
    for (const { page, summary } of pages) {
      if (!page.title.toLowerCase().includes(title)) {
        continue;
      }
      for (const found of headingsOf(page.body)) {
        if (found.text.toLowerCase().includes(heading)) {
          hits.push({ matched_in: "heading", page: summary, heading: found, heading_matches: heading === "" ? [] : matchRanges(found.text, [heading]) });
        }
      }
    }
    return hits;
  }
  const words = text.toLowerCase().split(/\s+/);
  const ranked: { rank: number; hit: WikiSearchHit }[] = [];
  for (const { page, summary } of pages) {
    if (holdsAll(page.title, words)) {
      ranked.push({ rank: page.title.toLowerCase().startsWith(words[0] ?? "") ? 0 : 1, hit: { matched_in: "title", page: summary, title_matches: matchRanges(page.title, words) } });
    } else if (holdsAll(page.body, words)) {
      ranked.push({ rank: 2, hit: { matched_in: "text", page: summary, ...snippetOf(page.body, words) } });
    }
  }
  return ranked.sort((a, b) => a.rank - b.rank || a.hit.page.title.localeCompare(b.hit.page.title)).map((entry) => entry.hit);
}

/** A slug from a title: lower case, words joined by "-". */
export function slugOf(title: string, taken: readonly string[]): string {
  const base = title.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "") || "page";
  let slug = base;
  for (let n = 2; taken.includes(slug); n++) {
    slug = `${base}-${String(n)}`;
  }
  return slug;
}

/** The day after `day`, for "until the end of today". */
export const nextDay = (day: string) => addDays(day, 1);
