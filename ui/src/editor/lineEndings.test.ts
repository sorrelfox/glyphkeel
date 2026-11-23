// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { checkEdits, convertLineBreaks, countLineBreaks, hasMixedLineBreaks, mainLineBreak, openText, splitsSurrogatePair } from "./lineEndings";

describe("line breaks", () => {
  it("are counted by kind", () => {
    expect(countLineBreaks("a\nb\r\nc\rd\r\n")).toEqual({ lf: 1, crlf: 2, cr: 1 });
    expect(countLineBreaks("no breaks")).toEqual({ lf: 0, crlf: 0, cr: 0 });
  });

  it("are mixed only when more than one kind occurs", () => {
    expect(hasMixedLineBreaks("a\r\nb\r\n")).toBe(false);
    expect(hasMixedLineBreaks("a\rb\r")).toBe(false);
    expect(hasMixedLineBreaks("a\nb\r\n")).toBe(true);
  });

  it("have a main kind: the most used, ties to the first, LF without any", () => {
    expect(mainLineBreak("a\r\nb\nc\r\n")).toBe("\r\n");
    expect(mainLineBreak("a\nb\r\n")).toBe("\n");
    expect(mainLineBreak("a\r\nb\n")).toBe("\r\n");
    expect(mainLineBreak("a\rb\rc\n")).toBe("\r");
    expect(mainLineBreak("single line")).toBe("\n");
  });

  it("convert to one kind", () => {
    expect(convertLineBreaks("a\nb\r\nc\rd", "\r\n")).toBe("a\r\nb\r\nc\r\nd");
    expect(convertLineBreaks("a\r\n\r\nb", "\n")).toBe("a\n\nb");
  });

  it("are kept when opening a text with one kind, and converted once when mixed", () => {
    expect(openText("x\r\ny\r\n")).toEqual({ text: "x\r\ny\r\n", lineBreak: "\r\n" });
    const opened = openText("x\r\ny\nz\r\n");
    expect(opened.text).toBe("x\r\ny\r\nz\r\n");
    expect(opened.lineBreak).toBe("\r\n");
    expect(opened.converted).toEqual({ from: { lf: 1, crlf: 2, cr: 0 }, to: "\r\n" });
  });
});

describe("outside edits", () => {
  const text = "a😀b"; // the emoji is two UTF-16 code units, at 1 and 2

  it("may not split a surrogate pair", () => {
    expect(splitsSurrogatePair(text, 2)).toBe(true);
    expect(splitsSurrogatePair(text, 1)).toBe(false);
    expect(splitsSurrogatePair(text, 3)).toBe(false);
    expect(checkEdits(text, [{ from: 2, to: 2, insert: "x" }])).toBe("splits-surrogate-pair");
    expect(checkEdits(text, [{ from: 0, to: 2, insert: "" }])).toBe("splits-surrogate-pair");
    expect(checkEdits(text, [{ from: 1, to: 3, insert: "🙂" }])).toBeNull();
  });

  it("must lie inside the text and not overlap", () => {
    expect(checkEdits(text, [{ from: 0, to: 9, insert: "" }])).toBe("out-of-range");
    expect(checkEdits(text, [{ from: -1, to: 0, insert: "" }])).toBe("out-of-range");
    expect(checkEdits(text, [{ from: 0.5, to: 1, insert: "" }])).toBe("out-of-range");
    expect(
      checkEdits(text, [
        { from: 0, to: 1, insert: "" },
        { from: 0, to: 3, insert: "" },
      ]),
    ).toBe("overlapping");
    expect(
      checkEdits(text, [
        { from: 3, to: 4, insert: "c" },
        { from: 0, to: 1, insert: "z" },
      ]),
    ).toBeNull();
  });
});
