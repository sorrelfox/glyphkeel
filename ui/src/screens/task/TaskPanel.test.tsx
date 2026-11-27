// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorView } from "@codemirror/view";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { flushDrafts, forgetDraftsInMemory } from "../../data/drafts";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type CommentChange, type CommentList, type DraftList, type TaskChange } from "../../generated/ops";
import { describeEntry } from "./Activity";

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
const named = (pattern: RegExp) => (name: string) => pattern.test(plain(name));

const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/** The sample workspace with two fresh tasks in Personal; the panel shows the first. */
async function renderPanel() {
  const transport = fakeTransport();
  const create = (title: string) => transport.call("task.create", { project: "PER", title }) as Promise<TaskChange>;
  const task = (await create("Plan the trip")).task;
  const other = (await create("Book the train")).task;
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
  const panel = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
  await within(panel).findByRole("heading", { name: "Plan the trip" });
  return { transport, task, other, panel };
}

const section = (panel: HTMLElement, name: string) => within(panel).getByRole("region", { name });

/** Replaces the text of a Markdown editor, as typing would. */
function typeInto(textbox: HTMLElement, text: string) {
  const view = EditorView.findFromDOM(textbox);
  if (!view) {
    throw new Error("not an editor");
  }
  act(() => {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, userEvent: "input.type" });
  });
}

/** The text in a Markdown editor. */
const editorText = (textbox: HTMLElement) => EditorView.findFromDOM(textbox)?.state.doc.toString();

describe("labels from the task panel", () => {
  it("creates a label by the typed name, in the suggested colour, and puts it on the task", async () => {
    const { panel, task, transport } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Change labels" }));
    const search = await screen.findByRole("searchbox", { name: "Find or create a label" });
    fireEvent.change(search, { target: { value: "garden" } });
    const create = await screen.findByRole("menuitemcheckbox", { name: named(/^Create label “garden”$/) });
    fireEvent.click(create);
    const chip = (await within(panel).findByText("garden", { selector: ".chip > span" })).parentElement as HTMLElement;
    // Personal's labels are green, orange and yellow: red is the first one free.
    expect(chip.className).toContain("hue-red");
    const stored = (await transport.call("task.get", { keys: [task.key] })) as { items: { labels: { name: string }[] }[] };
    expect(stored.items[0]?.labels.map((label) => label.name)).toEqual(["garden"]);
  });

  it("filters the labels as one types and offers no copy of one that exists", async () => {
    const { panel } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Change labels" }));
    const search = await screen.findByRole("searchbox", { name: "Find or create a label" });
    fireEvent.change(search, { target: { value: "HOME" } });
    await waitFor(() => {
      expect(screen.getAllByRole("menuitemcheckbox").map((item) => plain(item.textContent))).toEqual(["home"]);
    });
  });

  it("creates the first label of a project from a small form", async () => {
    const transport = fakeTransport();
    await transport.call("project.create", { name: "Empty", key: "EMP" });
    const task = ((await transport.call("task.create", { project: "EMP", title: "First" })) as TaskChange).task;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/EMP?task=${task.key}`} />);
    const panel = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
    const plus = await within(panel).findByRole("button", { name: "Change labels" });
    expect((plus as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(plus);
    const form = await screen.findByRole("dialog", { name: "New label" });
    const name = within(form).getByRole("textbox", { name: "Name" });
    fireEvent.change(name, { target: { value: "urgent" } });
    fireEvent.submit(name);
    const chip = (await within(panel).findByText("urgent", { selector: ".chip > span" })).parentElement as HTMLElement;
    expect(chip.className).toContain("hue-red");
  });
});

describe("the task panel's sections", () => {
  it("adds a subtask, closes it from its checkbox and counts it", async () => {
    const { panel } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Add subtask" }));
    const field = await within(panel).findByRole("textbox", { name: "New subtask" });
    fireEvent.change(field, { target: { value: "Pack the bags" } });
    fireEvent.keyDown(field, { key: "Enter" });
    const subtasks = section(panel, "Subtasks");
    const box = await within(subtasks).findByRole("checkbox", { name: named(/^Close PER-\d+$/) });
    fireEvent.click(box);
    await within(subtasks).findByRole("checkbox", { name: named(/^Reopen PER-\d+$/) });
    expect(within(subtasks).getByRole("img", { name: named(/^1 of 1 subtasks done$/) })).toBeDefined();
  });

  it("moves a subtask past one of another status, with Alt+Down and from its menu", async () => {
    const { panel, task, transport } = await renderPanel();
    const make = async (title: string) => ((await transport.call("task.create", { project: "PER", title, parent: task.key })) as TaskChange).task;
    const first = await make("Pack the bags");
    const second = await make("Water the plants");
    await transport.call("task.move", { key: first.key, status: "Done" });
    const subtasks = section(panel, "Subtasks");
    const order = () => [...subtasks.querySelectorAll<HTMLElement>("[data-subtask]")].map((row) => row.dataset.subtask);
    await waitFor(() => {
      expect(order()).toEqual([first.key, second.key]);
    });
    const row = subtasks.querySelector<HTMLElement>(`[data-subtask="${first.key}"]`);
    fireEvent.keyDown(within(row as HTMLElement).getByRole("button", { name: named(new RegExp(`^${first.key} `)) }), { key: "ArrowDown", altKey: true });
    await waitFor(() => {
      expect(order()).toEqual([second.key, first.key]);
    });
    expect(((await transport.call("task.get", { keys: [task.key], include: ["subtasks"] })) as { items: { subtasks: { key: string }[] }[] }).items[0]?.subtasks.map((sub) => sub.key)).toEqual([second.key, first.key]);
    fireEvent.click(within(subtasks).getByRole("button", { name: named(new RegExp(`^Subtask ${first.key}$`)) }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Move up" }));
    await waitFor(() => {
      expect(order()).toEqual([first.key, second.key]);
    });
  });

  it("links another task, shows Blocked on the blocked one, and removes the link", async () => {
    const { panel, other, transport } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Add link" }));
    fireEvent.click(within(panel).getByRole("button", { name: /Kind of link/ }));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: "Blocked by" }));
    const field = within(panel).getByRole("textbox", { name: "Task key" });
    fireEvent.change(field, { target: { value: other.key.toLowerCase() } });
    fireEvent.keyDown(field, { key: "Enter" });
    const links = section(panel, "Links");
    await within(links).findByRole("button", { name: named(new RegExp(`^Blocked by ${other.key} Book the train`)) });
    expect(await within(panel).findByRole("note", { name: named(new RegExp(`Blocked by ${other.key}`)) })).toBeDefined();
    fireEvent.click(within(links).getByRole("button", { name: named(/^Remove link: Blocked by/) }));
    await waitFor(() => {
      expect(within(panel).queryByRole("note", { name: named(/Blocked/) })).toBeNull();
    });
    expect(await transport.call("task.get", { keys: [other.key], include: ["links"] })).toMatchObject({ items: [{ links: [] }] });
  });

  it("attaches files from the file field and shows an image from its content", async () => {
    const { panel } = await renderPanel();
    const files = section(panel, "Files");
    const input = files.querySelector<HTMLInputElement>("input[type=file]");
    if (!input) {
      throw new Error("no file field");
    }
    const png = new File([Uint8Array.from(atob(PNG), (char) => char.charCodeAt(0))], "dot.png", { type: "image/png" });
    const text = new File(["hello"], "notes.txt", { type: "text/plain" });
    fireEvent.change(input, { target: { files: [png, text] } });
    await within(files).findByRole("button", { name: named(/^notes\.txt /) });
    const image = await within(files).findByRole("img", { name: "dot.png" });
    await waitFor(() => {
      expect(image.getAttribute("src")).toMatch(/^data:image\/png;base64,/);
    });
    // The engine's thumbnail gives its size.
    expect(image.dataset.thumbnail).toBe("1x1");
    fireEvent.click(within(files).getByRole("button", { name: named(/^File notes\.txt$/) }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    await waitFor(() => {
      expect(within(files).queryByRole("button", { name: named(/^notes\.txt /) })).toBeNull();
    });
  });
});

describe("comments and history", () => {
  it("adds a comment with Ctrl+Enter, edits it, shows the earlier version and moves it to the trash", async () => {
    const { panel } = await renderPanel();
    const composer = await within(panel).findByRole("textbox", { name: "Write a comment" });
    typeInto(composer, "Trains are **cheaper** in May.");
    fireEvent.keyDown(composer, { key: "Enter", ctrlKey: true });
    const comment = await within(panel).findByRole("article", { name: named(/^Comment by Me/) });
    // The comment shows as Markdown, rendered.
    expect((await within(comment).findByText("cheaper")).tagName).toBe("STRONG");
    await waitFor(() => {
      expect(editorText(composer)).toBe("");
    });

    fireEvent.click(within(comment).getByRole("button", { name: "Comment" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const edit = await within(comment).findByRole("textbox", { name: "Comment text" });
    typeInto(edit, "Trains are cheaper in June.");
    fireEvent.keyDown(edit, { key: "Enter", ctrlKey: true });
    const edited = await within(comment).findByRole("button", { name: named(/edited · 1 earlier version/) });
    fireEvent.click(edited);
    const earlier = within(comment).getByRole("list", { name: "Earlier versions" });
    expect(await within(earlier).findByText("cheaper")).toBeDefined();

    fireEvent.click(within(comment).getByRole("button", { name: "Comment" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Move to the trash" }));
    await within(panel).findByText("No comments yet.");
    expect(await screen.findByText("Comment moved to the trash")).toBeDefined();
  });

  it("quotes a comment in the reply, which answers it and links back to it", async () => {
    const { panel, task, transport } = await renderPanel();
    const first = ((await transport.call("comment.add", { key: task.key, body: "Trains are **cheaper** in May.\n\nBook early." })) as CommentChange).comment;
    const article = await within(panel).findByRole("article", { name: named(/^Comment by Me/) });
    await within(article).findByText("cheaper");
    fireEvent.click(within(article).getByRole("button", { name: "Quote in reply" }));
    const composer = await within(panel).findByRole("textbox", { name: "Write a comment" });
    const link = `${task.key}#c-${first.id.replace(/-/g, "").slice(-8)}`;
    await waitFor(() => {
      expect(editorText(composer)).toBe(`> Trains are **cheaper** in May.\n>\n> Book early.\n>\n> — ${link}\n\n`);
    });
    typeInto(composer, `${editorText(composer) ?? ""}Let's book now.`);
    fireEvent.keyDown(composer, { key: "Enter", ctrlKey: true });
    await waitFor(async () => {
      const listed = (await transport.call("comment.list", { key: task.key })) as CommentList;
      expect(listed.items.map((comment) => comment.reply_to ?? null)).toEqual([null, first.id]);
    });

    // The answer's link opens the quoted comment.
    const answer = await waitFor(() => {
      const found = within(panel).getAllByRole("article", { name: named(/^Comment by Me/) })[1];
      expect(found).toBeDefined();
      return found as HTMLElement;
    });
    const back = await within(answer).findByRole("link", { name: link });
    expect(back.getAttribute("data-comment")).toBe(link.slice(-8));
    fireEvent.click(back);
    await waitFor(() => {
      expect(within(panel).getAllByRole("article", { name: named(/^Comment by Me/) })[0]?.hasAttribute("data-shown")).toBe(true);
    });
  });

  it("keeps an unsent comment and an unsaved edit as drafts that come back after a restart", async () => {
    const { panel, task, transport } = await renderPanel();
    const posted = ((await transport.call("comment.add", { key: task.key, body: "First thoughts" })) as CommentChange).comment;
    const article = await within(panel).findByRole("article", { name: named(/^Comment by Me/) });
    fireEvent.click(await within(article).findByRole("button", { name: "Comment" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Edit" }));
    typeInto(await within(article).findByRole("textbox", { name: "Comment text" }), "Second thoughts");
    typeInto(await within(panel).findByRole("textbox", { name: "Write a comment" }), "Not sent yet");
    flushDrafts();
    await waitFor(async () => {
      const kept = (await transport.call("draft.list", { task: task.key })) as DraftList;
      expect(kept.items.map((draft) => [draft.kind, draft.body]).sort()).toEqual([
        ["comment", "Second thoughts"],
        ["new_comment", "Not sent yet"],
      ]);
    });

    // A restart: the window's memory is gone, the engine's drafts stay.
    cleanup();
    forgetDraftsInMemory();
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
    const again = await screen.findByRole("complementary", { name: named(new RegExp(`Task ${task.key}`)) });
    const composer = await within(again).findByRole("textbox", { name: "Write a comment" });
    await waitFor(() => {
      expect(editorText(composer)).toBe("Not sent yet");
    });
    const comment = await within(again).findByRole("article", { name: named(/^Comment by Me/) });
    expect(await within(comment).findByText("Unsaved changes to this comment are kept from earlier.")).toBeDefined();
    fireEvent.click(within(comment).getByRole("button", { name: "Continue editing" }));
    const edit = await within(comment).findByRole("textbox", { name: "Comment text" });
    expect(editorText(edit)).toBe("Second thoughts");
    fireEvent.keyDown(edit, { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(composer, { key: "Enter", ctrlKey: true });
    // Saved and sent: no drafts are left.
    await waitFor(async () => {
      const listed = (await transport.call("comment.list", { key: task.key })) as CommentList;
      expect(listed.items.map((item) => item.body)).toEqual(["Second thoughts", "Not sent yet"]);
      expect(listed.items[0]?.id).toBe(posted.id);
      const kept = (await transport.call("draft.list", { task: task.key })) as DraftList;
      expect(kept.items).toEqual([]);
    });
  });

  it("attaches a file pasted into the composer and links it in the text", async () => {
    const { panel } = await renderPanel();
    const composer = await within(panel).findByRole("textbox", { name: "Write a comment" });
    const png = new File([Uint8Array.from(atob(PNG), (char) => char.charCodeAt(0))], "shot [1].png", { type: "image/png" });
    fireEvent.paste(composer, { clipboardData: { files: [png], getData: () => "", types: ["Files"] } });
    await waitFor(() => {
      expect(editorText(composer)).toMatch(/^!\[shot \\\[1\\\]\.png\]\(gk-file:\/\/[^)]+\)$/);
    });
    await within(section(panel, "Files")).findByRole("button", { name: named(/^shot \[1\]\.png /) });
  });

  it("lists the task's changes and reverts the latest", async () => {
    const { panel } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: named(/^Priority:/) }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: /High/ }));
    await waitFor(() => {
      expect(plain(within(panel).getByRole("button", { name: named(/^Priority:/) }).getAttribute("aria-label"))).toBe("Priority: High");
    });
    fireEvent.click(within(panel).getByRole("tab", { name: /History/ }));
    const history = await within(panel).findByRole("list", { name: "History" });
    await within(history).findByText((text) => plain(text) === "Priority: No priority → High");
    expect(within(history).getByText((text) => /^Created PER-\d+$/.test(plain(text)))).toBeDefined();
    fireEvent.click(within(history).getAllByRole("button", { name: named(/^Revert the change of/) })[0] as HTMLElement);
    await screen.findByText("Change reverted");
    await waitFor(() => {
      expect(plain(within(panel).getByRole("button", { name: named(/^Priority:/) }).getAttribute("aria-label"))).toBe("Priority: No priority");
    });
  });

  it("filters the History tab by who made the change: everyone, people or agents", async () => {
    const { panel, task, transport } = await renderPanel();
    transport.workspace.call("task.update", { key: task.key, priority: "urgent" }, { actor: { kind: "agent", name: "Build agent" }, via: "mcp" });
    fireEvent.click(within(panel).getByRole("tab", { name: /History/ }));
    const history = await within(panel).findByRole("list", { name: "History" });
    await within(history).findByText((text) => plain(text) === "Priority: No priority → Urgent");
    expect(within(history).getByText((text) => /^Created PER-\d+$/.test(plain(text)))).toBeDefined();
    const who = within(panel).getByRole("radiogroup", { name: "Who" });

    const list = () => within(panel).getByRole("list", { name: "History" });
    fireEvent.click(within(who).getByRole("radio", { name: "Agents" }));
    await waitFor(() => {
      expect(within(list()).queryByText((text) => /^Created PER-\d+$/.test(plain(text)))).toBeNull();
    });
    expect(within(list()).getByText((text) => plain(text) === "Priority: No priority → Urgent")).toBeDefined();
    expect(within(list()).getByText((text) => plain(text).startsWith("Build agent · MCP"))).toBeDefined();

    fireEvent.click(within(who).getByRole("radio", { name: "People" }));
    const people = await within(panel).findByRole("list", { name: "History" });
    await within(people).findByText((text) => /^Created PER-\d+$/.test(plain(text)));
    expect(within(people).queryByText((text) => plain(text) === "Priority: No priority → Urgent")).toBeNull();

    fireEvent.click(within(who).getByRole("radio", { name: "Everyone" }));
    await within(panel).findByText((text) => plain(text) === "Priority: No priority → Urgent");
  });

  it("reads history entries as people say them", () => {
    const t = (id: string, vars?: Record<string, unknown>) => `${id}${vars ? JSON.stringify(vars) : ""}`;
    const project = { statuses: [{ id: "s-1", name: "To do" }, { id: "s-2", name: "Done" }], types: [], labels: [] };
    const names = { task: { id: "t-1", key: "TM-1" }, project, keys: new Map([["t-2", "TM-2"]]) } as unknown as Parameters<typeof describeEntry>[1];
    const at = { actor: { kind: "user", name: "You" }, at: "", group: "g", op: "x", seq: 1, via: "app" } as const;
    expect(describeEntry(t, names, { ...at, entity: "task", id: "t-1", fields: { status: { before: "s-1", after: "s-2" } } })).toEqual([
      'history-changed{"field":"history-field-status","before":"To do","after":"Done"}',
    ]);
    expect(describeEntry(t, names, { ...at, entity: "relation", id: "r", fields: { link: { after: { from: "t-2", to: "t-1", kind: "blocks" } } } })).toEqual([
      'history-link-added{"kind":"relation-blocked_by","key":"TM-2"}',
    ]);
    // A move among the parent's subtasks is told; the order set with a new parent isn't.
    expect(describeEntry(t, names, { ...at, entity: "task", id: "t-1", fields: { subtask_rank: { before: "a", after: "b" } } })).toEqual(["history-subtask-moved"]);
    expect(describeEntry(t, names, { ...at, entity: "task", id: "t-1", fields: { parent: { after: "t-2" }, subtask_rank: { after: "a" } } })).toEqual([
      'history-set{"field":"history-field-parent","value":"TM-2"}',
    ]);
    // A long text the engine cut short is marked as cut; a short one isn't.
    const long = "x".repeat(500);
    expect(describeEntry(t, names, { ...at, entity: "task", id: "t-1", fields: { title: { before: "Short", after: long, truncated: true } } })).toEqual([
      `history-changed{"field":"history-field-title","before":"Short","after":"history-value-cut{\\"value\\":\\"${long}\\"}"}`,
    ]);
    // Labels are listed for the locale.
    const labelled = { ...names, locale: "en-US", project: { ...project, labels: [{ id: "l-1", name: "docs" }, { id: "l-2", name: "ui" }, { id: "l-3", name: "cli" }] } } as unknown as Parameters<typeof describeEntry>[1];
    expect(describeEntry(t, labelled, { ...at, entity: "task", id: "t-1", fields: { labels: { after: ["l-1", "l-2", "l-3"] } } })).toEqual([
      'history-set{"field":"history-field-labels","value":"docs, ui, and cli"}',
    ]);
  });
});

describe("a workspace of one person", () => {
  it("has no members, mentions, watchers or assignees anywhere in the task panel", async () => {
    const { panel } = await renderPanel();
    const words = /\b(assignee|assigned|watchers?|watching|members?|mention|invite)\b/i;
    expect(words.test(panel.textContent)).toBe(false);
    const labels = [...panel.querySelectorAll("[aria-label]")].map((element) => element.getAttribute("aria-label") ?? "");
    expect(labels.filter((label) => words.test(label))).toEqual([]);
    // Nor in any of the app's messages.
    const messages = Object.values(import.meta.glob<string>("../../i18n/en/*.ftl", { query: "?raw", import: "default", eager: true })).join("\n");
    expect(messages.split("\n").filter((line) => !line.startsWith("#") && words.test(line))).toEqual([]);
  });
});

describe("the task's bar", () => {
  it("duplicates the task and opens the copy", async () => {
    const { panel, task } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Task menu" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));
    const copy = await screen.findByRole("complementary", { name: (name) => /^Task PER-\d+$/.test(plain(name)) && plain(name) !== `Task ${task.key}` });
    const links = await within(copy).findByRole("region", { name: "Links" });
    await within(links).findByRole("button", { name: named(new RegExp(`^Copy of ${task.key}`)) });
  });

  it("opens the task window's screen, without the board, in a plain browser", async () => {
    const { panel, task } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: "Open in its own window" }));
    await screen.findByRole("button", { name: "Close window" });
    expect(screen.queryByRole("group", { name: /Board of/ })).toBeNull();
    expect(document.querySelector(`[data-drop-task="${task.key}"]`)).not.toBeNull();
  });
});
