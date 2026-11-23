// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Sample content for the gallery: what a user's own data could look like.
 * It illustrates the components and never reaches the app's screens.
 */

export const HUES = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"] as const;

export const SWATCHES = ["bg", "surface", "sunken", "sidebar", "popover", "fg", "fg-muted", "fg-faint", "border", "accent"] as const;

export const PROJECTS = [
  { name: "Personal", hue: "green", count: 5, selected: false },
  { name: "Glyphkeel", hue: "blue", count: 24, selected: true },
] as const;

export const LANE = { name: "In progress", hue: "blue", count: 4, limit: 3 } as const;

export const CARDS = [
  {
    key: "TM-41",
    type: "feature",
    priority: "high",
    title: "Board: personal column order and hidden columns",
    label: { name: "board", hue: "blue" },
    due: { text: "Nov 24", state: "soon" },
    checklist: "5/7",
    selected: true,
  },
  {
    key: "TM-44",
    type: "bug",
    priority: "urgent",
    title: "Dark mode does not follow the desktop setting",
    label: { name: "linux", hue: "orange" },
    due: { text: "Nov 20", state: "overdue" },
    checklist: null,
    selected: false,
  },
] as const;

export const STATUSES = [
  { name: "To do", hue: "grey" },
  { name: "In progress", hue: "blue" },
  { name: "Review", hue: "purple" },
  { name: "Done", hue: "green" },
] as const;

export const VIEWS = ["Board", "List", "Gantt", "Wiki"] as const;

export const TABS = ["Comments", "History", "Time"] as const;

export const TABLE = {
  columns: ["Key", "Title", "Status", "Due"],
  group: "In progress · 2",
  rows: [
    { key: "TM-41", title: "Board: personal column order", status: "In progress", due: "Nov 24", selected: true },
    { key: "TM-38", title: "Query field highlighting", status: "In progress", due: "Nov 27", selected: false },
  ],
} as const;

export const PROPERTIES = [
  { name: "Status", value: "In progress" },
  { name: "Priority", value: "High" },
  { name: "Estimate", value: "3h · 1h 20m logged" },
] as const;

export const MENU = ["Open", "Open in its own window", "Copy link", "Move to…", "Delete"] as const;

export const MARKDOWN = {
  heading: "Release checklist",
  paragraph: "Every change is saved at once. Undo brings it back.",
  link: "See the plan",
  code: ["-- status:open due<+7d", "SELECT t.* FROM task t", "WHERE t.due_date < date('now', 7);"],
} as const;

export const BANNER = "Showing 10 of 42 tasks: label:linux";
