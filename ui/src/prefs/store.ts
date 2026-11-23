// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ACCENTS, DEFAULT_APPEARANCE, DENSITIES, THEMES, type Appearance } from "../appearance/appearance";

/**
 * Personal settings of this window that aren't workspace data: the
 * appearance and the sidebar's open projects. They live in the web view's
 * storage, behind this one module, until the app keeps them in its own
 * settings file. Storage that is missing or full is not an error: the
 * defaults apply.
 */
const PREFIX = "glyphkeel.";

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readPref(name: string): unknown {
  try {
    const text = storage()?.getItem(PREFIX + name);
    return text == null ? undefined : (JSON.parse(text) as unknown);
  } catch {
    return undefined;
  }
}

export function writePref(name: string, value: unknown): void {
  try {
    storage()?.setItem(PREFIX + name, JSON.stringify(value));
  } catch {
    // Full or switched off: the setting lasts until the window closes.
  }
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value) ? (value as T) : fallback;
}

/** The stored appearance; anything missing or unknown takes its default. */
export function loadAppearance(): Appearance {
  const stored = readPref("appearance");
  const value = typeof stored === "object" && stored !== null ? (stored as Record<string, unknown>) : {};
  return {
    theme: oneOf(value.theme, ["system", ...THEMES], DEFAULT_APPEARANCE.theme),
    systemLight: oneOf(value.systemLight, ["light", "sand"], DEFAULT_APPEARANCE.systemLight),
    systemDark: oneOf(value.systemDark, ["dark", "slate"], DEFAULT_APPEARANCE.systemDark),
    accent: oneOf(value.accent, ["system", ...ACCENTS], DEFAULT_APPEARANCE.accent),
    density: oneOf(value.density, DENSITIES, DEFAULT_APPEARANCE.density),
    reduceMotion: typeof value.reduceMotion === "boolean" ? value.reduceMotion : DEFAULT_APPEARANCE.reduceMotion,
  };
}

export function saveAppearance(appearance: Appearance): void {
  writePref("appearance", appearance);
}

/** Projects whose sub-projects the sidebar hides. */
export function loadCollapsedProjects(): string[] {
  const stored = readPref("sidebar.collapsed");
  return Array.isArray(stored) ? stored.filter((key): key is string => typeof key === "string") : [];
}

export function saveCollapsedProjects(keys: readonly string[]): void {
  writePref("sidebar.collapsed", keys);
}
