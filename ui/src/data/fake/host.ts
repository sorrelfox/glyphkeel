// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What the in-memory engine lends the modules that answer recurring tasks
// and templates: its records, its lookups and its way of making tasks.

import { type EntityChange, type Hue, type OpError, type Priority, type Project, type Task } from "../../generated/ops";
import { type FakeProject, type FakeState, type FakeTask } from "./state";

/** Fails the running operation with the engine's error. */
export type Fail = (code: OpError["code"], message: string, extra?: { hint?: string; details?: Record<string, unknown> }) => never;

/** A task about to be written, with its parts already looked up. */
export interface NewTask {
  title: string;
  description: string;
  /** Default: the project's first to-do status. */
  statusId: string | null;
  typeId: string;
  priority: Priority;
  labelIds: string[];
  parentId: string | null;
  startDate: string | null;
  dueDate: string | null;
  estimateMin: number | null;
}

/** One command's answer: its output and what it changed. */
export interface Answer<O> {
  output: O;
  changes: EntityChange[];
}

export interface FakeHost {
  state(): FakeState;
  /** The person's day. */
  today(): string;
  /** Now, as RFC 3339. */
  now(): string;
  fail: Fail;
  project(ref: string): FakeProject;
  /** A project that may change: not archived. */
  writable(ref: string): FakeProject;
  task(key: string): FakeTask;
  keyOf(task: FakeTask): string;
  projectOf(task: FakeTask): FakeProject;
  /** A day as typed: `2026-12-01`, `today`, `next mon`. */
  dayInput(text: string, path: string): string;
  /** A task's subtasks, in their order. */
  subtasksOf(task: FakeTask): FakeTask[];
  insertTask(project: FakeProject, task: NewTask): FakeTask;
  fullTask(task: FakeTask): Task;
  fullProject(project: FakeProject): Project;
  createProject(input: { name: string; key: string; parent: string | null; hue: Hue | null; description: string }): FakeProject;
  createPage(project: FakeProject, title: string, body: string, parentTitle: string | null): void;
}
