// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's own records, shaped like the workspace's tables.

import {
  type ActivityEntry,
  type Actor,
  type ProjectIcon,
  type BoardLayout,
  type GanttLayout,
  type ListLayout,
  type Category,
  type CommentRevision,
  type FileKind,
  type Glyph,
  type Hue,
  type Modules,
  type Priority,
  type ProjectBlueprint,
  type RecurrenceRule,
  type RevisionKind,
  type TaskBlueprint,
  type TemplateKind,
  type Backup,
  type SavedView,
  type Settings,
  type TrashEntry,
  type Via,
} from "../../generated/ops";

export interface FakeStatus {
  id: string;
  name: string;
  category: Category;
  hue: Hue;
  wip_limit: number | null;
}

export interface FakeType {
  id: string;
  name: string;
  glyph: Glyph;
  hue: Hue;
  is_default: boolean;
  /** New tasks of this type are due this many days after they are made. */
  due_offset_days?: number;
}

export interface FakeLabel {
  id: string;
  name: string;
  hue: Hue;
}

export interface FakeProject {
  id: string;
  key: string;
  name: string;
  hue: Hue;
  /** No icon: the circle. */
  icon?: ProjectIcon | undefined;
  /** The parent's ID. */
  parent: string | null;
  /** Order among its siblings. */
  rank: number;
  description: string;
  state: "active" | "archived";
  modules: Modules;
  /** In the project's order. */
  statuses: FakeStatus[];
  types: FakeType[];
  labels: FakeLabel[];
  nextNumber: number;
  /** Keys it had before a new key; they still find it and its tasks. */
  oldKeys?: string[];
}

export interface FakeTask {
  id: string;
  projectId: string;
  number: number;
  /** Keys it had in other projects before it moved; they still find it. */
  oldKeys?: string[];
  title: string;
  description: string;
  statusId: string;
  typeId: string;
  priority: Priority;
  labelIds: string[];
  parentId: string | null;
  /** Order among its parent's subtasks, whatever their statuses; set while it has a parent. */
  subtaskRank?: number | undefined;
  /** Order inside its status. */
  rank: number;
  startDate: string | null;
  dueDate: string | null;
  estimateMin: number | null;
  spentMin: number;
  /** How many sample files and comments it starts with; made into records when the workspace opens. */
  files: number;
  comments: number;
  createdAt: string;
  editedAt: string;
  statusEnteredAt: string;
  updatedAt: string;
  /** Who changed it last; the engine doesn't report it on cards yet. */
  updatedBy: { kind: "user" | "agent"; name: string };
  closedAt: string | null;
  version: number;
  /** The recurring series it is an occurrence of. */
  seriesId?: string;
}

/** A recurring task's series: its rule and the details each new occurrence starts from. */
export interface FakeSeries {
  id: string;
  projectId: string;
  rule: RecurrenceRule;
  pattern: TaskBlueprint;
  /** The current occurrence's ID. */
  current: string | null;
  /** Made ahead: the date of the next occurrence to make. */
  nextDate: string | null;
  /** Occurrences made or passed over, the first included. */
  made: number;
  endedAt: string | null;
  createdAt: string;
}

/** A task template (of a project) or a project template (of the workspace). */
export interface FakeTemplate {
  id: string;
  kind: TemplateKind;
  projectId: string | null;
  name: string;
  description: string;
  task?: TaskBlueprint;
  setup?: ProjectBlueprint;
  rank: number;
  createdAt: string;
  updatedAt: string;
}

/** A link between two tasks, stored the way it reads from `from`. */
export interface FakeRelation {
  id: string;
  from: string;
  to: string;
  kind: "blocks" | "relates" | "duplicates" | "clones";
}

export interface FakeComment {
  id: string;
  taskId: string;
  body: string;
  author: Actor;
  via: Via;
  createdAt: string;
  editedAt: string | null;
  version: number;
  /** Earlier texts, newest first. */
  revisions: CommentRevision[];
  replyTo: string | null;
  deletedAt: string | null;
}

export interface FakeFile {
  id: string;
  /** The task's ID; empty for a file on a wiki page. */
  taskId: string;
  /** The wiki page's ID, for a file on a page. */
  pageId?: string;
  commentId: string | null;
  name: string;
  mime: string;
  kind: FileKind;
  size: number;
  /** The content. */
  base64: string;
  width?: number;
  height?: number;
  createdAt: string;
  deletedAt: string | null;
}

/** One saved text of a task's description. */
export interface FakeRevision {
  id: string;
  /** The task's ID, or the wiki page's. */
  taskId: string;
  body: string;
  kind: RevisionKind;
  /** The revision it was edited from. */
  base: string | null;
  author: Actor;
  createdAt: string;
  /** For a conflict copy: when it was resolved. */
  resolvedAt: string | null;
  /** A note saved with it. */
  message?: string;
}

/** An agent's claim on a task. */
export interface FakeClaim {
  taskId: string;
  agent: string;
  session: string;
  until: string;
}

export interface FakeTimeEntry {
  id: string;
  taskId: string;
  /** The day of the work, `YYYY-MM-DD`. */
  date: string;
  minutes: number;
  note: string;
  author: Actor;
  createdAt: string;
  editedAt: string | null;
  version: number;
  deletedAt: string | null;
}

/** The workspace's one timer. */
export interface FakeTimer {
  taskId: string;
  startedAt: string;
  /** When the current running stretch began; null while paused. */
  runningSince: string | null;
  /** Earlier running stretches, in milliseconds. */
  elapsedMs: number;
  note: string;
}

/** A task's personal reminder: a moment, or days before the due date. */
export interface FakeReminder {
  id: string;
  taskId: string;
  at: string | null;
  daysBeforeDue: number | null;
  time: string | null;
  note: string;
  createdAt: string;
  acknowledged: boolean;
}

/** A wiki page; its revisions are kept with the descriptions', under its ID. */
export interface FakeWikiPage {
  id: string;
  projectId: string;
  title: string;
  slug: string;
  /** Slugs it had before a rename, which still find it. */
  oldSlugs?: string[];
  parentId: string | null;
  body: string;
  createdAt: string;
  updatedAt: string;
  /** Whether it is its project's brief for agents. */
  agentBrief?: boolean;
  /** Whether it is its wiki's home page. */
  home?: boolean;
  /** Whether it is a template for new pages. */
  template?: boolean;
}

/** Tasks in the trash: one deletion, with the subtasks that went with it; or wiki pages, with the pages inside. */
export interface FakeTrashed {
  entry: TrashEntry;
  tasks: FakeTask[];
  pages?: FakeWikiPage[];
}

export interface FakeState {
  projects: FakeProject[];
  tasks: FakeTask[];
  relations: FakeRelation[];
  comments: FakeComment[];
  files: FakeFile[];
  claims: FakeClaim[];
  /** Description revisions, oldest first; a task's current one is its latest that isn't a conflict copy. */
  revisions: FakeRevision[];
  /** The activity log, oldest first. */
  activity: ActivityEntry[];
  /** Board layouts by project ID. */
  layouts: Record<string, BoardLayout>;
  /** List layouts by project ID, and `all` for the whole workspace's list; none until one changes. */
  listLayouts?: Record<string, ListLayout>;
  /** Gantt chart layouts by project ID; none until one changes. */
  ganttLayouts?: Record<string, GanttLayout>;
  timeEntries: FakeTimeEntry[];
  timer: FakeTimer | null;
  reminders: FakeReminder[];
  settings: Settings;
  /** Saved views of the list and other screens; none until one is saved. */
  views?: SavedView[];
  /** Wiki pages; none until one is made. */
  wiki?: FakeWikiPage[];
  /** Deleted tasks, newest deletion last; none until one is deleted. */
  trash?: FakeTrashed[];
  /** Recurring series; none until a task repeats. */
  series?: FakeSeries[];
  /** Templates; none until one is made. */
  templates?: FakeTemplate[];
  /** Backups, newest first; none until one is made. */
  backups?: Backup[];
  workspaceName: string;
  seq: number;
}
