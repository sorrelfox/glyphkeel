// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's own records, shaped like the workspace's tables.

import { type BoardLayout, type Category, type Glyph, type Hue, type Modules, type Priority, type Settings } from "../../generated/ops";

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
  /** Sample data for the files and comments to come. */
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

/** Sample links between tasks, for when tasks show them. */
export interface FakeRelation {
  from: string;
  to: string;
  kind: "blocks" | "relates" | "duplicates" | "clones";
}

export interface FakeState {
  projects: FakeProject[];
  tasks: FakeTask[];
  relations: FakeRelation[];
  /** Board layouts by project ID. */
  layouts: Record<string, BoardLayout>;
  settings: Settings;
  workspaceName: string;
  seq: number;
}
