// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { FluentResource } from "@fluent/bundle";
import { describe, expect, it } from "vitest";

import { ACCENTS, DENSITIES, THEMES } from "../appearance/appearance";
import { applyLanguage, createBundle, MESSAGES, messageLanguage, textDirection } from "./i18n";

/** The app's source files, as text, without the tests. */
const SOURCES = Object.entries(import.meta.glob<string>("../**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }))
  .filter(([path]) => !/\.test\.tsx?$/.test(path))
  .map(([, text]) => text);

describe("messages", () => {
  it("parse without errors in every language", () => {
    for (const language of Object.keys(MESSAGES)) {
      expect(() => createBundle(language)).not.toThrow();
    }
  });

  it("are loaded from every message file", () => {
    const files = import.meta.glob<string>("./en/*.ftl", { query: "?raw", import: "default", eager: true });
    const names = Object.keys(files).map((path) => path.replace(/^.*\//, ""));
    expect(names).toEqual(expect.arrayContaining(["calendar.ftl", "gantt.ftl", "wiki.ftl"]));
    const loaded = MESSAGES.en ?? [];
    const missing = Object.entries(files)
      .filter(([, text]) => !loaded.includes(text))
      .map(([path]) => path);
    expect(missing).toEqual([]);
  });

  it("are the same in every language as in English, without duplicates", () => {
    const ids = (sources: readonly string[]) => sources.flatMap((source) => [...new FluentResource(source).body].map((entry) => entry.id));
    const english = ids(MESSAGES.en ?? []);
    expect(english.length).toBeGreaterThan(500);
    expect(english.filter((id, index) => english.indexOf(id) !== index)).toEqual([]);
    for (const [language, sources] of Object.entries(MESSAGES)) {
      const theirs = new Set(ids(sources));
      expect(english.filter((id) => !theirs.has(id)), `missing in ${language}`).toEqual([]);
      expect([...theirs].filter((id) => !english.includes(id)), `only in ${language}`).toEqual([]);
    }
  });

  it("exist for every message ID the code names", () => {
    const bundle = createBundle("en");
    const named = new Set<string>();
    for (const text of SOURCES) {
      for (const match of text.matchAll(/\bt\(\s*"([a-z0-9-]+)"|\b(?:label|reason|text):\s*"([a-z0-9-]+)"|"((?:action|shortcut-group|view|empty|startup)-[a-z0-9-]+)"/g)) {
        named.add(match[1] ?? match[2] ?? match[3] ?? "");
      }
    }
    named.delete("");
    expect(named.size).toBeGreaterThan(30);
    const missing = [...named].filter((id) => !bundle.hasMessage(id));
    expect(missing).toEqual([]);
  });

  it("exist for every theme, accent and density", () => {
    const bundle = createBundle("en");
    const ids = [
      ...THEMES.flatMap((theme) => [`theme-${theme}`, `theme-note-${theme}`]),
      ...ACCENTS.map((accent) => `accent-${accent}`),
      ...DENSITIES.map((density) => `density-${density}`),
    ];
    expect(ids.filter((id) => !bundle.hasMessage(id))).toEqual([]);
  });

  it("fall back to English for languages without messages", () => {
    expect(messageLanguage("de-DE")).toBe("en");
    expect(messageLanguage("en-GB")).toBe("en");
  });
});

describe("the writing direction", () => {
  it("is right to left for the languages and scripts written that way", () => {
    for (const locale of ["ar", "ar-EG", "he-IL", "fa", "ur-PK", "yi", "ckb", "ks-Arab", "pa-Arab-PK"]) {
      expect(textDirection(locale), locale).toBe("rtl");
    }
  });

  it("is left to right for the others, the pseudo-locale and tags it can't read", () => {
    for (const locale of ["en", "en-US", "de-CH", "ja", "zh-Hant-TW", "pa-IN", "en-XA", "not a tag"]) {
      expect(textDirection(locale), locale).toBe("ltr");
    }
  });

  it("goes onto the root with the language of the messages", () => {
    const root = document.createElement("html");
    applyLanguage("he-IL", root);
    // No Hebrew messages yet: the text is English, laid out right to left.
    expect(root.lang).toBe("en");
    expect(root.dir).toBe("rtl");
    applyLanguage("en-GB", root);
    expect(root.dir).toBe("ltr");
  });
});
