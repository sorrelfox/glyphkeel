// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's versions (releases): the same rules as the
// engine's, kept small. A task can be planned for an open version of its
// project or one an ancestor shares; closing a version can move its open
// tasks on; deleting one plans its tasks for none.

import {
  type EntityChange,
  type Version,
  type VersionChange,
  type VersionCreateInput,
  type VersionDeleteInput,
  type VersionGetInput,
  type VersionList,
  type VersionListInput,
  type VersionRef,
  type VersionSharing,
  type VersionStatus,
  type VersionTask,
  type VersionUpdateInput,
} from "../../generated/ops";
import { type FakeProject, type FakeTask } from "./state";

export interface FakeVersion {
  id: string;
  projectId: string;
  name: string;
  description: string;
  status: VersionStatus;
  startDate: string | null;
  dueDate: string | null;
  sharing: VersionSharing;
  rank: number;
  createdAt: string;
  updatedAt: string;
}

/** What the versions need of the workspace around them. */
export interface VersionHost {
  versions: () => FakeVersion[];
  tasks: () => FakeTask[];
  projects: () => FakeProject[];
  project: (ref: string) => FakeProject;
  writable: (ref: string) => FakeProject;
  keyOf: (task: FakeTask) => string;
  statusOf: (task: FakeTask) => { id: string; name: string; category: VersionTask["status"]["category"] };
  typeName: (task: FakeTask) => string;
  progress: (task: FakeTask) => number;
  touch: (task: FakeTask) => void;
  now: () => string;
  newId: () => string;
  fail: (code: "invalid" | "not_found" | "conflict", message: string, path?: string) => never;
}

interface Result<O> {
  output: O;
  changes: EntityChange[];
}

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();

/** The project and its ancestors, nearest first. */
function ancestry(host: VersionHost, project: FakeProject): FakeProject[] {
  const out = [project];
  let at = project;
  for (let depth = 0; depth < 8; depth += 1) {
    const parent = host.projects().find((candidate) => candidate.id === at.parent);
    if (!parent) break;
    out.push(parent);
    at = parent;
  }
  return out;
}

/** The versions tasks of `project` can be planned for, own first, each with whether it is inherited. */
export function visibleVersions(host: VersionHost, project: FakeProject): { version: FakeVersion; inherited: boolean }[] {
  return ancestry(host, project).flatMap((each, depth) =>
    host
      .versions()
      .filter((version) => version.projectId === each.id && (depth === 0 || version.sharing === "descendants"))
      .map((version) => ({ version, inherited: depth > 0 })),
  );
}

function byDue(a: Version, b: Version): number {
  const ad = a.due_date ?? "￿";
  const bd = b.due_date ?? "￿";
  return ad < bd ? -1 : ad > bd ? 1 : Number(a.inherited !== true) - Number(b.inherited !== true) || Number(a.rank) - Number(b.rank) || fold(a.name).localeCompare(fold(b.name));
}

export function versionOut(host: VersionHost, version: FakeVersion, inherited: boolean, withTasks: boolean): Version {
  const project = host.projects().find((candidate) => candidate.id === version.projectId);
  const tasks = host.tasks().filter((task) => task.fixVersionId === version.id);
  const closed = tasks.filter((task) => task.closedAt !== null).length;
  const estimated = tasks.filter((task) => (task.estimateMin ?? 0) > 0);
  const average = estimated.length === 0 ? 1 : estimated.reduce((sum, task) => sum + (task.estimateMin ?? 0), 0) / estimated.length;
  const weight = (task: FakeTask) => (estimated.length === 0 ? 1 : (task.estimateMin ?? 0) > 0 ? (task.estimateMin ?? 0) : average);
  const total = tasks.reduce((sum, task) => sum + weight(task), 0);
  const percent = total === 0 ? 0 : Math.round(tasks.reduce((sum, task) => sum + weight(task) * (task.closedAt !== null ? 100 : host.progress(task)), 0) / total);
  return {
    id: version.id,
    name: version.name,
    project: project?.key ?? "",
    ...(version.description === "" ? {} : { description: version.description }),
    status: version.status,
    ...(version.startDate === null ? {} : { start_date: version.startDate }),
    ...(version.dueDate === null ? {} : { due_date: version.dueDate }),
    sharing: version.sharing,
    ...(inherited ? { inherited: true } : {}),
    progress: {
      total: tasks.length,
      closed,
      percent,
      estimate_minutes: tasks.reduce((sum, task) => sum + (task.estimateMin ?? 0), 0),
      spent_minutes: tasks.reduce((sum, task) => sum + task.spentMin, 0),
    },
    ...(withTasks
      ? {
          tasks: tasks.map(
            (task): VersionTask => ({
              key: host.keyOf(task),
              title: task.title,
              project: host.projects().find((candidate) => candidate.id === task.projectId)?.key ?? "",
              status: host.statusOf(task),
              closed: task.closedAt !== null,
              type: host.typeName(task),
              progress_percent: host.progress(task),
              id: task.id,
            }),
          ),
        }
      : {}),
    rank: String(version.rank),
    created_at: version.createdAt,
    updated_at: version.updatedAt,
  };
}

/** The version a task is planned for, as tasks show it. */
export function versionRef(host: VersionHost, task: FakeTask): VersionRef | undefined {
  const version = host.versions().find((candidate) => candidate.id === task.fixVersionId);
  if (!version) return undefined;
  const project = host.projects().find((candidate) => candidate.id === version.projectId);
  return { id: version.id, name: version.name, project: project?.key ?? "", status: version.status, ...(version.dueDate === null ? {} : { due_date: version.dueDate }) };
}

/** The open version `ref` names among those tasks of `project` can use. */
export function versionForTask(host: VersionHost, project: FakeProject, ref: string): FakeVersion {
  const found = visibleVersions(host, project).find(({ version }) => version.id === ref || fold(version.name) === fold(ref));
  if (!found) {
    host.fail("not_found", `Tasks of ${project.key} can't be planned for a version "${ref}".`, "fix_version");
  }
  if (found.version.status !== "open") {
    host.fail("conflict", `Version "${found.version.name}" is ${found.version.status}, so no tasks can be added to it.`, "fix_version");
  }
  return found.version;
}

export function find(host: VersionHost, ref: string, projectRef: string | null | undefined): { version: FakeVersion; inherited: boolean } {
  const byId = host.versions().find((candidate) => candidate.id === ref);
  if (byId) return { version: byId, inherited: false };
  const candidates = projectRef ? visibleVersions(host, host.project(projectRef)) : host.versions().map((version) => ({ version, inherited: false }));
  const found = candidates.filter(({ version }) => fold(version.name) === fold(ref));
  const own = found.find(({ inherited }) => !inherited) ?? (found.length === 1 ? found[0] : undefined);
  if (!own) {
    host.fail(found.length > 1 ? "invalid" : "not_found", found.length > 1 ? `Several projects have a version named "${ref}".` : `There is no version "${ref}".`, "version");
  }
  return own;
}

function change(host: VersionHost, version: FakeVersion, fields: string[]): EntityChange {
  const project = host.projects().find((candidate) => candidate.id === version.projectId);
  return { entity: "version", id: version.id, fields, ...(project ? { project: project.key } : {}) };
}

function taskChange(host: VersionHost, task: FakeTask): EntityChange {
  const project = host.projects().find((candidate) => candidate.id === task.projectId);
  return { entity: "task", id: task.id, fields: ["fix_version"], ...(project ? { project: project.key } : {}) };
}

export function listVersions(host: VersionHost, input: VersionListInput): VersionList {
  const project = host.project(input.project);
  const items = visibleVersions(host, project)
    .filter(({ inherited }) => !(inherited && input.own_only === true))
    .filter(({ version }) => (input.status ?? []).length === 0 || (input.status ?? []).includes(version.status))
    .map(({ version, inherited }) => versionOut(host, version, inherited, input.include_tasks === true))
    .sort(byDue);
  const unplanned = host.tasks().filter((task) => task.projectId === project.id && (task.fixVersionId ?? null) === null && task.closedAt === null).length;
  return { items, total: items.length, ...(input.include_tasks === true ? { unplanned } : {}) };
}

export function getVersion(host: VersionHost, input: VersionGetInput): Version {
  const { version, inherited } = find(host, input.version, input.project);
  return versionOut(host, version, inherited, true);
}

function checkName(host: VersionHost, projectId: string, name: string, id: string | null): string {
  const trimmed = name.trim();
  if (trimmed === "" || trimmed.length > 100) {
    host.fail("invalid", "A version's name is 1 to 100 characters long.", "name");
  }
  if (host.versions().some((other) => other.projectId === projectId && other.id !== id && fold(other.name) === fold(trimmed))) {
    host.fail("conflict", `The project already has a version named "${trimmed}".`, "name");
  }
  return trimmed;
}

function checkDates(host: VersionHost, start: string | null, due: string | null): void {
  if (start !== null && due !== null && start > due) {
    host.fail("invalid", "A version's start date can't be after its due date.", "due_date");
  }
}

export function createVersion(host: VersionHost, input: VersionCreateInput): Result<VersionChange> {
  const project = host.writable(input.project);
  const name = checkName(host, project.id, input.name, null);
  const start = input.start_date ?? null;
  const due = input.due_date ?? null;
  checkDates(host, start, due);
  const now = host.now();
  const rank = Math.max(0, ...host.versions().filter((other) => other.projectId === project.id).map((other) => other.rank)) + 1;
  const version: FakeVersion = {
    id: host.newId(),
    projectId: project.id,
    name,
    description: (input.description ?? "").trim(),
    status: input.status ?? "open",
    startDate: start,
    dueDate: due,
    sharing: input.sharing ?? "none",
    rank,
    createdAt: now,
    updatedAt: now,
  };
  host.versions().push(version);
  return { output: { version: versionOut(host, version, false, false) }, changes: [change(host, version, ["record"])] };
}

/** Takes the version off the tasks of projects that no longer see it. */
function dropUnreachable(host: VersionHost, version: FakeVersion): FakeTask[] {
  const dropped: FakeTask[] = [];
  for (const task of host.tasks()) {
    if (task.fixVersionId !== version.id) continue;
    const project = host.projects().find((candidate) => candidate.id === task.projectId);
    if (project && !visibleVersions(host, project).some((each) => each.version.id === version.id)) {
      task.fixVersionId = null;
      host.touch(task);
      dropped.push(task);
    }
  }
  return dropped;
}

export function updateVersion(host: VersionHost, input: VersionUpdateInput): Result<VersionChange> {
  const { version, inherited } = find(host, input.version, input.project);
  const project = host.projects().find((candidate) => candidate.id === version.projectId);
  host.writable(project?.key ?? "");
  const clear = input.clear ?? [];
  const next = { ...version };
  if (input.name !== null && input.name !== undefined) next.name = checkName(host, version.projectId, input.name, version.id);
  if (input.description !== null && input.description !== undefined) next.description = input.description.trim();
  if (clear.includes("description")) next.description = "";
  if (input.start_date !== null && input.start_date !== undefined) next.startDate = input.start_date;
  if (clear.includes("start_date")) next.startDate = null;
  if (input.due_date !== null && input.due_date !== undefined) next.dueDate = input.due_date;
  if (clear.includes("due_date")) next.dueDate = null;
  if (input.status !== null && input.status !== undefined) next.status = input.status;
  if (input.sharing !== null && input.sharing !== undefined) next.sharing = input.sharing;
  checkDates(host, next.startDate, next.dueDate);
  const moveTo = input.move_open_tasks_to;
  let target: FakeVersion | null | undefined;
  if (moveTo !== null && moveTo !== undefined) {
    if (next.status !== "closed") {
      host.fail("invalid", "Open tasks can only be moved on when the version is closed: give status closed too.", "move_open_tasks_to");
    }
    target = fold(moveTo) === "none" ? null : find(host, moveTo, project?.key).version;
    if (target && target.status !== "open") {
      host.fail("conflict", `Version "${target.name}" is ${target.status}, so no tasks can be added to it.`, "move_open_tasks_to");
    }
  }
  if (target) {
    for (const task of host.tasks()) {
      const home = host.projects().find((candidate) => candidate.id === task.projectId);
      if (task.fixVersionId === version.id && task.closedAt === null && home && !visibleVersions(host, home).some((each) => each.version.id === target.id)) {
        host.fail("conflict", `${host.keyOf(task)} can't be planned for the chosen version, because that version isn't shared with its project.`, "move_open_tasks_to");
      }
    }
  }
  const changes: EntityChange[] = [];
  const changedTasks: FakeTask[] = [];
  const changed = (Object.keys(next) as (keyof FakeVersion)[]).some((field) => next[field] !== version[field]);
  if (changed) {
    Object.assign(version, next, { updatedAt: host.now() });
    changes.push(change(host, version, ["record"]));
    changedTasks.push(...dropUnreachable(host, version));
  }
  if (target !== undefined) {
    for (const task of host.tasks()) {
      if (task.fixVersionId === version.id && task.closedAt === null) {
        task.fixVersionId = target?.id ?? null;
        host.touch(task);
        changedTasks.push(task);
      }
    }
  }
  changes.push(...changedTasks.map((task) => taskChange(host, task)));
  return {
    output: { version: versionOut(host, version, inherited, false), ...(changedTasks.length > 0 ? { tasks_changed: changedTasks.map((task) => host.keyOf(task)) } : {}) },
    changes,
  };
}

export function deleteVersion(host: VersionHost, input: VersionDeleteInput): Result<VersionChange> {
  const { version, inherited } = find(host, input.version, input.project);
  const project = host.projects().find((candidate) => candidate.id === version.projectId);
  host.writable(project?.key ?? "");
  const before = versionOut(host, version, inherited, false);
  const tasks = host.tasks().filter((task) => task.fixVersionId === version.id);
  for (const task of tasks) {
    task.fixVersionId = null;
    host.touch(task);
  }
  const list = host.versions();
  list.splice(list.indexOf(version), 1);
  return {
    output: { version: before, deleted: true, ...(tasks.length > 0 ? { tasks_changed: tasks.map((task) => host.keyOf(task)) } : {}) },
    changes: [change(host, version, ["record"]), ...tasks.map((task) => taskChange(host, task))],
  };
}
