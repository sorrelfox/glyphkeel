// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_APPEARANCE } from "../appearance/appearance";
import { loadAppearance, loadCollapsedProjects, saveAppearance, saveCollapsedProjects } from "./store";

afterEach(() => {
  localStorage.clear();
});

describe("the personal settings store", () => {
  it("keeps the appearance for the next start", () => {
    expect(loadAppearance()).toEqual(DEFAULT_APPEARANCE);
    saveAppearance({ ...DEFAULT_APPEARANCE, theme: "sand", accent: "teal", density: "compact" });
    expect(loadAppearance()).toMatchObject({ theme: "sand", accent: "teal", density: "compact" });
  });

  it("takes the default for anything unknown or broken", () => {
    localStorage.setItem("glyphkeel.appearance", JSON.stringify({ theme: "neon", density: "compact", reduceMotion: "yes" }));
    expect(loadAppearance()).toEqual({ ...DEFAULT_APPEARANCE, density: "compact" });
    localStorage.setItem("glyphkeel.appearance", "{not json");
    expect(loadAppearance()).toEqual(DEFAULT_APPEARANCE);
  });

  it("keeps the sidebar's closed projects", () => {
    saveCollapsedProjects(["TM", "LAB"]);
    expect(loadCollapsedProjects()).toEqual(["TM", "LAB"]);
    localStorage.setItem("glyphkeel.sidebar.collapsed", JSON.stringify([1, "PER"]));
    expect(loadCollapsedProjects()).toEqual(["PER"]);
  });
});

describe("in the app", () => {
  afterEach(() => {
    vi.doUnmock("@tauri-apps/api/core");
    vi.resetModules();
    Reflect.deleteProperty(window, "__GLYPHKEEL_START__");
  });

  it("reads what the app handed over at the start and hands every change back to it", async () => {
    const invoke = vi.fn(() => Promise.resolve());
    vi.doMock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke }));
    vi.resetModules();
    Object.defineProperty(window, "__GLYPHKEEL_START__", {
      value: { titleBar: "app", prefs: { appearance: { theme: "slate", density: "compact" } } },
      configurable: true,
    });
    const store = await import("./store");
    expect(store.loadAppearance()).toMatchObject({ theme: "slate", density: "compact" });
    store.saveCardSize("p-1", "compact");
    expect(store.loadCardSize("p-1")).toBe("compact");
    expect(invoke).toHaveBeenCalledWith("set_ui_pref", { name: "board.cardSize.p-1", value: "compact" });
    // Nothing goes to the web view's storage, which the app doesn't keep.
    expect(localStorage.length).toBe(0);
  });
});
