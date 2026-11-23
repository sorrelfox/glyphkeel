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
  /** A task to open first, from the link the app was started with. */
  open?: string;
}

declare global {
  interface Window {
    /** Set by the app's initialization script; absent in a plain browser. */
    readonly __GLYPHKEEL_START__?: { titleBar?: unknown; locale?: unknown; task?: unknown; open?: unknown };
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
  const locale = typeof given.locale === "string" && isLocale(given.locale) ? given.locale : win.navigator.language;
  const start: StartData = { titleBar, locale };
  if (isTaskKey(given.task)) {
    start.task = given.task;
  }
  if (isTaskKey(given.open)) {
    start.open = given.open;
  }
  return start;
}

/** A task key such as `TM-41`. */
export function isTaskKey(value: unknown): value is string {
  return typeof value === "string" && /^[A-Z][A-Z0-9]{1,9}-\d{1,12}$/.test(value);
}

/** The project key of a task key: `TM` of `TM-41`. */
export function projectOfKey(key: string): string {
  return key.slice(0, key.lastIndexOf("-"));
}

function isLocale(tag: string): boolean {
  try {
    return Intl.getCanonicalLocales(tag).length === 1;
  } catch {
    return false;
  }
}
