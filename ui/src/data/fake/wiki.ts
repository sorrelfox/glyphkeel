// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's links between texts: `[[Page]]`, `[[Page#Heading]]`,
// `[[Page|label]]` and task keys, found the way the engine finds them
// closely enough for the screens (code is skipped), and the order of pages
// among their siblings.

import { nextId, nextUuid } from "./seed";
import { type FakeState, type FakeWikiPage } from "./state";

/** One link in a text, as written. */
export interface TextLink {
  kind: "page" | "task";
  /** The page's name or the task's key. */
  target: string;
  heading?: string | undefined;
}

const TASK_KEY = /\b[A-Z][A-Z0-9]{1,9}-\d{1,12}\b/gu;
const PAGE_LINK = /\[\[([^\]\r\n]+)\]\]/gu;

/** The text without fenced code, indented code or inline code, keeping its length. */
function withoutCode(text: string): string {
  const blank = (match: string) => " ".repeat(match.length);
  let fence: string | null = null;
  return text
    .split("\n")
    .map((line) => {
      const marker = /^\s*(```|~~~)/u.exec(line)?.[1];
      if (fence !== null) {
        if (marker === fence) {
          fence = null;
        }
        return blank(line);
      }
      if (marker !== undefined) {
        fence = marker;
        return blank(line);
      }
      return line.replace(/`[^`]*`/gu, blank);
    })
    .join("\n");
}

/** The links of a text, in the order they appear. */
export function linksIn(text: string): TextLink[] {
  const plain = withoutCode(text);
  const found: { at: number; link: TextLink }[] = [];
  const covered: [number, number][] = [];
  for (const match of plain.matchAll(PAGE_LINK)) {
    const inner = (match[1] ?? "").split("|")[0] ?? "";
    const hash = inner.indexOf("#");
    const target = (hash < 0 ? inner : inner.slice(0, hash)).trim();
    const heading = hash < 0 ? undefined : inner.slice(hash + 1).trim() || undefined;
    covered.push([match.index, match.index + match[0].length]);
    // `[[#Heading]]` points inside the same text.
    if (target !== "") {
      found.push({ at: match.index, link: { kind: "page", target, heading } });
    }
  }
  for (const match of plain.matchAll(TASK_KEY)) {
    if (!covered.some(([from, to]) => match.index >= from && match.index < to)) {
      found.push({ at: match.index, link: { kind: "task", target: match[0] } });
    }
  }
  return found.sort((a, b) => a.at - b.at).map((entry) => entry.link);
}

/** A page and every page inside it, the page first. */
export function subtree(pages: readonly FakeWikiPage[], root: FakeWikiPage): FakeWikiPage[] {
  const out = [root];
  for (let index = 0; index < out.length; index += 1) {
    const parent = out[index];
    out.push(...pages.filter((page) => page.parentId === parent?.id));
  }
  return out;
}

/**
 * Puts `page` among the pages of `parentId`: before or after one of them,
 * first or last. The array's order is the order of siblings.
 */
export function placePage(pages: FakeWikiPage[], page: FakeWikiPage, parentId: string | null, place: { before?: string | undefined; after?: string | undefined; at?: "top" | "bottom" | undefined }): void {
  const from = pages.indexOf(page);
  if (from >= 0) {
    pages.splice(from, 1);
  }
  page.parentId = parentId;
  const siblings = pages.filter((other) => other.parentId === parentId);
  let index = pages.length;
  if (place.before !== undefined) {
    index = pages.findIndex((other) => other.id === place.before);
  } else if (place.after !== undefined) {
    index = pages.findIndex((other) => other.id === place.after) + 1;
  } else if (place.at === "top" && siblings[0]) {
    index = pages.indexOf(siblings[0]);
  }
  pages.splice(index < 0 ? pages.length : index, 0, page);
}

const QUERY_LANGUAGE = `# Query language

Every filter has a text form. The same text works in the filter bar of the list and the board, in the search box, and in the command line. The parser landed in TM-3.

> [!NOTE]
> The condition chips and the text form always stay in sync.

## Grammar

| You type | Meaning |
| --- | --- |
| \`field:value\` | The field equals the value. |
| \`label:bug,crash\` | A comma inside a value means OR. |
| \`-status:Done\` | A leading minus means NOT. |

## Examples

\`\`\`
status:open due<+7d
label:bug,crash -status:Done
\`\`\`

## Progress

- [x] One parser shared by the app and the command line
- [x] Relative dates
- [ ] Formal grammar written down

See also [[Release plan#Dates]] and [[Saved views]].
`;

/**
 * Sample pages for the wiki of Task Manager: a small tree with headings,
 * links between pages, a link to a page that doesn't exist yet, and task
 * keys; one task's description links back to a page, and another task's
 * description and a comment name that task.
 */
export function addSampleWiki(state: FakeState, today: Date): void {
  const project = state.projects.find((candidate) => candidate.key === "TM");
  if (!project) {
    return;
  }
  const at = (daysAgo: number) => new Date(today.getTime() - daysAgo * 86_400_000).toISOString();
  const pages: FakeWikiPage[] = [];
  const add = (title: string, slug: string, parent: FakeWikiPage | null, versions: readonly string[], daysAgo: number) => {
    const page: FakeWikiPage = { id: nextId("w"), projectId: project.id, title, slug, parentId: parent?.id ?? null, body: versions.at(-1) ?? "", createdAt: at(daysAgo + versions.length), updatedAt: at(daysAgo) };
    let base: string | null = null;
    versions.forEach((body, index) => {
      const id = nextId("r");
      state.revisions.push({ id, taskId: page.id, body, kind: "normal", base, author: { kind: "user", name: "You" }, createdAt: at(daysAgo + versions.length - 1 - index), resolvedAt: null });
      base = id;
    });
    pages.push(page);
    return page;
  };
  add("Home", "home", null, ["# Home\n\nStart with [[Architecture]] and the [[Release plan]].\n"], 9);
  const architecture = add("Architecture", "architecture", null, ["# Architecture\n\nOne engine, many front ends. The query language is described in [[Query language]].\n"], 7);
  add("Query language", "query-language", architecture, [QUERY_LANGUAGE.replace("\n## Progress", "\n## Draft notes\n\nTo be written.\n\n## Progress"), QUERY_LANGUAGE], 0);
  add("Storage", "storage", architecture, ["# Storage\n\nOne SQLite file per workspace.\n"], 12);
  add("Release plan", "release-plan", null, ["# Release plan\n\n## Dates\n\nThe first public release follows the second milestone.\n\nTracked in TM-1 and [[Roadmap ideas]].\n"], 3);
  state.wiki = [...(state.wiki ?? []), ...pages];
  const task = state.tasks.find((candidate) => candidate.projectId === project.id && candidate.number === 3);
  if (task) {
    task.description = `${task.description}${task.description === "" ? "" : "\n\n"}Written up in [[Query language#Grammar]].`;
    // Another task's description and a comment name it, so its "Linked from" lists all three kinds.
    const others = state.tasks.filter((candidate) => candidate.projectId === project.id && candidate.id !== task.id);
    const described = others.find((candidate) => candidate.number === 4);
    if (described) {
      described.description = `${described.description}${described.description === "" ? "" : "\n\n"}Needs the parser from TM-3.`;
    }
    const commented = others.find((candidate) => candidate.number === 5);
    if (commented) {
      const createdAt = at(1);
      state.comments.push({ id: nextUuid(), taskId: commented.id, body: "The dates here follow TM-3.", author: { kind: "user", name: "You" }, via: "app", createdAt, editedAt: null, version: 1, revisions: [], replyTo: null, deletedAt: null });
    }
  }
}
