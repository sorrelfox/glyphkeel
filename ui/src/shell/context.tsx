// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, use, useEffect, useRef } from "react";

/** What the screen in the window tells the status bar. */
export interface StatusInfo {
  /** Counts for the view: "42 tasks · 24 open". */
  summary: string;
  /** The selection: "TM-41 selected". */
  selection?: string | undefined;
  /** The result of the last action, also read out by screen readers. */
  note?: string | undefined;
}

export interface ShellHooks {
  setStatus: (info: StatusInfo | null) => void;
  /** What New task does on the current screen; null for the default. */
  setNewTask: (run: (() => void) | null) => void;
}

export const ShellContext = createContext<ShellHooks>({ setStatus: () => undefined, setNewTask: () => undefined });

/** Shows the screen's counts and selection in the status bar while it is on screen. */
export function useStatus(info: StatusInfo): void {
  const { setStatus } = use(ShellContext);
  const { summary, selection, note } = info;
  useEffect(() => {
    setStatus({ summary, selection, note });
  }, [setStatus, summary, selection, note]);
  useEffect(() => () => {
    setStatus(null);
  }, [setStatus]);
}

/** Makes New task (`N`, the header bar's button) run `run` while the screen is on screen. */
export function useNewTask(run: () => void): void {
  const { setNewTask } = use(ShellContext);
  const latest = useRef(run);
  useEffect(() => {
    latest.current = run;
  });
  useEffect(() => {
    setNewTask(() => {
      latest.current();
    });
    return () => {
      setNewTask(null);
    };
  }, [setNewTask]);
}
