// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The planning reports of the in-memory engine: velocity, burn-up and
// estimate accuracy. The fake keeps no field history, so it reads the
// tasks as they are now: a task counts in a sprint it is in or was ever in,
// from the day it was created, and is completed from the day it closed.

import {
  type BurnupDay,
  type BurnupForecast,
  type BurnupInput,
  type BurnupReport,
  type EstimateGroup,
  type EstimateGroupBy,
  type EstimatesInput,
  type EstimatesReport,
  type EstimateTask,
  type PlanMeasure,
  type SprintRef,
  type VelocityInput,
  type VelocityReport,
  type VelocitySprint,
  type VersionRef,
} from "../../generated/ops";
import { type FakeField } from "./fields";
import { type FakeSprint } from "./sprints";
import { type FakeProject, type FakeTask, type FakeTimeEntry } from "./state";
import { addDays, csvOf } from "./time";
import { type FakeVersion } from "./versions";

export interface PlanningHost {
  project: (ref: string) => FakeProject;
  /** The project and its sub-projects. */
  subtree: (project: FakeProject) => FakeProject[];
  sprints: () => FakeSprint[];
  findSprint: (ref: string, project: string | null | undefined) => FakeSprint;
  findVersion: (ref: string, project: string | null | undefined) => FakeVersion;
  sprintRef: (sprint: FakeSprint) => SprintRef;
  versionRef: (version: FakeVersion) => VersionRef;
  /** Live tasks. */
  tasks: () => FakeTask[];
  /** The tasks a sprint had at some time, by the history. */
  formerly: (sprint: string) => FakeTask[];
  /** Whether the task's status is a done one. */
  done: (task: FakeTask) => boolean;
  keyOf: (task: FakeTask) => string;
  typeName: (task: FakeTask) => string;
  labelNames: (task: FakeTask) => string[];
  projectOf: (task: FakeTask) => FakeProject;
  fields: (projectId: string) => FakeField[];
  timeEntries: () => FakeTimeEntry[];
  workKindName: (id: string | undefined) => string;
  /** The keys of the tasks a query matches. */
  matching: (query: string) => Set<string>;
  today: () => string;
  dayOf: (moment: string) => string;
  fail: (code: "invalid" | "not_found" | "conflict", message: string, path: string, messageId?: string) => never;
}

const round2 = (n: number) => Math.round(n * 100) / 100;
const round4 = (n: number) => Math.round(n * 10_000) / 10_000;
const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();

function numberField(host: PlanningHost, projectId: string, measure: PlanMeasure, ref: string | null | undefined): FakeField | undefined {
  const text = ref?.trim() ?? "";
  if (measure !== "field") {
    if (text !== "") {
      host.fail("invalid", 'A field is named only with the measure "field".', "field", "planning-field-unused");
    }
    return undefined;
  }
  if (text === "") {
    host.fail("invalid", "Name the number field to measure by, such as cf.points.", "field", "planning-field-needed");
  }
  const bare = fold(text.replace(/^cf\./i, ""));
  const field = host.fields(projectId).find((each) => each.id === text || fold(each.name) === bare || fold(each.name).replace(/\s+/g, "-") === bare);
  if (!field) {
    return host.fail("not_found", `There is no custom field "${text}" here.`, "field", "field-not-found");
  }
  if (field.kind !== "integer" && field.kind !== "decimal") {
    host.fail("invalid", `Field "${field.name}" doesn't hold numbers; choose a whole-number or decimal field.`, "field", "planning-field-not-number");
  }
  return field;
}

function valueOf(task: FakeTask, measure: PlanMeasure, field: FakeField | undefined): number {
  if (measure === "tasks") {
    return 1;
  }
  if (measure === "estimate") {
    return (task.estimateMin ?? 0) / 60;
  }
  const v = field === undefined ? undefined : task.fields?.[field.id];
  return typeof v === "number" ? v : 0;
}

function inSprint(host: PlanningHost, sprint: FakeSprint): FakeTask[] {
  const now = host.tasks().filter((task) => task.sprintId === sprint.id);
  return [...new Set([...now, ...host.formerly(sprint.id)])];
}

export function velocityReport(host: PlanningHost, input: VelocityInput): VelocityReport {
  const project = host.project(input.project);
  const measure = input.measure ?? "tasks";
  const field = numberField(host, project.id, measure, input.field);
  const last = input.last ?? 10;
  if (last < 1 || last > 50) {
    host.fail("invalid", "Show 1 to 50 sprints.", "last", "planning-last");
  }
  const sprints = host.sprints().filter((sprint) => sprint.projectId === project.id);
  const done: VelocitySprint[] = [];
  const closed = sprints.filter((sprint) => sprint.state === "closed" && sprint.completedAt !== null).sort((a, b) => (a.completedAt ?? "").localeCompare(b.completedAt ?? ""));
  for (const sprint of closed) {
    const started = sprint.startedAt ?? `${sprint.startDate ?? host.today()}T00:00:00.000Z`;
    const ended = sprint.completedAt ?? started;
    const window = sprint.startDate === null ? host.dayOf(started) : sprint.startDate;
    let committed = 0;
    let completed = 0;
    let committedTasks = 0;
    let completedTasks = 0;
    for (const task of inSprint(host, sprint)) {
      if (task.createdAt <= started && (task.closedAt === null || task.closedAt > started)) {
        committed += valueOf(task, measure, field);
        committedTasks += 1;
      }
      if (task.sprintId === sprint.id && task.closedAt !== null && task.closedAt <= ended && host.dayOf(task.closedAt) >= window) {
        completed += valueOf(task, measure, field);
        completedTasks += 1;
      }
    }
    const recent = [...done.slice(-2).map((each) => each.completed), completed];
    done.push({
      sprint: host.sprintRef(sprint),
      completed_on: host.dayOf(ended),
      committed: round2(committed),
      completed: round2(completed),
      committed_tasks: committedTasks,
      completed_tasks: completedTasks,
      average: round2(recent.reduce((sum, n) => sum + n, 0) / recent.length),
    });
  }
  const averageOf = Math.min(3, done.length);
  const average = averageOf === 0 ? undefined : round2(done.slice(-averageOf).reduce((sum, each) => sum + each.completed, 0) / averageOf);
  const shown = done.slice(-last);
  const upcoming = sprints
    .filter((sprint) => sprint.state !== "closed")
    .map((sprint) => {
      const tasks = host.tasks().filter((task) => task.sprintId === sprint.id);
      return { sprint: host.sprintRef(sprint), planned: round2(tasks.reduce((sum, task) => sum + valueOf(task, measure, field), 0)), tasks: tasks.length };
    });
  const csv =
    input.csv === true
      ? csvOf(
          ["sprint", "start_date", "end_date", "completed_on", "committed", "completed", "committed_tasks", "completed_tasks", "average"],
          shown.map((s) => [s.sprint.name, s.sprint.start_date ?? "", s.sprint.end_date ?? "", s.completed_on, String(s.committed), String(s.completed), String(s.committed_tasks), String(s.completed_tasks), String(s.average)]),
        )
      : undefined;
  return {
    project: project.key,
    measure,
    ...(field === undefined ? {} : { field: field.name }),
    sprints: shown,
    ...(average === undefined ? {} : { average }),
    average_of: averageOf,
    upcoming,
    ...(csv === undefined ? {} : { csv }),
  };
}

/** The least-squares line through the completed work, to the day it meets the last day's scope. */
export function forecastOf(from: string, days: readonly BurnupDay[]): BurnupForecast | undefined {
  const n = days.length;
  const last = days.at(-1);
  if (n < 3 || last === undefined || last.completed >= last.scope) {
    return undefined;
  }
  const meanX = (n - 1) / 2;
  const meanY = days.reduce((sum, d) => sum + d.completed, 0) / n;
  let sxy = 0;
  let sxx = 0;
  days.forEach((d, i) => {
    sxy += (i - meanX) * (d.completed - meanY);
    sxx += (i - meanX) ** 2;
  });
  const perDay = sxy / sxx;
  if (!(perDay > 0)) {
    return undefined;
  }
  const start = meanY - perDay * meanX;
  const reach = Math.max(n, Math.ceil((last.scope - start) / perDay - 1e-9));
  if (reach > 3650 + n) {
    return undefined;
  }
  return { date: addDays(from, reach), start: round2(start), per_day: round2(perDay) };
}

export function burnupReport(host: PlanningHost, input: BurnupInput): BurnupReport {
  const today = host.today();
  const measure = input.measure ?? "tasks";
  const versionText = input.version?.trim() ?? "";
  const sprintText = input.sprint?.trim() ?? "";
  if ((versionText === "") === (sprintText === "")) {
    host.fail("invalid", "Name a version or a sprint, one of the two.", "version", "burnup-version-or-sprint");
  }
  let tasks: FakeTask[];
  let first: string | null;
  let last = today;
  let end: string | null;
  let projectId: string;
  let head: { version: VersionRef } | { sprint: SprintRef };
  if (versionText !== "") {
    const version = host.findVersion(versionText, input.project);
    tasks = host.tasks().filter((task) => task.fixVersionId === version.id);
    first = version.startDate;
    end = version.dueDate;
    if (version.status === "closed" && version.dueDate !== null && version.dueDate < today) {
      last = version.dueDate;
    }
    projectId = version.projectId;
    head = { version: host.versionRef(version) };
  } else {
    const sprint = host.findSprint(sprintText, input.project);
    if (sprint.state === "planned") {
      host.fail("conflict", `Sprint "${sprint.name}" hasn't started yet, so it has no burn-up.`, "sprint", "burnup-sprint-not-started");
    }
    tasks = inSprint(host, sprint);
    first = sprint.startDate ?? (sprint.startedAt === null ? null : host.dayOf(sprint.startedAt));
    end = sprint.endDate;
    if (sprint.state === "closed" && sprint.completedAt !== null) {
      last = host.dayOf(sprint.completedAt);
    }
    projectId = sprint.projectId;
    head = { sprint: host.sprintRef(sprint) };
  }
  const field = numberField(host, projectId, measure, input.field);
  const from = first ?? tasks.map((task) => host.dayOf(task.createdAt)).sort()[0] ?? today;
  const days: BurnupDay[] = [];
  for (let day = from; day <= last && days.length < 1000; day = addDays(day, 1)) {
    const scope = tasks.filter((task) => host.dayOf(task.createdAt) <= day);
    const closed = scope.filter((task) => task.closedAt !== null && host.dayOf(task.closedAt) <= day);
    const sum = (list: FakeTask[]) => round2(list.reduce((total, task) => total + valueOf(task, measure, field), 0));
    days.push({ date: day, scope: sum(scope), completed: sum(closed), scope_tasks: scope.length, completed_tasks: closed.length });
  }
  const forecast = input.forecast === false ? undefined : forecastOf(from, days);
  const csv = input.csv === true ? csvOf(["date", "scope", "completed", "scope_tasks", "completed_tasks"], days.map((d) => [d.date, String(d.scope), String(d.completed), String(d.scope_tasks), String(d.completed_tasks)])) : undefined;
  return {
    ...head,
    measure,
    ...(field === undefined ? {} : { field: field.name }),
    from,
    to: last < from ? from : last,
    ...(end === null ? {} : { end }),
    days,
    ...(forecast === undefined ? {} : { forecast }),
    ...(csv === undefined ? {} : { csv }),
  };
}

const distance = (ratio: number) => Math.abs(Math.log(ratio));

function median(sorted: readonly number[]): number | undefined {
  const n = sorted.length;
  if (n === 0) {
    return undefined;
  }
  return n % 2 === 1 ? sorted[(n - 1) / 2] : ((sorted[n / 2 - 1] ?? 0) + (sorted[n / 2] ?? 0)) / 2;
}

function rounded(task: EstimateTask): EstimateTask {
  return { ...task, ratio: round2(task.ratio) };
}

function groupOf(key: string, name: string, tasks: readonly EstimateTask[]): EstimateGroup {
  const ratios = tasks.map((t) => t.ratio).sort((a, b) => a - b);
  const within = ratios.filter((r) => round4(r) >= 0.8 && round4(r) <= 1.2).length;
  const mid = median(ratios);
  return {
    key,
    name,
    count: tasks.length,
    estimate_minutes: tasks.reduce((sum, t) => sum + t.estimate_minutes, 0),
    spent_minutes: tasks.reduce((sum, t) => sum + t.spent_minutes, 0),
    ...(mid === undefined ? {} : { median_ratio: round2(mid) }),
    ...(ratios.length === 0 ? {} : { within_share: round4(within / ratios.length) }),
    over: ratios.filter((r) => round4(r) > 1.2).length,
    under: ratios.filter((r) => round4(r) < 0.8).length,
    outliers: [...tasks]
      .sort((a, b) => distance(b.ratio) - distance(a.ratio) || a.key.localeCompare(b.key))
      .slice(0, 10)
      .map(rounded),
  };
}

export function estimatesReport(host: PlanningHost, input: EstimatesInput): EstimatesReport {
  const today = host.today();
  const to = input.to?.trim() === "today" || input.to === undefined || input.to === null ? today : input.to;
  const from = input.from === undefined || input.from === null ? addDays(to, -89) : input.from;
  if (to < from) {
    host.fail("invalid", "The last day comes before the first.", "to", "activity-to-before-from");
  }
  const groupBy: EstimateGroupBy = input.group_by ?? "none";
  const scope = input.project === undefined || input.project === null || input.project.trim() === "" ? null : new Set(host.subtree(host.project(input.project)).map((p) => p.id));
  const query = input.query?.trim() ?? "";
  const matching = query === "" ? null : host.matching(query);
  const logged = new Map<string, { kind: string; minutes: number }[]>();
  for (const entry of host.timeEntries()) {
    if (entry.deletedAt !== null) {
      continue;
    }
    const list = logged.get(entry.taskId) ?? [];
    const kind = host.workKindName(entry.workKindId);
    const same = list.find((each) => each.kind === kind);
    if (same) {
      same.minutes += entry.minutes;
    } else {
      list.push({ kind, minutes: entry.minutes });
    }
    logged.set(entry.taskId, list);
  }
  let withoutEstimate = 0;
  let withoutTime = 0;
  const compared: { task: EstimateTask; closedAt: string; groups: [string, string][] }[] = [];
  for (const task of host.tasks()) {
    if (task.closedAt === null || !host.done(task)) {
      continue;
    }
    const day = host.dayOf(task.closedAt);
    if (day < from || day > to || (scope !== null && !scope.has(task.projectId)) || (matching !== null && !matching.has(host.keyOf(task)))) {
      continue;
    }
    if (task.estimateMin === null || task.estimateMin <= 0) {
      withoutEstimate += 1;
      continue;
    }
    const kinds = logged.get(task.id) ?? [];
    const spent = kinds.reduce((sum, each) => sum + each.minutes, 0);
    if (spent <= 0) {
      withoutTime += 1;
      continue;
    }
    const type = host.typeName(task);
    const project = host.projectOf(task);
    const top = [...kinds].sort((a, b) => b.minutes - a.minutes || fold(a.kind).localeCompare(fold(b.kind)))[0]?.kind ?? "";
    const labels = host.labelNames(task);
    const groups: [string, string][] =
      groupBy === "type"
        ? [[fold(type), type]]
        : groupBy === "project"
          ? [[project.key, project.name]]
          : groupBy === "month"
            ? [[day.slice(0, 7), day.slice(0, 7)]]
            : groupBy === "label"
              ? labels.length === 0
                ? [["", ""]]
                : labels.map((name): [string, string] => [fold(name), name])
              : groupBy === "work_kind"
                ? [[fold(top), top]]
                : [];
    compared.push({
      task: { key: host.keyOf(task), title: task.title, type, closed_on: day, estimate_minutes: task.estimateMin, spent_minutes: spent, ratio: spent / task.estimateMin },
      closedAt: task.closedAt,
      groups,
    });
  }
  compared.sort((a, b) => a.closedAt.localeCompare(b.closedAt) || a.task.key.localeCompare(b.task.key));
  const members = new Map<string, { name: string; tasks: EstimateTask[] }>();
  for (const each of compared) {
    for (const [key, name] of each.groups) {
      const group = members.get(key) ?? { name, tasks: [] };
      group.tasks.push(each.task);
      members.set(key, group);
    }
  }
  const groups = [...members.entries()].map(([key, group]) => groupOf(key, group.name, group.tasks));
  if (groupBy === "month") {
    groups.sort((a, b) => a.key.localeCompare(b.key));
  } else {
    groups.sort((a, b) => b.count - a.count || Number(a.key === "") - Number(b.key === "") || a.key.localeCompare(b.key));
  }
  const tasks = compared.map((each) => rounded(each.task));
  const csv =
    input.csv === true
      ? csvOf(["key", "title", "type", "closed_on", "estimate_minutes", "spent_minutes", "ratio"], tasks.map((t) => [t.key, t.title, t.type, t.closed_on, String(t.estimate_minutes), String(t.spent_minutes), String(t.ratio)]))
      : undefined;
  return {
    from,
    to,
    group_by: groupBy,
    total: groupOf("", "", compared.map((each) => each.task)),
    groups,
    without_estimate: withoutEstimate,
    without_time: withoutTime,
    ...(input.tasks === true ? { tasks } : {}),
    ...(csv === undefined ? {} : { csv }),
  };
}
