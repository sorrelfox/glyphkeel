// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorView } from "@codemirror/view";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { flushDrafts, forgetDraftsInMemory } from "../../data/drafts";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type CommentChange, type CommentList, type TaskChange, type TaskList } from "../../generated/ops";
import { diffLines } from "./DiffView";
import { editorDropAt } from "./MarkdownField";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();

afterEach(() => {
  cleanup();
  forgetDraftsInMemory();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

const TEXT = "Before the trip:\n\n- [ ] Book the train\n- [x] Pack the bags\n\nSee the *list*.\n";

async function renderDescription(description = TEXT, workspace?: { transport: ReturnType<typeof fakeTransport>; task: TaskChange["task"] }) {
  const transport = workspace?.transport ?? fakeTransport();
  const task = workspace?.task ?? ((await transport.call("task.create", { project: "PER", title: "Plan the trip", description })) as TaskChange).task;
  const other = workspace ? task : ((await transport.call("task.create", { project: "PER", title: "Book the hotel" })) as TaskChange).task;
  render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
  const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
  const section = await within(panel).findByRole("region", { name: "Description" });
  return { transport, task, other, panel, section };
}

const stored = async (transport: ReturnType<typeof fakeTransport>, key: string) => ((await transport.call("task.get", { keys: [key] })) as TaskList).items[0]?.description;

function typeInto(textbox: HTMLElement, text: string) {
  const view = EditorView.findFromDOM(textbox);
  if (!view) {
    throw new Error("not an editor");
  }
  act(() => {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, userEvent: "input.type" });
  });
}

describe("the description", () => {
  it("shows the rendered text and ticks a checkbox through the engine, changing one character", async () => {
    const { transport, task, section } = await renderDescription();
    expect((await within(section).findByText("list")).tagName).toBe("EM");
    const box = within(section).getByRole("checkbox", { name: "Book the train" });
    expect(box.getAttribute("data-item")).toBe("1");
    fireEvent.click(box);
    await waitFor(async () => {
      expect(await stored(transport, task.key)).toBe(TEXT.replace("- [ ] Book", "- [x] Book"));
    });
    await waitFor(() => {
      expect(within(section).getByRole<HTMLInputElement>("checkbox", { name: "Book the train" }).checked).toBe(true);
    });
  });

  it("refuses a tick when the checklist changed meanwhile, and puts the box back", async () => {
    const { transport, task, section } = await renderDescription();
    const box = await within(section).findByRole<HTMLInputElement>("checkbox", { name: "Book the train" });
    // Another program renames the item; this reader still shows the old text.
    transport.workspace.call("task.update", { key: task.key, description: TEXT.replace("Book the train", "Book a train") });
    fireEvent.click(box);
    expect(await screen.findByText(/The checklist changed; please tick it again/)).toBeDefined();
    expect(box.checked).toBe(false);
  });

  it("links task keys, which open the task", async () => {
    const { other, section, panel } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    typeInto(field, `Depends on ${other.key}.`);
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    const link = await within(section).findByRole("link", { name: other.key });
    fireEvent.click(link);
    await within(panel.ownerDocument.body).findByRole("heading", { name: "Book the hotel" });
  });

  it("saves an edit as a new revision, and merges it with a change made meanwhile", async () => {
    const { transport, task, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    // Meanwhile an agent changes the last line.
    transport.workspace.call("task.update", { key: task.key, description: TEXT.replace("See the *list*.", "See the *full* list."), base_revision: task.description_revision ?? null });
    typeInto(field, TEXT.replace("Before the trip:", "Before we go:"));
    fireEvent.click(within(section).getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved, merged with a change made meanwhile.")).toBeDefined();
    expect(await stored(transport, task.key)).toBe("Before we go:\n\n- [ ] Book the train\n- [x] Pack the bags\n\nSee the *full* list.\n");
  });

  it("formats the selection from the toolbar and saves the text exactly", async () => {
    const { transport, task, section } = await renderDescription("Pack light\r\nand early\r\n");
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    const toolbar = within(section).getByRole("toolbar", { name: "Formatting" });
    const bold = within(toolbar).getByRole("button", { name: "Bold" });
    const view = EditorView.findFromDOM(field);
    act(() => {
      view?.dispatch({ selection: { anchor: 5, head: 10 } });
    });
    fireEvent.click(bold);
    fireEvent.click(within(toolbar).getByRole("button", { name: "Bulleted list" }));
    fireEvent.click(within(section).getByRole("button", { name: "Save" }));
    await waitFor(async () => {
      expect(await stored(transport, task.key)).toBe("- Pack **light**\r\nand early\r\n");
    });
  });

  it("keeps a clashing edit as a conflict copy, and resolves it with the copy", async () => {
    const { transport, task, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    transport.workspace.call("task.update", { key: task.key, description: TEXT.replace("Before the trip:", "Before the journey:"), base_revision: task.description_revision ?? null });
    typeInto(field, TEXT.replace("Before the trip:", "Before we go:"));
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    const banner = await within(section).findByRole("alert");
    expect(plain(banner.textContent)).toContain("kept as a conflict copy");
    expect(await stored(transport, task.key)).toContain("Before the journey:");

    fireEvent.click(within(banner).getByRole("button", { name: "Resolve" }));
    const resolver = await within(section).findByRole("group", { name: "Conflict copy" });
    const diff = await within(resolver).findByRole("group", { name: "Differences between the description and the copy" });
    expect(within(diff).getByText("Before the journey:").tagName).toBe("DEL");
    expect(within(diff).getByText("Before we go:").tagName).toBe("INS");
    fireEvent.click(within(resolver).getByRole("button", { name: "Use the copy" }));
    await waitFor(async () => {
      expect(await stored(transport, task.key)).toContain("Before we go:");
    });
    await waitFor(() => {
      expect(within(section).queryByRole("alert")).toBeNull();
    });
  });

  it("keeps the revision it opened at when another window saves before the first keystroke: the edits are merged or kept as a copy, never overwritten", async () => {
    const { transport, task, panel, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    // Another window saves through the engine; its change reaches this one before anything is typed here.
    await transport.call("task.update", { key: task.key, title: "Plan the journey", description: TEXT.replace("Before the trip:", "Before the journey:"), base_revision: task.description_revision ?? null });
    await within(panel).findByRole("heading", { name: "Plan the journey" });
    // The editor still holds the text it opened with, not the newer one.
    expect(EditorView.findFromDOM(field)?.state.doc.toString()).toBe(TEXT);
    typeInto(field, TEXT.replace("Before the trip:", "Before we go:"));
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    const banner = await within(section).findByRole("alert");
    expect(plain(banner.textContent)).toContain("kept as a conflict copy");
    expect(await stored(transport, task.key)).toContain("Before the journey:");
  });

  it("merges an edit of another line saved elsewhere after editing started", async () => {
    const { transport, task, panel, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    await transport.call("task.update", { key: task.key, title: "Plan the journey", description: TEXT.replace("See the *list*.", "See the *full* list."), base_revision: task.description_revision ?? null });
    await within(panel).findByRole("heading", { name: "Plan the journey" });
    typeInto(field, TEXT.replace("Before the trip:", "Before we go:"));
    fireEvent.click(within(section).getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved, merged with a change made meanwhile.")).toBeDefined();
    expect(await stored(transport, task.key)).toBe("Before we go:\n\n- [ ] Book the train\n- [x] Pack the bags\n\nSee the *full* list.\n");
  });

  it("closes an editor left untouched without saving, after a change elsewhere", async () => {
    const { transport, task, panel, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    const field = await within(section).findByRole("textbox", { name: "Description" });
    await transport.call("task.update", { key: task.key, title: "Plan the journey", description: TEXT.replace("Before the trip:", "Before the journey:"), base_revision: task.description_revision ?? null });
    await within(panel).findByRole("heading", { name: "Plan the journey" });
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    await within(section).findByText("Before the journey:");
    expect(within(section).queryByRole("alert")).toBeNull();
    expect(await stored(transport, task.key)).toContain("Before the journey:");
  });

  it("refuses a comment edit when the comment changed elsewhere after editing started", async () => {
    const transport = fakeTransport();
    const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip" })) as TaskChange).task;
    const { comment } = (await transport.call("comment.add", { key: task.key, body: "Book the train" })) as CommentChange;
    render(<App start={{ titleBar: "app", locale: "en-US" }} client={createClient(transport)} path={`/projects/PER?task=${task.key}`} />);
    const panel = await screen.findByRole("complementary", { name: (name) => plain(name).startsWith(`Task ${task.key}`) });
    const article = await within(panel).findByRole("article", { name: (name) => plain(name).startsWith("Comment by Me") });
    await within(article).findByText("Book the train");
    fireEvent.click(within(article).getByRole("button", { name: "Comment" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const field = await within(article).findByRole("textbox", { name: "Comment text" });
    // Another window edits the comment meanwhile.
    await transport.call("comment.update", { comment: comment.id, body: "Book the night train" });
    await within(article).findByRole("button", { name: (name) => plain(name).includes("edited · 1 earlier version") });
    typeInto(field, "Book the bus");
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(await screen.findByText(/The comment changed since you read it/)).toBeDefined();
    const comments = (await transport.call("comment.list", { key: task.key })) as CommentList;
    expect(comments.items.map((item) => item.body)).toEqual(["Book the night train"]);
  });

  it("lists the revisions and shows the differences between two", async () => {
    const { transport, task, section } = await renderDescription();
    transport.workspace.call("task.update", { key: task.key, description: TEXT.replace("Pack the bags", "Pack light") });
    fireEvent.click(await within(section).findByRole("button", { name: "History" }));
    const list = await within(section).findByRole("list", { name: "History" });
    await waitFor(() => {
      expect(within(list).getAllByRole("checkbox")).toHaveLength(2);
    });
    const [newer, older] = within(list).getAllByRole("checkbox");
    fireEvent.click(newer as HTMLElement);
    fireEvent.click(older as HTMLElement);
    const diff = await within(section).findByRole("group", { name: "Differences between the two revisions" });
    expect(within(diff).getByText("- [x] Pack the bags").tagName).toBe("DEL");
    expect(within(diff).getByText("- [x] Pack light").tagName).toBe("INS");
  });

  it("keeps an unsaved draft when the editor closes with the panel", async () => {
    const { transport, task, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    typeInto(await within(section).findByRole("textbox", { name: "Description" }), "A draft");
    cleanup();
    const again = await renderDescription(TEXT, { transport, task });
    expect(await within(again.section).findByText("Unsaved changes to this description are kept from earlier.")).toBeDefined();
    fireEvent.click(within(again.section).getByRole("button", { name: "Discard them" }));
    expect(within(again.section).queryByRole("status")).toBeNull();
    await waitFor(async () => {
      expect(await transport.call("draft.get", { kind: "task", target: task.key })).toEqual({});
    });
  });

  it("offers the draft again after a restart, from the engine, and saves it against its revision", async () => {
    const { transport, task, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    typeInto(await within(section).findByRole("textbox", { name: "Description" }), "Before the trip: pack.");
    flushDrafts();
    await waitFor(async () => {
      const kept = (await transport.call("draft.get", { kind: "task", target: task.key })) as { draft?: { body: string; base_revision?: string } };
      expect(kept.draft?.body).toBe("Before the trip: pack.");
      expect(kept.draft?.base_revision).toBe(task.description_revision);
    });
    // Meanwhile an agent changes the description.
    transport.workspace.call("task.update", { key: task.key, description: `${TEXT}Bring snacks.\n` }, { actor: { kind: "agent", name: "Helper" }, via: "mcp" });
    cleanup();
    forgetDraftsInMemory();

    const again = await renderDescription(TEXT, { transport, task });
    expect(await within(again.section).findByText("Unsaved changes to this description are kept from earlier.")).toBeDefined();
    fireEvent.click(within(again.section).getByRole("button", { name: "Continue editing" }));
    const editor = await within(again.section).findByRole("textbox", { name: "Description" });
    expect(EditorView.findFromDOM(editor)?.state.doc.toString()).toBe("Before the trip: pack.");
    fireEvent.keyDown(editor, { key: "Enter", ctrlKey: true });
    await waitFor(async () => {
      expect(await transport.call("draft.get", { kind: "task", target: task.key })).toEqual({});
    });
  });
});

describe("diff lines", () => {
  it("leave out the file headers and keep hunks, additions and removals", () => {
    const diff = "--- a\n+++ b\n@@ -1,2 +1,2 @@\n same\n-old\n+new\n\\ No newline at end of file\n";
    expect(diffLines(diff)).toEqual([
      { kind: "hunk", text: "@@ -1,2 +1,2 @@" },
      { kind: "context", text: "same" },
      { kind: "remove", text: "old" },
      { kind: "add", text: "new" },
    ]);
  });
});

describe("files dropped from the desktop onto the editor", () => {
  it("go in the text as links where they were dropped, as when pasted", async () => {
    const { task, panel } = await renderDescription("Notes\n");
    const composer = await within(panel).findByRole("textbox", { name: "Write a comment" });
    // The window reports where the files were dropped; jsdom has no layout, so the point names the composer.
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: () => composer });
    try {
      expect(editorDropAt(10, 10, "OTHER-1")).toBeNull();
      const insert = editorDropAt(10, 10, task.key);
      expect(insert).not.toBeNull();
      act(() => {
        insert?.("[notes.txt](gk-file://00000000-0000-4000-8000-000000000001)");
      });
      expect(EditorView.findFromDOM(composer)?.state.doc.toString()).toContain("[notes.txt](gk-file://00000000-0000-4000-8000-000000000001)");
    } finally {
      Reflect.deleteProperty(document, "elementFromPoint");
    }
  });
});
