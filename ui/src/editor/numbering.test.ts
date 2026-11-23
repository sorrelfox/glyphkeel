// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ensureSyntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";

import { openText } from "./lineEndings";
import { editorExtensions } from "./setup";
import { checklistToggleAt } from "./widgets";

/**
 * Texts with the checklist the engine reads in them: each item's plain text,
 * in the engine's numbering (from 1, in reading order). Recorded from the
 * engine's Markdown crate.
 */
const ENGINE: readonly (readonly [string, readonly string[]])[] = [
  ["- [ ] a\n- [x] b\n  - [ ] nested\n- [ ] c\n", ["a", "b", "nested", "c"]],
  ["- [ ]\n- [ ] x\n", ["", "x"]],
  ["> - [ ] quoted\n", ["quoted"]],
  ["> [!NOTE]\n> - [ ] in callout\n", ["in callout"]],
  ["1. [ ] ordered\n2) [x] paren\n", ["ordered", "paren"]],
  ["```\n- [ ] in code\n```\n- [ ] after\n", ["after"]],
  ["- [ ]\tTab\n", ["Tab"]],
  ["* [x]  two spaces\n", ["two spaces"]],
  ["- [ ] item\n\n      - [ ] deep\n", ["item"]],
  ["- not [ ] task\n- [ ] yes\n", ["yes"]],
  ["Text\n- [ ] after paragraph\n", ["after paragraph"]],
  ["| a |\n|---|\n| - [ ] cell |\n", []],
  ["- [ ]  \n- [ ] z\n", ["", "z"]],
  ["-   [ ] wide marker\n", ["wide marker"]],
  ["- [~] relaxed\n- [ ] plain\n", ["plain"]],
  ["+ [X] upper\n", ["upper"]],
  ["Note[^1]\n\n[^1]: - [ ] in footnote\n\n- [ ] body\n", ["body", "in footnote"]],
  ["<details>\n\n- [ ] in html block\n\n</details>\n", ["in html block"]],
  ["- [ ] a\n<!-- - [ ] comment -->\n- [ ] b\n", ["a", "b"]],
  ["- [ ] a\r\n- [x] b\r\n", ["a", "b"]],
  ["- [ ] **bold** `code` [link](https://x.example)\n", ["bold code link"]],
  ["- [ ] one\n    - [ ] four-space nested\n\t- [ ] tab nested\n", ["one", "four-space nested", "tab nested"]],
  ["- [ ]\n  text on next line\n", ["text on next line"]],
  ["- \n  [ ] lazy\n", ["lazy"]],
  ["$$\n- [ ] math\n$$\n- [ ] after math\n", ["math $$", "after math"]],
  ["    - [ ] indented code\n- [ ] real\n", ["real"]],
  ["- [x]x no space\n- [ ] ok\n", ["ok"]],
  ["1. a\n   - [ ] nested in ordered\n", ["nested in ordered"]],
  ["> > - [ ] double quote\n", ["double quote"]],
  ["- [ ] a\n\n  > - [ ] quote in item\n", ["a", "quote in item"]],
];

/** Where the engine and the editor's parser are known to read a text differently. */
const KNOWN_DIFFERENCES = new Set<string>([
  // The engine numbers footnotes after the body, as the reader shows them; the editor's parser has no footnotes.
  "Note[^1]\n\n[^1]: - [ ] in footnote\n\n- [ ] body\n",
  // A `$$` line that the engine reads as text opens a math block in the editor.
  "$$\n- [ ] math\n$$\n- [ ] after math\n",
]);

/** The checklist as the editor reads it: number and plain text of every checkbox. */
function editorChecklist(text: string): { number: number; text: string }[] {
  const opened = openText(text);
  const state = EditorState.create({
    doc: opened.text,
    extensions: editorExtensions({
      lineBreak: opened.lineBreak,
      mode: "preview",
      readOnly: false,
      spellChecker: null,
      renderBlock: null,
      label: "Text",
      environment: { providers: () => ({}), text: (id) => id, toggle: () => null },
    }),
  });
  const tree = ensureSyntaxTree(state, state.doc.length, 5000);
  const markers: number[] = [];
  tree?.iterate({
    enter(node) {
      if (node.name === "TaskMarker") {
        markers.push(node.from);
      }
    },
  });
  return markers.map((pos) => {
    const toggle = checklistToggleAt(state, pos);
    return { number: toggle.number, text: toggle.text };
  });
}

describe("checkbox numbers", () => {
  it.each(ENGINE.filter(([text]) => !KNOWN_DIFFERENCES.has(text)))("match the engine's for %j", (text, items) => {
    expect(editorChecklist(text)).toEqual(items.map((item, index) => ({ number: index + 1, text: item })));
  });
});
