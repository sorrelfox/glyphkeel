// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's own records, shaped like the workspace's tables.

import {
  type ActivityEntry,
  type Actor,
  type BoardLayout,
  type Category,
  type CommentRevision,
  type FileKind,
  type Glyph,
  type Hue,
  type Modules,
  type Priority,
  type RevisionKind,
  type Settings,
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
}

export interface FakeTask {
  id: string;
  projectId: string;
  number: number;
  title: string;
  description: string;
  statusId: string;
  typeId: string;
  priority: Priority;
  labelIds: string[];
  parentId: string | null;
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
  taskId: string;
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
  taskId: string;
  body: string;
  kind: RevisionKind;
  /** The revision it was edited from. */
  base: string | null;
  author: Actor;
  createdAt: string;
  /** For a conflict copy: when it was resolved. */
  resolvedAt: string | null;
}

/** An agent's claim on a task. */
export interface FakeClaim {
  taskId: string;
  agent: string;
  session: string;
  until: string;
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
  settings: Settings;
  workspaceName: string;
  seq: number;
}
