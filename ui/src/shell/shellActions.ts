// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { keysOf, type Action, type ActionGroup } from "../actions/actions";
import { appWindow } from "../ipc/window";
import { focusRegion } from "./regions";

export type View = "today" | "all-tasks" | "time" | "search" | "project" | "trash" | "activity";

export interface ShellCommands {
  openPalette: () => void;
  /** Shows the quick-add window; null where there is none (a plain browser). */
  quickAdd: (() => void) | null;
  setView: (view: "today" | "all-tasks" | "time" | "trash" | "activity") => void;
  /** Shows the open project as a board, a list, a calendar or the wiki; null outside a project. */
  projectView: ((view: "board" | "list" | "calendar" | "wiki") => void) | null;
  toggleSidebar: () => void;
  showShortcuts: () => void;
  openPreferences: () => void;
  /** New task on the current screen; null where there is nowhere to put one. */
  newTask: (() => void) | null;
  /** Starts or stops the timer on the open or selected task; null without one. */
  toggleTimer: (() => void) | null;
  undo: () => void;
  redo: () => void;
  /** Makes a backup now. */
  backUp: () => void;
  /** Opens the export dialog. */
  exportData: () => void;
  /** Opens the import dialog. */
  importData: () => void;
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
    action("palette.open", "action-palette", "navigation", commands.openPalette),
    action("nav.today", "action-today", "navigation", () => { commands.setView("today"); }),
    action("nav.allTasks", "action-all-tasks", "navigation", () => { commands.setView("all-tasks"); }),
    action("nav.timeReport", "action-time-report", "navigation", () => { commands.setView("time"); }),
    action("nav.activity", "action-activity", "navigation", () => { commands.setView("activity"); }),
    action("nav.trash", "action-trash", "navigation", () => { commands.setView("trash"); }),
    action("view.board", "action-view-board", "navigation", () => commands.projectView?.("board"), commands.projectView ? {} : UNAVAILABLE),
    action("view.list", "action-view-list", "navigation", () => commands.projectView?.("list"), commands.projectView ? {} : UNAVAILABLE),
    action("view.calendar", "action-view-calendar", "navigation", () => commands.projectView?.("calendar"), commands.projectView ? {} : UNAVAILABLE),
    action("view.gantt", "action-view-gantt", "navigation", nothing, UNAVAILABLE),
    action("view.wiki", "action-view-wiki", "navigation", () => commands.projectView?.("wiki"), commands.projectView ? {} : UNAVAILABLE),
    action("sidebar.toggle", "action-toggle-sidebar", "window", commands.toggleSidebar),
    action("focus.nextRegion", "action-next-region", "window", () => { focusRegion(1); }),
    action("focus.previousRegion", "action-previous-region", "window", () => { focusRegion(-1); }),
    action("task.new", "action-new-task", "tasks", commands.newTask ?? nothing, commands.newTask ? {} : UNAVAILABLE),
    action("timer.toggle", "action-timer-toggle", "tasks", commands.toggleTimer ?? nothing, commands.toggleTimer ? {} : UNAVAILABLE),
    action("app.quickAdd", "action-quick-add", "tasks", commands.quickAdd ?? nothing, commands.quickAdd ? {} : UNAVAILABLE),
    // Inside a text field, the field's own undo comes first.
    action("app.undo", "action-undo", "app", commands.undo, { inText: false }),
    action("app.redo", "action-redo", "app", commands.redo, { inText: false }),
    action("app.backup", "action-backup", "app", commands.backUp),
    action("app.import", "action-import", "app", commands.importData),
    action("app.export", "action-export", "app", commands.exportData),
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
