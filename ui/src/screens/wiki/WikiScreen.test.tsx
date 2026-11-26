// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorView } from "@codemirror/view";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { forgetDraftsInMemory } from "../../data/drafts";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type WikiPage, type WikiTree } from "../../generated/ops";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();
Element.prototype.scrollIntoView = () => undefined;

afterEach(() => {
  cleanup();
  forgetDraftsInMemory();
  localStorage.clear();
});

/** Fluent wraps placeables in invisible isolation marks. */
const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

async function renderWiki(path = "/projects/TM/wiki", prepare?: (transport: ReturnType<typeof fakeTransport>) => Promise<unknown>) {
  const transport = fakeTransport({ sampleWiki: true });
  await prepare?.(transport);
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={path} />);
  const tree = await screen.findByRole("treegrid", { name: "Pages" });
  return { transport, tree };
}

const page = (title: string) => screen.findByRole("region", { name: (name) => plain(name) === `Wiki page ${title}` });

async function openPage(tree: HTMLElement, title: string) {
  fireEvent.click(await within(tree).findByText(title));
  return page(title);
}

const stored = async (transport: ReturnType<typeof fakeTransport>, title: string) => (await transport.call("wiki.get", { project: "TM", page: title })) as WikiPage;
const titles = async (transport: ReturnType<typeof fakeTransport>) => ((await transport.call("wiki.list", { project: "TM" })) as WikiTree).items.map((item) => item.title);

function typeInto(textbox: HTMLElement, text: string) {
  const view = EditorView.findFromDOM(textbox);
  if (!view) {
    throw new Error("not an editor");
  }
  act(() => {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, userEvent: "input.type" });
  });
}

describe("the wiki screen", () => {
  it("shows the tree and the first page, and the view switcher marks the wiki", async () => {
    const { tree } = await renderWiki();
    expect(within(tree).getAllByRole("row").map((row) => row.getAttribute("aria-label") ?? row.textContent)).toEqual(["Home", "Architecture", "Query language", "Storage", "Release plan"]);
    const home = await page("Home");
    expect(await within(home).findByRole("link", { name: "Architecture" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Wiki" }).getAttribute("aria-current")).toBe("page");
    expect(await screen.findByText((_content, element) => element?.tagName === "SPAN" && plain(element.textContent) === "5 pages")).toBeDefined();
  });

  it("marks a link to a page that doesn't exist, in the text and in the aside", async () => {
    const { tree } = await renderWiki();
    const view = await openPage(tree, "Release plan");
    const missing = await waitFor(() => {
      const link = view.querySelector<HTMLAnchorElement>('a[data-wiki="Roadmap ideas"]');
      expect(link?.dataset.missing).toBe("");
      return link;
    });
    expect(plain(missing?.getAttribute("aria-description"))).toBe("There is no page “Roadmap ideas” yet.");
    const links = within(view).getByRole("region", { name: "Links on this page" });
    expect(within(links).getByText("Missing")).toBeDefined();
    // Following it offers to make the page.
    fireEvent.click(missing as HTMLAnchorElement);
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole<HTMLInputElement>("textbox", { name: "Title" }).value).toBe("Roadmap ideas");
  });

  it("lists the outline, the pages and tasks that link here, and follows a link to a heading", async () => {
    const { tree } = await renderWiki();
    const view = await openPage(tree, "Query language");
    const outline = within(view).getByRole("navigation", { name: "On this page" });
    const headings = within(outline).getAllByRole("button").filter((button) => button.dataset.level !== undefined);
    expect(headings.map((button) => button.textContent)).toEqual(["Query language", "Grammar", "Examples", "Progress"]);
    // Each heading has a button that copies a link to it.
    expect(within(outline).getAllByRole("button", { name: /^Copy a link to/ })).toHaveLength(4);
    const linked = within(view).getByRole("region", { name: "Linked from" });
    expect(await within(linked).findByRole("button", { name: "Architecture" })).toBeDefined();
    const tasks = within(view).getByRole("region", { name: "Tasks that link here" });
    expect(await within(tasks).findByText("TM-3")).toBeDefined();
    // A task opens beside the page.
    fireEvent.click(within(tasks).getByText("TM-3"));
    expect(await screen.findByRole("complementary", { name: (name) => plain(name).startsWith("Task TM-3") })).toBeDefined();
    // A link with a heading opens that page.
    fireEvent.click(await within(view).findByRole("link", { name: "Release plan#Dates" }));
    expect(await page("Release plan")).toBeDefined();
  });

  it("edits a page with the formatting toolbar, saves a revision, and restores the one before from the history", async () => {
    const { transport, tree } = await renderWiki();
    const view = await openPage(tree, "Storage");
    fireEvent.click(within(view).getByRole("button", { name: /^Edit/ }));
    const field = await within(view).findByRole("textbox", { name: (name) => plain(name) === "Text of Storage" });
    expect(within(view).getByRole("toolbar", { name: "Formatting" })).toBeDefined();
    expect(within(view).getByRole("button", { name: "Link wiki page" })).toBeDefined();
    typeInto(field, "# Storage\n\nOne SQLite file per workspace, in WAL mode.\n");
    fireEvent.click(within(view).getByRole("button", { name: "Save" }));
    await waitFor(async () => {
      expect((await stored(transport, "Storage")).body).toContain("WAL mode");
    });

    fireEvent.click(await within(view).findByRole("button", { name: "History" }));
    const revisions = await within(view).findByRole("list", { name: "Revisions" });
    await waitFor(() => {
      expect(within(revisions).getAllByRole("checkbox")).toHaveLength(2);
    });
    // The latest change: the old line removed, the new one added.
    const diff = await within(view).findByRole("group", { name: "Differences between the two revisions" });
    expect(within(diff).getByText("One SQLite file per workspace.").tagName).toBe("DEL");
    // Choose the older revision alone: restore it.
    fireEvent.click(within(revisions).getAllByRole("checkbox")[1] as HTMLElement);
    fireEvent.click(await within(view).findByRole("button", { name: "Restore this revision" }));
    await waitFor(async () => {
      expect((await stored(transport, "Storage")).body).toBe("# Storage\n\nOne SQLite file per workspace.\n");
    });
  });

  it("moves a page with Alt and the arrow keys", async () => {
    const { transport, tree } = await renderWiki();
    const row = within(tree).getByRole("row", { name: "Storage" });
    act(() => {
      row.focus();
    });
    fireEvent.keyDown(row, { key: "ArrowUp", altKey: true });
    await waitFor(async () => {
      expect(await titles(transport)).toEqual(["Home", "Architecture", "Storage", "Query language", "Release plan"]);
    });
    fireEvent.keyDown(within(tree).getByRole("row", { name: "Storage" }), { key: "ArrowLeft", altKey: true });
    await waitFor(async () => {
      expect(await titles(transport)).toEqual(["Home", "Architecture", "Query language", "Storage", "Release plan"]);
      expect((await stored(transport, "Storage")).parent).toBeUndefined();
    });
  });

  it("makes a page inside another and opens it in the editor", async () => {
    const { transport, tree } = await renderWiki();
    const view = await openPage(tree, "Architecture");
    fireEvent.click(within(view).getByRole("button", { name: "Page menu" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "New page inside" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Title" }), { target: { value: "Search" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create" }));
    const created = await page("Search");
    expect(await within(created).findByRole("textbox", { name: (name) => plain(name) === "Text of Search" })).toBeDefined();
    expect((await stored(transport, "Search")).parent?.title).toBe("Architecture");
  });

  it("reads but doesn't change pages while the wiki is switched off", async () => {
    await renderWiki("/projects/TM/wiki", (transport) => transport.call("project.update", { project: "TM", modules: { wiki: false } }));
    const home = await page("Home");
    expect(await within(home).findByText(/The wiki is switched off in this project/)).toBeDefined();
    expect(within(home).getByRole<HTMLButtonElement>("button", { name: /^Edit/ }).disabled).toBe(true);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "New page" }).disabled).toBe(true);
  });
});

describe("what links to a task", () => {
  it("lists the wiki pages that name the task in the task panel, and opens one in the wiki", async () => {
    const transport = fakeTransport({ sampleWiki: true });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/projects/TM?task=TM-1" />);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith("Task TM-1") });
    const section = await within(panel).findByRole("region", { name: "Linked from" });
    const link = await within(section).findByRole("button", { name: (name) => plain(name) === "Open the wiki page Release plan" });
    fireEvent.click(link);
    expect(await page("Release plan")).toBeDefined();
  });

  it("opens a page a description links to, read only first and then in the wiki", async () => {
    const transport = fakeTransport({ sampleWiki: true });
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path="/projects/TM?task=TM-3" />);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith("Task TM-3") });
    const description = await within(panel).findByRole("region", { name: "Description" });
    fireEvent.click(await within(description).findByRole("link", { name: "Query language#Grammar" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(await within(dialog).findByRole("button", { name: "Open in the wiki" }));
    expect(await page("Query language")).toBeDefined();
  });
});

describe("a wiki window", () => {
  it("shows one page without the tree, and follows a link to another page in the same window", async () => {
    const transport = fakeTransport({ sampleWiki: true });
    const home = (await transport.call("wiki.get", { project: "TM", page: "Home" })) as WikiPage;
    render(<App start={{ titleBar: "system", locale: "en-US", wiki: home.id }} client={createClient(transport)} path={`/wiki/${home.id}`} />);
    const view = await page("Home");
    expect(screen.queryByRole("treegrid", { name: "Pages" })).toBeNull();
    expect(within(view).queryByRole("button", { name: "Open in its own window" })).toBeNull();
    fireEvent.click(await within(view).findByRole("link", { name: "Release plan" }));
    expect(await page("Release plan")).toBeDefined();
  });
});
