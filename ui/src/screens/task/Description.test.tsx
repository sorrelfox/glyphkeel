// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorView } from "@codemirror/view";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { App } from "../../App";
import { fakeTransport } from "../../data/fake/transport";
import { createClient } from "../../data/transport";
import { type TaskChange, type TaskList } from "../../generated/ops";
import { diffLines } from "./DiffView";
import { editorDropAt } from "./MarkdownField";

// jsdom lays nothing out; the editor measures text ranges.
Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
Range.prototype.getBoundingClientRect = () => new DOMRect();

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const plain = (text: string | null | undefined) => (text ?? "").replace(/[⁨⁩]/g, "");

const TEXT = "Before the trip:\n\n- [ ] Book the train\n- [x] Pack the bags\n\nSee the *list*.\n";

async function renderDescription(description = TEXT) {
  const transport = fakeTransport();
  const task = ((await transport.call("task.create", { project: "PER", title: "Plan the trip", description })) as TaskChange).task;
  const other = ((await transport.call("task.create", { project: "PER", title: "Book the hotel" })) as TaskChange).task;
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
    const { task, section } = await renderDescription();
    fireEvent.click(within(section).getByRole("button", { name: "Edit" }));
    typeInto(await within(section).findByRole("textbox", { name: "Description" }), "A draft");
    cleanup();
    const again = await renderDescription();
    // A new workspace has its own task with the same key: the draft is offered for it.
    expect(again.task.key).toBe(task.key);
    expect(await within(again.section).findByText("Unsaved changes to this description are kept from earlier.")).toBeDefined();
    fireEvent.click(within(again.section).getByRole("button", { name: "Discard them" }));
    expect(within(again.section).queryByRole("status")).toBeNull();
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
