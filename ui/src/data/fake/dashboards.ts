// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's dashboards: the same rules as the engine's, kept
// small. A widget computes nothing of its own: it asks the other fake
// operations (task search, the reports, the burndown, versions, activity,
// the automation run log) with its settings.

import {
  type Actor,
  type ActivityPage,
  type AutomationRunList,
  type CreatedClosedReport,
  type CumulativeFlowReport,
  type CycleTimeReport,
  type Dashboard,
  type DashboardAddWidgetInput,
  type DashboardChange,
  type DashboardCreateInput,
  type DashboardList,
  type DashboardListInput,
  type DashboardRef,
  type DashboardRemoveWidgetInput,
  type DashboardShow,
  type DashboardUpdateInput,
  type DashboardUpdateWidgetInput,
  type DashboardWidgetInput,
  type EntityChange,
  type ListGrouping,
  type OpError,
  type SprintBurndown,
  type TaskSearchResult,
  type TimeReport,
  type VersionList,
  type Widget,
  type WidgetKind,
  type WidgetSetting,
  type WidgetSettings,
  type WidgetSpec,
  type WidgetValue,
} from "../../generated/ops";
import { defaultWidth, KIND_SETTINGS } from "../dashboards";
import { type FakeProject } from "./state";

export interface FakeWidget {
  id: string;
  kind: WidgetKind;
  title: string;
  width: number;
  /** Its settings; the project by ID. */
  settings: WidgetSettings;
}

export interface FakeDashboard {
  id: string;
  projectId: string | null;
  name: string;
  description: string;
  widgets: FakeWidget[];
  rank: number;
  createdAt: string;
  createdBy: Actor;
  updatedAt: string;
}

/** What the dashboards need of the workspace around them. */
export interface DashboardHost {
  dashboards: () => FakeDashboard[];
  projects: () => FakeProject[];
  project: (ref: string) => FakeProject;
  writable: (ref: string) => FakeProject;
  /** Runs a reading operation of the fake. */
  read: (op: string, input: unknown) => unknown;
  /** Checks a query; throws its mistake. */
  checkQuery: (query: string, path: string) => void;
  /** A custom field of this query name (without `cf.`) that tasks can be counted by. */
  countableField: (name: string) => boolean;
  actor: () => Actor;
  now: () => string;
  today: () => string;
  newId: () => string;
  fail: (code: OpError["code"], message: string, path?: string) => never;
}

const WIDGETS_MAX = 40;
const FIELDS = ["status", "priority", "type", "label"];

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();

function some<K extends string, V>(key: K, value: V | null | undefined): { [P in K]?: V } {
  return (value === null || value === undefined ? {} : { [key]: value }) as { [P in K]?: V };
}

function projectKey(host: DashboardHost, id: string | null | undefined): string | undefined {
  return id === null || id === undefined ? undefined : host.projects().find((project) => project.id === id)?.key;
}

function widgetOut(host: DashboardHost, widget: FakeWidget): Widget {
  const settings = { ...widget.settings };
  if (settings.project !== undefined) {
    settings.project = projectKey(host, settings.project) ?? settings.project;
  }
  return { id: widget.id, kind: widget.kind, ...(widget.title === "" ? {} : { title: widget.title }), width: widget.width, settings: structuredClone(settings) };
}

function out(host: DashboardHost, board: FakeDashboard): Dashboard {
  return {
    id: board.id,
    name: board.name,
    ...some("project", projectKey(host, board.projectId)),
    ...(board.description === "" ? {} : { description: board.description }),
    widgets: board.widgets.map((widget) => widgetOut(host, widget)),
    rank: String(board.rank).padStart(6, "0"),
    created_by: board.createdBy,
    created_at: board.createdAt,
    updated_at: board.updatedAt,
  };
}

function find(host: DashboardHost, ref: string, project: string | null | undefined): FakeDashboard {
  const byId = host.dashboards().find((board) => board.id === ref.trim());
  if (byId) {
    return byId;
  }
  const scope = project === null || project === undefined ? null : host.project(project).id;
  const found = host.dashboards().find((board) => board.projectId === scope && fold(board.name) === fold(ref));
  return found ?? host.fail("not_found", `There is no dashboard "${ref.trim()}".`, "dashboard");
}

function widgetAt(host: DashboardHost, board: FakeDashboard, ref: string): number {
  const byId = board.widgets.findIndex((widget) => widget.id === ref.trim());
  if (byId >= 0) {
    return byId;
  }
  const place = Number(ref);
  if (Number.isInteger(place) && place >= 1 && place <= board.widgets.length) {
    return place - 1;
  }
  return host.fail("not_found", `The dashboard "${board.name}" has no widget "${ref.trim()}".`, "widget");
}

function checkSettings(host: DashboardHost, kind: WidgetKind, settings: WidgetSettings, pinned: string | null, at: string): WidgetSettings {
  const allowed = KIND_SETTINGS[kind];
  for (const [name, value] of Object.entries(settings)) {
    if ((value as unknown) !== undefined && !allowed.includes(name as WidgetSetting)) {
      host.fail("invalid", `A ${kind} widget takes no "${name}" setting.`, `${at}settings.${name}`);
    }
  }
  const next: WidgetSettings = { ...settings };
  const query = settings.query?.trim();
  if (query === undefined || query === "") {
    delete next.query;
  } else {
    host.checkQuery(query, `${at}settings.query`);
    next.query = query;
  }
  const project = settings.project?.trim();
  if (project === undefined || project === "") {
    delete next.project;
  } else {
    next.project = host.project(project).id;
  }
  if ((kind === "burndown" || kind === "version_progress") && next.project === undefined && pinned === null) {
    host.fail("invalid", `A ${kind} widget needs a project: give one, or pin the dashboard to a project.`, `${at}settings.project`);
  }
  if (kind === "tasks_by_field") {
    const field = settings.field?.trim() ?? "";
    if (FIELDS.includes(field.toLowerCase())) {
      next.field = field.toLowerCase();
    } else if (field.startsWith("cf.") && host.countableField(field.slice(3))) {
      next.field = field;
    } else {
      host.fail("invalid", field === "" ? "Name the field to count tasks by." : `Tasks can't be counted by "${field}".`, `${at}settings.field`);
    }
  }
  if (settings.limit !== undefined && (settings.limit < 1 || settings.limit > 50)) {
    host.fail("invalid", "A widget lists 1 to 50 rows.", `${at}settings.limit`);
  }
  if (settings.days !== undefined && (settings.days < 1 || settings.days > 366)) {
    host.fail("invalid", "A widget looks at 1 to 366 days.", `${at}settings.days`);
  }
  if (settings.text !== undefined && settings.text.length > 10_000) {
    host.fail("invalid", "A note is at most 10000 characters long.", `${at}settings.text`);
  }
  return next;
}

function checkWidth(host: DashboardHost, width: number, at: string): number {
  return Number.isInteger(width) && width >= 1 && width <= 3 ? width : host.fail("invalid", "A widget is 1, 2 or 3 columns wide.", `${at}width`);
}

function newWidget(host: DashboardHost, spec: WidgetSpec, pinned: string | null, at: string): FakeWidget {
  return {
    id: host.newId(),
    kind: spec.kind,
    title: (spec.title ?? "").trim(),
    width: checkWidth(host, spec.width ?? defaultWidth(spec.kind), at),
    settings: checkSettings(host, spec.kind, spec.settings ?? {}, pinned, at),
  };
}

/** The starter widgets, as the engine's. */
export function starter(): WidgetSpec[] {
  return [
    { kind: "query_count", settings: { query: "is:open" } },
    { kind: "overdue" },
    { kind: "due_soon" },
    { kind: "tasks_by_field", settings: { field: "status" } },
    { kind: "created_closed" },
    { kind: "activity" },
  ];
}

function change(host: DashboardHost, board: FakeDashboard, widget?: FakeWidget, deleted = false): { output: DashboardChange; changes: EntityChange[] } {
  return {
    output: { dashboard: out(host, board), ...(widget ? { widget: widgetOut(host, widget) } : {}), ...(deleted ? { deleted } : {}) },
    changes: [{ entity: "dashboard", id: board.id, fields: ["record"], ...some("project", projectKey(host, board.projectId)) }],
  };
}

function unique(host: DashboardHost, board: FakeDashboard): void {
  if (host.dashboards().some((other) => other.id !== board.id && other.projectId === board.projectId && fold(other.name) === fold(board.name))) {
    host.fail("conflict", `There is already a dashboard named "${board.name}" here.`, "name");
  }
}

function checkName(host: DashboardHost, name: string): string {
  const clean = name.trim();
  return clean === "" || clean.length > 100 ? host.fail("invalid", "A name is 1 to 100 characters long.", "name") : clean;
}

export function listDashboards(host: DashboardHost, input: DashboardListInput): DashboardList {
  const scope = input.project === null || input.project === undefined ? null : host.project(input.project).id;
  const items = host
    .dashboards()
    .filter((board) => input.all === true || board.projectId === scope)
    .sort((a, b) => (a.projectId === null ? 0 : 1) - (b.projectId === null ? 0 : 1) || (a.projectId ?? "").localeCompare(b.projectId ?? "") || a.rank - b.rank)
    .map((board) => out(host, board));
  return { items, total: items.length };
}

export function createDashboard(host: DashboardHost, input: DashboardCreateInput) {
  const name = checkName(host, input.name);
  const projectId = input.project === null || input.project === undefined ? null : host.writable(input.project).id;
  const specs = [...(input.starter === true ? starter() : []), ...(input.widgets ?? [])];
  if (specs.length > WIDGETS_MAX) {
    host.fail("invalid", `A dashboard holds at most ${String(WIDGETS_MAX)} widgets.`, "widgets");
  }
  const skip = input.starter === true ? starter().length : 0;
  const widgets = specs.map((spec, index) => newWidget(host, spec, projectId, index < skip ? "" : `widgets[${String(index - skip)}].`));
  const rank = Math.max(0, ...host.dashboards().filter((board) => board.projectId === projectId).map((board) => board.rank)) + 1;
  const board: FakeDashboard = {
    id: host.newId(),
    projectId,
    name,
    description: (input.description ?? "").trim(),
    widgets,
    rank,
    createdAt: host.now(),
    createdBy: host.actor(),
    updatedAt: host.now(),
  };
  unique(host, board);
  host.dashboards().push(board);
  return change(host, board);
}

function editable(host: DashboardHost, board: FakeDashboard): void {
  const key = projectKey(host, board.projectId);
  if (key !== undefined) {
    host.writable(key);
  }
}

export function updateDashboard(host: DashboardHost, input: DashboardUpdateInput) {
  const board = find(host, input.dashboard, input.project);
  editable(host, board);
  const next = { ...board };
  if (input.name !== null && input.name !== undefined) {
    next.name = checkName(host, input.name);
  }
  if (input.description !== null && input.description !== undefined) {
    next.description = input.description.trim();
  }
  if (input.clear?.includes("description") === true) {
    next.description = "";
  }
  const unpins = input.clear?.includes("pin_to") === true;
  if (unpins && input.pin_to !== null && input.pin_to !== undefined) {
    host.fail("invalid", "pin_to is both given and cleared.", "clear");
  }
  if (input.pin_to !== null && input.pin_to !== undefined) {
    next.projectId = host.writable(input.pin_to).id;
  }
  if (unpins) {
    next.projectId = null;
  }
  if (next.name === board.name && next.description === board.description && next.projectId === board.projectId) {
    return { output: { dashboard: out(host, board) }, changes: [] };
  }
  unique(host, next);
  Object.assign(board, next, { updatedAt: host.now() });
  return change(host, board);
}

export function deleteDashboard(host: DashboardHost, input: DashboardRef) {
  const board = find(host, input.dashboard, input.project);
  const answer = change(host, board, undefined, true);
  const all = host.dashboards();
  all.splice(all.indexOf(board), 1);
  return answer;
}

function place(host: DashboardHost, position: number, length: number): number {
  return Number.isInteger(position) && position >= 1 && position <= length ? position - 1 : host.fail("invalid", `Give a place from 1 to ${String(length)}.`, "position");
}

export function addWidget(host: DashboardHost, input: DashboardAddWidgetInput) {
  const board = find(host, input.dashboard, input.project);
  editable(host, board);
  if (board.widgets.length + 1 > WIDGETS_MAX) {
    host.fail("invalid", `A dashboard holds at most ${String(WIDGETS_MAX)} widgets.`, "widgets");
  }
  const widget = newWidget(host, input.widget, board.projectId, "widget.");
  const at = input.position === null || input.position === undefined ? board.widgets.length : place(host, input.position, board.widgets.length + 1);
  board.widgets.splice(at, 0, widget);
  board.updatedAt = host.now();
  return change(host, board, widget);
}

export function updateWidget(host: DashboardHost, input: DashboardUpdateWidgetInput) {
  const board = find(host, input.dashboard, input.project);
  editable(host, board);
  const at = widgetAt(host, board, input.widget);
  const old = board.widgets[at] as FakeWidget;
  const widget: FakeWidget = { ...old, settings: { ...old.settings } };
  if (input.title !== null && input.title !== undefined) {
    widget.title = input.title.trim();
  }
  if (input.width !== null && input.width !== undefined) {
    widget.width = checkWidth(host, input.width, "");
  }
  if ((input.settings !== null && input.settings !== undefined) || (input.clear ?? []).length > 0) {
    // The stored project is an ID; the fake looks projects up by ID too.
    const cleared = new Set<string>(input.clear ?? []);
    const merged = Object.fromEntries(Object.entries({ ...widget.settings, ...(input.settings ?? {}) }).filter(([key]) => !cleared.has(key))) as WidgetSettings;
    widget.settings = checkSettings(host, widget.kind, merged, board.projectId, "");
  }
  const rest = board.widgets.filter((_widget, index) => index !== at);
  const to = input.position === null || input.position === undefined ? at : place(host, input.position, board.widgets.length);
  rest.splice(to, 0, widget);
  const same = to === at && JSON.stringify(widget) === JSON.stringify(old);
  if (same) {
    return { output: { dashboard: out(host, board), widget: widgetOut(host, widget) }, changes: [] };
  }
  board.widgets = rest;
  board.updatedAt = host.now();
  return change(host, board, widget);
}

export function removeWidget(host: DashboardHost, input: DashboardRemoveWidgetInput) {
  const board = find(host, input.dashboard, input.project);
  editable(host, board);
  const at = widgetAt(host, board, input.widget);
  const [widget] = board.widgets.splice(at, 1);
  board.updatedAt = host.now();
  return change(host, board, widget);
}

// ---------------------------------------------------------------------
// Values

/** A value for a query: quoted when it is not a plain word. */
export function quote(text: string): string {
  return /^[\p{L}\p{N}_.-]+$/u.test(text) ? text : `"${text.replace(/["\\]/g, (c) => `\\${c}`)}"`;
}

/** A condition on a field that chooses tasks by their state: `is:closed`, a category, a status, or when a task closed. */
const STATE_CONDITION = /(?:^|[\s(])-?(?:is|category|cat|status|closed)(?::|!=|<=|>=|<|>|=)/i;

/** Whether a query chooses tasks by their state, so that a widget of open tasks leaves the choice to it. */
export function namesState(query: string): boolean {
  return STATE_CONDITION.test(query);
}

/** The widget's query with its project and extra conditions, as the engine combines them. */
export function combined(project: string | undefined, query: string | undefined, extra: readonly string[], sort?: string): string {
  const parts: string[] = [];
  if (project !== undefined) {
    parts.push(`project:${project}/*`);
  }
  const q = query?.trim();
  const sorted = q !== undefined && q.includes("sort:");
  if (q !== undefined && q !== "") {
    const or = q.split(/\s+/).includes("OR");
    parts.push(or && !sorted && (project !== undefined || extra.length > 0) ? `(${q})` : q);
  }
  parts.push(...extra);
  if (sort !== undefined && !sorted) {
    parts.push(`sort:${sort}`);
  }
  return parts.join(" ");
}

function scopeKey(host: DashboardHost, board: FakeDashboard, widget: FakeWidget): string | undefined {
  const id = widget.settings.project ?? board.projectId;
  if (id === null) {
    return undefined;
  }
  return projectKey(host, id) ?? host.fail("not_found", `The widget's project (${id}) no longer exists.`);
}

function tasks(host: DashboardHost, query: string, limit: number, value: WidgetValue): void {
  const found = host.read("task.search", { query, limit }) as TaskSearchResult;
  value.count = found.total;
  value.query = found.query;
  value.tasks = found.items.map((task) => ({ key: task.key, title: task.title, status: task.status, priority: task.priority, ...some("due_date", task.due_date) }));
}

function compute(host: DashboardHost, board: FakeDashboard, widget: FakeWidget, value: WidgetValue): void {
  const s = widget.settings;
  const limit = s.limit ?? 10;
  const days = s.days ?? 7;
  const flow = () => {
    const project = scopeKey(host, board, widget);
    return { ...some("bucket", s.bucket), ...(project === undefined ? {} : { project, include_subprojects: true }), ...some("query", s.query) };
  };
  switch (widget.kind) {
    case "note":
      value.text = s.text ?? "";
      return;
    case "query_count":
      tasks(host, combined(scopeKey(host, board, widget), s.query, []), 1, value);
      delete value.tasks;
      return;
    case "query_list":
      tasks(host, combined(scopeKey(host, board, widget), s.query, []), limit, value);
      return;
    case "due_soon":
      tasks(host, combined(scopeKey(host, board, widget), s.query, ["is:open", "due>=today", `due<=+${String(days)}d`], "due,-priority"), limit, value);
      return;
    case "overdue":
      tasks(host, combined(scopeKey(host, board, widget), s.query, ["is:open", "due<today"], "due,-priority"), limit, value);
      return;
    case "tasks_by_field": {
      const field = s.field ?? "status";
      // Open tasks, narrowed by the query, unless the query chooses tasks by their state itself.
      const given = s.query?.trim() === "" ? undefined : s.query;
      const query = combined(scopeKey(host, board, widget), given, given !== undefined && namesState(given) ? [] : ["is:open"]);
      const builtIn = FIELDS.includes(field);
      const found = host.read("task.search", { query, limit: 1, ...(builtIn ? { group_by: field as ListGrouping } : { group_by_field: field }) }) as TaskSearchResult;
      value.count = found.total;
      value.query = found.query;
      const term = (text: string) => (builtIn ? `${field}:${quote(text)}` : `cf.${field.replace(/^cf\./, "")}:${quote(text)}`);
      value.bars = (found.groups ?? []).map((group) => ({
        count: group.count,
        ...some("value", group.value),
        ...some("hue", group.hue),
        ...some("query", group.value !== undefined ? combined(undefined, query, [term(group.value)]) : field === "label" ? combined(undefined, query, ["-has:labels"]) : undefined),
      }));
      return;
    }
    case "created_closed":
      value.created_closed = host.read("report.created_closed", flow()) as CreatedClosedReport;
      return;
    case "cumulative_flow":
      value.cumulative_flow = host.read("report.cumulative_flow", flow()) as CumulativeFlowReport;
      return;
    case "cycle_time":
      value.cycle_time = host.read("report.cycle_time", { ...flow(), tasks: true }) as CycleTimeReport;
      return;
    case "time_report": {
      const project = scopeKey(host, board, widget);
      const to = host.today();
      const from = new Date(`${to}T00:00:00Z`);
      from.setUTCDate(from.getUTCDate() - (days - 1));
      value.time = host.read("time.report", { from: from.toISOString().slice(0, 10), to, group_by: s.group_by ?? "project", ...(project === undefined ? {} : { project, include_subprojects: true }) }) as TimeReport;
      return;
    }
    case "burndown": {
      const project = scopeKey(host, board, widget) ?? host.fail("invalid", "A burndown widget needs a project.", "settings.project");
      value.burndown = host.read("sprint.burndown", { sprint: "active", project }) as SprintBurndown;
      return;
    }
    case "version_progress": {
      const project = scopeKey(host, board, widget) ?? host.fail("invalid", "A version_progress widget needs a project.", "settings.project");
      value.versions = (host.read("version.list", { project, status: ["open", "locked"] }) as VersionList).items;
      return;
    }
    case "activity": {
      const project = scopeKey(host, board, widget);
      const page = host.read("history.feed", { limit, ...some("project", project), ...some("actor_kind", s.actor_kind), ...(s.days === undefined ? {} : { from: `-${String(s.days - 1)}d` }) }) as ActivityPage;
      value.count = page.total;
      value.activity = page.items;
      return;
    }
    case "automation_failures": {
      const runs = host.read("automation.runs", { problems_only: true, limit, ...some("project", scopeKey(host, board, widget)) }) as AutomationRunList;
      value.count = runs.total;
      value.runs = runs.items;
      return;
    }
  }
}

function valueOf(host: DashboardHost, board: FakeDashboard, widget: FakeWidget): WidgetValue {
  const value: WidgetValue = { widget: widgetOut(host, widget) };
  try {
    compute(host, board, widget, value);
    return value;
  } catch (error) {
    if (typeof error === "object" && error !== null && "error" in error) {
      return { widget: value.widget, error: (error as { error: OpError }).error };
    }
    throw error;
  }
}

export function showDashboard(host: DashboardHost, input: DashboardRef): DashboardShow {
  const board = find(host, input.dashboard, input.project);
  return { dashboard: out(host, board), widgets: board.widgets.map((widget) => valueOf(host, board, widget)) };
}

export function dashboardWidget(host: DashboardHost, input: DashboardWidgetInput): WidgetValue {
  const board = find(host, input.dashboard, input.project);
  const widget = board.widgets[widgetAt(host, board, input.widget)] as FakeWidget;
  return valueOf(host, board, widget);
}
