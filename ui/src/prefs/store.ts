// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { invoke, isTauri } from "@tauri-apps/api/core";

import { ACCENTS, DEFAULT_APPEARANCE, DENSITIES, THEMES, type Appearance } from "../appearance/appearance";

/**
 * Personal settings of this window that aren't workspace data: the
 * appearance, the sidebar's closed projects, the board's card size and its
 * collapsed swimlanes. The
 * web view keeps nothing between starts, so in the app they live in the
 * app's settings file: the app hands them over before the page's first
 * frame (so the theme doesn't flash) and keeps every change. A plain
 * browser (the development bridge, the sample data, tests) keeps them in
 * its own storage. Storage that is missing or full is not an error: the
 * defaults apply.
 */
const PREFIX = "glyphkeel.";

/** The preferences the app handed over, while the page runs in the app. */
let kept: Map<string, unknown> | null = null;

function fromApp(): Map<string, unknown> | null {
  if (kept === null && typeof window !== "undefined" && isTauri()) {
    const given = window.__GLYPHKEEL_START__?.prefs;
    kept = new Map(typeof given === "object" && given !== null ? Object.entries(given as Record<string, unknown>) : []);
  }
  return kept;
}

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readPref(name: string): unknown {
  const app = fromApp();
  if (app) {
    return app.get(name);
  }
  try {
    const text = storage()?.getItem(PREFIX + name);
    return text == null ? undefined : (JSON.parse(text) as unknown);
  } catch {
    return undefined;
  }
}

export function writePref(name: string, value: unknown): void {
  const app = fromApp();
  if (app) {
    app.set(name, value);
    // The app keeps it for the next start; if it can't, the setting lasts until the window closes.
    invoke("set_ui_pref", { name, value }).catch(() => undefined);
    return;
  }
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
  return appearanceFrom(readPref("appearance"));
}

/** An appearance as stored, checked; anything missing or unknown takes its default. */
export function appearanceFrom(stored: unknown): Appearance {
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

/** Project folders (by ID) the sidebar shows closed. */
export function loadCollapsedFolders(): string[] {
  const stored = readPref("sidebar.collapsedFolders");
  return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === "string") : [];
}

export function saveCollapsedFolders(ids: readonly string[]): void {
  writePref("sidebar.collapsedFolders", ids);
}

/** How big the board's cards are. */
export type CardSize = "regular" | "compact";

/** The card size of a project's board (by project ID); the engine's layout doesn't hold it. */
export function loadCardSize(project: string): CardSize {
  return oneOf(readPref(`board.cardSize.${project}`), ["regular", "compact"], "regular");
}

export function saveCardSize(project: string, size: CardSize): void {
  writePref(`board.cardSize.${project}`, size);
}

/**
 * The swimlanes (by lane ID) collapsed on a project's board (by project ID),
 * for one grouping: a lane's ID means something only within its grouping
 * (`priority`, `label`, `field:cf.area`…).
 */
export function loadCollapsedLanes(project: string, grouping: string): string[] {
  const stored = readPref(`board.collapsedLanes.${project}.${grouping}`);
  return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === "string") : [];
}

export function saveCollapsedLanes(project: string, grouping: string, ids: readonly string[]): void {
  writePref(`board.collapsedLanes.${project}.${grouping}`, ids);
}
