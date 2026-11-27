// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's sprints: the same rules as the engine's, kept
// small. A task is in at most one sprint, of its own project; a project has
// one active sprint at a time; completing one moves its open tasks to a
// planned sprint or back to the backlog; deleting one sends its tasks back.

import {
  type BurndownDay,
  type EntityChange,
  type Sprint,
  type SprintAddTasksInput,
  type SprintBurndown,
  type SprintBurndownInput,
  type SprintChange,
  type SprintCompleteInput,
  type SprintCreateInput,
  type SprintDeleteInput,
  type SprintGetInput,
  type SprintList,
  type SprintListInput,
  type SprintRef,
  type SprintRemoveTasksInput,
  type SprintStartInput,
  type SprintState,
  type SprintTask,
  type SprintUpdateInput,
} from "../../generated/ops";
import { type FakeProject, type FakeTask } from "./state";

export interface FakeSprint {
  id: string;
  projectId: string;
  name: string;
  goal: string;
  state: SprintState;
  startDate: string | null;
  endDate: string | null;
  startedAt: string | null;
  completedAt: string | null;
  rank: number;
  createdAt: string;
  updatedAt: string;
}

/** What the sprints need of the workspace around them. */
export interface SprintHost {
  sprints: () => FakeSprint[];
  tasks: () => FakeTask[];
  projects: () => FakeProject[];
  project: (ref: string) => FakeProject;
  writable: (ref: string) => FakeProject;
  task: (ref: string) => FakeTask;
  keyOf: (task: FakeTask) => string;
  statusOf: (task: FakeTask) => SprintTask["status"];
  typeName: (task: FakeTask) => string;
  /** The tasks a sprint had at some time, by the history. */
  formerly: (sprint: string) => FakeTask[];
  touch: (task: FakeTask) => void;
  now: () => string;
  /** Today, `YYYY-MM-DD`. */
  today: () => string;
  /** The day of a moment, `YYYY-MM-DD`. */
  dayOf: (moment: string) => string;
  newId: () => string;
  fail: (code: "invalid" | "not_found" | "conflict", message: string, path?: string) => never;
}

interface Result<O> {
  output: O;
  changes: EntityChange[];
}

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();
const RESERVED = ["active", "none", "backlog"];
const MAX_TASKS = 500;
const STATE_ORDER: Record<SprintState, number> = { closed: 0, active: 1, planned: 2 };

function addDays(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

function byOrder(a: FakeSprint, b: FakeSprint): number {
  return STATE_ORDER[a.state] - STATE_ORDER[b.state] || (a.completedAt ?? "").localeCompare(b.completedAt ?? "") || a.rank - b.rank;
}

function liveTasks(host: SprintHost, sprint: FakeSprint): FakeTask[] {
  return host.tasks().filter((task) => task.sprintId === sprint.id);
}

function taskOut(host: SprintHost, task: FakeTask): SprintTask {
  const parent = task.parentId === null ? undefined : host.tasks().find((candidate) => candidate.id === task.parentId);
  return {
    key: host.keyOf(task),
    title: task.title,
    status: host.statusOf(task),
    closed: task.closedAt !== null,
    type: host.typeName(task),
    priority: task.priority,
    ...(task.estimateMin === null ? {} : { estimate_minutes: task.estimateMin }),
    ...(parent ? { parent: host.keyOf(parent) } : {}),
    id: task.id,
  };
}

/** Top-level tasks first, then by their place on the board. */
function boardOrder(a: FakeTask, b: FakeTask): number {
  return Number(a.parentId !== null) - Number(b.parentId !== null) || a.rank - b.rank;
}

export function sprintOut(host: SprintHost, sprint: FakeSprint, withTasks: boolean): Sprint {
  const project = host.projects().find((candidate) => candidate.id === sprint.projectId);
  const tasks = liveTasks(host, sprint).sort(boardOrder);
  const closed = tasks.filter((task) => task.closedAt !== null);
  const minutes = (list: FakeTask[]) => list.reduce((sum, task) => sum + (task.estimateMin ?? 0), 0);
  return {
    id: sprint.id,
    name: sprint.name,
    project: project?.key ?? "",
    ...(sprint.goal === "" ? {} : { goal: sprint.goal }),
    state: sprint.state,
    ...(sprint.startDate === null ? {} : { start_date: sprint.startDate }),
    ...(sprint.endDate === null ? {} : { end_date: sprint.endDate }),
    ...(sprint.startedAt === null ? {} : { started_at: sprint.startedAt }),
    ...(sprint.completedAt === null ? {} : { completed_at: sprint.completedAt }),
    progress: { total: tasks.length, closed: closed.length, estimate_minutes: minutes(tasks), closed_estimate_minutes: minutes(closed) },
    ...(withTasks ? { tasks: tasks.map((task) => taskOut(host, task)) } : {}),
    rank: String(sprint.rank),
    created_at: sprint.createdAt,
    updated_at: sprint.updatedAt,
  };
}

function refOf(sprint: FakeSprint): SprintRef {
  return {
    id: sprint.id,
    name: sprint.name,
    state: sprint.state,
    ...(sprint.startDate === null ? {} : { start_date: sprint.startDate }),
    ...(sprint.endDate === null ? {} : { end_date: sprint.endDate }),
  };
}

/** The sprint a task is in, as tasks show it. */
export function sprintRef(host: SprintHost, task: FakeTask): SprintRef | undefined {
  const sprint = host.sprints().find((candidate) => candidate.id === task.sprintId);
  return sprint ? refOf(sprint) : undefined;
}

function find(host: SprintHost, ref: string, projectRef: string | null | undefined, path = "sprint"): FakeSprint {
  const byId = host.sprints().find((candidate) => candidate.id === ref);
  if (byId) return byId;
  const candidates = projectRef ? host.sprints().filter((sprint) => sprint.projectId === host.project(projectRef).id) : host.sprints();
  if (fold(ref) === "active") {
    if (!projectRef) host.fail("invalid", "Give the project with `project` to name its active sprint.", "project");
    const active = candidates.find((sprint) => sprint.state === "active");
    if (!active) host.fail("not_found", "The project has no active sprint.", path);
    return active;
  }
  const found = candidates.filter((sprint) => fold(sprint.name) === fold(ref));
  if (found.length !== 1 || found[0] === undefined) {
    host.fail(found.length > 1 ? "invalid" : "not_found", found.length > 1 ? `Several projects have a sprint named "${ref}".` : `There is no sprint "${ref}".`, path);
  }
  return found[0];
}

/** The sprint `ref` names among `project`'s, not closed, for a task to go in. */
export function sprintForTask(host: SprintHost, project: FakeProject, ref: string): FakeSprint {
  const sprint = find(host, ref, project.key);
  if (sprint.projectId !== project.id) {
    host.fail("invalid", `Sprint "${sprint.name}" isn't one of ${project.key}'s sprints; a task can only be in a sprint of its own project.`, "sprint");
  }
  if (sprint.state === "closed") {
    host.fail("conflict", `Sprint "${sprint.name}" is completed, so no tasks can be added to it.`, "sprint");
  }
  return sprint;
}

function change(host: SprintHost, sprint: FakeSprint): EntityChange {
  const project = host.projects().find((candidate) => candidate.id === sprint.projectId);
  return { entity: "sprint", id: sprint.id, fields: ["record"], ...(project ? { project: project.key } : {}) };
}

function taskChange(host: SprintHost, task: FakeTask): EntityChange {
  const project = host.projects().find((candidate) => candidate.id === task.projectId);
  return { entity: "task", id: task.id, fields: ["sprint"], ...(project ? { project: project.key } : {}) };
}

/** Puts `task` in `sprint` (or none); whether it changed. */
function put(host: SprintHost, task: FakeTask, sprint: string | null): boolean {
  if ((task.sprintId ?? null) === sprint) return false;
  task.sprintId = sprint;
  host.touch(task);
  return true;
}

export function listSprints(host: SprintHost, input: SprintListInput): SprintList {
  const project = host.project(input.project);
  const states = input.state ?? [];
  const items = host
    .sprints()
    .filter((sprint) => sprint.projectId === project.id && (states.length === 0 || states.includes(sprint.state)))
    .sort(byOrder)
    .map((sprint) => sprintOut(host, sprint, input.include_tasks === true));
  const backlog = host
    .tasks()
    .filter((task) => task.projectId === project.id && (task.sprintId ?? null) === null && task.closedAt === null)
    .sort(boardOrder)
    .map((task) => taskOut(host, task));
  return { items, total: items.length, ...(input.include_tasks === true ? { backlog } : {}) };
}

export function getSprint(host: SprintHost, input: SprintGetInput): Sprint {
  return sprintOut(host, find(host, input.sprint, input.project), true);
}

function checkName(host: SprintHost, projectId: string, name: string, id: string | null): string {
  const trimmed = name.trim();
  if (trimmed === "" || trimmed.length > 100) {
    host.fail("invalid", "A sprint's name is 1 to 100 characters long.", "name");
  }
  if (RESERVED.includes(fold(trimmed))) {
    host.fail("invalid", `A sprint can't be named "${trimmed}": the word has a meaning of its own in queries.`, "name");
  }
  if (host.sprints().some((other) => other.projectId === projectId && other.id !== id && fold(other.name) === fold(trimmed))) {
    host.fail("conflict", `The project already has a sprint named "${trimmed}".`, "name");
  }
  return trimmed;
}

function checkDates(host: SprintHost, start: string | null, end: string | null): void {
  if (start !== null && end !== null && start > end) {
    host.fail("invalid", "A sprint's start date can't be after its end date.", "end_date");
  }
}

/** Puts the tasks `refs` names in `sprint`; the ones that moved. */
function putTasks(host: SprintHost, sprint: FakeSprint, refs: readonly string[]): FakeTask[] {
  if (refs.length > MAX_TASKS) host.fail("invalid", `At most ${String(MAX_TASKS)} tasks at a time.`, "tasks");
  const project = host.projects().find((candidate) => candidate.id === sprint.projectId);
  const tasks = refs.map((ref) => host.task(ref));
  for (const task of tasks) {
    if (task.projectId !== sprint.projectId) {
      host.fail("invalid", `${host.keyOf(task)} isn't a task of ${project?.key ?? ""}; a sprint only takes tasks of its own project.`, "tasks");
    }
  }
  return tasks.filter((task) => put(host, task, sprint.id));
}

function answer(host: SprintHost, sprint: FakeSprint, moved: readonly FakeTask[], extra: EntityChange[] = []): Result<SprintChange> {
  return {
    output: { sprint: sprintOut(host, sprint, false), ...(moved.length > 0 ? { tasks_changed: moved.map((task) => host.keyOf(task)) } : {}) },
    changes: [change(host, sprint), ...extra, ...moved.map((task) => taskChange(host, task))],
  };
}

export function createSprint(host: SprintHost, input: SprintCreateInput): Result<SprintChange> {
  const project = host.writable(input.project);
  const name = checkName(host, project.id, input.name, null);
  const start = input.start_date ?? null;
  const end = input.end_date ?? null;
  checkDates(host, start, end);
  const now = host.now();
  const rank = Math.max(0, ...host.sprints().filter((other) => other.projectId === project.id).map((other) => other.rank)) + 1;
  const sprint: FakeSprint = { id: host.newId(), projectId: project.id, name, goal: (input.goal ?? "").trim(), state: "planned", startDate: start, endDate: end, startedAt: null, completedAt: null, rank, createdAt: now, updatedAt: now };
  host.sprints().push(sprint);
  const moved = putTasks(host, sprint, input.tasks ?? []);
  return answer(host, sprint, moved);
}

export function updateSprint(host: SprintHost, input: SprintUpdateInput): Result<SprintChange> {
  const sprint = find(host, input.sprint, input.project);
  host.writable(host.projects().find((candidate) => candidate.id === sprint.projectId)?.key ?? "");
  const clear = input.clear ?? [];
  const next = { ...sprint };
  if (input.name !== null && input.name !== undefined) next.name = checkName(host, sprint.projectId, input.name, sprint.id);
  if (input.goal !== null && input.goal !== undefined) next.goal = input.goal.trim();
  if (clear.includes("goal")) next.goal = "";
  if (input.start_date !== null && input.start_date !== undefined) next.startDate = input.start_date;
  if (clear.includes("start_date")) next.startDate = null;
  if (input.end_date !== null && input.end_date !== undefined) next.endDate = input.end_date;
  if (clear.includes("end_date")) next.endDate = null;
  checkDates(host, next.startDate, next.endDate);
  const changed = (Object.keys(next) as (keyof FakeSprint)[]).some((field) => next[field] !== sprint[field]);
  if (!changed) {
    return { output: { sprint: sprintOut(host, sprint, false) }, changes: [] };
  }
  Object.assign(sprint, next, { updatedAt: host.now() });
  return answer(host, sprint, []);
}

export function deleteSprint(host: SprintHost, input: SprintDeleteInput): Result<SprintChange> {
  const sprint = find(host, input.sprint, input.project);
  host.writable(host.projects().find((candidate) => candidate.id === sprint.projectId)?.key ?? "");
  if (sprint.state === "active") {
    host.fail("conflict", `Sprint "${sprint.name}" is active, so it can't be deleted.`, "sprint");
  }
  const before = sprintOut(host, sprint, false);
  const moved = liveTasks(host, sprint).filter((task) => put(host, task, null));
  const list = host.sprints();
  list.splice(list.indexOf(sprint), 1);
  return {
    output: { sprint: before, deleted: true, ...(moved.length > 0 ? { tasks_changed: moved.map((task) => host.keyOf(task)) } : {}) },
    changes: [change(host, sprint), ...moved.map((task) => taskChange(host, task))],
  };
}

export function startSprint(host: SprintHost, input: SprintStartInput): Result<SprintChange> {
  const sprint = find(host, input.sprint, input.project);
  host.writable(host.projects().find((candidate) => candidate.id === sprint.projectId)?.key ?? "");
  if (sprint.state !== "planned") {
    host.fail("conflict", sprint.state === "active" ? `Sprint "${sprint.name}" has already started.` : `Sprint "${sprint.name}" is completed and can't be started again.`, "sprint");
  }
  const active = host.sprints().find((other) => other.projectId === sprint.projectId && other.state === "active");
  if (active) {
    host.fail("conflict", `Sprint "${active.name}" is already active; a project has one active sprint at a time.`, "sprint");
  }
  const start = input.start_date ?? sprint.startDate ?? host.today();
  const ownEnd = sprint.endDate !== null && sprint.endDate >= start ? sprint.endDate : null;
  const end = input.end_date ?? ownEnd ?? addDays(start, 13);
  checkDates(host, start, end);
  Object.assign(sprint, {
    state: "active",
    startDate: start,
    endDate: end,
    startedAt: host.now(),
    completedAt: null,
    updatedAt: host.now(),
    ...(input.goal !== null && input.goal !== undefined ? { goal: input.goal.trim() } : {}),
  });
  return answer(host, sprint, []);
}

export function completeSprint(host: SprintHost, input: SprintCompleteInput): Result<SprintChange> {
  const sprint = find(host, input.sprint, input.project);
  const project = host.writable(host.projects().find((candidate) => candidate.id === sprint.projectId)?.key ?? "");
  if (sprint.state !== "active") {
    host.fail("conflict", sprint.state === "planned" ? `Sprint "${sprint.name}" hasn't started yet, so it can't be completed.` : `Sprint "${sprint.name}" is already completed.`, "sprint");
  }
  const to = (input.move_open_tasks_to ?? "").trim();
  let target: FakeSprint | null = null;
  if (to !== "" && fold(to) !== "backlog" && fold(to) !== "none") {
    target = find(host, to, project.key, "move_open_tasks_to");
    if (target.id === sprint.id) host.fail("invalid", "Choose another sprint to move the open tasks to, or the backlog.", "move_open_tasks_to");
    if (target.state !== "planned") host.fail("conflict", `Sprint "${target.name}" isn't planned; open tasks can only move to a planned sprint or the backlog.`, "move_open_tasks_to");
  }
  const moved = liveTasks(host, sprint).filter((task) => task.closedAt === null && put(host, task, target?.id ?? null));
  Object.assign(sprint, { state: "closed", completedAt: host.now(), updatedAt: host.now() });
  return answer(host, sprint, moved);
}

export function addSprintTasks(host: SprintHost, input: SprintAddTasksInput): Result<SprintChange> {
  const sprint = find(host, input.sprint, input.project);
  host.writable(host.projects().find((candidate) => candidate.id === sprint.projectId)?.key ?? "");
  if (sprint.state === "closed") host.fail("conflict", `Sprint "${sprint.name}" is completed, so no tasks can be added to it.`, "sprint");
  if (input.tasks.length === 0) host.fail("invalid", "Name at least one task.", "tasks");
  const moved = putTasks(host, sprint, input.tasks);
  return moved.length === 0 ? { output: { sprint: sprintOut(host, sprint, false) }, changes: [] } : answer(host, sprint, moved);
}

export function removeSprintTasks(host: SprintHost, input: SprintRemoveTasksInput): Result<SprintChange> {
  if (input.tasks.length === 0) host.fail("invalid", "Name at least one task.", "tasks");
  if (input.tasks.length > MAX_TASKS) host.fail("invalid", `At most ${String(MAX_TASKS)} tasks at a time.`, "tasks");
  const moved = input.tasks.map((ref) => host.task(ref)).filter((task) => put(host, task, null));
  return { output: moved.length > 0 ? { tasks_changed: moved.map((task) => host.keyOf(task)) } : {}, changes: moved.map((task) => taskChange(host, task)) };
}

/**
 * The burndown, simplified: the tasks in the sprint now (or, once it is
 * completed, the ones it had), each open on a day unless it closed by then.
 */
export function sprintBurndown(host: SprintHost, input: SprintBurndownInput): SprintBurndown {
  const sprint = find(host, input.sprint, input.project);
  if (sprint.state === "planned") {
    host.fail("conflict", `Sprint "${sprint.name}" hasn't started yet, so it has no burndown.`, "sprint");
  }
  const tasks = [...new Set([...liveTasks(host, sprint), ...host.formerly(sprint.id)])];
  const first = sprint.startDate ?? (sprint.startedAt === null ? host.today() : host.dayOf(sprint.startedAt));
  const last = sprint.state === "closed" && sprint.completedAt !== null ? host.dayOf(sprint.completedAt) : host.today();
  const days: BurndownDay[] = [];
  for (let day = first; day <= last && days.length < 366; day = addDays(day, 1)) {
    const inScope = tasks.filter((task) => host.dayOf(task.createdAt) <= day);
    const open = inScope.filter((task) => task.closedAt === null || host.dayOf(task.closedAt) > day);
    const minutes = (list: FakeTask[]) => list.reduce((sum, task) => sum + (task.estimateMin ?? 0), 0);
    days.push({ date: day, open_tasks: open.length, total_tasks: inScope.length, open_estimate_minutes: minutes(open), total_estimate_minutes: minutes(inScope) });
  }
  return { sprint: refOf(sprint), days, start_tasks: days[0]?.total_tasks ?? 0, start_estimate_minutes: days[0]?.total_estimate_minutes ?? 0 };
}
