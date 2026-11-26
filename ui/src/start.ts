// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/** Which title bar the window has: the app's own header bar, or the system's. */
export type TitleBar = "app" | "system";

/** What the app hands the page before its first frame. */
export interface StartData {
  titleBar: TitleBar;
  /** The user's locale for dates and numbers, as a BCP 47 tag. */
  locale: string;
  /** In a task window: its task, shown alone. */
  task?: string;
  /** In a wiki window: its page's ID, the page shown alone. */
  wiki?: string;
  /** A task to open first, from the link the app was started with. */
  open?: string;
  /** The quick-add window: its page shows the quick-add field alone. */
  quickAdd?: boolean;
  /** The workspace was made at this start: the main window offers the first-run set-up. */
  firstRun?: boolean;
}

declare global {
  interface Window {
    /** Set by the app's initialization script; absent in a plain browser. */
    readonly __GLYPHKEEL_START__?: { titleBar?: unknown; locale?: unknown; task?: unknown; wiki?: unknown; open?: unknown; prefs?: unknown; quickAdd?: unknown; firstRun?: unknown };
  }
}

/**
 * Reads the start data, with defaults for whatever is missing: in a plain
 * browser (tests, the development server) the app's header bar and the
 * browser's own locale.
 */
export function readStart(win: Window = window): StartData {
  const given = win.__GLYPHKEEL_START__ ?? {};
  const titleBar: TitleBar = given.titleBar === "system" ? "system" : "app";
  const locale = firstLocale(given.locale, (win.navigator as Navigator | undefined)?.language);
  const start: StartData = { titleBar, locale };
  if (isTaskKey(given.task)) {
    start.task = given.task;
  }
  if (isPageId(given.wiki)) {
    start.wiki = given.wiki;
  }
  if (isTaskKey(given.open)) {
    start.open = given.open;
  }
  if (given.quickAdd === true) {
    start.quickAdd = true;
  }
  // Outside the app, `?first-run` asks for the set-up, for work on it and its browser flows.
  if (given.firstRun === true || (win.__GLYPHKEEL_START__ === undefined && new URLSearchParams((win.location as Location | undefined)?.search ?? "").has("first-run"))) {
    start.firstRun = true;
  }
  return start;
}

/** A task key such as `TM-41`. */
export function isTaskKey(value: unknown): value is string {
  return typeof value === "string" && /^[A-Z][A-Z0-9]{1,9}-\d{1,12}$/.test(value);
}

/** A wiki page's ID as a window names it: letters, digits and hyphens. */
export function isPageId(value: unknown): value is string {
  return typeof value === "string" && /^[0-9A-Za-z-]{1,64}$/.test(value);
}

/** The project key of a task key: `TM` of `TM-41`. */
export function projectOfKey(key: string): string {
  return key.slice(0, key.lastIndexOf("-"));
}

/**
 * The first of the given tags that Intl accepts, or `en`. Under `LANG=C` the
 * web view reports a language such as `c`, which every Intl formatter rejects;
 * `POSIX` passes Intl but names no language.
 */
function firstLocale(...tags: unknown[]): string {
  for (const tag of tags) {
    if (typeof tag === "string" && !/^(c|posix)([._@-]|$)/i.test(tag) && isLocale(tag)) {
      return tag;
    }
  }
  return "en";
}

function isLocale(tag: string): boolean {
  try {
    return Intl.getCanonicalLocales(tag).length === 1;
  } catch {
    return false;
  }
}
