// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, use, useEffect, useRef } from "react";

import { type Action } from "../actions/actions";

/** What the screen in the window tells the status bar. */
export interface StatusInfo {
  /** Counts for the view: "42 tasks · 24 open". */
  summary: string;
  /** The selection: "TM-41 selected". */
  selection?: string | undefined;
  /** The result of the last action, also read out by screen readers. */
  note?: string | undefined;
  /** A count that needs attention, shown in the danger colour: "3 overdue". */
  alert?: string | undefined;
}

export interface ShellHooks {
  setStatus: (info: StatusInfo | null) => void;
  /** What New task does on the current screen; null for the default. */
  setNewTask: (run: (() => void) | null) => void;
  /** The task `T` starts or stops the timer on: the open or selected one; null for none. */
  setTimerTask: (key: string | null) => void;
  /** The screen's own actions, in the keymap and the command palette while it is on screen. */
  setScreenActions: (actions: readonly Action[]) => void;
}

export const ShellContext = createContext<ShellHooks>({ setStatus: () => undefined, setNewTask: () => undefined, setTimerTask: () => undefined, setScreenActions: () => undefined });

/** Shows the screen's counts and selection in the status bar while it is on screen. */
export function useStatus(info: StatusInfo): void {
  const { setStatus } = use(ShellContext);
  const { summary, selection, note, alert } = info;
  useEffect(() => {
    setStatus({ summary, selection, note, alert });
  }, [setStatus, summary, selection, note, alert]);
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

/** Makes `T` start or stop the timer on task `key` while the screen is on screen; null for none. */
export function useTimerTask(key: string | null): void {
  const { setTimerTask } = use(ShellContext);
  useEffect(() => {
    setTimerTask(key);
  }, [setTimerTask, key]);
  useEffect(() => () => {
    setTimerTask(null);
  }, [setTimerTask]);
}

/**
 * Adds the screen's own actions to the window's while the screen is on
 * screen: the keymap runs them and the command palette lists them. They
 * are registered again only when their IDs or availability change; a run
 * always calls the latest `run`.
 */
export function useScreenActions(actions: readonly Action[]): void {
  const { setScreenActions } = use(ShellContext);
  const latest = useRef(actions);
  useEffect(() => {
    latest.current = actions;
  });
  const signature = actions.map((action) => `${action.id}${action.disabled === true ? "-" : "+"}`).join(" ");
  useEffect(() => {
    setScreenActions(
      latest.current.map((action) => ({
        ...action,
        run: () => {
          latest.current.find((candidate) => candidate.id === action.id)?.run();
        },
      })),
    );
  }, [setScreenActions, signature]);
  useEffect(() => () => {
    setScreenActions([]);
  }, [setScreenActions]);
}
