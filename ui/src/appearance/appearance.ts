// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Theme, accent and density: the settings, how they resolve against the
 * desktop, and how they reach the page. The only switch is the three
 * attributes on `<html>`; tokens.css does the rest. They sit on `<html>`, not
 * on the app's root element, because menus, popovers and dialogs are portalled
 * out of the root and must still get the theme.
 */

export const THEMES = ["light", "dark", "slate", "sand", "contrast"] as const;
export const ACCENTS = ["blue", "teal", "green", "amber", "orange", "red", "pink", "purple", "slate"] as const;
export const DENSITIES = ["regular", "compact"] as const;

export type Theme = (typeof THEMES)[number];
export type Accent = (typeof ACCENTS)[number];
export type Density = (typeof DENSITIES)[number];

/** The accent a theme suggests when the accent follows the desktop and the desktop has none. */
export const THEME_ACCENT: Readonly<Record<Theme, Accent>> = {
  light: "blue",
  dark: "blue",
  slate: "teal",
  sand: "orange",
  contrast: "blue",
};

export interface Appearance {
  theme: "system" | Theme;
  /** The theme used when the theme follows a light desktop. */
  systemLight: "light" | "sand";
  /** The theme used when the theme follows a dark desktop. */
  systemDark: "dark" | "slate";
  accent: "system" | Accent;
  density: Density;
  /** On: no motion. Off: follow the desktop. */
  reduceMotion: boolean;
}

export const DEFAULT_APPEARANCE: Readonly<Appearance> = {
  theme: "system",
  systemLight: "light",
  systemDark: "dark",
  accent: "system",
  density: "regular",
  reduceMotion: false,
};

/** What the desktop asks for. */
export interface Desktop {
  dark: boolean;
  highContrast: boolean;
  reducedMotion: boolean;
  /** The desktop's accent, mapped to the nearest named accent, if it has one. */
  accent: Accent | null;
}

export interface Resolved {
  theme: Theme;
  accent: Accent;
  density: Density;
  reducedMotion: boolean;
}

export function resolve(appearance: Appearance, desktop: Desktop): Resolved {
  let theme: Theme;
  if (appearance.theme !== "system") {
    theme = appearance.theme;
  } else if (desktop.highContrast) {
    theme = "contrast";
  } else {
    theme = desktop.dark ? appearance.systemDark : appearance.systemLight;
  }
  const accent = appearance.accent !== "system" ? appearance.accent : (desktop.accent ?? THEME_ACCENT[theme]);
  return {
    theme,
    accent,
    density: appearance.density,
    reducedMotion: appearance.reduceMotion || desktop.reducedMotion,
  };
}

/** Writes the resolved appearance onto the root element. */
export function apply(resolved: Resolved, root: HTMLElement): void {
  root.dataset.theme = resolved.theme;
  root.dataset.accent = resolved.accent;
  root.dataset.density = resolved.density;
  if (resolved.reducedMotion) {
    root.style.setProperty("--dur-fast", "0ms");
    root.style.setProperty("--dur", "0ms");
  } else {
    root.style.removeProperty("--dur-fast");
    root.style.removeProperty("--dur");
  }
}

/**
 * Whether motion is off: the setting or the desktop asks for it. For motion
 * that scripts run themselves (drag and drop); CSS follows the duration
 * tokens. It reads the root's own style, which `apply` writes, so it is cheap.
 */
export function motionReduced(root: HTMLElement = document.documentElement): boolean {
  return root.style.getPropertyValue("--dur") === "0ms";
}

const QUERIES = {
  dark: "(prefers-color-scheme: dark)",
  highContrast: "(prefers-contrast: more)",
  reducedMotion: "(prefers-reduced-motion: reduce)",
} as const;

/**
 * Reads the desktop's wishes from the web view's media queries. The app will
 * pass the desktop's settings in from Rust as well, because the web view
 * doesn't always follow them.
 */
export function readDesktop(win: Window): Desktop {
  const matches = (query: string): boolean => (typeof win.matchMedia === "function" ? win.matchMedia(query).matches : false);
  return {
    dark: matches(QUERIES.dark),
    highContrast: matches(QUERIES.highContrast),
    reducedMotion: matches(QUERIES.reducedMotion),
    accent: null,
  };
}

/** Calls `onChange` whenever one of the desktop's wishes changes; returns the unsubscribe function. */
export function watchDesktop(win: Window, onChange: () => void): () => void {
  if (typeof win.matchMedia !== "function") {
    return () => undefined;
  }
  const lists = Object.values(QUERIES).map((query) => win.matchMedia(query));
  for (const list of lists) {
    list.addEventListener("change", onChange);
  }
  return () => {
    for (const list of lists) {
      list.removeEventListener("change", onChange);
    }
  };
}
