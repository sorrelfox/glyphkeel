// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { ACCENTS, DENSITIES, THEMES } from "../appearance/appearance";
import { createBundle, MESSAGES, messageLanguage } from "./i18n";

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
