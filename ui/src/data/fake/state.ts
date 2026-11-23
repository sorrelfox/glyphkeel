// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's own records, shaped like the workspace's tables.

import { type Settings } from "../../generated/ops";
import { type BoardLayout, type Hue, type Label, type Module, type Priority, type Status, type TaskType } from "../catalog";

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
  modules: Record<Module, boolean>;
  /** In the project's order. */
  statuses: Status[];
  types: TaskType[];
  labels: Label[];
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
  files: number;
  comments: number;
  createdAt: string;
  editedAt: string;
  updatedAt: string;
  updatedBy: { kind: "user" | "agent"; name: string };
  closedAt: string | null;
  version: number;
}

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
