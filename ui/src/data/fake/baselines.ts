// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's schedule baselines: named snapshots of a project's
// dated tasks, with the same rules as the engine's. Undo comes from the
// workspace's own snapshots.

import {
  type Actor,
  type Baseline,
  type BaselineChange,
  type BaselineCreateInput,
  type BaselineDates,
  type BaselineList,
  type BaselineListInput,
  type BaselineRef,
  type BaselineRenameInput,
  type EntityChange,
  type ListedTask,
  type OpError,
  type RemovedTask,
} from "../../generated/ops";
import { type FakeProject } from "./state";

export interface FakeBaselineTask {
  /** The task's ID. */
  task: string;
  key: string;
  title: string;
  start?: string | undefined;
  due?: string | undefined;
  milestone: boolean;
}

export interface FakeBaseline {
  id: string;
  projectId: string;
  name: string;
  createdAt: string;
  createdBy: Actor;
  updatedAt: string;
  tasks: FakeBaselineTask[];
}

/** What the baselines need of the workspace around them. */
export interface BaselineHost {
  baselines: () => FakeBaseline[];
  project: (ref: string) => FakeProject;
  writable: (ref: string) => FakeProject;
  /** The project's live tasks, as the list shows them. */
  tasksOf: (project: FakeProject) => ListedTask[];
  milestone: (task: ListedTask) => boolean;
  actor: () => Actor;
  now: () => string;
  newId: () => string;
  fail: (code: OpError["code"], message: string, path?: string) => never;
}

type Answer = { output: BaselineChange; changes: EntityChange[] };

const NAME_MAX = 100;
const fold = (text: string) => text.trim().toLowerCase();

function out(host: BaselineHost, baseline: FakeBaseline): Baseline {
  const project = host.project(baseline.projectId);
  return {
    id: baseline.id,
    project: project.key,
    name: baseline.name,
    tasks: baseline.tasks.length,
    created_by: baseline.createdBy,
    created_at: baseline.createdAt,
    updated_at: baseline.updatedAt,
  };
}

/** A project's baselines, newest first. */
function ofProject(host: BaselineHost, project: FakeProject): FakeBaseline[] {
  return host
    .baselines()
    .filter((baseline) => baseline.projectId === project.id)
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt) || b.id.localeCompare(a.id));
}

export function listBaselines(host: BaselineHost, input: BaselineListInput): BaselineList {
  const items = ofProject(host, host.project(input.project)).map((baseline) => out(host, baseline));
  return { items, total: items.length };
}

/** The baseline of `project` that `text` names: its ID, or its name (ignoring case). */
export function findBaseline(host: BaselineHost, project: FakeProject, text: string, path = "baseline"): FakeBaseline {
  const all = ofProject(host, project);
  const found = all.find((baseline) => baseline.id === text.trim()) ?? all.find((baseline) => fold(baseline.name) === fold(text));
  return found ?? host.fail("not_found", `There is no baseline "${text.trim()}".`, path);
}

function checkName(host: BaselineHost, project: FakeProject, name: string, except?: FakeBaseline): string {
  const trimmed = name.trim();
  if (trimmed === "" || Array.from(trimmed).length > NAME_MAX) {
    host.fail("invalid", `A name is 1 to ${String(NAME_MAX)} characters long.`, "name");
  }
  if (ofProject(host, project).some((baseline) => baseline !== except && fold(baseline.name) === fold(trimmed))) {
    host.fail("conflict", `The project already has a baseline named "${trimmed}".`, "name");
  }
  return trimmed;
}

const change = (baseline: FakeBaseline, project: FakeProject, fields: string[]): EntityChange => ({ entity: "baseline", id: baseline.id, fields, project: project.key });

export function createBaseline(host: BaselineHost, input: BaselineCreateInput): Answer {
  const project = host.writable(input.project);
  const name = checkName(host, project, input.name);
  const tasks = host
    .tasksOf(project)
    .filter((task) => task.project === project.key && (task.start_date !== undefined || task.due_date !== undefined))
    .map((task) => ({ task: task.id, key: task.key, title: task.title, start: task.start_date, due: task.due_date, milestone: host.milestone(task) }));
  const baseline: FakeBaseline = { id: host.newId(), projectId: project.id, name, createdAt: host.now(), createdBy: host.actor(), updatedAt: host.now(), tasks };
  host.baselines().push(baseline);
  return { output: { baseline: out(host, baseline) }, changes: [change(baseline, project, ["record", "tasks"])] };
}

export function renameBaseline(host: BaselineHost, input: BaselineRenameInput): Answer {
  const project = host.writable(input.project);
  const baseline = findBaseline(host, project, input.baseline);
  baseline.name = checkName(host, project, input.name, baseline);
  baseline.updatedAt = host.now();
  return { output: { baseline: out(host, baseline) }, changes: [change(baseline, project, ["record"])] };
}

export function deleteBaseline(host: BaselineHost, input: BaselineRef): Answer {
  const project = host.writable(input.project);
  const baseline = findBaseline(host, project, input.baseline);
  const shown = out(host, baseline);
  const all = host.baselines();
  all.splice(all.indexOf(baseline), 1);
  return { output: { baseline: shown, deleted: true }, changes: [change(baseline, project, ["record", "tasks"])] };
}

const DAY_MS = 86_400_000;
const days = (day: string) => Math.round(Date.parse(`${day}T00:00:00Z`) / DAY_MS);

/** A task's baseline dates and its slip, in calendar days, from its end now and then. */
export function baselineDates(entry: FakeBaselineTask, start: string | undefined, due: string | undefined): BaselineDates {
  const now = due ?? start;
  const then = entry.due ?? entry.start;
  return {
    ...(entry.start === undefined ? {} : { start_date: entry.start }),
    ...(entry.due === undefined ? {} : { due_date: entry.due }),
    ...(now !== undefined && then !== undefined ? { slip_days: days(now) - days(then) } : {}),
  };
}

/** The baseline's tasks that are no longer dated tasks of its project. */
export function removedSince(baseline: FakeBaseline, live: ReadonlyMap<string, ListedTask>, projectKey: string): { removed: RemovedTask[]; total: number } {
  const gone = baseline.tasks
    .filter((entry) => {
      const task = live.get(entry.task);
      return !task || task.project !== projectKey || (task.start_date === undefined && task.due_date === undefined);
    })
    .sort((a, b) => a.key.localeCompare(b.key, "en", { numeric: true }));
  return {
    removed: gone.slice(0, 200).map((entry) => ({
      key: entry.key,
      title: entry.title,
      ...(entry.start === undefined ? {} : { start_date: entry.start }),
      ...(entry.due === undefined ? {} : { due_date: entry.due }),
      deleted: !live.has(entry.task),
    })),
    total: gone.length,
  };
}
