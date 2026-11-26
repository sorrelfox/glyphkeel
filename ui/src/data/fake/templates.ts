// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Task templates and project templates in the in-memory engine, with the
// engine's rules: names unique per project (or among project templates),
// statuses, types, labels, custom fields and versions named by name and
// left out where a project lacks them (a version also when it is no longer
// open), checklists unticked, dates as days after the day of use.

import {
  type EntityChange,
  type OpMap,
  type ProjectBlueprint,
  type SubtaskBlueprint,
  type TaskBlueprint,
  type Template,
  type TemplateApplied,
  type TemplateChange,
  type TemplateKind,
  type TemplateList,
  type TemplateSummary,
} from "../../generated/ops";
import { fieldsFor, parseValue, shownFor, taskValues, type FieldValue } from "./fields";
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
  return { ...plainOf(host, task, anchor), ...(subtasks.length > 0 ? { subtasks } : {}) };
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
    created_by: { kind: "user", name: "You" },
    created_at: template.createdAt,
    updated_at: template.updatedAt,
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
  };
}

const ordered = (host: FakeHost) => {
  const projects = host.state().projects.map((project) => project.id);
  return [...all(host)].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === "project" ? -1 : 1) || projects.indexOf(a.projectId ?? "") - projects.indexOf(b.projectId ?? "") || a.rank - b.rank);
};

/** The template `text` names: its ID, or its name among the project templates and the task templates of `project`. */
function find(host: FakeHost, text: string, project: string | null | undefined): FakeTemplate {
  const byId = all(host).find((template) => template.id === text.trim());
  if (byId) {
    return byId;
  }
  const scope = project === null || project === undefined ? null : host.project(project).id;
  const candidates = ordered(host).filter((template) => template.kind === "project" || scope === null || template.projectId === scope);
  let found = candidates.filter((template) => fold(template.name) === fold(text));
  if (found.length > 1 && scope !== null) {
    found = found.filter((template) => template.kind === "task");
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
  return {
    ...(project.description === "" ? {} : { description: project.description }),
    modules: { ...project.modules },
    statuses: project.statuses.map((status) => ({ name: status.name, category: status.category, hue: status.hue, ...(status.wip_limit === null ? {} : { wip_limit: status.wip_limit }) })),
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

/** Gives a new project the set-up's statuses, types, labels and modules. */
function setUp(project: FakeProject, setup: ProjectBlueprint): void {
  if (setup.statuses && setup.statuses.length > 0) {
    project.statuses = setup.statuses.map((status) => ({ id: nextId("st"), name: status.name, category: status.category, hue: status.hue, wip_limit: status.wip_limit ?? null }));
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
          task = structuredClone(input.task ?? { title: name });
        }
        host.writable(projectId);
        template = add(host, { kind, projectId, name, description, task });
      } else {
        if ((input.from_task ?? input.task) !== undefined && (input.from_task ?? input.task) !== null) {
          host.fail("invalid", "A task template holds a task, and a project template a project's set-up.", { details: { path: "kind", message_id: "template-kind-mismatch" } });
        }
        const setup = input.from_project !== null && input.from_project !== undefined ? setupOf(host, host.project(input.from_project), input.include_tasks === true) : structuredClone(input.setup ?? {});
        template = add(host, { kind, projectId: null, name, description, setup });
      }
      return { output: { template: output(host, template) }, changes: [change(host, template)] };
    },
    "template.update": (input): Answer<TemplateChange> | TemplateChange => {
      const template = find(host, input.template, input.project);
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
      if (input.task) next.task = structuredClone(input.task);
      if (input.setup) next.setup = structuredClone(input.setup);
      if (JSON.stringify(next) === JSON.stringify(template)) {
        return { template: output(host, template) };
      }
      checkUnique(host, next);
      Object.assign(template, next, { updatedAt: host.now() });
      return { output: { template: output(host, template) }, changes: [change(host, template)] };
    },
    "template.delete": ({ template: ref, project }): Answer<TemplateChange> => {
      const template = find(host, ref, project);
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
        const { task, made } = makeTask(host, project, bp, title === "" ? null : title, anchor, parentId, statusId, leftOut);
        return {
          output: { task: host.fullTask(task), tasks: made.length, ...(leftOut.length > 0 ? { left_out: leftOut } : {}) },
          changes: made.map((each) => ({ entity: "task", id: each.id, fields: ["*"], project: project.key })),
        };
      }
      const setup = template.setup ?? {};
      if (input.name === null || input.name === undefined || input.key === null || input.key === undefined) {
        host.fail("invalid", "A project template makes a project: give its `name` and `key`.", { details: { path: "name", message_id: "template-project-needs-name" } });
      }
      const project = host.createProject({ name: input.name, key: input.key, parent: input.parent_project ?? null, hue: input.hue ?? null, description: setup.description ?? "" });
      setUp(project, setup);
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
