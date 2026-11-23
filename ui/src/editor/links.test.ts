// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CompletionContext, type CompletionResult, type CompletionSource } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";

import { completionSources, startLink } from "./complete";
import { openText } from "./lineEndings";
import { editorExtensions } from "./setup";
import type { CompletionProviders } from "./types";

const views: EditorView[] = [];
afterEach(() => {
  for (const view of views.splice(0)) {
    view.destroy();
  }
});

const TASKS = [
  { key: "TM-41", title: "Fix the sync bug" },
  { key: "TM-42", title: "Fix the export dialog" },
  { key: "PER-7", title: "Book the car service" },
];

const providers: CompletionProviders = {
  findTasks: (query) => TASKS.filter((task) => task.key.toLowerCase().startsWith(query.toLowerCase()) || task.title.toLowerCase().includes(query.toLowerCase())),
  tasks: (project, query) => TASKS.filter((task) => task.key.startsWith(`${project}-${query}`)),
  pages: (query) =>
    query.includes("#")
      ? [{ target: "Release plan#Checklist", detail: "Release plan" }]
      : [{ target: "Release plan", detail: "TM" }, "Roadmap"].filter((page) => (typeof page === "string" ? page : page.target).toLowerCase().includes(query.toLowerCase())),
};

function editor(text: string): EditorView {
  const opened = openText(text);
  const view = new EditorView({
    parent: document.body.appendChild(document.createElement("div")),
    state: EditorState.create({
      doc: opened.text,
      extensions: editorExtensions({
        lineBreak: opened.lineBreak,
        mode: "source",
        readOnly: false,
        spellChecker: null,
        renderBlock: null,
        label: "Description",
        environment: { providers: () => providers, text: (id) => id, toggle: () => null },
      }),
    }),
  });
  ensureSyntaxTree(view.state, view.state.doc.length, 5000);
  views.push(view);
  return view;
}

/** Every source's answer at the cursor, as the editor merges them. */
async function complete(view: EditorView): Promise<CompletionResult[]> {
  const context = new CompletionContext(view.state, view.state.selection.main.head, true);
  const results: CompletionResult[] = [];
  for (const source of completionSources as CompletionSource[]) {
    const result = await source(context);
    if (result) {
      results.push(result);
    }
  }
  return results;
}

const at = (view: EditorView, pos: number) => {
  view.dispatch({ selection: EditorSelection.cursor(pos) });
};

describe("links to tasks", () => {
  it("search by a word of the title after #, and the key replaces the search", async () => {
    const view = editor("See #sync");
    at(view, 9);
    const results = await complete(view);
    expect(results).toHaveLength(1);
    const [result] = results;
    expect(result?.from).toBe(4);
    expect(result?.options.map((option) => [option.label, option.detail])).toEqual([["TM-41", "Fix the sync bug"]]);
    view.dispatch({ changes: { from: result?.from ?? 0, to: 9, insert: String(result?.options[0]?.apply) } });
    expect(view.state.doc.toString()).toBe("See TM-41");
  });

  it("search by key after #, without the capitals-only key completion answering too", async () => {
    const view = editor("(#TM-4");
    at(view, 6);
    const results = await complete(view);
    expect(results).toHaveLength(1);
    expect(results[0]?.options.map((option) => option.label)).toEqual(["TM-41", "TM-42"]);
  });

  it("offer every task on a bare #, but not in headings or words", async () => {
    const bare = editor("Text #");
    at(bare, 6);
    expect((await complete(bare))[0]?.options).toHaveLength(3);
    for (const [text, pos] of [["## Title", 2], ["a#b", 3], ["`#sync`", 6]] as const) {
      const view = editor(text);
      at(view, pos);
      expect((await complete(view)).filter((result) => result.options.some((option) => option.label.includes("-")))).toEqual([]);
    }
  });
});

describe("links to wiki pages", () => {
  it("offer pages after [[, with their project, and close the link", async () => {
    const view = editor("Read [[rel");
    at(view, 10);
    const [result] = await complete(view);
    expect(result?.options.map((option) => [option.label, option.detail, option.apply])).toEqual([["Release plan", "TM", "Release plan]]"]]);
  });

  it("offer a page's headings after Page#", async () => {
    const view = editor("Read [[Release plan#]]");
    at(view, 20);
    const [result] = await complete(view);
    expect(result?.options.map((option) => option.apply)).toEqual(["Release plan#Checklist"]);
  });
});

describe("the toolbar's link buttons", () => {
  it("type # after the selection and open the task search", () => {
    const view = editor("Fixed by");
    view.dispatch({ selection: EditorSelection.cursor(8) });
    startLink(view, "task");
    expect(view.state.doc.toString()).toBe("Fixed by #");
    expect(view.state.selection.main.head).toBe(10);
  });

  it("search for a selected word", () => {
    const view = editor("See sync");
    view.dispatch({ selection: EditorSelection.range(4, 8) });
    startLink(view, "task");
    expect(view.state.doc.toString()).toBe("See #sync");
  });

  it("put a page link's brackets around the selection, with the cursor before the closing ones", () => {
    const view = editor("Read the release plan");
    view.dispatch({ selection: EditorSelection.range(9, 21) });
    startLink(view, "page");
    expect(view.state.doc.toString()).toBe("Read the [[release plan]]");
    expect(view.state.selection.main.head).toBe(23);
  });
});
