// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CompletionContext, type CompletionResult, type CompletionSource } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it, vi } from "vitest";

import { completionSources } from "./complete";
import { openText } from "./lineEndings";
import { blockPreview, previewStats, resetPreviewStats } from "./livePreview";
import { editorExtensions, type EditorEnvironment } from "./setup";
import { misspeltAt, proseChunks } from "./spell";
import type { ChecklistToggle, CompletionProviders, SpellChecker } from "./types";
import { checklistToggleAt, MathWidget, MermaidWidget, RenderedBlockWidget, toggleChecklistItem } from "./widgets";

const CORPUS = import.meta.glob<string>("./corpus/*.md", { query: "?raw", import: "default", eager: true });

const views: EditorView[] = [];
afterEach(() => {
  for (const view of views.splice(0)) {
    view.destroy();
  }
});

function editor(text: string, environment: Partial<EditorEnvironment> = {}, spellChecker: SpellChecker | null = null): EditorView {
  const opened = openText(text);
  const view = new EditorView({
    parent: document.body.appendChild(document.createElement("div")),
    state: EditorState.create({
      doc: opened.text,
      extensions: editorExtensions({
        lineBreak: opened.lineBreak,
        mode: "preview",
        readOnly: false,
        spellChecker,
        renderBlock: null,
        label: "Description",
        environment: { providers: () => ({}), text: (id) => `«${id}»`, toggle: () => null, ...environment },
      }),
    }),
  });
  ensureSyntaxTree(view.state, view.state.doc.length, 5000);
  views.push(view);
  return view;
}

/** The positions of every task item's "[" in a text. */
function taskMarkers(text: string): number[] {
  return [...text.matchAll(/^[ \t>]*(?:[-*+]|\d+[.)]) \[[ xX]\]/gm)].map((match) => (match.index) + match[0].length - 3);
}

describe("checkboxes", () => {
  it("are drawn as widgets, and a click changes exactly one character", () => {
    const text = CORPUS["./corpus/sp4-sample.md"] ?? "";
    const view = editor(text);
    view.dispatch({ selection: EditorSelection.cursor(0) });
    const boxes = view.dom.querySelectorAll<HTMLInputElement>("input.cm-gk-task");
    expect(boxes.length).toBeGreaterThanOrEqual(3);
    for (const box of boxes) {
      const before = view.state.sliceDoc();
      box.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
      const after = view.state.sliceDoc();
      expect(after.length).toBe(before.length);
      const changed = Array.from({ length: before.length }, (_, i) => i).filter((i) => before.charCodeAt(i) !== after.charCodeAt(i));
      expect(changed.length).toBe(1);
    }
  });

  it("tick and untick like the engine: [ ] ↔ [x], and [X] unticks", () => {
    const view = editor("- [ ] a\n- [X] b\n");
    toggleChecklistItem(view, 2);
    expect(view.state.sliceDoc()).toBe("- [x] a\n- [X] b\n");
    toggleChecklistItem(view, 10);
    expect(view.state.sliceDoc()).toBe("- [x] a\n- [ ] b\n");
    toggleChecklistItem(view, 2);
    expect(view.state.sliceDoc()).toBe("- [ ] a\n- [ ] b\n");
  });

  it("change one character in every item of the corpus, in CRLF and CR texts too", () => {
    for (const name of ["./corpus/tasks-many.md", "./corpus/crlf.md", "./corpus/cr-only.md", "./corpus/sp4-sample.md"]) {
      const original = CORPUS[name] ?? "";
      const view = editor(original);
      for (const offset of taskMarkers(original)) {
        // The editor's position of the "[": a CRLF is one position.
        const pos = offset - (original.slice(0, offset).match(/\r\n/g) ?? []).length;
        expect(view.state.sliceDoc(pos, pos + 1)).toBe("[");
        toggleChecklistItem(view, pos);
        const after = view.state.sliceDoc();
        expect(after.length).toBe(original.length);
        expect(after.slice(0, offset + 1)).toBe(original.slice(0, offset + 1));
        expect(after.slice(offset + 2)).toBe(original.slice(offset + 2));
        expect(after[offset + 1]).not.toBe(original[offset + 1]);
        toggleChecklistItem(view, pos);
      }
    }
  });

  it("ask the engine instead when it owns the text", () => {
    const handler = vi.fn<(toggle: ChecklistToggle) => void>();
    const text = "Intro\r\n\r\n- [ ] first\r\n  - [x] nested **one**\r\n";
    const view = editor(text, { toggle: () => handler });
    view.dispatch({ selection: EditorSelection.cursor(0) });
    const boxes = view.dom.querySelectorAll<HTMLInputElement>("input.cm-gk-task");
    expect(boxes.length).toBe(2);
    boxes[1]?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    expect(view.state.sliceDoc()).toBe(text);
    expect(handler).toHaveBeenCalledWith({ index: 1, offset: text.indexOf("[x]"), checked: true, text: "nested **one**" });
    expect(checklistToggleAt(view.state, view.state.doc.line(3).from + 2)).toEqual({ index: 0, offset: text.indexOf("[ ]"), checked: false, text: "first" });
  });
});

describe("block widgets", () => {
  const sample = CORPUS["./corpus/sp4-sample.md"] ?? "";

  it("draw math, Mermaid, tables and callouts, and show the source while the selection touches them", () => {
    const view = editor(sample);
    view.dispatch({ selection: EditorSelection.cursor(0) });
    const kinds = () => view.state.field(blockPreview).shown.map((block) => block.widget.constructor.name + ("kind" in block.widget ? `:${block.widget.kind}` : ""));
    expect(kinds()).toEqual(["RenderedBlockWidget:callout", "MathWidget", "MermaidWidget", "RenderedBlockWidget:table"]);
    const blocks = view.state.field(blockPreview).blocks;
    const math = blocks[1];
    expect(math?.widget).toBeInstanceOf(MathWidget);
    expect((math?.widget as MathWidget).tex).toBe("\\int_0^1 x^2\\,dx = \\frac13");
    expect((blocks[2]?.widget as MermaidWidget).source).toBe("graph TD; A-->B;");
    expect((blocks[3]?.widget as RenderedBlockWidget).source).toBe("| a | b |\n|:-:|--:|\n| 1 | 2 |");
    // The cursor in the table: its source shows, the others stay widgets.
    view.dispatch({ selection: EditorSelection.cursor(sample.indexOf("| 1 |") + 2) });
    expect(kinds()).toEqual(["RenderedBlockWidget:callout", "MathWidget", "MermaidWidget"]);
  });

  it("keep their list when only the cursor moves", () => {
    const view = editor(sample);
    const blocks = view.state.field(blockPreview).blocks;
    resetPreviewStats();
    view.dispatch({ selection: EditorSelection.cursor(5) });
    view.dispatch({ selection: EditorSelection.cursor(20) });
    expect(view.state.field(blockPreview).blocks).toBe(blocks);
    expect(previewStats.blockScans).toBe(0);
  });

  it("after any change, equal the blocks of the whole text read afresh", () => {
    const describeBlocks = (view: EditorView) =>
      view.state.field(blockPreview).blocks.map((block) => {
        const widget = block.widget;
        const content = widget instanceof MathWidget ? widget.tex : widget instanceof MermaidWidget ? widget.source : widget.source;
        return `${String(block.from)}-${String(block.to)} ${widget.constructor.name} ${content}`;
      });
    const text = `${sample}\n${CORPUS["./corpus/sp4-typical.md"] ?? ""}\n${sample}`;
    const view = editor(text);
    const pieces = ["```\n", "$$\n", "| x | y |\n", "\n", "> [!NOTE]\n", "text ", "|", "> ", "\n\n", "```mermaid\n"];
    let seed = 12345;
    const next = () => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return seed / 2 ** 32;
    };
    for (let i = 0; i < 300; i++) {
      const length = view.state.doc.length;
      const from = Math.floor(next() * (length + 1));
      const to = Math.min(length, from + (next() < 0.3 ? Math.floor(next() * 40) : 0));
      view.dispatch({ changes: { from, to, insert: pieces[Math.floor(next() * pieces.length)] ?? "" } });
      ensureSyntaxTree(view.state, view.state.doc.length, 5000);
      const fresh = editor(view.state.sliceDoc());
      expect(describeBlocks(view)).toEqual(describeBlocks(fresh));
      fresh.destroy();
      views.pop();
    }
  });

  it("cost little per key on a 100 KB text", () => {
    const typical = CORPUS["./corpus/sp4-typical.md"] ?? "";
    let text = "";
    while (text.length < 100_000) {
      text += `${typical}\n${sample}\n`;
    }
    const view = editor(text);
    const middle = Math.floor(view.state.doc.length / 2);
    view.dispatch({ selection: EditorSelection.cursor(view.state.doc.lineAt(middle).to) });
    resetPreviewStats();
    for (let i = 0; i < 50; i++) {
      view.dispatch(view.state.replaceSelection("a"));
    }
    const perKey = previewStats.blockMs / previewStats.blockPasses;
    console.info(`block pass on ${String(text.length)} characters: ${perKey.toFixed(3)} ms per key`);
    expect(previewStats.blockPasses).toBe(50);
    expect(previewStats.blockRescans).toBe(50);
    // A generous bound: the old pass took about 0.9 ms here in the web view.
    expect(perKey).toBeLessThan(5);
  });
});

describe("completion", () => {
  async function complete(view: EditorView, explicit = false): Promise<CompletionResult | null> {
    const context = new CompletionContext(view.state, view.state.selection.main.head, explicit);
    for (const source of completionSources as CompletionSource[]) {
      const result = await source(context);
      if (result) {
        return result;
      }
    }
    return null;
  }

  const providers: CompletionProviders = {
    tasks: (project, query) =>
      [
        { key: `${project}-41`, title: "Fix the sync bug" },
        { key: `${project}-42`, title: "Fix the export dialog" },
      ].filter((task) => task.key.slice(project.length + 1).startsWith(query) || task.title.toLowerCase().includes(query)),
    pages: (query) => ["Roadmap", "Release plan"].filter((page) => page.toLowerCase().includes(query.toLowerCase())),
  };

  it("offers task keys by number and by title", async () => {
    const view = editor("See TM-4", { providers: () => providers });
    view.dispatch({ selection: EditorSelection.cursor(8) });
    expect((await complete(view))?.options.map((option) => option.label)).toEqual(["TM-41", "TM-42"]);
    view.dispatch({ changes: { from: 4, to: 8, insert: "TM-exp" }, selection: EditorSelection.cursor(10) });
    expect((await complete(view))?.options.map((option) => option.label)).toEqual(["TM-42"]);
  });

  it("offers wiki pages after [[ and closes the link", async () => {
    const view = editor("Read [[Road", { providers: () => providers });
    view.dispatch({ selection: EditorSelection.cursor(11) });
    const result = await complete(view);
    expect(result?.from).toBe(7);
    expect(result?.options.map((option) => [option.label, option.apply])).toEqual([["Roadmap", "Roadmap]]"]]);
  });

  it("offers the slash commands with their translated names, and nothing in code", async () => {
    const view = editor("Text\n/ta");
    view.dispatch({ selection: EditorSelection.cursor(8) });
    const result = await complete(view);
    expect(result?.options.map((option) => option.label)).toEqual(["/table", "/task", "/code", "/math", "/mermaid", "/callout", "/heading"]);
    expect(result?.options[0]?.detail).toBe("«editor-slash-table»");
    const code = editor("```\n/ta");
    code.dispatch({ selection: EditorSelection.cursor(7) });
    expect(await complete(code)).toBeNull();
  });
});

describe("spelling", () => {
  it("sends prose only, with code, links, math, task keys and wiki links blanked out", () => {
    const text = "Teh word `cod` and https://x.example $y^2$ TM-41 [[Paeg]] [lnk](http://u.example) end";
    const view = editor(text);
    const chunks = proseChunks(view);
    const prose = chunks.map(([, chunk]) => chunk).join("");
    expect(prose.length).toBe(text.length);
    for (const word of ["Teh", "word", "and", "end", "lnk"]) {
      expect(prose).toContain(word);
    }
    for (const word of ["cod", "example", "y^2", "TM-41", "Paeg", "http"]) {
      expect(prose).not.toContain(word);
    }
  });

  it("keeps positions in CRLF texts, where a line break is one position", () => {
    const view = editor("Frist\r\nSecnod\r\n");
    const [[from, chunk] = [0, ""]] = proseChunks(view);
    expect(from).toBe(0);
    expect(chunk.indexOf("Secnod")).toBe(view.state.doc.line(2).from);
  });

  it("underlines the words the spell checker marks, in loaded text", async () => {
    const checker: SpellChecker = {
      check: vi.fn((chunks: readonly (readonly [number, string])[]) =>
        Promise.resolve(chunks.flatMap(([from, chunk]) => [...chunk.matchAll(/\bTeh\b/g)].map((m) => [from + m.index, from + m.index + 3] as const))),
      ),
      suggest: () => Promise.resolve(["The"]),
    };
    const view = editor("Teh start\n\nand Teh end `Teh`", {}, checker);
    await vi.waitFor(() => {
      expect(view.dom.querySelectorAll(".cm-gk-misspelt").length).toBe(2);
    });
    expect(misspeltAt(view, 1)).toEqual({ from: 0, to: 3 });
    expect(misspeltAt(view, 6)).toBeNull();
  });
});
