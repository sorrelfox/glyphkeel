// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The operations on projects, tasks and the board, with their inputs and
// outputs, as the screens use them. The generated client doesn't list them
// yet; once it does, these types give way to the generated ones and
// `AppOpMap` becomes the generated `OpMap`.

import { type OpMap } from "../generated/ops";

/** The nine hues of the design. */
export const HUES = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"] as const;
export type Hue = (typeof HUES)[number];

/** What a status means across projects. */
export const STATUS_CATEGORIES = ["todo", "in_progress", "done", "cancelled"] as const;
export type StatusCategory = (typeof STATUS_CATEGORIES)[number];

/** The shapes of task types. */
export const GLYPHS = ["task", "bug", "feature", "milestone"] as const;
export type Glyph = (typeof GLYPHS)[number];

/** Priorities, lowest first. */
export const PRIORITIES = ["none", "low", "normal", "high", "urgent"] as const;
export type Priority = (typeof PRIORITIES)[number];

/** The parts of a project that can be switched off. */
export const MODULES = ["board", "list", "gantt", "wiki", "time"] as const;
export type Module = (typeof MODULES)[number];

export interface Status {
  id: string;
  name: string;
  category: StatusCategory;
  hue: Hue;
  /** The most tasks the column should hold; a warning, never a block. */
  wip_limit: number | null;
}

export interface TaskType {
  id: string;
  name: string;
  glyph: Glyph;
  hue: Hue;
  is_default: boolean;
}

export interface Label {
  id: string;
  name: string;
  hue: Hue;
}

/** A project as the sidebar lists it. */
export interface ProjectSummary {
  id: string;
  key: string;
  name: string;
  hue: Hue;
  /** The parent project's key; null at the top level. */
  parent: string | null;
  state: "active" | "archived";
  /** Open tasks of this project alone. */
  open: number;
}

/** A project with its settings. */
export interface Project extends ProjectSummary {
  description: string;
  modules: Record<Module, boolean>;
  statuses: Status[];
  types: TaskType[];
  labels: Label[];
  /** Keys of the direct sub-projects, in order. */
  children: string[];
}

export interface ProjectList {
  /** Every project in tree order: each parent before its children. */
  items: ProjectSummary[];
  total: number;
}

/** Who changed a task last. */
export interface LastChange {
  at: string;
  actor: { kind: "user" | "agent"; name: string };
}

/** A task on the board: the fields a card shows. */
export interface Card {
  id: string;
  key: string;
  project: string;
  title: string;
  status: string;
  type: Glyph;
  type_name: string;
  priority: Priority;
  labels: Label[];
  due_date: string | null;
  checklist: { done: number; total: number } | null;
  subtasks: { done: number; total: number } | null;
  files: number;
  comments: number;
  spent_min: number;
  blocked: boolean;
  closed_at: string | null;
  updated: LastChange;
}

/**
 * A board column. Alone, a project's columns are its statuses. With its
 * sub-projects, columns are matched by status name, and a status that only a
 * sub-project has becomes an extra column at the end.
 */
export interface BoardColumn {
  /** The status ID of the board's own project, or `name:<folded name>` for an extra column. */
  id: string;
  name: string;
  category: StatusCategory;
  hue: Hue;
  wip_limit: number | null;
  /** The status this column stands for in each project on the board. */
  statuses: Record<string, string>;
}

export interface Board {
  project: string;
  include_subprojects: boolean;
  columns: BoardColumn[];
  /** Every card, each column's cards in their shared order. */
  cards: Card[];
  /** The column of each card, by card key. */
  column_of: Record<string, string>;
}

export const SORT_MODES = ["manual", "priority", "due", "updated"] as const;
export type SortMode = (typeof SORT_MODES)[number];

export const CARD_FIELDS = ["labels", "due", "checklist", "subtasks", "files", "comments", "spent"] as const;
export type CardField = (typeof CARD_FIELDS)[number];

/** The personal layout of one project's board. */
export interface BoardLayout {
  /** Column IDs in the user's order; columns not listed keep their project position. */
  order: string[];
  hidden: string[];
  collapsed: string[];
  sort_mode: SortMode;
  /** Columns sorted differently from the board. */
  column_sort: Record<string, SortMode>;
  hide_done_after_days: number;
  card_fields: CardField[];
  card_size: "regular" | "compact";
  include_subprojects: boolean;
}

export const DEFAULT_LAYOUT: Readonly<BoardLayout> = {
  order: [],
  hidden: [],
  collapsed: [],
  sort_mode: "manual",
  column_sort: {},
  hide_done_after_days: 14,
  card_fields: ["labels", "due", "checklist", "subtasks", "files", "comments", "spent"],
  card_size: "regular",
  include_subprojects: false,
};

/** Where an item goes among its siblings. */
export type Position = { before: string } | { after: string } | "top" | "bottom";

export interface TaskRef {
  key: string;
  title: string;
  status: { name: string; category: StatusCategory; hue: Hue };
}

export interface Relation {
  kind: "blocks" | "blocked_by" | "relates" | "duplicates" | "duplicated_by" | "clones" | "cloned_by";
  task: TaskRef;
}

/** A task with all its fields. */
export interface Task {
  id: string;
  key: string;
  project: string;
  title: string;
  description: string;
  status: Status;
  type: TaskType;
  priority: Priority;
  labels: Label[];
  parent: TaskRef | null;
  subtasks: TaskRef[];
  relations: Relation[];
  start_date: string | null;
  due_date: string | null;
  estimate_min: number | null;
  spent_min: number;
  checklist: { done: number; total: number };
  blocked: boolean;
  created_at: string;
  edited_at: string;
  updated: LastChange;
  closed_at: string | null;
  version: number;
}

/** A change to a task, and the history group that holds it. */
export interface TaskChange {
  task: Task;
  group: string;
}

export interface TaskPatch {
  title?: string;
  description?: string;
  type?: string;
  priority?: Priority;
  labels?: { set?: string[]; add?: string[]; remove?: string[] };
  start_date?: string | null;
  due_date?: string | null;
  estimate_min?: number | null;
}

export interface ProjectPatch {
  name?: string;
  description?: string;
  hue?: Hue;
  modules?: Partial<Record<Module, boolean>>;
}

export interface StatusPatch {
  name?: string;
  category?: StatusCategory;
  hue?: Hue;
  wip_limit?: number | null;
}

export interface ProjectChange {
  project: Project;
  group: string;
}

export interface Counts {
  today: number;
  all: number;
  /** Open tasks by project key. */
  projects: Record<string, number>;
}

type Query<I, O> = { input: I; output: O; kind: "query" };
type Command<I, O> = { input: I; output: O; kind: "command" };
type ProjectRef = { project: string };

/** The operations of this file, as the generated `OpMap` will list them. */
export interface BoardOpMap {
  "workspace.counts": Query<Record<string, never>, Counts>;
  "project.list": Query<{ include_archived?: boolean }, ProjectList>;
  "project.get": Query<ProjectRef, Project>;
  "project.create": Command<{ name: string; key: string; parent?: string | null; hue?: Hue }, ProjectChange>;
  "project.update": Command<ProjectRef & { patch: ProjectPatch }, ProjectChange>;
  "project.move": Command<ProjectRef & { parent: string | null; position?: Position }, ProjectChange>;
  "project.archive": Command<ProjectRef, ProjectChange>;
  "project.unarchive": Command<ProjectRef, ProjectChange>;
  "status.create": Command<ProjectRef & { name: string; category: StatusCategory; hue?: Hue; wip_limit?: number | null }, ProjectChange>;
  "status.update": Command<ProjectRef & { status: string; patch: StatusPatch }, ProjectChange>;
  "status.reorder": Command<ProjectRef & { status: string; position: Position }, ProjectChange>;
  "status.delete": Command<ProjectRef & { status: string; move_tasks_to: string }, ProjectChange>;
  "type.create": Command<ProjectRef & { name: string; glyph: Glyph; hue?: Hue }, ProjectChange>;
  "type.update": Command<ProjectRef & { type: string; patch: { name?: string; glyph?: Glyph; hue?: Hue } }, ProjectChange>;
  "type.delete": Command<ProjectRef & { type: string; move_tasks_to: string }, ProjectChange>;
  "label.create": Command<ProjectRef & { name: string; hue?: Hue }, ProjectChange>;
  "label.update": Command<ProjectRef & { label: string; patch: { name?: string; hue?: Hue } }, ProjectChange>;
  "label.delete": Command<ProjectRef & { label: string }, ProjectChange>;
  "board.get": Query<ProjectRef & { include_subprojects?: boolean }, Board>;
  "layout.get": Query<ProjectRef & { view: "board" }, BoardLayout>;
  "layout.update": Command<ProjectRef & { view: "board"; patch?: Partial<BoardLayout>; reset?: boolean }, BoardLayout>;
  "task.get": Query<{ key: string }, Task>;
  "task.create": Command<
    ProjectRef & { title: string; status?: string; position?: Position; client_request_id?: string },
    TaskChange
  >;
  "task.update": Command<{ key: string; expected_version?: number; patch: TaskPatch }, TaskChange>;
  "task.move": Command<{ key: string; status?: string; position?: Position }, TaskChange>;
}

type Generated = { [K in keyof OpMap]: Pick<OpMap[K], "input" | "output" | "kind"> };

/** Every operation the UI calls. */
export type AppOpMap = Generated & BoardOpMap;
export type AppOpId = keyof AppOpMap;
