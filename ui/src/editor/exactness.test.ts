// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { undo } from "@codemirror/commands";
import { insertNewlineContinueMarkup } from "@codemirror/lang-markdown";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";

import { insertWithLineBreaks } from "./complete";
import { convertLineBreaks, openText } from "./lineEndings";
import { pasteHtml } from "./paste";
import { applyTextEdits, editorExtensions } from "./setup";
import type { EditorMode } from "./types";

const CORPUS = import.meta.glob<string>("./corpus/*.md", { query: "?raw", import: "default", eager: true });

/** The 100 KB document of the corpus: the typical description, repeated. */
function bigDocument(): string {
  const typical = CORPUS["./corpus/typical.md"] ?? "";
  let text = "";
  for (let i = 0; text.length < 100_000; i++) {
    text += `## Part ${String(i)}\n\n${typical}\n`;
  }
  return text;
}

const views: EditorView[] = [];
afterEach(() => {
  for (const view of views.splice(0)) {
    view.destroy();
  }
});

function editor(text: string, mode: EditorMode = "preview"): EditorView {
  const opened = openText(text);
  const view = new EditorView({
    parent: document.body.appendChild(document.createElement("div")),
    state: EditorState.create({
      doc: opened.text,
      extensions: editorExtensions({
        lineBreak: opened.lineBreak,
        mode,
        readOnly: false,
        spellChecker: null,
        renderBlock: null,
        label: "Description",
        environment: { providers: () => ({}), text: (id) => id, toggle: () => null },
      }),
    }),
  });
  views.push(view);
  return view;
}

/** The text's UTF-16 offset of an editor position: a CRLF is one position in the editor and two code units in the text. */
function offsetOf(state: EditorState, pos: number): number {
  return pos + (state.lineBreak.length - 1) * (state.doc.lineAt(pos).number - 1);
}

/** A small, repeatable random source. */
function random(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

const bareBreaks = (text: string, lineBreak: string) =>
  lineBreak === "\r\n" ? (text.match(/\r(?!\n)|(?<!\r)\n/g) ?? []).length : (text.match(lineBreak === "\n" ? /\r/g : /\n/g) ?? []).length;

describe("the corpus", () => {
  it("has its line breaks and byte order mark as stored", () => {
    expect(CORPUS["./corpus/crlf.md"]).toContain("\r\n");
    expect(CORPUS["./corpus/cr-only.md"]).toMatch(/\r[^\n]/);
    expect(CORPUS["./corpus/bom.md"]?.charCodeAt(0)).toBe(0xfeff);
    expect(Object.keys(CORPUS).length).toBe(11);
  });

  const files = Object.entries(CORPUS).filter(([path]) => !path.includes("mixed"));
  files.push(["the 100 KB document", bigDocument()]);

  for (const [name, text] of files) {
    it(`reads back ${name} exactly, in Live Preview and in Source`, () => {
      for (const mode of ["preview", "source"] as const) {
        const view = editor(text, mode);
        expect(view.state.sliceDoc()).toBe(text);
      }
    });
  }

  it("converts mixed line breaks to the main kind on opening", () => {
    const text = CORPUS["./corpus/mixed-endings.md"] ?? "";
    const opened = openText(text);
    expect(opened.converted).toBeDefined();
    const view = editor(text);
    expect(view.state.sliceDoc()).toBe(convertLineBreaks(text, opened.lineBreak));
  });
});

describe("random edits", () => {
  for (const name of ["./corpus/typical.md", "./corpus/crlf.md", "./corpus/cr-only.md", "./corpus/unicode.md"]) {
    it(`give the same text as on a plain string, and undo to the original, for ${name}`, () => {
      const original = CORPUS[name] ?? "";
      const view = editor(original);
      const next = random(name.length * 7919);
      let mirror = original;
      const pieces = ["x", "é", "漢字", "😀", "\n", "- [ ] item\n", "**bold**", "$x^2$", "[[Page]]", "TM-41 ", ""];
      for (let i = 0; i < 80; i++) {
        const { state } = view;
        const length = state.doc.length;
        let from = Math.floor(next() * (length + 1));
        let to = Math.min(length, from + Math.floor(next() * 6));
        // Never between the halves of a surrogate pair, as the editor's own motions never are.
        const text = state.sliceDoc();
        const clamp = (pos: number) => {
          const offset = offsetOf(state, pos);
          const code = text.charCodeAt(offset);
          return code >= 0xdc00 && code <= 0xdfff ? pos - 1 : pos;
        };
        from = clamp(from);
        to = Math.max(from, clamp(to));
        const insert = convertLineBreaks(pieces[Math.floor(next() * pieces.length)] ?? "", state.lineBreak);
        const offsetFrom = offsetOf(state, from);
        const offsetTo = offsetOf(state, to);
        view.dispatch({ changes: { from, to, insert }, userEvent: "input" });
        mirror = mirror.slice(0, offsetFrom) + insert + mirror.slice(offsetTo);
        expect(view.state.sliceDoc()).toBe(mirror);
      }
      while (undo(view)) {
        // all the way back
      }
      expect(view.state.sliceDoc()).toBe(original);
    });
  }
});

describe("in a CRLF text, inserted text gets CRLF", () => {
  const crlf = () => CORPUS["./corpus/crlf.md"] ?? "";

  it("from Enter, which continues a list", () => {
    const view = editor("- [ ] one\r\n- [x] two\r\n");
    view.dispatch({ selection: EditorSelection.cursor(view.state.doc.line(2).to) });
    expect(insertNewlineContinueMarkup(view)).toBe(true);
    expect(view.state.sliceDoc()).toBe("- [ ] one\r\n- [x] two\r\n- [ ] \r\n");
  });

  it("from a plain paste", () => {
    const view = editor(crlf());
    const filters = view.state.facet(EditorView.clipboardInputFilter);
    const pasted = filters.reduce((text, filter) => filter(text, view.state), "one\ntwo\rthree\r\nfour");
    view.dispatch(view.state.replaceSelection(pasted));
    expect(bareBreaks(view.state.sliceDoc(), "\r\n")).toBe(0);
    expect(view.state.sliceDoc()).toContain("one\r\ntwo\r\nthree\r\nfour");
  });

  it("from an HTML paste", async () => {
    const view = editor(crlf());
    await pasteHtml(view, "<p>First</p><ul><li>one</li><li>two</li></ul><table><tr><td>a</td><td>b</td></tr><tr><td>1</td><td>2</td></tr></table>");
    expect(bareBreaks(view.state.sliceDoc(), "\r\n")).toBe(0);
    expect(view.state.sliceDoc()).toContain("- one\r\n- two");
  });

  it("from a slash command's snippet", () => {
    const view = editor(crlf());
    const end = view.state.doc.length;
    insertWithLineBreaks("| Column | Column |\n| --- | --- |\n|  |  |")(view, { label: "/table" }, end, end);
    expect(bareBreaks(view.state.sliceDoc(), "\r\n")).toBe(0);
    expect(view.state.sliceDoc().endsWith("| Column | Column |\r\n| --- | --- |\r\n|  |  |")).toBe(true);
  });

  it("from outside edits, given in the text's own offsets", () => {
    const view = editor("ab\r\ncd\r\n");
    // "c" is at offset 4 in the text (position 3 in the editor).
    expect(applyTextEdits(view, [{ from: 4, to: 5, insert: "C\nD" }])).toBe(true);
    expect(view.state.sliceDoc()).toBe("ab\r\nC\r\nDd\r\n");
    // Between "\r" and "\n": refused.
    expect(applyTextEdits(view, [{ from: 3, to: 3, insert: "x" }])).toBe(false);
    expect(view.state.sliceDoc()).toBe("ab\r\nC\r\nDd\r\n");
  });
});

describe("outside edits", () => {
  it("are refused when they would split a surrogate pair", () => {
    const view = editor("a😀b");
    expect(applyTextEdits(view, [{ from: 2, to: 2, insert: "x" }])).toBe(false);
    expect(view.state.sliceDoc()).toBe("a😀b");
  });
});
