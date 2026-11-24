// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { projectOfKey, readStart } from "./start";

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

  it("takes a task window's task and the task to open first, when they are task keys", () => {
    expect(readStart(windowWith({ titleBar: "system", locale: "de-DE", task: "TM-41", open: "CLI-2" }))).toEqual({
      titleBar: "system",
      locale: "de-DE",
      task: "TM-41",
      open: "CLI-2",
    });
    expect(readStart(windowWith({ task: "../TM-41", open: 41 }))).toEqual({ titleBar: "app", locale: "fr-FR" });
  });

  it("offers the first-run set-up when the app made the workspace, or the address asks for it outside the app", () => {
    expect(readStart(windowWith({ firstRun: true })).firstRun).toBe(true);
    expect(readStart(windowWith({ firstRun: "yes" })).firstRun).toBeUndefined();
    const browser = { navigator: { language: "fr-FR" }, location: { search: "?first-run" } } as unknown as Window;
    expect(readStart(browser).firstRun).toBe(true);
  });

  it("finds a task's project in its key", () => {
    expect(projectOfKey("TM-41")).toBe("TM");
    expect(projectOfKey("CLI2-7")).toBe("CLI2");
  });
});
