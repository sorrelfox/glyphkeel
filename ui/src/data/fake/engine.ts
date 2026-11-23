// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// An engine stand-in that keeps a workspace in memory and answers the
// operations the screens use, with the engine's rules where the screens can
// see them: keys, default statuses, closing, WIP, sub-project columns,
// layouts, typed errors, change events and Undo.

import { type EntityChange, type OpError, type Settings, type SettingsPatch } from "../../generated/ops";
import {
  DEFAULT_LAYOUT,
  type AppOpId,
  type AppOpMap,
  type Board,
  type BoardColumn,
  type BoardLayout,
  type Card,
  type Counts,
  type Hue,
  type Position,
  type Project,
  type ProjectChange,
  type ProjectSummary,
  type Relation,
  type Task,
  type TaskChange,
  type TaskRef,
} from "../catalog";
import { dayOffset, newProject, nextId } from "./seed";
import { type FakeProject, type FakeState, type FakeTask } from "./state";

/** An operation failed: the engine's error, as the transports deliver it. */
export class FakeError extends Error {
  readonly error: OpError;
  constructor(code: OpError["code"], message: string, extra: { hint?: string; details?: Record<string, unknown> } = {}) {
    super(message);
    this.name = "FakeError";
    this.error = { code, message, ...extra };
  }
}

function fail(code: OpError["code"], message: string, extra: { hint?: string; details?: Record<string, unknown> } = {}): never {
  throw new FakeError(code, message, extra);
}

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();
const isClosed = (category: string) => category === "done" || category === "cancelled";
const MAX_TITLE = 500;
const MAX_DEPTH = 3;
const KEY_PATTERN = /^[A-Z][A-Z0-9]{1,9}$/;

/** What one command changed, for its change event. */
interface Result<O> {
  output: O;
  changes: EntityChange[];
}

type Handler<K extends AppOpId> = (input: AppOpMap[K]["input"]) => AppOpMap[K]["output"] | Result<AppOpMap[K]["output"]>;

export interface FakeAnswer {
  output: unknown;
  /** Present when the call changed something. */
  changes?: { group: string; seq: number; changes: EntityChange[] };
}

export class FakeWorkspace {
  private state: FakeState;
  private readonly today: () => Date;
  private undoStack: { state: FakeState; op: string; group: string }[] = [];
  private redoStack: { state: FakeState; op: string; group: string }[] = [];

  constructor(state: FakeState, today: () => Date = () => new Date()) {
    this.state = state;
    this.today = today;
  }

  /** Runs one operation. Commands record an Undo step and return their changes. */
  call(op: string, input: unknown): FakeAnswer {
    if (op === "history.undo" || op === "history.redo") {
      return this.undoOrRedo(op);
    }
    const handler = (this.handlers as Record<string, ((input: unknown) => unknown) | undefined>)[op];
    if (!handler) {
      fail("not_found", `There is no operation "${op}".`);
    }
    const before = structuredClone(this.state);
    const answer = handler(input ?? {});
    if (!isResult(answer)) {
      return { output: answer };
    }
    if (answer.changes.length === 0) {
      return { output: answer.output };
    }
    this.state.seq += 1;
    const group = `g-${String(this.state.seq)}`;
    this.undoStack.push({ state: before, op, group });
    this.redoStack = [];
    return { output: withGroup(answer.output, group), changes: { group, seq: this.state.seq, changes: answer.changes } };
  }

  private undoOrRedo(op: "history.undo" | "history.redo"): FakeAnswer {
    const from = op === "history.undo" ? this.undoStack : this.redoStack;
    const to = op === "history.undo" ? this.redoStack : this.undoStack;
    const step = from.pop();
    if (!step) {
      fail("not_found", op === "history.undo" ? "There is nothing to undo." : "There is nothing to redo.");
    }
    to.push({ state: structuredClone(this.state), op: step.op, group: step.group });
    const seq = this.state.seq + 1;
    this.state = step.state;
    this.state.seq = seq;
    const group = `g-${String(seq)}`;
    return {
      output: { group, op: step.op, target: step.group },
      changes: { group, seq, changes: [{ entity: "workspace", id: "w-1", fields: ["*"] }] },
    };
  }

  // ---------- lookups ----------

  private project(ref: string): FakeProject {
    const folded = ref.toUpperCase();
    const project = this.state.projects.find((candidate) => candidate.key === folded || candidate.id === ref);
    if (!project) {
      fail("not_found", `There is no project "${ref}".`, {
        details: { choices: this.state.projects.map((candidate) => candidate.key) },
      });
    }
    return project;
  }

  private writable(ref: string): FakeProject {
    const project = this.project(ref);
    if (project.state === "archived") {
      fail("forbidden", `Project ${project.key} is archived.`, { hint: "Unarchive it to change it." });
    }
    return project;
  }

  private projectOf(task: FakeTask): FakeProject {
    const project = this.state.projects.find((candidate) => candidate.id === task.projectId);
    if (!project) {
      throw new Error(`task ${task.id} has no project`);
    }
    return project;
  }

  private keyOf(task: FakeTask): string {
    return `${this.projectOf(task).key}-${String(task.number)}`;
  }

  private task(key: string): FakeTask {
    const [projectKey, number] = key.toUpperCase().split("-");
    const task = this.state.tasks.find(
      (candidate) => candidate.id === key || (String(candidate.number) === number && this.projectOf(candidate).key === projectKey),
    );
    if (!task) {
      fail("not_found", `There is no task ${key}.`);
    }
    return task;
  }

  private statusIn(project: FakeProject, ref: string) {
    const status = project.statuses.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
    if (!status) {
      fail("invalid", `Project ${project.key} has no status "${ref}".`, {
        details: { path: "status", choices: project.statuses.map((candidate) => candidate.name) },
      });
    }
    return status;
  }

  private children(project: FakeProject): FakeProject[] {
    return this.state.projects.filter((candidate) => candidate.parent === project.id).sort((a, b) => a.rank - b.rank);
  }

  private descendants(project: FakeProject): FakeProject[] {
    return this.children(project).flatMap((child) => [child, ...this.descendants(child)]);
  }

  private depth(project: FakeProject): number {
    const parent = this.state.projects.find((candidate) => candidate.id === project.parent);
    return parent ? 1 + this.depth(parent) : 1;
  }

  private isBlocked(task: FakeTask): boolean {
    return this.state.relations.some((relation) => {
      if (relation.kind !== "blocks" || relation.to !== task.id) {
        return false;
      }
      const blocker = this.state.tasks.find((candidate) => candidate.id === relation.from);
      return blocker !== undefined && blocker.closedAt === null;
    });
  }

  private checklist(task: FakeTask): { done: number; total: number } {
    const items = task.description.match(/^\s*[-*+] \[[ xX]\]/gm) ?? [];
    const done = items.filter((item) => /\[[xX]\]/.test(item)).length;
    return { done, total: items.length };
  }

  // ---------- output shapes ----------

  private summary(project: FakeProject): ProjectSummary {
    const parent = this.state.projects.find((candidate) => candidate.id === project.parent);
    return {
      id: project.id,
      key: project.key,
      name: project.name,
      hue: project.hue,
      parent: parent?.key ?? null,
      state: project.state,
      open: this.state.tasks.filter((task) => task.projectId === project.id && task.closedAt === null).length,
    };
  }

  private fullProject(project: FakeProject): Project {
    return {
      ...this.summary(project),
      description: project.description,
      modules: { ...project.modules },
      statuses: structuredClone(project.statuses),
      types: structuredClone(project.types),
      labels: structuredClone(project.labels),
      children: this.children(project).map((child) => child.key),
    };
  }

  private ref(task: FakeTask): TaskRef {
    const status = this.statusIn(this.projectOf(task), task.statusId);
    return { key: this.keyOf(task), title: task.title, status: { name: status.name, category: status.category, hue: status.hue } };
  }

  private card(task: FakeTask): Card {
    const project = this.projectOf(task);
    const type = project.types.find((candidate) => candidate.id === task.typeId);
    const subtasks = this.state.tasks.filter((candidate) => candidate.parentId === task.id);
    const checklist = this.checklist(task);
    return {
      id: task.id,
      key: this.keyOf(task),
      project: project.key,
      title: task.title,
      status: task.statusId,
      type: type?.glyph ?? "task",
      type_name: type?.name ?? "",
      priority: task.priority,
      labels: project.labels.filter((label) => task.labelIds.includes(label.id)),
      due_date: task.dueDate,
      checklist: checklist.total > 0 ? checklist : null,
      subtasks: subtasks.length > 0 ? { done: subtasks.filter((sub) => sub.closedAt !== null).length, total: subtasks.length } : null,
      files: task.files,
      comments: task.comments,
      spent_min: task.spentMin,
      blocked: this.isBlocked(task),
      closed_at: task.closedAt,
      updated: { at: task.updatedAt, actor: { ...task.updatedBy } },
    };
  }

  private fullTask(task: FakeTask): Task {
    const project = this.projectOf(task);
    const type = project.types.find((candidate) => candidate.id === task.typeId) ?? project.types[0];
    if (!type) {
      throw new Error(`project ${project.key} has no task types`);
    }
    const relations: Relation[] = [];
    for (const relation of this.state.relations) {
      const forward = relation.from === task.id;
      if (!forward && relation.to !== task.id) {
        continue;
      }
      const other = this.state.tasks.find((candidate) => candidate.id === (forward ? relation.to : relation.from));
      if (!other) {
        continue;
      }
      const kind =
        relation.kind === "relates"
          ? "relates"
          : forward
            ? relation.kind
            : relation.kind === "blocks"
              ? "blocked_by"
              : relation.kind === "duplicates"
                ? "duplicated_by"
                : "cloned_by";
      relations.push({ kind, task: this.ref(other) });
    }
    const parent = this.state.tasks.find((candidate) => candidate.id === task.parentId);
    return {
      id: task.id,
      key: this.keyOf(task),
      project: project.key,
      title: task.title,
      description: task.description,
      status: structuredClone(this.statusIn(project, task.statusId)),
      type: structuredClone(type),
      priority: task.priority,
      labels: project.labels.filter((label) => task.labelIds.includes(label.id)),
      parent: parent ? this.ref(parent) : null,
      subtasks: this.state.tasks.filter((candidate) => candidate.parentId === task.id).map((sub) => this.ref(sub)),
      relations,
      start_date: task.startDate,
      due_date: task.dueDate,
      estimate_min: task.estimateMin,
      spent_min: task.spentMin,
      checklist: this.checklist(task),
      blocked: this.isBlocked(task),
      created_at: task.createdAt,
      edited_at: task.editedAt,
      updated: { at: task.updatedAt, actor: { ...task.updatedBy } },
      closed_at: task.closedAt,
      version: task.version,
    };
  }

  private board(ref: string, includeSubprojects: boolean): Board {
    const root = this.project(ref);
    const onBoard = [root, ...(includeSubprojects ? this.descendants(root).filter((project) => project.state === "active") : [])];
    const columns: BoardColumn[] = root.statuses.map((status) => ({
      id: status.id,
      name: status.name,
      category: status.category,
      hue: status.hue,
      wip_limit: status.wip_limit,
      statuses: { [root.key]: status.id },
    }));
    for (const project of onBoard.slice(1)) {
      for (const status of project.statuses) {
        const column = columns.find((candidate) => fold(candidate.name) === fold(status.name));
        if (column) {
          column.statuses[project.key] = status.id;
        } else {
          columns.push({
            id: `name:${fold(status.name)}`,
            name: status.name,
            category: status.category,
            hue: status.hue,
            wip_limit: status.wip_limit,
            statuses: { [project.key]: status.id },
          });
        }
      }
    }
    const projectIds = new Set(onBoard.map((project) => project.id));
    const tasks = this.state.tasks.filter((task) => projectIds.has(task.projectId)).sort((a, b) => a.rank - b.rank);
    const columnOf: Record<string, string> = {};
    const cards: Card[] = [];
    for (const column of columns) {
      for (const task of tasks) {
        if (column.statuses[this.projectOf(task).key] === task.statusId) {
          const card = this.card(task);
          columnOf[card.key] = column.id;
          cards.push(card);
        }
      }
    }
    return { project: root.key, include_subprojects: includeSubprojects, columns, cards, column_of: columnOf };
  }

  // ---------- changes ----------

  private touch(task: FakeTask, edited = true): void {
    const now = this.today().toISOString();
    task.updatedAt = now;
    task.updatedBy = { kind: "user", name: "You" };
    if (edited) {
      task.editedAt = now;
    }
    task.version += 1;
  }

  /** The rank for a task placed at `position` among the tasks of `statusId`. */
  private rankAt(statusId: string, position: Position | undefined, moving: FakeTask | null): number {
    const column = this.state.tasks.filter((task) => task.statusId === statusId && task !== moving).sort((a, b) => a.rank - b.rank);
    const first = column[0];
    const last = column[column.length - 1];
    if (position === "top") {
      return first ? first.rank - 1 : 1;
    }
    if (position === undefined || position === "bottom") {
      return last ? last.rank + 1 : 1;
    }
    const anchorKey = "before" in position ? position.before : position.after;
    const anchor = this.task(anchorKey);
    const all = this.state.tasks.filter((task) => task.statusId === anchor.statusId && task !== moving).sort((a, b) => a.rank - b.rank);
    const at = all.indexOf(anchor);
    if ("before" in position) {
      const previous = all[at - 1];
      return previous ? (previous.rank + anchor.rank) / 2 : anchor.rank - 1;
    }
    const next = all[at + 1];
    return next ? (anchor.rank + next.rank) / 2 : anchor.rank + 1;
  }

  private taskChange(task: FakeTask, fields: string[]): Result<TaskChange> {
    return {
      output: { task: this.fullTask(task), group: "" },
      changes: [{ entity: "task", id: task.id, fields, project: this.projectOf(task).key }],
    };
  }

  private projectChange(project: FakeProject, entity: string, fields: string[]): Result<ProjectChange> {
    return { output: { project: this.fullProject(project), group: "" }, changes: [{ entity, id: project.id, fields, project: project.key }] };
  }

  private enterStatus(task: FakeTask, statusId: string): void {
    const project = this.projectOf(task);
    const status = this.statusIn(project, statusId);
    const wasClosed = task.closedAt !== null;
    task.statusId = status.id;
    if (isClosed(status.category) && !wasClosed) {
      task.closedAt = this.today().toISOString();
    } else if (!isClosed(status.category)) {
      task.closedAt = null;
    }
  }

  private validHue(hue: Hue | undefined): void {
    if (hue !== undefined && !["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"].includes(hue)) {
      fail("invalid", `"${hue}" is not a colour.`, { details: { path: "hue" } });
    }
  }

  private uniqueName(list: readonly { id: string; name: string }[], name: string, what: string, except?: string): string {
    const trimmed = name.trim();
    if (trimmed === "") {
      fail("invalid", `A ${what} needs a name.`, { details: { path: "name" } });
    }
    if (list.some((item) => item.id !== except && fold(item.name) === fold(trimmed))) {
      fail("conflict", `There is already a ${what} called "${trimmed}".`);
    }
    return trimmed;
  }

  private layout(project: FakeProject): BoardLayout {
    return structuredClone(this.state.layouts[project.id] ?? DEFAULT_LAYOUT);
  }

  // ---------- the operations ----------

  private readonly handlers: { [K in AppOpId]?: Handler<K> } = {
    "workspace.get": () => ({
      caller: { actor: { kind: "user", name: "You" }, via: "app" },
      guide: "",
      id: "w-1",
      name: this.state.workspaceName,
      program_version: "0.0.0",
      schema_version: 1,
      settings: { ...this.state.settings },
    }),
    "settings.get": () => ({ ...this.state.settings }),
    "settings.update": (patch: SettingsPatch) => {
      const changed: string[] = [];
      for (const [name, value] of Object.entries(patch)) {
        if (value !== null && (this.state.settings as Record<string, unknown>)[name] !== value) {
          (this.state.settings as Record<string, unknown>)[name] = value;
          changed.push(name);
        }
      }
      const settings: Settings = { ...this.state.settings };
      return { output: { changed, settings }, changes: changed.length ? [{ entity: "settings", id: "w-1", fields: changed }] : [] };
    },
    "history.feed": () => ({ items: [], total: 0 }),
    "agent.sessions": () => ({ items: [], total: 0 }),
    "workspace.counts": (): Counts => {
      const today = dayOffset(this.today(), 0);
      const open = this.state.tasks.filter((task) => task.closedAt === null && this.projectOf(task).state === "active");
      const projects: Record<string, number> = {};
      for (const project of this.state.projects) {
        projects[project.key] = open.filter((task) => task.projectId === project.id).length;
      }
      return { today: open.filter((task) => task.dueDate !== null && task.dueDate <= today).length, all: open.length, projects };
    },
    "project.list": ({ include_archived }) => {
      const ordered: FakeProject[] = [];
      const visit = (parent: string | null) => {
        for (const project of this.state.projects.filter((candidate) => candidate.parent === parent).sort((a, b) => a.rank - b.rank)) {
          if (include_archived !== false || project.state === "active") {
            ordered.push(project);
          }
          visit(project.id);
        }
      };
      visit(null);
      return { items: ordered.map((project) => this.summary(project)), total: ordered.length };
    },
    "project.get": ({ project }) => this.fullProject(this.project(project)),
    "project.create": ({ name, key, parent, hue }) => {
      const trimmed = name.trim();
      if (trimmed === "") {
        fail("invalid", "A project needs a name.", { details: { path: "name" } });
      }
      if (!KEY_PATTERN.test(key)) {
        fail("invalid", `"${key}" can't be a project key.`, {
          hint: "Use 2 to 10 capital letters and digits, starting with a letter.",
          details: { path: "key" },
        });
      }
      if (this.state.projects.some((project) => project.key === key)) {
        fail("conflict", `The key ${key} is taken.`, { details: { path: "key" } });
      }
      this.validHue(hue);
      const parentProject = parent ? this.writable(parent) : null;
      if (parentProject && this.depth(parentProject) >= MAX_DEPTH) {
        fail("limit", "Sub-projects go at most three levels deep.");
      }
      const siblings = this.state.projects.filter((project) => project.parent === (parentProject?.id ?? null));
      const project = newProject({
        key,
        name: trimmed,
        hue: hue ?? "blue",
        parent: parentProject?.id ?? null,
        rank: Math.max(0, ...siblings.map((sibling) => sibling.rank)) + 1,
      });
      if (parentProject) {
        // A sub-project starts with copies of its parent's statuses, types and labels.
        project.statuses = parentProject.statuses.map((status) => ({ ...status, id: nextId("s") }));
        project.types = parentProject.types.map((type) => ({ ...type, id: nextId("ty") }));
        project.labels = parentProject.labels.map((label) => ({ ...label, id: nextId("l") }));
      }
      this.state.projects.push(project);
      return this.projectChange(project, "project", ["*"]);
    },
    "project.update": ({ project: ref, patch }) => {
      const project = this.writable(ref);
      const fields: string[] = [];
      if (patch.name !== undefined) {
        const name = patch.name.trim();
        if (name === "") {
          fail("invalid", "A project needs a name.", { details: { path: "name" } });
        }
        project.name = name;
        fields.push("name");
      }
      if (patch.description !== undefined) {
        project.description = patch.description;
        fields.push("description");
      }
      if (patch.hue !== undefined) {
        this.validHue(patch.hue);
        project.hue = patch.hue;
        fields.push("hue");
      }
      if (patch.modules !== undefined) {
        project.modules = { ...project.modules, ...patch.modules };
        fields.push("modules");
      }
      return this.projectChange(project, "project", fields);
    },
    "project.move": ({ project: ref, parent }) => {
      const project = this.writable(ref);
      const target = parent ? this.project(parent) : null;
      if (target && (target === project || this.descendants(project).includes(target))) {
        fail("invalid", "A project can't move under itself or its own sub-projects.");
      }
      const height = Math.max(0, ...this.descendants(project).map((child) => this.depth(child) - this.depth(project)));
      if (target && this.depth(target) + 1 + height > MAX_DEPTH) {
        fail("limit", "Sub-projects go at most three levels deep.");
      }
      project.parent = target?.id ?? null;
      return this.projectChange(project, "project", ["parent"]);
    },
    "project.archive": ({ project: ref }) => {
      const project = this.project(ref);
      for (const item of [project, ...this.descendants(project)]) {
        item.state = "archived";
      }
      return this.projectChange(project, "project", ["state"]);
    },
    "project.unarchive": ({ project: ref }) => {
      const project = this.project(ref);
      for (const item of [project, ...this.descendants(project)]) {
        item.state = "active";
      }
      return this.projectChange(project, "project", ["state"]);
    },
    "status.create": ({ project: ref, name, category, hue, wip_limit }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      project.statuses.push({
        id: nextId("s"),
        name: this.uniqueName(project.statuses, name, "status"),
        category,
        hue: hue ?? "grey",
        wip_limit: wip_limit ?? null,
      });
      return this.projectChange(project, "status", ["*"]);
    },
    "status.update": ({ project: ref, status: statusRef, patch }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      if (patch.name !== undefined) {
        status.name = this.uniqueName(project.statuses, patch.name, "status", status.id);
      }
      if (patch.category !== undefined) {
        status.category = patch.category;
      }
      if (patch.hue !== undefined) {
        this.validHue(patch.hue);
        status.hue = patch.hue;
      }
      if (patch.wip_limit !== undefined) {
        if (patch.wip_limit !== null && (!Number.isInteger(patch.wip_limit) || patch.wip_limit < 1)) {
          fail("invalid", "A WIP limit is a whole number of at least 1.", { details: { path: "wip_limit" } });
        }
        status.wip_limit = patch.wip_limit;
      }
      return this.projectChange(project, "status", Object.keys(patch));
    },
    "status.reorder": ({ project: ref, status: statusRef, position }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      const rest = project.statuses.filter((candidate) => candidate !== status);
      let at = rest.length;
      if (position === "top") {
        at = 0;
      } else if (typeof position === "object") {
        const anchor = this.statusIn(project, "before" in position ? position.before : position.after);
        at = rest.indexOf(anchor) + ("before" in position ? 0 : 1);
      }
      rest.splice(at, 0, status);
      project.statuses = rest;
      return this.projectChange(project, "status", ["rank"]);
    },
    "status.delete": ({ project: ref, status: statusRef, move_tasks_to }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      const target = this.statusIn(project, move_tasks_to);
      if (target === status) {
        fail("invalid", "Choose another status for its tasks.", { details: { path: "move_tasks_to" } });
      }
      for (const task of this.state.tasks.filter((candidate) => candidate.statusId === status.id)) {
        this.enterStatus(task, target.id);
        this.touch(task, false);
      }
      project.statuses = project.statuses.filter((candidate) => candidate !== status);
      return this.projectChange(project, "status", ["*"]);
    },
    "type.create": ({ project: ref, name, glyph, hue }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      project.types.push({ id: nextId("ty"), name: this.uniqueName(project.types, name, "task type"), glyph, hue: hue ?? "blue", is_default: false });
      return this.projectChange(project, "type", ["*"]);
    },
    "type.update": ({ project: ref, type: typeRef, patch }) => {
      const project = this.writable(ref);
      const type = project.types.find((candidate) => candidate.id === typeRef || fold(candidate.name) === fold(typeRef));
      if (!type) {
        fail("not_found", `Project ${project.key} has no task type "${typeRef}".`);
      }
      if (patch.name !== undefined) {
        type.name = this.uniqueName(project.types, patch.name, "task type", type.id);
      }
      if (patch.glyph !== undefined) {
        type.glyph = patch.glyph;
      }
      if (patch.hue !== undefined) {
        this.validHue(patch.hue);
        type.hue = patch.hue;
      }
      return this.projectChange(project, "type", Object.keys(patch));
    },
    "type.delete": ({ project: ref, type: typeRef, move_tasks_to }) => {
      const project = this.writable(ref);
      const type = project.types.find((candidate) => candidate.id === typeRef);
      const target = project.types.find((candidate) => candidate.id === move_tasks_to);
      if (!type || !target || type === target) {
        fail("invalid", "Choose another task type for its tasks.", { details: { path: "move_tasks_to" } });
      }
      for (const task of this.state.tasks.filter((candidate) => candidate.typeId === type.id)) {
        task.typeId = target.id;
        this.touch(task, false);
      }
      project.types = project.types.filter((candidate) => candidate !== type);
      if (type.is_default) {
        target.is_default = true;
      }
      return this.projectChange(project, "type", ["*"]);
    },
    "label.create": ({ project: ref, name, hue }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      project.labels.push({ id: nextId("l"), name: this.uniqueName(project.labels, name, "label"), hue: hue ?? "grey" });
      return this.projectChange(project, "label", ["*"]);
    },
    "label.update": ({ project: ref, label: labelRef, patch }) => {
      const project = this.writable(ref);
      const label = project.labels.find((candidate) => candidate.id === labelRef || fold(candidate.name) === fold(labelRef));
      if (!label) {
        fail("not_found", `Project ${project.key} has no label "${labelRef}".`);
      }
      if (patch.name !== undefined) {
        label.name = this.uniqueName(project.labels, patch.name, "label", label.id);
      }
      if (patch.hue !== undefined) {
        this.validHue(patch.hue);
        label.hue = patch.hue;
      }
      return this.projectChange(project, "label", Object.keys(patch));
    },
    "label.delete": ({ project: ref, label: labelRef }) => {
      const project = this.writable(ref);
      const label = project.labels.find((candidate) => candidate.id === labelRef);
      if (!label) {
        fail("not_found", `Project ${project.key} has no label "${labelRef}".`);
      }
      for (const task of this.state.tasks.filter((candidate) => candidate.labelIds.includes(label.id))) {
        task.labelIds = task.labelIds.filter((id) => id !== label.id);
        this.touch(task);
      }
      project.labels = project.labels.filter((candidate) => candidate !== label);
      return this.projectChange(project, "label", ["*"]);
    },
    "board.get": ({ project, include_subprojects }) => this.board(project, include_subprojects ?? false),
    "layout.get": ({ project }) => this.layout(this.project(project)),
    "layout.update": ({ project: ref, patch, reset }) => {
      const project = this.project(ref);
      const current = this.layout(project);
      const next: BoardLayout = reset
        ? { ...current, order: [], hidden: [], collapsed: [], sort_mode: "manual", column_sort: {} }
        : { ...current, ...patch };
      if (!Number.isInteger(next.hide_done_after_days) || next.hide_done_after_days < 0) {
        fail("invalid", "Days must be a whole number, 0 or more.", { details: { path: "hide_done_after_days" } });
      }
      this.state.layouts[project.id] = next;
      return { output: structuredClone(next), changes: [{ entity: "layout", id: project.id, fields: Object.keys(patch ?? {}), project: project.key }] };
    },
    "task.get": ({ key }) => this.fullTask(this.task(key)),
    "task.create": ({ project: ref, title, status, position }) => {
      const project = this.writable(ref);
      const trimmed = title.trim();
      if (trimmed === "") {
        fail("invalid", "A task needs a title.", { details: { path: "title" } });
      }
      if (trimmed.length > MAX_TITLE) {
        fail("limit", `A title has at most ${String(MAX_TITLE)} characters.`, { details: { path: "title", max: MAX_TITLE } });
      }
      const target = status
        ? this.statusIn(project, status)
        : (project.statuses.find((candidate) => candidate.category === "todo") ?? project.statuses[0]);
      const type = project.types.find((candidate) => candidate.is_default) ?? project.types[0];
      if (!target || !type) {
        fail("invalid", `Project ${project.key} has no statuses or task types.`);
      }
      const now = this.today().toISOString();
      const task: FakeTask = {
        id: nextId("t"),
        projectId: project.id,
        number: project.nextNumber,
        title: trimmed,
        description: "",
        statusId: target.id,
        typeId: type.id,
        priority: "none",
        labelIds: [],
        parentId: null,
        rank: this.rankAt(target.id, position, null),
        startDate: null,
        dueDate: null,
        estimateMin: null,
        spentMin: 0,
        files: 0,
        comments: 0,
        createdAt: now,
        editedAt: now,
        updatedAt: now,
        updatedBy: { kind: "user", name: "You" },
        closedAt: isClosed(target.category) ? now : null,
        version: 1,
      };
      project.nextNumber += 1;
      this.state.tasks.push(task);
      return this.taskChange(task, ["*"]);
    },
    "task.update": ({ key, expected_version, patch }) => {
      const task = this.task(key);
      const project = this.writable(this.projectOf(task).key);
      if (expected_version !== undefined && expected_version !== task.version) {
        fail("conflict", `${this.keyOf(task)} changed since you read it.`, {
          hint: "Read it again and repeat the change.",
          details: { current_version: task.version },
        });
      }
      const fields: string[] = [];
      if (patch.title !== undefined) {
        const title = patch.title.trim();
        if (title === "") {
          fail("invalid", "A task needs a title.", { details: { path: "title" } });
        }
        if (title.length > MAX_TITLE) {
          fail("limit", `A title has at most ${String(MAX_TITLE)} characters.`, { details: { path: "title", max: MAX_TITLE } });
        }
        task.title = title;
        fields.push("title");
      }
      if (patch.description !== undefined) {
        task.description = patch.description;
        fields.push("description");
      }
      if (patch.type !== undefined) {
        const ref = patch.type;
        const type = project.types.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
        if (!type) {
          fail("invalid", `Project ${project.key} has no task type "${ref}".`, { details: { path: "type" } });
        }
        task.typeId = type.id;
        fields.push("type");
      }
      if (patch.priority !== undefined) {
        task.priority = patch.priority;
        fields.push("priority");
      }
      if (patch.labels !== undefined) {
        const resolve = (refs: readonly string[]) =>
          refs.map((ref) => {
            const label = project.labels.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
            if (!label) {
              fail("invalid", `Project ${project.key} has no label "${ref}".`, { details: { path: "labels" } });
            }
            return label.id;
          });
        let labels = patch.labels.set ? resolve(patch.labels.set) : [...task.labelIds];
        labels = [...labels, ...resolve(patch.labels.add ?? []).filter((id) => !labels.includes(id))];
        const removed = resolve(patch.labels.remove ?? []);
        task.labelIds = labels.filter((id) => !removed.includes(id));
        fields.push("labels");
      }
      const start = patch.start_date !== undefined ? patch.start_date : task.startDate;
      const due = patch.due_date !== undefined ? patch.due_date : task.dueDate;
      if (start !== null && due !== null && start > due) {
        fail("invalid", "The start date is after the due date.", { details: { path: patch.start_date !== undefined ? "start_date" : "due_date" } });
      }
      if (patch.start_date !== undefined) {
        task.startDate = patch.start_date;
        fields.push("start_date");
      }
      if (patch.due_date !== undefined) {
        task.dueDate = patch.due_date;
        fields.push("due_date");
      }
      if (patch.estimate_min !== undefined) {
        if (patch.estimate_min !== null && (!Number.isInteger(patch.estimate_min) || patch.estimate_min < 0)) {
          fail("invalid", "An estimate is a duration such as 1h 30m.", { details: { path: "estimate" } });
        }
        task.estimateMin = patch.estimate_min;
        fields.push("estimate");
      }
      if (fields.length === 0) {
        return { output: { task: this.fullTask(task), group: "" }, changes: [] };
      }
      this.touch(task);
      return this.taskChange(task, fields);
    },
    "task.move": ({ key, status, position }) => {
      const task = this.task(key);
      const project = this.writable(this.projectOf(task).key);
      const fields: string[] = [];
      const target = status === undefined ? this.statusIn(project, task.statusId) : this.statusIn(project, status);
      if (target.id !== task.statusId) {
        this.enterStatus(task, target.id);
        fields.push("status");
      }
      if (position !== undefined || fields.length > 0) {
        task.rank = this.rankAt(target.id, position, task);
        fields.push("rank");
      }
      this.touch(task, fields.includes("status"));
      return this.taskChange(task, fields);
    },
  };
}

function isResult(value: unknown): value is Result<unknown> {
  return typeof value === "object" && value !== null && "changes" in value && "output" in value && Array.isArray(value.changes);
}

/** Fills in the history group of a change's output. */
function withGroup(output: unknown, group: string): unknown {
  if (typeof output === "object" && output !== null && "group" in output) {
    return { ...output, group };
  }
  return output;
}
