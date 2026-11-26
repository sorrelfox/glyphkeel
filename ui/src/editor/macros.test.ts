// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CompletionContext, type Completion, type CompletionResult, type CompletionSource } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";

import { completionSources } from "./complete";
import { editorExtensions } from "./setup";
import type { CompletionProviders } from "./types";

const views: EditorView[] = [];
afterEach(() => {
  for (const view of views.splice(0)) {
    view.destroy();
  }
});

function editor(text: string, providers: CompletionProviders): EditorView {
  const view = new EditorView({
    parent: document.body.appendChild(document.createElement("div")),
    state: EditorState.create({
      doc: text,
      extensions: editorExtensions({
        lineBreak: "\n",
        mode: "preview",
        readOnly: false,
        spellChecker: null,
        renderBlock: null,
        label: "Page",
        environment: { providers: () => providers, text: (id) => `«${id}»`, toggle: () => null },
      }),
    }),
  });
  ensureSyntaxTree(view.state, view.state.doc.length, 5000);
  view.dispatch({ selection: EditorSelection.cursor(text.length) });
  views.push(view);
  return view;
}

async function complete(view: EditorView): Promise<CompletionResult | null> {
  const context = new CompletionContext(view.state, view.state.selection.main.head, false);
  for (const source of completionSources as CompletionSource[]) {
    const result = await source(context);
    if (result) {
      return result;
    }
  }
  return null;
}

function apply(view: EditorView, result: CompletionResult, option: Completion): void {
  if (typeof option.apply === "function") {
    option.apply(view, option, result.from, view.state.selection.main.head);
  }
}

const pages = () => [{ target: "Release plan", detail: "TM" }, { target: "Release plan#Dates" }];
const wiki: CompletionProviders = { macros: true, pages };

describe("wiki macros in the editor", () => {
  it("are offered after {{ at the start of a line of a wiki page", async () => {
    const view = editor("Intro\n\n{{", wiki);
    const result = await complete(view);
    expect(result?.options.map((option) => option.label)).toEqual(["{{toc}}", "{{children}}", "{{include …}}", "{{tasks …}}"]);
    expect(result?.options[3]?.detail).toBe("«editor-macro-tasks»");
    const toc = result?.options[0];
    if (result && toc) {
      apply(view, result, toc);
    }
    expect(view.state.sliceDoc()).toBe("Intro\n\n{{toc}}");
  });

  it("leave the cursor inside a macro that takes a query, and offer pages to include", async () => {
    const view = editor("{{ta", wiki);
    const result = await complete(view);
    const tasks = result?.options.find((option) => option.label === "{{tasks …}}");
    if (result && tasks) {
      apply(view, result, tasks);
    }
    expect(view.state.sliceDoc()).toBe("{{tasks }}");
    expect(view.state.selection.main.head).toBe(8);

    const include = editor("{{include Rel", wiki);
    const pages = await complete(include);
    expect(pages?.options.map((option) => option.label)).toEqual(["Release plan"]);
    expect(pages?.from).toBe(10);
  });

  it("are not offered in descriptions, inside a line or in code", async () => {
    expect(await complete(editor("{{", { pages }))).toBeNull();
    expect(await complete(editor("text {{", wiki))).toBeNull();
    expect(await complete(editor("```\n{{", wiki))).toBeNull();
  });
});
