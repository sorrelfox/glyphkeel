// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { contextOf, matches, parseChord, typesText } from "./keymap";

const key = (k: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  key: k,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("parseChord", () => {
  it("reads modifiers and the key", () => {
    expect(parseChord("Ctrl+Shift+Z")).toEqual({ ctrl: true, alt: false, shift: true, meta: false, key: "z" });
    expect(parseChord("F9")).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: "F9" });
    expect(parseChord("Alt+ArrowLeft").key).toBe("ArrowLeft");
    expect(parseChord("Ctrl++")).toMatchObject({ ctrl: true, key: "+" });
    expect(parseChord("Space").key).toBe(" ");
    expect(parseChord("?").key).toBe("?");
  });

  it("refuses a chord without a key", () => {
    expect(() => parseChord("")).toThrow();
  });
});

describe("matches", () => {
  it("compares letters case-insensitively and modifiers exactly", () => {
    expect(matches(parseChord("Ctrl+K"), key("k", { ctrlKey: true }))).toBe(true);
    expect(matches(parseChord("Ctrl+K"), key("K", { ctrlKey: true }))).toBe(true);
    expect(matches(parseChord("Ctrl+K"), key("k"))).toBe(false);
    expect(matches(parseChord("Ctrl+K"), key("k", { ctrlKey: true, altKey: true }))).toBe(false);
    expect(matches(parseChord("Ctrl+Shift+Z"), key("Z", { ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(matches(parseChord("Ctrl+Z"), key("Z", { ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(matches(parseChord("N"), key("N", { shiftKey: true }))).toBe(false);
  });

  it("ignores Shift for symbols that need it on some layouts", () => {
    expect(matches(parseChord("?"), key("?", { shiftKey: true }))).toBe(true);
    expect(matches(parseChord("?"), key("?"))).toBe(true);
    expect(matches(parseChord("Ctrl+,"), key(",", { ctrlKey: true }))).toBe(true);
  });

  it("compares named keys and Shift on them", () => {
    expect(matches(parseChord("F6"), key("F6"))).toBe(true);
    expect(matches(parseChord("F6"), key("F6", { shiftKey: true }))).toBe(false);
    expect(matches(parseChord("Shift+F6"), key("F6", { shiftKey: true }))).toBe(true);
  });
});

describe("typesText", () => {
  it("is true for single printable keys without Ctrl, Alt or Super", () => {
    expect(typesText(parseChord("N"))).toBe(true);
    expect(typesText(parseChord("?"))).toBe(true);
    expect(typesText(parseChord("Space"))).toBe(true);
    expect(typesText(parseChord("Ctrl+N"))).toBe(false);
    expect(typesText(parseChord("F9"))).toBe(false);
    expect(typesText(parseChord("Escape"))).toBe(false);
  });
});

describe("contextOf", () => {
  it("tells text fields and editors from everything else", () => {
    const text = document.createElement("input");
    const checkbox = Object.assign(document.createElement("input"), { type: "checkbox" });
    const area = document.createElement("textarea");
    const editor = document.createElement("div");
    editor.setAttribute("contenteditable", "true");
    const inner = document.createElement("span");
    editor.append(inner);
    const button = document.createElement("button");
    expect(contextOf(text)).toBe("text");
    expect(contextOf(area)).toBe("text");
    expect(contextOf(inner)).toBe("text");
    expect(contextOf(checkbox)).toBe("outside-text");
    expect(contextOf(button)).toBe("outside-text");
    expect(contextOf(null)).toBe("outside-text");
  });
});
