// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { afterEach, describe, expect, it } from "vitest";

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
