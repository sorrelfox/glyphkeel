// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { readStart } from "./start";

function windowWith(start: unknown): Window {
  return { __GLYPHKEEL_START__: start, navigator: { language: "fr-FR" } } as unknown as Window;
}

describe("readStart", () => {
  it("takes what the app hands over", () => {
    expect(readStart(windowWith({ titleBar: "system", locale: "de-DE" }))).toEqual({ titleBar: "system", locale: "de-DE" });
  });

  it("falls back to the app's header bar and the browser's locale", () => {
    expect(readStart(windowWith(undefined))).toEqual({ titleBar: "app", locale: "fr-FR" });
    expect(readStart(windowWith({ titleBar: "fancy", locale: "not a locale!" }))).toEqual({ titleBar: "app", locale: "fr-FR" });
  });
});
