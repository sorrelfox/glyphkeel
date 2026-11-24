// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { checklistOf, merge3, renderFake, setChecked, unifiedDiff } from "./markdown";

describe("the sample engine's Markdown", () => {
  it("numbers checkboxes from 1, skipping code, and ticks one character", () => {
    const text = "- [ ] a\n```\n- [ ] code\n```\n  - [x] **b**\r\n";
    const items = checklistOf(text);
    expect(items.map((item) => [item.number, item.text, item.checked])).toEqual([
      [1, "a", false],
      [2, "b", true],
    ]);
    const [first] = items;
    if (!first) {
      throw new Error("no items");
    }
    const ticked = setChecked(text, first, true);
    expect(ticked).toBe(text.replace("- [ ] a", "- [x] a"));
  });

  it("renders checkboxes with the same numbers, and keys only when they exist", () => {
    const html = renderFake("- [ ] a\n- [x] b\n\nTM-1 and XX-9 <b>", (key) => key === "TM-1");
    expect(html).toContain('<input type="checkbox" data-item="2" checked="" />');
    expect(html).toContain('<a data-ref="TM-1">TM-1</a> and XX-9 &lt;b&gt;');
  });

  it("links a key with a comment anchor to the comment, as the engine does", () => {
    const html = renderFake("TM-1#c-0000abcd, TM-1#c-0000ABCD and TM-1#c-0000abcdx", (key) => key === "TM-1");
    expect(html).toContain('<a data-ref="TM-1" data-comment="0000abcd">TM-1#c-0000abcd</a>, ');
    expect(html).toContain('<a data-ref="TM-1">TM-1</a>#c-0000ABCD and <a data-ref="TM-1">TM-1</a>#c-0000abcdx');
  });

  it("merges edits of different lines and refuses edits of the same line", () => {
    const base = "one\ntwo\nthree\n";
    expect(merge3(base, "ONE\ntwo\nthree\n", "one\ntwo\nTHREE\n")).toBe("ONE\ntwo\nTHREE\n");
    expect(merge3(base, "uno\ntwo\nthree\n", "eins\ntwo\nthree\n")).toBeNull();
  });

  it("writes a unified diff with context", () => {
    expect(unifiedDiff("a\nb\nc\n", "a\nB\nc\n", "r1", "r2")).toBe("--- r1\n+++ r2\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n");
    expect(unifiedDiff("same", "same", "r1", "r2")).toBe("");
  });
});
