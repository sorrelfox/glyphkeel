// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The engine's types as the screens use them. Every shape comes from the
// generated client; this file only adds the lists a screen offers to choose
// from, and the few settings that live in the UI alone.

import { type CardSize } from "../prefs/store";
import {
  type Category,
  type Edge,
  type Glyph,
  type ProjectIcon,
  type Hue,
  type Modules,
  type Priority,
  type SortMode,
} from "../generated/ops";

export type {
  Board,
  BoardLayout,
  Card,
  Category,
  Column,
  Edge,
  Glyph,
  Hue,
  Label,
  LabelRef,
  Modules,
  Priority,
  Project,
  ProjectChange,
  ProjectSummary,
  SortMode,
  Status,
  Task,
  TaskChange,
  TaskField,
  TaskType,
  TaskUpdateInput,
} from "../generated/ops";
export type { CardSize };

/** The nine hues of the design. */
export const HUES = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"] as const satisfies readonly Hue[];

/**
 * The colour a new label (or project) gets when none is given, as the
 * engine picks it: the first hue in palette order that `used` doesn't
 * contain, else the least-used one, the earlier on a tie.
 */
export function suggestHue(used: readonly Hue[]): Hue {
  let best: Hue = HUES[0];
  let fewest = Number.POSITIVE_INFINITY;
  for (const hue of HUES) {
    const count = used.filter((each) => each === hue).length;
    if (count < fewest) {
      best = hue;
      fewest = count;
    }
  }
  return best;
}

/** The icons a project can have instead of its circle, in the picker's order. */
export const PROJECT_ICONS = [
  "folder", "star", "bug", "rocket", "book", "house", "briefcase", "code", "terminal", "globe",
  "heart", "flag", "zap", "target", "lightbulb", "wrench", "hammer", "palette", "music", "camera",
  "graduation-cap", "shopping-cart", "wallet", "chart-line", "calendar", "users", "building", "plane", "leaf", "coffee",
  "gamepad", "puzzle", "shield", "flask-conical", "layers", "package", "compass", "mountain", "sparkles", "database",
] as const satisfies readonly ProjectIcon[];

/** What a status means across projects. */
export const STATUS_CATEGORIES = ["todo", "in_progress", "done", "cancelled"] as const satisfies readonly Category[];

/** The shapes of task types. */
export const GLYPHS = ["task", "bug", "feature", "milestone", "epic", "story"] as const satisfies readonly Glyph[];

/** Priorities, lowest first. */
export const PRIORITIES = ["none", "low", "normal", "high", "urgent"] as const satisfies readonly Priority[];

/** The parts of a project that can be switched off. */
export const MODULES = ["board", "list", "gantt", "wiki", "time"] as const satisfies readonly (keyof Modules)[];
export type Module = (typeof MODULES)[number];

export const SORT_MODES = ["manual", "priority", "due", "updated"] as const satisfies readonly SortMode[];

/**
 * The card fields the board offers. The engine knows a few more (key, type,
 * priority, updated); a card always shows the first three.
 */
export const CARD_FIELDS = ["labels", "due", "start", "checklist", "subtasks", "estimate", "days_in_column"] as const;
export type CardField = (typeof CARD_FIELDS)[number];

/**
 * Where an item goes among its siblings: before or after one of them, or at
 * an end. The engine takes these as flat fields of the input.
 */
export type Place = { before: string } | { after: string } | { at: Edge };
