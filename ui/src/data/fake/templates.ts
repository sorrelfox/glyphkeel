// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Task templates and project templates in the in-memory engine, with the
// engine's rules: names unique per project (or among project templates),
// statuses, types, labels, custom fields and versions named by name and
// left out where a project lacks them (a version also when it is no longer
// open), checklists unticked, dates as days after the day of use.

import {
  type AgentRules,
  type EntityChange,
  type FieldBlueprint,
  type OpMap,
  type ProjectBlueprint,
  type StatusRules,
  type SubtaskBlueprint,
  type TaskBlueprint,
  type Template,
  type TemplateApplied,
  type TemplateChange,
  type TemplateKind,
  type TemplateList,
  type TemplateSummary,
} from "../../generated/ops";
import { fieldsFor, fieldsOf, parseValue, shownFor, taskValues, type FieldValue } from "./fields";
import { type Answer, type FakeHost } from "./host";
import { dayNumber, plusDays } from "./recurrenceRules";
import { nextId } from "./seed";
import { type FakeProject, type FakeTask, type FakeTemplate } from "./state";
import { localDay } from "./time";

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();
const MAX_NAME = 100;
const MAX_DESCRIPTION = 1000;

/** `markdown` with every ticked checklist item cleared. */
export function untick(markdown: string): string {
  return markdown.replace(/^(\s*(?:[-*+]|\d+[.)])\s+\[)[xX](\])/gm, "$1 $2");
}

const offset = (day: string | null, anchor: string) => (day === null ? undefined : dayNumber(day) - dayNumber(anchor));

function plainOf(host: FakeHost, task: FakeTask, anchor: string): Omit<TaskBlueprint, "subtasks"> {
  const project = host.projectOf(task);
  const type = project.types.find((candidate) => candidate.id === task.typeId);
  const labels = project.labels.filter((label) => task.labelIds.includes(label.id)).map((label) => label.name);
  const start = offset(task.startDate, anchor);
  const due = offset(task.dueDate, anchor);
  const values = taskValues(host.state(), task);
  const version = (host.state().versions ?? []).find((candidate) => candidate.id === task.fixVersionId && candidate.status === "open");
  return {
    title: task.title,
    ...(task.description === "" ? {} : { description: untick(task.description) }),
    ...(type ? { type: type.name } : {}),
    ...(task.priority === "none" ? {} : { priority: task.priority }),
    ...(labels.length > 0 ? { labels } : {}),
    ...(task.estimateMin === null ? {} : { estimate_minutes: task.estimateMin }),
    ...(start === undefined ? {} : { start_in_days: start }),
    ...(due === undefined ? {} : { due_in_days: due }),
    ...(values.length > 0 ? { fields: Object.fromEntries(values.map((value) => [value.name, value.value])) } : {}),
    ...(version ? { fix_version: version.name } : {}),
  };
}

/** Gives a task made from a blueprint its version and custom field values; what the project lacks goes to `leftOut`. */
function carry(host: FakeHost, project: FakeProject, task: FakeTask, typeName: string, t: Omit<TaskBlueprint, "subtasks">, leftOut: string[]): void {
  const leave = (name: string) => {
    if (!leftOut.includes(name)) {
      leftOut.push(name);
    }
  };
  if (t.fix_version !== null && t.fix_version !== undefined) {
    const wanted = fold(t.fix_version);
    const version = (host.state().versions ?? []).find((candidate) => candidate.projectId === project.id && fold(candidate.name) === wanted);
    if (version?.status === "open") {
      task.fixVersionId = version.id;
    } else {
      leave(t.fix_version);
    }
  }
  const fields = fieldsFor(host.state(), project.id);
  const values: Record<string, FieldValue> = {};
  const refuse = (): never => {
    throw new Error("left out");
  };
  for (const [name, raw] of Object.entries(t.fields ?? {})) {
    const field = fields.find((candidate) => fold(candidate.name) === fold(name));
    let value: FieldValue | undefined;
    try {
      value = field && shownFor(field, typeName) ? parseValue(field, raw, "fields", refuse) : undefined;
    } catch {
      value = undefined;
    }
    if (field && value !== undefined && values[field.id] === undefined) {
      values[field.id] = value;
    } else {
      leave(name);
    }
  }
  for (const field of fields) {
    if (values[field.id] === undefined && field.default !== undefined && shownFor(field, typeName)) {
      values[field.id] = field.default;
    }
  }
  if (Object.keys(values).length > 0) {
    task.fields = values;
  }
}

/** Task `task` with its subtasks as a blueprint, dates as days after `anchor`. */
export function blueprintOf(host: FakeHost, task: FakeTask, anchor: string): TaskBlueprint {
  const subtasks: SubtaskBlueprint[] = [];
  const index = new Map<string, number>();
  const walk = (parent: FakeTask, under: number | undefined) => {
    for (const sub of host.subtasksOf(parent)) {
      index.set(sub.id, subtasks.length);
      subtasks.push({ ...plainOf(host, sub, anchor), ...(under === undefined ? {} : { under }) });
      walk(sub, index.get(sub.id));
    }
  };
  walk(task, undefined);
  // Only the running sprint is carried, as `active`.
  const sprint = (host.state().sprints ?? []).find((candidate) => candidate.id === task.sprintId);
  return { ...plainOf(host, task, anchor), ...(sprint?.state === "active" ? { sprint: ACTIVE_SPRINT } : {}), ...(subtasks.length > 0 ? { subtasks } : {}) };
}

/** The one sprint a blueprint may name. */
const ACTIVE_SPRINT = "active";

/** Refuses a blueprint that names a sprint other than `active`. */
function checkSprint(host: FakeHost, bp: TaskBlueprint | null | undefined): void {
  const sprint = bp?.sprint;
  if (sprint !== null && sprint !== undefined && fold(sprint) !== ACTIVE_SPRINT) {
    host.fail("invalid", `A template can only name the sprint \`active\` (the sprint running when a task is made from it), not "${sprint}".`, {
      details: { path: "task.sprint", message_id: "template-sprint-active-only", args: { sprint } },
    });
  }
}

function writeOne(host: FakeHost, project: FakeProject, t: Omit<TaskBlueprint, "subtasks">, anchor: string, parentId: string | null, statusId: string | null, leftOut: string[]): FakeTask {
  const named = t.type ?? null;
  let type = named === null ? undefined : project.types.find((candidate) => fold(candidate.name) === fold(named));
  if (!type) {
    if (named !== null && !leftOut.includes(named)) {
      leftOut.push(named);
    }
    type = project.types.find((candidate) => candidate.is_default) ?? project.types[0];
  }
  if (!type) {
    host.fail("invalid", `Project ${project.key} has no task types.`);
  }
  const labelIds: string[] = [];
  for (const name of t.labels ?? []) {
    const label = project.labels.find((candidate) => fold(candidate.name) === fold(name));
    if (label) {
      labelIds.push(label.id);
    } else if (!leftOut.includes(name)) {
      leftOut.push(name);
    }
  }
  let start = t.start_in_days === null || t.start_in_days === undefined ? null : plusDays(anchor, t.start_in_days);
  let due = t.due_in_days === null || t.due_in_days === undefined ? null : plusDays(anchor, t.due_in_days);
  if (due === null && type.due_offset_days !== undefined) {
    const byType = plusDays(host.today(), type.due_offset_days);
    due = start !== null && start > byType ? null : byType;
  }
  if (start !== null && due !== null && start > due) {
    start = null;
  }
  const task = host.insertTask(project, {
    title: t.title,
    description: t.description ?? "",
    statusId,
    typeId: type.id,
    priority: t.priority ?? "none",
    labelIds,
    parentId,
    startDate: start,
    dueDate: due,
    estimateMin: t.estimate_minutes === null || t.estimate_minutes === undefined || t.estimate_minutes < 0 ? null : t.estimate_minutes,
  });
  carry(host, project, task, type.name, t, leftOut);
  return task;
}

/** Makes the task `bp` describes with its subtasks; returns it and every task made. */
export function makeTask(host: FakeHost, project: FakeProject, bp: TaskBlueprint, title: string | null, anchor: string, parentId: string | null, statusId: string | null, leftOut: string[]): { task: FakeTask; made: FakeTask[] } {
  const root = writeOne(host, project, { ...bp, title: title ?? bp.title }, anchor, parentId, statusId, leftOut);
  if (bp.sprint !== null && bp.sprint !== undefined && fold(bp.sprint) === ACTIVE_SPRINT) {
    const running = (host.state().sprints ?? []).find((candidate) => candidate.projectId === project.id && candidate.state === "active");
    if (running) {
      root.sprintId = running.id;
    }
  }
  const made: FakeTask[] = [root];
  const subs: FakeTask[] = [];
  for (const sub of bp.subtasks ?? []) {
    const under = sub.under === null || sub.under === undefined ? root : (subs[sub.under] ?? root);
    const task = writeOne(host, project, sub, anchor, under.id, null, leftOut);
    subs.push(task);
    made.push(task);
  }
  return { task: root, made };
}

// ---------- the records ----------

const all = (host: FakeHost) => (host.state().templates ??= []);

/** The templates that come with the program, as the engine has them: in every workspace, never changed. */
const BUILT_IN: readonly (FakeTemplate & { key: string })[] = [
  {
    id: "019ab000-0000-7000-8000-000000000001",
    key: "software",
    kind: "project",
    projectId: null,
    name: "Software project",
    description: "Epics, stories, tasks and bugs on a board from the backlog through review to done.",
    setup: {
      statuses: [
        { name: "Backlog", category: "todo", hue: "grey" },
        { name: "To do", category: "todo", hue: "blue" },
        { name: "In progress", category: "in_progress", hue: "orange", wip_limit: 5 },
        { name: "In review", category: "in_progress", hue: "purple", wip_limit: 3 },
        { name: "Done", category: "done", hue: "green", rules: { stop_timer: true, release_claim: true, agents_may_not_enter: true } },
        { name: "Won't do", category: "cancelled", hue: "grey" },
      ],
      types: [
        { name: "Story", glyph: "story", hue: "green", is_default: true },
        { name: "Task", glyph: "task", hue: "blue" },
        { name: "Bug", glyph: "bug", hue: "red" },
        { name: "Epic", glyph: "epic", hue: "purple" },
      ],
      labels: [
        { name: "Frontend", hue: "teal" },
        { name: "Backend", hue: "blue" },
        { name: "Documentation", hue: "yellow" },
      ],
      // A bug needs a severity: it starts at "Major", also for a task whose type becomes Bug.
      fields: [
        { id: "019ab000-0000-7000-8000-000000000101", name: "Severity", kind: "select", options: ["Critical", "Major", "Minor", "Trivial"], default: "Major", types: ["Bug"], required: true },
        { id: "019ab000-0000-7000-8000-000000000102", name: "Story points", kind: "integer", types: ["Story", "Task"] },
      ],
      wiki: [
        { title: "Overview", body: "## Goal\n\nWhat the software is for, and for whom.\n" },
        { title: "Releases", body: "## Next release\n\n## Earlier releases\n" },
      ],
      task_templates: [
        { name: "Story", description: "A user story with its acceptance criteria.", task: { title: "Story", type: "Story", description: "As a …, I want … so that ….\n\n## Acceptance criteria\n\n- [ ] \n" } },
        { name: "Bug report", description: "A bug with the steps that show it.", task: { title: "Bug", type: "Bug", priority: "high", fields: { Severity: "Major" }, description: "## Steps to reproduce\n\n1. \n\n## Expected\n\n## Actual\n" } },
      ],
    },
    rank: 0,
    createdAt: "1970-01-01T00:00:00Z",
    updatedAt: "1970-01-01T00:00:00Z",
  },
];

const builtInKey = (template: FakeTemplate) => BUILT_IN.find((each) => each.id === template.id)?.key;

/** The `built_in` key of a template that comes with Glyphkeel, left out for the workspace's own. */
function builtInField(template: FakeTemplate): { built_in?: string } {
  const key = builtInKey(template);
  return key === undefined ? {} : { built_in: key };
}

function refuseBuiltIn(host: FakeHost, template: FakeTemplate): void {
  if (builtInKey(template) !== undefined) {
    host.fail("invalid", `The template "${template.name}" comes with Glyphkeel and can't be changed or deleted.`, {
      hint: "Make a project from it, change that project, and save it as a template of your own.",
      details: { path: "template", message_id: "template-built-in", args: { template: template.name } },
    });
  }
}

const projectKey = (host: FakeHost, template: FakeTemplate) => {
  const project = host.state().projects.find((candidate) => candidate.id === template.projectId);
  return project?.key;
};

function output(host: FakeHost, template: FakeTemplate): Template {
  const key = projectKey(host, template);
  return {
    id: template.id,
    name: template.name,
    kind: template.kind,
    ...(key === undefined ? {} : { project: key }),
    ...(template.description === "" ? {} : { description: template.description }),
    ...(template.task ? { task: structuredClone(template.task) } : {}),
    ...(template.setup ? { setup: structuredClone(template.setup) } : {}),
    rank: String(template.rank),
    created_by: { kind: "user", name: builtInKey(template) === undefined ? "You" : "Glyphkeel" },
    created_at: template.createdAt,
    updated_at: template.updatedAt,
    ...builtInField(template),
  };
}

function summary(host: FakeHost, template: FakeTemplate): TemplateSummary {
  const key = projectKey(host, template);
  const items = template.task ? 1 + (template.task.subtasks?.length ?? 0) : (template.setup?.tasks ?? []).reduce((sum, task) => sum + 1 + (task.subtasks?.length ?? 0), 0) + (template.setup?.wiki?.length ?? 0);
  return {
    id: template.id,
    name: template.name,
    kind: template.kind,
    ...(key === undefined ? {} : { project: key }),
    ...(template.description === "" ? {} : { description: template.description }),
    ...(template.task ? { title: template.task.title } : {}),
    items,
    ...builtInField(template),
  };
}

const ordered = (host: FakeHost) => {
  const projects = host.state().projects.map((project) => project.id);
  return [...BUILT_IN, ...all(host)].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === "project" ? -1 : 1) || projects.indexOf(a.projectId ?? "") - projects.indexOf(b.projectId ?? "") || a.rank - b.rank);
};

/** The template `text` names: its ID, or its name among the project templates and the task templates of `project`. */
function find(host: FakeHost, text: string, project: string | null | undefined): FakeTemplate {
  const byId = [...all(host), ...BUILT_IN].find((template) => template.id === text.trim());
  if (byId) {
    return byId;
  }
  const scope = project === null || project === undefined ? null : host.project(project).id;
  const candidates = ordered(host).filter((template) => template.kind === "project" || scope === null || template.projectId === scope);
  let found = candidates.filter((template) => fold(template.name) === fold(text));
  if (found.length > 1 && scope !== null) {
    found = found.filter((template) => template.kind === "task");
  }
  // The workspace's own template wins over a built-in one of its name.
  if (found.length > 1) {
    found = found.filter((template) => builtInKey(template) === undefined);
  }
  const [one] = found;
  if (found.length === 1 && one) {
    return one;
  }
  if (found.length === 0) {
    host.fail("not_found", `There is no template "${text.trim()}".`, { details: { field: "template", choices: candidates.map((template) => template.name), message_id: "template-not-found", args: { template: text.trim() } } });
  }
  host.fail("invalid", `Several templates are named "${text.trim()}".`, { details: { path: "template", message_id: "template-name-ambiguous", args: { template: text.trim() } } });
}

function checkName(host: FakeHost, name: string): string {
  const trimmed = name.trim();
  if (trimmed === "" || trimmed.length > MAX_NAME) {
    host.fail("invalid", `A template's name is 1 to ${String(MAX_NAME)} characters.`, { details: { path: "name" } });
  }
  return trimmed;
}

function checkDescription(host: FakeHost, text: string | null | undefined): string {
  const trimmed = (text ?? "").trim();
  if (trimmed.length > MAX_DESCRIPTION) {
    host.fail("invalid", `A template's description is at most ${String(MAX_DESCRIPTION)} characters.`, { details: { path: "description" } });
  }
  return trimmed;
}

function checkUnique(host: FakeHost, template: Pick<FakeTemplate, "id" | "kind" | "projectId" | "name">): void {
  const taken = all(host).some((other) => other.id !== template.id && other.kind === template.kind && other.projectId === template.projectId && fold(other.name) === fold(template.name));
  if (taken) {
    host.fail("conflict", `There is already a template named "${template.name}".`, { hint: "Choose another name, or change that template.", details: { message_id: "template-name-taken", args: { template: template.name } } });
  }
}

function change(host: FakeHost, template: FakeTemplate): EntityChange {
  const key = projectKey(host, template);
  return { entity: "template", id: template.id, fields: ["*"], ...(key === undefined ? {} : { project: key }) };
}

/** Adds a template at the end of its list. */
function add(host: FakeHost, fields: Pick<FakeTemplate, "kind" | "projectId" | "name" | "description" | "task" | "setup">): FakeTemplate {
  checkUnique(host, { id: "", ...fields });
  const now = host.now();
  const rank = Math.max(0, ...all(host).filter((other) => other.kind === fields.kind && other.projectId === fields.projectId).map((other) => other.rank)) + 1;
  const template: FakeTemplate = { id: nextId("tpl"), ...fields, rank, createdAt: now, updatedAt: now };
  all(host).push(template);
  return template;
}

/** Project `project`'s set-up as a blueprint; with `tasks`, its open top-level tasks too. */
function setupOf(host: FakeHost, project: FakeProject, tasks: boolean): ProjectBlueprint {
  const pages = (host.state().wiki ?? []).filter((page) => page.projectId === project.id);
  const wiki: NonNullable<ProjectBlueprint["wiki"]> = [];
  const walk = (parent: string | null) => {
    for (const page of pages.filter((candidate) => candidate.parentId === parent)) {
      const above = pages.find((candidate) => candidate.id === parent);
      wiki.push({ title: page.title, ...(page.body === "" ? {} : { body: page.body }), ...(above ? { parent: above.title } : {}) });
      walk(page.id);
    }
  };
  walk(null);
  const today = host.today();
  // The project's own custom fields; the rules name them by their ID here.
  const own = fieldsOf(host.state()).filter((field) => field.projectId === project.id);
  const fields: FieldBlueprint[] = own.map((field) => ({
    id: field.id,
    name: field.name,
    kind: field.kind,
    ...(field.options.length > 0 ? { options: [...field.options] } : {}),
    ...(field.default === undefined ? {} : { default: structuredClone(field.default) }),
    ...(field.types.length > 0 ? { types: [...field.types] } : {}),
    ...(field.required ? { required: true } : {}),
  }));
  // Rules name fields as `cf:<id>`: those of the project's own fields get the new project's, the others stay.
  const rulesOf = (rules: StatusRules | undefined): StatusRules | undefined => (rules && !isEmptyRules(rules) ? structuredClone(rules) : undefined);
  const agentRules = project.agentRules?.read_only && project.agentRules.read_only.length > 0 ? structuredClone(project.agentRules) : undefined;
  return {
    ...(project.description === "" ? {} : { description: project.description }),
    modules: { ...project.modules },
    statuses: project.statuses.map((status) => {
      const rules = rulesOf(status.rules);
      return {
        name: status.name,
        category: status.category,
        hue: status.hue,
        ...(status.wip_limit === null ? {} : { wip_limit: status.wip_limit }),
        ...(status.wip_min === null || status.wip_min === undefined ? {} : { wip_min: status.wip_min }),
        ...(rules ? { rules } : {}),
      };
    }),
    ...(fields.length > 0 ? { fields } : {}),
    ...(agentRules ? { agent_rules: agentRules } : {}),
    types: project.types.map((type) => ({ name: type.name, glyph: type.glyph, hue: type.hue, ...(type.is_default ? { is_default: true } : {}), ...(type.due_offset_days === undefined ? {} : { due_offset_days: type.due_offset_days }) })),
    ...(project.labels.length > 0 ? { labels: project.labels.map((label) => ({ name: label.name, hue: label.hue })) } : {}),
    ...(wiki.length > 0 ? { wiki } : {}),
    ...(tasks
      ? { tasks: host.state().tasks.filter((task) => task.projectId === project.id && task.parentId === null && task.closedAt === null).slice(0, 500).map((task) => blueprintOf(host, task, today)) }
      : {}),
    task_templates: ordered(host)
      .filter((template) => template.kind === "task" && template.projectId === project.id && template.task)
      .map((template) => ({ name: template.name, ...(template.description === "" ? {} : { description: template.description }), task: structuredClone(template.task as TaskBlueprint) })),
  };
}

/** Whether a status has no rules at all. */
function isEmptyRules(rules: StatusRules): boolean {
  return (
    (rules.required ?? []).length === 0 &&
    (rules.read_only ?? []).length === 0 &&
    (rules.clear ?? []).length === 0 &&
    (rules.set ?? []).length === 0 &&
    rules.stop_timer !== true &&
    rules.release_claim !== true &&
    rules.agents_may_not_enter !== true
  );
}

/** Rules with the template's field IDs (`cf:<id>`) turned into the new project's. */
function mapRules<T extends StatusRules | AgentRules>(rules: T, copies: ReadonlyMap<string, string>): T {
  const one = (name: string) => (name.startsWith("cf:") ? `cf:${copies.get(name.slice(3)) ?? name.slice(3)}` : name);
  const out: Record<string, unknown> = { ...rules };
  for (const list of ["required", "read_only", "clear"] as const) {
    const items = (rules as Record<string, unknown>)[list];
    if (Array.isArray(items)) {
      out[list] = items.map((item) => one(String(item)));
    }
  }
  const set = (rules as StatusRules).set;
  if (set) {
    out["set"] = set.map((each) => ({ ...each, field: one(each.field) }));
  }
  return out as T;
}

/**
 * Refuses a set-up whose task, task template or one of their subtasks lacks a value for one of its required fields
 * without a default: the template couldn't be used. With the default types, only fields of every type are checked.
 */
function checkRequired(host: FakeHost, setup: ProjectBlueprint): ProjectBlueprint {
  const required = (setup.fields ?? []).filter((field) => field.required === true && (field.default === null || field.default === undefined));
  if (required.length === 0) {
    return setup;
  }
  const types = setup.types ?? [];
  const typeOf = (name: string | null | undefined): string | null => {
    if (types.length === 0) {
      return null;
    }
    const named = name === null || name === undefined ? undefined : types.find((type) => fold(type.name) === fold(name));
    return (named ?? types.find((type) => type.is_default === true) ?? types[0])?.name ?? null;
  };
  const check = (task: { type?: string | null; fields?: Record<string, unknown> | null }, path: string) => {
    const type = typeOf(task.type);
    const given = Object.keys(task.fields ?? {}).map((key) => fold(key.trim().replace(/^cf\./i, "")));
    for (const field of required) {
      const shown = (field.types ?? []).length === 0 || (type !== null && (field.types ?? []).some((each) => fold(each) === fold(type)));
      if (shown && !Object.keys(task.fields ?? {}).includes(field.id) && !given.includes(fold(field.name))) {
        host.fail("invalid", `${field.name} is required; give it a value.`, { details: { path: `${path}.fields.${field.name}`, message_id: "field-required", args: { field: field.name } } });
      }
    }
  };
  const walk = (task: TaskBlueprint, path: string) => {
    check(task, path);
    (task.subtasks ?? []).forEach((sub, index) => {
      check(sub, `${path}.subtasks[${String(index)}]`);
    });
  };
  (setup.tasks ?? []).forEach((task, index) => {
    walk(task, `setup.tasks[${String(index)}]`);
  });
  (setup.task_templates ?? []).forEach((named, index) => {
    walk(named.task, `setup.task_templates[${String(index)}].task`);
  });
  return setup;
}

/** Gives a new project the set-up's statuses, types, labels, modules, custom fields and rules. */
function setUp(host: FakeHost, project: FakeProject, setup: ProjectBlueprint): void {
  if (setup.statuses && setup.statuses.length > 0) {
    project.statuses = setup.statuses.map((status) => ({
      id: nextId("st"),
      name: status.name,
      category: status.category,
      hue: status.hue,
      wip_limit: status.wip_limit ?? null,
      ...(status.wip_min === null || status.wip_min === undefined ? {} : { wip_min: status.wip_min }),
    }));
  }
  if (setup.types && setup.types.length > 0) {
    project.types = setup.types.map((type) => ({ id: nextId("ty"), name: type.name, glyph: type.glyph, hue: type.hue, is_default: type.is_default === true, ...(type.due_offset_days === null || type.due_offset_days === undefined ? {} : { due_offset_days: type.due_offset_days }) }));
    if (!project.types.some((type) => type.is_default) && project.types[0]) {
      project.types[0].is_default = true;
    }
  }
  if (setup.labels && setup.labels.length > 0) {
    project.labels = setup.labels.map((label) => ({ id: nextId("lb"), name: label.name, hue: label.hue }));
  }
  if (setup.modules) {
    project.modules = { ...setup.modules };
  }
  // Fields after the types they show for; rules after the fields they name.
  const copies = new Map<string, string>();
  const known = project.types.map((type) => type.name);
  for (const field of setup.fields ?? []) {
    const id = nextId("cf");
    copies.set(field.id, id);
    fieldsOf(host.state()).push({
      id,
      projectId: project.id,
      name: field.name,
      kind: field.kind,
      options: [...(field.options ?? [])],
      ...(field.default === undefined || field.default === null ? {} : { default: structuredClone(field.default) as FieldValue }),
      types: (field.types ?? []).filter((name) => known.some((type) => fold(type) === fold(name))),
      required: field.required === true,
    });
  }
  (setup.statuses ?? []).forEach((status, index) => {
    const made = project.statuses[index];
    if (made && status.rules && !isEmptyRules(status.rules)) {
      made.rules = mapRules(status.rules, copies);
    }
  });
  if (setup.agent_rules?.read_only && setup.agent_rules.read_only.length > 0) {
    project.agentRules = mapRules(setup.agent_rules, copies);
  }
}

type Handlers = { [K in "template.list" | "template.get" | "template.create" | "template.update" | "template.delete" | "template.apply"]: (input: OpMap[K]["input"]) => OpMap[K]["output"] | Answer<OpMap[K]["output"]> };

/** The template operations. */
export function templateHandlers(host: FakeHost): Handlers {
  return {
    "template.list": ({ project, kind }): TemplateList => {
      const scope = project === null || project === undefined ? null : host.project(project).id;
      const items = ordered(host)
        .filter((template) => (kind === null || kind === undefined || template.kind === kind) && (template.kind === "project" || scope === null || template.projectId === scope))
        .map((template) => summary(host, template));
      return { items, total: items.length };
    },
    "template.get": ({ template, project }) => output(host, find(host, template, project)),
    "template.create": (input): Answer<TemplateChange> => {
      const name = checkName(host, input.name);
      const description = checkDescription(host, input.description);
      const kind: TemplateKind = input.kind ?? (input.from_project !== null && input.from_project !== undefined ? "project" : input.setup ? "project" : "task");
      const sources = [input.from_task, input.from_project, input.task, input.setup].filter((value) => value !== null && value !== undefined).length;
      if (sources !== 1) {
        host.fail("invalid", "Give exactly one of `from_task`, `from_project`, `task` and `setup`.", { details: { path: "from_task", message_id: "template-one-source" } });
      }
      let template: FakeTemplate;
      if (kind === "task") {
        if ((input.from_project ?? input.setup) !== undefined && (input.from_project ?? input.setup) !== null) {
          host.fail("invalid", "A task template holds a task, and a project template a project's set-up.", { details: { path: "kind", message_id: "template-kind-mismatch" } });
        }
        let projectId: string;
        let task: TaskBlueprint;
        if (input.from_task !== null && input.from_task !== undefined) {
          const source = host.task(input.from_task);
          projectId = input.project === null || input.project === undefined ? source.projectId : host.project(input.project).id;
          task = blueprintOf(host, source, localDay(new Date(source.createdAt)));
        } else {
          if (input.project === null || input.project === undefined) {
            host.fail("invalid", "A task template belongs to a project: give `project`.", { details: { path: "project", message_id: "template-needs-project" } });
          }
          projectId = host.project(input.project).id;
          checkSprint(host, input.task);
          task = structuredClone(input.task ?? { title: name });
        }
        host.writable(projectId);
        template = add(host, { kind, projectId, name, description, task });
      } else {
        if ((input.from_task ?? input.task) !== undefined && (input.from_task ?? input.task) !== null) {
          host.fail("invalid", "A task template holds a task, and a project template a project's set-up.", { details: { path: "kind", message_id: "template-kind-mismatch" } });
        }
        const setup = input.from_project !== null && input.from_project !== undefined ? setupOf(host, host.project(input.from_project), input.include_tasks === true) : checkRequired(host, structuredClone(input.setup ?? {}));
        template = add(host, { kind, projectId: null, name, description, setup });
      }
      return { output: { template: output(host, template) }, changes: [change(host, template)] };
    },
    "template.update": (input): Answer<TemplateChange> | TemplateChange => {
      const template = find(host, input.template, input.project);
      refuseBuiltIn(host, template);
      const next = { ...template };
      if (input.name !== null && input.name !== undefined) {
        next.name = checkName(host, input.name);
      }
      if (input.description !== null && input.description !== undefined) {
        next.description = checkDescription(host, input.description);
      }
      if ((input.task && template.kind !== "task") || (input.setup && template.kind !== "project")) {
        host.fail("invalid", "A task template holds a task, and a project template a project's set-up.", { details: { path: "task", message_id: "template-kind-mismatch" } });
      }
      checkSprint(host, input.task);
      if (input.task) next.task = structuredClone(input.task);
      if (input.setup) next.setup = checkRequired(host, structuredClone(input.setup));
      if (JSON.stringify(next) === JSON.stringify(template)) {
        return { template: output(host, template) };
      }
      checkUnique(host, next);
      Object.assign(template, next, { updatedAt: host.now() });
      return { output: { template: output(host, template) }, changes: [change(host, template)] };
    },
    "template.delete": ({ template: ref, project }): Answer<TemplateChange> => {
      const template = find(host, ref, project);
      refuseBuiltIn(host, template);
      const before = output(host, template);
      const entity = change(host, template);
      host.state().templates = all(host).filter((other) => other !== template);
      return { output: { template: before, deleted: true }, changes: [entity] };
    },
    "template.apply": (input): Answer<TemplateApplied> => {
      const template = find(host, input.template, input.project);
      const anchor = input.date === null || input.date === undefined ? host.today() : host.dayInput(input.date, "date");
      const leftOut: string[] = [];
      if (template.kind === "task") {
        const bp = template.task ?? host.fail("storage", "The template's content can't be read.");
        const ref = input.project ?? template.projectId ?? host.fail("invalid", "A task template belongs to a project: give `project`.", { details: { path: "project" } });
        const project = host.writable(ref);
        let statusId: string | null = null;
        if (input.status !== null && input.status !== undefined) {
          const wanted = input.status;
          const status = project.statuses.find((candidate) => candidate.id === wanted || fold(candidate.name) === fold(wanted));
          if (!status) {
            host.fail("invalid", `Project ${project.key} has no status "${wanted}".`, { details: { path: "status", choices: project.statuses.map((candidate) => candidate.name) } });
          }
          statusId = status.id;
        }
        let parentId: string | null = null;
        if (input.parent !== null && input.parent !== undefined) {
          const parent = host.task(input.parent);
          if (parent.projectId !== project.id) {
            host.fail("invalid", "A subtask belongs to its parent's project.", { details: { path: "parent" } });
          }
          parentId = parent.id;
        }
        const title = input.title === null || input.title === undefined ? null : input.title.trim();
        // A due date or priority given with the template wins over its own; an unread date makes nothing.
        const due = input.due_date === null || input.due_date === undefined ? null : host.dayInput(input.due_date, "due_date");
        const start = bp.start_in_days === null || bp.start_in_days === undefined ? null : plusDays(anchor, bp.start_in_days);
        if (due !== null && start !== null && start > due) {
          host.fail("invalid", `The start date ${start} is after the due date ${due}.`, { details: { path: "due_date", message_id: "value-day-start-after-due", args: { start, due } } });
        }
        const { task, made } = makeTask(host, project, bp, title === "" ? null : title, anchor, parentId, statusId, leftOut);
        if (due !== null) {
          task.dueDate = due;
        }
        if (input.priority !== null && input.priority !== undefined) {
          task.priority = input.priority;
        }
        return {
          output: { task: host.fullTask(task), tasks: made.length, ...(leftOut.length > 0 ? { left_out: leftOut } : {}) },
          changes: made.map((each) => ({ entity: "task", id: each.id, fields: ["*"], project: project.key })),
        };
      }
      const setup = template.setup ?? {};
      if (input.name === null || input.name === undefined || input.key === null || input.key === undefined) {
        host.fail("invalid", "A project template makes a project: give its `name` and `key`.", { details: { path: "name", message_id: "template-project-needs-name" } });
      }
      const project = host.createProject({ name: input.name, key: input.key, parent: input.parent_project ?? null, folder: input.folder ?? null, hue: input.hue ?? null, description: setup.description ?? "" });
      setUp(host, project, setup);
      if (project.modules.wiki) {
        for (const page of setup.wiki ?? []) {
          host.createPage(project, page.title, page.body ?? "", page.parent ?? null);
        }
      }
      const changes: EntityChange[] = [{ entity: "project", id: project.id, fields: ["*"], project: project.key }];
      for (const named of setup.task_templates ?? []) {
        const made = add(host, { kind: "task", projectId: project.id, name: named.name, description: named.description ?? "", task: structuredClone(named.task) });
        changes.push(change(host, made));
      }
      let tasks = 0;
      for (const bp of setup.tasks ?? []) {
        const { made } = makeTask(host, project, bp, null, anchor, null, null, leftOut);
        tasks += made.length;
        changes.push(...made.map((each) => ({ entity: "task", id: each.id, fields: ["*"], project: project.key })));
      }
      return { output: { project: host.fullProject(project), tasks, ...(leftOut.length > 0 ? { left_out: leftOut } : {}) }, changes };
    },
  };
}
