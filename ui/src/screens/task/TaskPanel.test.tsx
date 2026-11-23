// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorView } from "@codemirror/view";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange } from "../../generated/ops";
import { describeEntry } from "./Activity";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();

afterEach(() => {
  cleanup();
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
    fireEvent.keyDown(within(row as HTMLElement).getByRole("button", { name: named(/^Open /) }), { key: "ArrowDown", altKey: true });
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
    await within(links).findByRole("button", { name: named(new RegExp(`^Blocked by ${other.key}: Book the train`)) });
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
    await within(files).findByRole("button", { name: named(/^Open notes\.txt$/) });
    const image = await within(files).findByRole("img", { name: "dot.png" });
    await waitFor(() => {
      expect(image.getAttribute("src")).toMatch(/^data:image\/png;base64,/);
    });
    // The engine's thumbnail gives its size.
    expect(image.dataset.thumbnail).toBe("1x1");
    fireEvent.click(within(files).getByRole("button", { name: named(/^File notes\.txt$/) }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    await waitFor(() => {
      expect(within(files).queryByRole("button", { name: named(/^Open notes\.txt$/) })).toBeNull();
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

  it("attaches a file pasted into the composer and links it in the text", async () => {
    const { panel } = await renderPanel();
    const composer = await within(panel).findByRole("textbox", { name: "Write a comment" });
    const png = new File([Uint8Array.from(atob(PNG), (char) => char.charCodeAt(0))], "shot [1].png", { type: "image/png" });
    fireEvent.paste(composer, { clipboardData: { files: [png], getData: () => "", types: ["Files"] } });
    await waitFor(() => {
      expect(editorText(composer)).toMatch(/^!\[shot \\\[1\\\]\.png\]\(gk-file:\/\/[^)]+\)$/);
    });
    await within(section(panel, "Files")).findByRole("button", { name: named(/^Open shot \[1\]\.png$/) });
  });

  it("lists the task's changes and reverts the latest", async () => {
    const { panel } = await renderPanel();
    fireEvent.click(within(panel).getByRole("button", { name: named(/^Priority:/) }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /High/ }));
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
