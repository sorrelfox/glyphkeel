// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { markWords } from "./DiffView";
import { wordDiff } from "./wordDiff";

const changed = (parts: { text: string; changed: boolean }[]) => parts.filter((part) => part.changed).map((part) => part.text);

describe("word diff", () => {
  it("marks the words that changed inside a changed line", () => {
    const words = wordDiff("Ship the release on Monday.", "Ship the big release on Friday.");
    expect(words).not.toBeNull();
    expect(changed(words?.removed ?? [])).toEqual(["Monday"]);
    expect(changed(words?.added ?? [])).toEqual(["big ", "Friday"]);
    expect(words?.added.map((part) => part.text).join("")).toBe("Ship the big release on Friday.");
  });

  it("joins neighbouring changed words with the space between them", () => {
    const words = wordDiff("call Alice and Bob today", "call Carol or Dave today");
    expect(changed(words?.removed ?? [])).toEqual(["Alice and Bob"]);
    expect(changed(words?.added ?? [])).toEqual(["Carol or Dave"]);
  });

  it("leaves lines that share no word unmarked", () => {
    expect(wordDiff("one two", "three four")).toBeNull();
  });

  it("compares words of scripts without spaces", () => {
    const words = wordDiff("今日は会議です", "明日は会議です");
    expect(words).not.toBeNull();
    expect(words?.added.some((part) => part.changed)).toBe(true);
    expect(words?.added.map((part) => part.text).join("")).toBe("明日は会議です");
  });

  it("pairs each removed line with the added line after it", () => {
    const lines = markWords([
      { kind: "context", text: "# Plan" },
      { kind: "remove", text: "- ship on Monday" },
      { kind: "remove", text: "- tag it" },
      { kind: "add", text: "- ship on Friday" },
      { kind: "add", text: "- brand new line" },
      { kind: "add", text: "- another" },
    ]);
    expect(changed(lines[1]?.words ?? [])).toEqual(["Monday"]);
    expect(changed(lines[3]?.words ?? [])).toEqual(["Friday"]);
    // "- tag it" and "- brand new line" share only the dash.
    expect(lines[2]?.words).toBeUndefined();
    expect(lines[5]?.words).toBeUndefined();
    expect(lines[0]?.words).toBeUndefined();
  });
});
