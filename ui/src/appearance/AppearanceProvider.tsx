// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, use, useEffect, useState, type ReactNode } from "react";

import { apply, DEFAULT_APPEARANCE, readDesktop, resolve, watchDesktop, type Appearance, type Desktop, type Resolved } from "./appearance";

interface AppearanceState {
  appearance: Appearance;
  resolved: Resolved;
  update: (change: Partial<Appearance>) => void;
}

const AppearanceContext = createContext<AppearanceState | null>(null);

interface AppearanceProviderProps {
  children: ReactNode;
  initial?: Appearance;
  /** Called with every change, to keep it for the next start. */
  onChange?: (appearance: Appearance) => void;
}

/**
 * Holds the appearance settings, resolves them against the desktop, and keeps
 * the attributes on `<html>` up to date, also when the desktop switches
 * between light and dark while the app runs. `onChange` stores them.
 */
export function AppearanceProvider({ children, initial = DEFAULT_APPEARANCE, onChange }: AppearanceProviderProps) {
  const [appearance, setAppearance] = useState<Appearance>(initial);
  const [desktop, setDesktop] = useState<Desktop>(() => readDesktop(window));
  const resolved = resolve(appearance, desktop);

  useEffect(() => watchDesktop(window, () => { setDesktop(readDesktop(window)); }), []);

  useEffect(() => {
    apply(resolved, document.documentElement);
  }, [resolved]);

  const update = (change: Partial<Appearance>) => {
    const next = { ...appearance, ...change };
    setAppearance(next);
    onChange?.(next);
  };

  return <AppearanceContext value={{ appearance, resolved, update }}>{children}</AppearanceContext>;
}

export function useAppearance(): AppearanceState {
  const state = use(AppearanceContext);
  if (!state) {
    throw new Error("useAppearance needs an AppearanceProvider");
  }
  return state;
}
