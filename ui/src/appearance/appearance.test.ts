// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { apply, DEFAULT_APPEARANCE, resolve, type Desktop } from "./appearance";

const light: Desktop = { dark: false, highContrast: false, reducedMotion: false, accent: null };
const dark: Desktop = { ...light, dark: true };

describe("resolve", () => {
  it("follows the desktop's light or dark setting by default", () => {
    expect(resolve(DEFAULT_APPEARANCE, light).theme).toBe("light");
    expect(resolve(DEFAULT_APPEARANCE, dark).theme).toBe("dark");
  });

  it("uses the chosen themes for a light and a dark desktop", () => {
    const appearance = { ...DEFAULT_APPEARANCE, systemLight: "sand", systemDark: "slate" } as const;
    expect(resolve(appearance, light).theme).toBe("sand");
    expect(resolve(appearance, dark).theme).toBe("slate");
  });

  it("switches to the contrast theme when the desktop asks for high contrast", () => {
    expect(resolve(DEFAULT_APPEARANCE, { ...dark, highContrast: true }).theme).toBe("contrast");
  });

  it("keeps a theme chosen explicitly", () => {
    expect(resolve({ ...DEFAULT_APPEARANCE, theme: "slate" }, { ...light, highContrast: true }).theme).toBe("slate");
  });

  it("takes the accent from the setting, then the desktop, then the theme", () => {
    expect(resolve({ ...DEFAULT_APPEARANCE, accent: "pink" }, { ...light, accent: "green" }).accent).toBe("pink");
    expect(resolve(DEFAULT_APPEARANCE, { ...light, accent: "green" }).accent).toBe("green");
    expect(resolve({ ...DEFAULT_APPEARANCE, theme: "sand" }, light).accent).toBe("orange");
    expect(resolve({ ...DEFAULT_APPEARANCE, theme: "slate" }, light).accent).toBe("teal");
  });

  it("reduces motion when the setting or the desktop asks for it", () => {
    expect(resolve(DEFAULT_APPEARANCE, light).reducedMotion).toBe(false);
    expect(resolve({ ...DEFAULT_APPEARANCE, reduceMotion: true }, light).reducedMotion).toBe(true);
    expect(resolve(DEFAULT_APPEARANCE, { ...light, reducedMotion: true }).reducedMotion).toBe(true);
  });
});

describe("apply", () => {
  it("writes the three attributes and the motion durations onto the root", () => {
    const root = document.createElement("html");
    apply({ theme: "slate", accent: "teal", density: "compact", reducedMotion: true }, root);
    expect(root.dataset).toMatchObject({ theme: "slate", accent: "teal", density: "compact" });
    expect(root.style.getPropertyValue("--dur")).toBe("0ms");
    apply({ theme: "light", accent: "blue", density: "regular", reducedMotion: false }, root);
    expect(root.dataset).toMatchObject({ theme: "light", accent: "blue", density: "regular" });
    expect(root.style.getPropertyValue("--dur")).toBe("");
  });
});
