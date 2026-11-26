// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import { type ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { type PageSuggestion } from "../editor/types";
import { type BacklinkList, type ReferenceList, type TrashChange, type WikiChange, type WikiPageSummary, type WikiTree } from "../generated/ops";
import { createQueryClient, DataProvider } from "./DataProvider";
import { fakeTransport } from "./fake/transport";
import { linksIn } from "./fake/wiki";
import { useLinkCompletion } from "./linking";
import { createClient } from "./transport";
import { buildTree, keyboardMove, moveFor, referenceIndex, splitBacklinks } from "./wiki";

const page = (id: string, parent?: string): WikiPageSummary => ({ id, title: id, slug: id.toLowerCase(), project: "TM", depth: 0, children: 0, updated_at: "2026-11-20T10:00:00Z", ...(parent === undefined ? {} : { parent }) });

// Home, Guide (with Install and Use inside; Install holds Linux), Notes.
const PAGES = [page("Home"), page("Guide"), page("Install", "Guide"), page("Linux", "Install"), page("Use", "Guide"), page("Notes")];

describe("the page tree", () => {
  it("nests the pages of the list in their order", () => {
    const tree = buildTree(PAGES);
    expect(tree.map((node) => node.page.id)).toEqual(["Home", "Guide", "Notes"]);
    expect(tree[1]?.children.map((node) => node.page.id)).toEqual(["Install", "Use"]);
    expect(tree[1]?.children[0]?.children.map((node) => node.page.id)).toEqual(["Linux"]);
  });

  it("turns a drop into a move: beside a page of the same parent, of another parent, at the top, or inside", () => {
    expect(moveFor(PAGES, "Notes", "Home", "before")).toEqual({ page: "Notes", before: "Home" });
    expect(moveFor(PAGES, "Notes", "Install", "after")).toEqual({ page: "Notes", parent: "Guide", after: "Install" });
    expect(moveFor(PAGES, "Use", "Home", "after")).toEqual({ page: "Use", top_level: true, after: "Home" });
    expect(moveFor(PAGES, "Home", "Linux", "on")).toEqual({ page: "Home", parent: "Linux", at: "bottom" });
  });

  it("refuses a drop on the page itself or a page inside it", () => {
    expect(moveFor(PAGES, "Guide", "Guide", "on")).toBeNull();
    expect(moveFor(PAGES, "Guide", "Linux", "before")).toBeNull();
    expect(moveFor(PAGES, "Guide", "Missing", "on")).toBeNull();
  });

  it("moves with the keyboard: up and down among siblings, into the page above, out of the parent", () => {
    expect(keyboardMove(PAGES, "Guide", "up")).toEqual({ page: "Guide", before: "Home" });
    expect(keyboardMove(PAGES, "Home", "up")).toBeNull();
    expect(keyboardMove(PAGES, "Install", "down")).toEqual({ page: "Install", after: "Use" });
    expect(keyboardMove(PAGES, "Use", "down")).toBeNull();
    expect(keyboardMove(PAGES, "Use", "in")).toEqual({ page: "Use", parent: "Install", at: "bottom" });
    expect(keyboardMove(PAGES, "Home", "in")).toBeNull();
    expect(keyboardMove(PAGES, "Linux", "out")).toEqual({ page: "Linux", parent: "Guide", after: "Install" });
    expect(keyboardMove(PAGES, "Install", "out")).toEqual({ page: "Install", top_level: true, after: "Guide" });
    expect(keyboardMove(PAGES, "Home", "out")).toBeNull();
  });

  it("splits what links here into pages and tasks, each task once", () => {
    const { pages, tasks } = splitBacklinks([
      { project: "TM", title: "Home", page: { id: "w1", slug: "home", title: "Home" } },
      { project: "TM", title: "Fix it", task: "TM-3" },
      { project: "TM", title: "Fix it", task: "TM-3", comment: "c1" },
      { project: "TM", title: "Other", task: "TM-4", comment: "c2" },
    ]);
    expect(pages.map((link) => link.title)).toEqual(["Home"]);
    expect(tasks.map((link) => link.task)).toEqual(["TM-3", "TM-4"]);
  });

  it("finds a link's target by its name as written, ignoring case", () => {
    const index = referenceIndex([
      { kind: "page", target: "Release Plan", found: true },
      { kind: "page", target: "Roadmap ideas", found: false },
      { kind: "task", target: "TM-1", found: true },
    ]);
    expect(index.get("release plan")?.found).toBe(true);
    expect(index.get("roadmap ideas")?.found).toBe(false);
    expect(index.has("tm-1")).toBe(false);
  });
});

describe("links in the sample engine", () => {
  it("finds page links, headings and task keys in order, but not in code", () => {
    expect(linksIn("See [[Guide#Install|the guide]], TM-4 and `TM-9`.\n\n```\n[[Not a link]] TM-5\n```\n[[#Here]] [[Notes]]")).toEqual([
      { kind: "page", target: "Guide", heading: "Install" },
      { kind: "task", target: "TM-4" },
      { kind: "page", target: "Notes", heading: undefined },
    ]);
  });
});

function workspace() {
  const transport = fakeTransport({ sampleWiki: true });
  const call = <T,>(op: string, input: unknown) => transport.call(op, input) as Promise<T>;
  return { transport, call };
}

describe("the sample engine's wiki", () => {
  it("moves pages with the pages inside them, and never into themselves", async () => {
    const { call } = workspace();
    const titles = async () => (await call<WikiTree>("wiki.list", { project: "TM" })).items.map((item) => `${"  ".repeat(item.depth)}${item.title}`);
    expect(await titles()).toEqual(["Home", "Architecture", "  Query language", "  Storage", "Release plan"]);
    await call<WikiChange>("wiki.move", { project: "TM", page: "Architecture", parent: "Release plan" });
    expect(await titles()).toEqual(["Home", "Release plan", "  Architecture", "    Query language", "    Storage"]);
    await call<WikiChange>("wiki.move", { project: "TM", page: "Storage", before: "Query language" });
    expect(await titles()).toEqual(["Home", "Release plan", "  Architecture", "    Storage", "    Query language"]);
    await call<WikiChange>("wiki.move", { project: "TM", page: "Architecture", top_level: true, at: "top" });
    expect(await titles()).toEqual(["Architecture", "  Storage", "  Query language", "Home", "Release plan"]);
    await expect(call("wiki.move", { project: "TM", page: "Architecture", parent: "Storage" })).rejects.toMatchObject({ details: { message_id: "wiki-parent-loop" } });
  });

  it("lists what links to a page and what a page links to, broken links marked", async () => {
    const { call } = workspace();
    const backlinks = await call<BacklinkList>("wiki.backlinks", { project: "TM", page: "Query language" });
    expect(backlinks.items.map((item) => item.title)).toEqual(["Architecture", expect.any(String) as unknown]);
    expect(backlinks.items[1]?.task).toBe("TM-3");
    expect(backlinks.items[1]?.heading).toBe("Grammar");
    const references = await call<ReferenceList>("wiki.references", { project: "TM", page: "Release plan" });
    expect(references.items.map((item) => [item.target, item.found])).toEqual([
      ["TM-1", true],
      ["Roadmap ideas", false],
    ]);
  });

  it("keeps a page's revisions, finds it by its old title, and brings it back from the trash with the pages inside", async () => {
    const { call } = workspace();
    const before = await call<{ items: { id: string }[] }>("wiki.history", { project: "TM", page: "Query language" });
    expect(before.items).toHaveLength(2);
    await call<WikiChange>("wiki.update", { project: "TM", page: "Architecture", title: "Design" });
    expect((await call<{ title: string }>("wiki.get", { project: "TM", page: "architecture" })).title).toBe("Design");
    const trashed = await call<TrashChange>("wiki.delete", { project: "TM", page: "Design" });
    expect(trashed.total).toBe(3);
    expect((await call<WikiTree>("wiki.list", { project: "TM" })).total).toBe(2);
    await call<TrashChange>("trash.restore", { items: [trashed.items[0]?.id] });
    expect((await call<WikiTree>("wiki.list", { project: "TM" })).total).toBe(5);
  });

  it("refuses changes while the wiki is switched off, and still reads", async () => {
    const { call } = workspace();
    await call("project.update", { project: "TM", modules: { wiki: false } });
    await expect(call("wiki.create", { project: "TM", title: "New" })).rejects.toMatchObject({ code: "forbidden", details: { message_id: "project-wiki-off" } });
    await expect(call("wiki.update", { project: "TM", page: "Home", body: "x" })).rejects.toMatchObject({ code: "forbidden" });
    expect((await call<WikiTree>("wiki.list", { project: "TM" })).total).toBe(5);
  });
});

describe("[[ completion", () => {
  it("offers pages by title and a page's headings after #, from the wiki search", async () => {
    const transport = fakeTransport({ sampleWiki: true });
    const queryClient = createQueryClient();
    const Wrapper = ({ children }: { children: ReactNode }) => (
      <DataProvider client={createClient(transport)} queryClient={queryClient}>
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      </DataProvider>
    );
    const { result } = renderHook(() => useLinkCompletion("TM"), { wrapper: Wrapper });
    const pages = result.current.pages;
    if (!pages) {
      throw new Error("no page completion");
    }
    const target = (suggestion: string | PageSuggestion) => (typeof suggestion === "string" ? suggestion : suggestion.target);
    // Titles first, then pages that mention the words.
    expect((await pages("query")).map(target)).toEqual(["Query language", "Architecture"]);
    expect((await pages("Query language#gram")).map(target)).toEqual(["Query language#Grammar"]);
    expect((await pages("Query language#")).map(target)).toEqual(["Query language#Query language", "Query language#Grammar", "Query language#Examples", "Query language#Progress"]);
  });
});
