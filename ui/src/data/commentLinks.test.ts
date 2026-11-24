// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { appendQuote, commentLink, isComment, linkedComments, quoteMarkdown, shortCommentId } from "./commentLinks";

const ID = "0193b2a4-5c6d-7e8f-9a0b-1C2D3E4F5A6B";

describe("links to comments", () => {
  it("name the task and the last eight hexadecimal digits of the comment's ID", () => {
    expect(shortCommentId(ID)).toBe("3e4f5a6b");
    expect(commentLink("TM-41", ID)).toBe("TM-41#c-3e4f5a6b");
    expect(isComment(ID, "3E4F5A6B")).toBe(true);
    expect(isComment(ID, "1c2d3e4f")).toBe(false);
  });

  it("are found in a text only when they stand on their own and name the task", () => {
    const text = "See TM-41#c-3e4f5a6b and (TM-41#c-00000001). Not xTM-41#c-3e4f5a6b, TM-41#c-3e4f5a6bx, TM-4#c-3e4f5a6b or TM-41#c-3E4F5A6B.";
    expect(linkedComments(text, "TM-41")).toEqual(["3e4f5a6b", "00000001"]);
    expect(linkedComments("AXB-1#c-3e4f5a6b", "A.B-1")).toEqual([]);
  });
});

describe("a quote in a reply", () => {
  it("quotes every line, ends with its source and leaves a line to answer on", () => {
    expect(quoteMarkdown("First line\r\n\r\n  - a point\n", "⁨— TM-41#c-3e4f5a6b⁩")).toBe("> First line\n>\n>   - a point\n>\n> — TM-41#c-3e4f5a6b\n\n");
  });

  it("goes after the text already written, as a paragraph of its own", () => {
    expect(appendQuote("", "> q\n\n")).toBe("> q\n\n");
    expect(appendQuote("  \n", "> q\n\n")).toBe("> q\n\n");
    expect(appendQuote("Hi", "> q\n\n")).toBe("Hi\n\n> q\n\n");
    expect(appendQuote("Hi\n", "> q\n\n")).toBe("Hi\n\n> q\n\n");
    expect(appendQuote("Hi\n\n", "> q\n\n")).toBe("Hi\n\n> q\n\n");
  });
});
