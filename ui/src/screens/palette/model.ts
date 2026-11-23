// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What the command palette lists for what was typed: the pure part, without
// React, so it can be tested on its own.

/** Which results the palette shows; `Tab` cycles them. */
export const SCOPES = ["all", "tasks", "projects", "actions"] as const;
export type Scope = (typeof SCOPES)[number];

/** The scope after `scope`, or before it with `back`. */
export function nextScope(scope: Scope, back = false): Scope {
  const at = SCOPES.indexOf(scope);
  const step = back ? SCOPES.length - 1 : 1;
  return SCOPES[(at + step) % SCOPES.length] ?? "all";
}

/** The actions listed while the field is empty, in this order, when registered. */
export const COMMON_ACTIONS = [
  "task.new",
  "app.quickAdd",
  "nav.today",
  "nav.allTasks",
  "timer.toggle",
  "sidebar.toggle",
  "app.preferences",
  "help.shortcuts",
] as const;

/** The palette never lists itself. */
const HIDDEN_ACTIONS = new Set(["palette.open"]);

/** Where `query` is found in `text`, ignoring case: start and end, or null. */
export function findMatch(text: string, query: string): { start: number; end: number } | null {
  const needle = query.trim().toLocaleLowerCase();
  if (needle === "") {
    return null;
  }
  const start = text.toLocaleLowerCase().indexOf(needle);
  return start < 0 ? null : { start, end: start + needle.length };
}

/** How well `text` matches: at its start 0, at a word's start 1, inside a word 2, not at all null. */
function matchRank(text: string, query: string): number | null {
  const found = findMatch(text, query);
  if (!found) {
    return null;
  }
  if (found.start === 0) {
    return 0;
  }
  return /[\s\-/(]/.test(text.charAt(found.start - 1)) ? 1 : 2;
}

export interface NamedAction {
  id: string;
  /** Its name, already translated. */
  name: string;
}

/**
 * The actions for `query`: with an empty field the common ones; otherwise
 * every action whose name has the text, those starting with it first, then
 * in the registry's order.
 */
export function matchActions<T extends NamedAction>(actions: readonly T[], query: string): T[] {
  const shown = actions.filter((action) => !HIDDEN_ACTIONS.has(action.id));
  if (query.trim() === "") {
    return COMMON_ACTIONS.map((id) => shown.find((action) => action.id === id)).filter((action): action is T => action !== undefined);
  }
  return shown
    .map((action, order) => ({ action, order, rank: matchRank(action.name, query) }))
    .filter((entry): entry is { action: T; order: number; rank: number } => entry.rank !== null)
    .sort((a, b) => a.rank - b.rank || a.order - b.order)
    .map((entry) => entry.action);
}

export interface NamedProject {
  key: string;
  name: string;
  /** The name with its parents': `Task Manager › CLI`. */
  path: string;
}

/**
 * The projects for `query`: the one whose key it is first, then those whose
 * name or path has the text. With an empty field, none.
 */
export function matchProjects<T extends NamedProject>(projects: readonly T[], query: string, limit = 5): T[] {
  const text = query.trim();
  if (text === "") {
    return [];
  }
  return projects
    .map((project, order) => {
      const rank = project.key.toLowerCase() === text.toLowerCase() ? -1 : (matchRank(project.name, text) ?? matchRank(project.path, text));
      return { project, order, rank };
    })
    .filter((entry): entry is { project: T; order: number; rank: number } => entry.rank !== null)
    .sort((a, b) => a.rank - b.rank || a.order - b.order)
    .slice(0, limit)
    .map((entry) => entry.project);
}

/** The task key typed, in capitals (`tm-41` → `TM-41`), or null. */
export function typedKey(query: string): string | null {
  const text = query.trim();
  return /^[a-z][a-z0-9]{1,9}-\d{1,12}$/i.test(text) ? text.toUpperCase() : null;
}

/** Something opened from the palette, shown again while its field is empty. */
export interface Recent {
  kind: "task" | "project";
  key: string;
  title: string;
}

/** How many recent items the palette keeps. */
export const RECENT_KEPT = 6;

/** `recent` with `item` first and without its older entry. */
export function remember(recent: readonly Recent[], item: Recent): Recent[] {
  return [item, ...recent.filter((each) => each.kind !== item.kind || each.key !== item.key)].slice(0, RECENT_KEPT);
}

/** The recent items as stored; anything malformed is dropped. */
export function recentFrom(stored: unknown): Recent[] {
  if (!Array.isArray(stored)) {
    return [];
  }
  return stored
    .filter(
      (item): item is Recent =>
        typeof item === "object" &&
        item !== null &&
        ((item as Recent).kind === "task" || (item as Recent).kind === "project") &&
        typeof (item as Recent).key === "string" &&
        typeof (item as Recent).title === "string",
    )
    .slice(0, RECENT_KEPT);
}
