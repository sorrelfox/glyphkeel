// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";

import { applyFormat } from "./format";
import { codeMirrorKey, FORMAT_KINDS, FORMAT_SHORTCUTS, type FormatKind } from "./formats";

const views: EditorView[] = [];
afterEach(() => {
  for (const view of views.splice(0)) {
    view.destroy();
  }
});

/** An editor on `text` with the selection from `anchor` to `head`. */
function editor(text: string, anchor: number, head = anchor, lineBreak = "\n"): EditorView {
  const view = new EditorView({
    state: EditorState.create({ doc: text, selection: EditorSelection.single(anchor, head), extensions: [EditorState.lineSeparator.of(lineBreak)] }),
  });
  views.push(view);
  return view;
}

function run(view: EditorView, kind: FormatKind): { text: string; selected: string } {
  applyFormat(view, kind);
  const { from, to } = view.state.selection.main;
  return { text: view.state.sliceDoc(), selected: view.state.sliceDoc(from, to) };
}

describe("formatting commands", () => {
  it("wraps the selection and takes the markers away again", () => {
    const view = editor("say hello now", 4, 9);
    expect(run(view, "bold")).toEqual({ text: "say **hello** now", selected: "hello" });
    expect(run(view, "bold")).toEqual({ text: "say hello now", selected: "hello" });
    expect(run(view, "strikethrough").text).toBe("say ~~hello~~ now");
    expect(run(editor("a `b` c", 2, 5), "code")).toEqual({ text: "a b c", selected: "b" });
  });

  it("puts the cursor between an empty pair", () => {
    const view = editor("x", 1);
    expect(run(view, "italic").text).toBe("x**");
    expect(view.state.selection.main.head).toBe(2);
  });

  it("doesn't take italic for half of bold", () => {
    expect(run(editor("**word**", 2, 6), "italic").text).toBe("***word***");
  });

  it("makes a link with the cursor where the address goes", () => {
    const view = editor("see docs", 4, 8);
    expect(run(view, "link").text).toBe("see [docs]()");
    expect(view.state.selection.main.head).toBe(11);
  });

  it("starts each selected line with the marker, numbers a list, and toggles back", () => {
    const view = editor("one\ntwo\n\nthree", 0, 14);
    expect(run(view, "numbered-list").text).toBe("1. one\n2. two\n\n3. three");
    expect(run(view, "numbered-list").text).toBe("one\ntwo\n\nthree");
    expect(run(view, "checklist").text).toBe("- [ ] one\n- [ ] two\n\n- [ ] three");
    // A list item of another kind changes its marker.
    expect(run(view, "bullet-list").text).toBe("- one\n- two\n\n- three");
    expect(run(view, "quote").text).toBe("> - one\n> - two\n\n> - three");
  });

  it("keeps indentation and makes a heading of the line", () => {
    expect(run(editor("  - item", 4), "checklist").text).toBe("  - [ ] item");
    const view = editor("Title", 2);
    expect(run(view, "heading").text).toBe("## Title");
    expect(run(view, "heading").text).toBe("Title");
    expect(run(editor("", 0), "bullet-list").text).toBe("- ");
  });

  it("fences a code block with the text's own line break", () => {
    const view = editor("a\r\nb\r\nc", 3, 4, "\r\n");
    expect(run(view, "code-block")).toEqual({ text: "a\r\n```\r\nb\r\n```\r\nc", selected: "b" });
    const empty = editor("", 0);
    expect(run(empty, "code-block").text).toBe("```\n\n```");
    expect(empty.state.selection.main.head).toBe(4);
  });

  it("changes nothing in a read-only editor", () => {
    const view = new EditorView({ state: EditorState.create({ doc: "x", extensions: [EditorState.readOnly.of(true)] }) });
    views.push(view);
    expect(applyFormat(view, "bold")).toBe(false);
    expect(view.state.sliceDoc()).toBe("x");
  });

  it("has a distinct shortcut for every command", () => {
    const keys = FORMAT_KINDS.map((kind) => codeMirrorKey(FORMAT_SHORTCUTS[kind]));
    expect(new Set(keys).size).toBe(FORMAT_KINDS.length);
    expect(codeMirrorKey("Ctrl+Shift+X")).toBe("Mod-Shift-x");
  });
});
