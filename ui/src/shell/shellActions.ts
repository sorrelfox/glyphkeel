// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { keysOf, type Action, type ActionGroup } from "../actions/actions";
import { appWindow } from "../ipc/window";
import { focusRegion } from "./regions";

export type View = "today" | "all-tasks" | "time" | "project";

export interface ShellCommands {
  setView: (view: "today" | "all-tasks" | "time") => void;
  toggleSidebar: () => void;
  showShortcuts: () => void;
  openPreferences: () => void;
  /** New task on the current screen; null where there is nowhere to put one. */
  newTask: (() => void) | null;
  /** Starts or stops the timer on the open or selected task; null without one. */
  toggleTimer: (() => void) | null;
  undo: () => void;
  redo: () => void;
  /** Opens the development gallery; absent in release builds. */
  openGallery?: (() => void) | undefined;
}

function action(id: string, label: string, group: ActionGroup, run: () => void, extra: Partial<Action> = {}): Action {
  return { id, label, group, keys: keysOf(id), run, ...extra };
}

/** Not available yet: the screens behind them come later. */
const UNAVAILABLE = { disabled: true } as const;
const nothing = () => undefined;

/** The window's actions, with their default keys. */
export function shellActions(commands: ShellCommands): Action[] {
  const actions = [
    action("palette.open", "action-palette", "navigation", nothing, UNAVAILABLE),
    action("nav.today", "action-today", "navigation", () => { commands.setView("today"); }),
    action("nav.allTasks", "action-all-tasks", "navigation", () => { commands.setView("all-tasks"); }),
    action("nav.timeReport", "action-time-report", "navigation", () => { commands.setView("time"); }),
    action("sidebar.toggle", "action-toggle-sidebar", "window", commands.toggleSidebar),
    action("focus.nextRegion", "action-next-region", "window", () => { focusRegion(1); }),
    action("focus.previousRegion", "action-previous-region", "window", () => { focusRegion(-1); }),
    action("task.new", "action-new-task", "tasks", commands.newTask ?? nothing, commands.newTask ? {} : UNAVAILABLE),
    action("timer.toggle", "action-timer-toggle", "tasks", commands.toggleTimer ?? nothing, commands.toggleTimer ? {} : UNAVAILABLE),
    // Inside a text field, the field's own undo comes first.
    action("app.undo", "action-undo", "app", commands.undo, { inText: false }),
    action("app.redo", "action-redo", "app", commands.redo, { inText: false }),
    action("app.backup", "action-backup", "app", nothing, UNAVAILABLE),
    action("app.import", "action-import", "app", nothing, UNAVAILABLE),
    action("app.export", "action-export", "app", nothing, UNAVAILABLE),
    action("help.shortcuts", "action-shortcuts", "app", commands.showShortcuts),
    action("app.preferences", "action-preferences", "app", commands.openPreferences),
    action("app.about", "action-about", "app", nothing, UNAVAILABLE),
    action("window.close", "action-close-window", "window", () => void appWindow.close()),
    // Quitting ends the app, also when it would keep running in the background.
    action("app.quit", "action-quit", "app", () => void appWindow.quit()),
  ];
  if (commands.openGallery) {
    actions.push(action("dev.gallery", "action-gallery", "app", commands.openGallery));
  }
  return actions;
}
