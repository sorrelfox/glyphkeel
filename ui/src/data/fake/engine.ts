// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// An engine stand-in that keeps a workspace in memory and answers the
// operations the screens use, with the engine's rules where the screens can
// see them: keys, default statuses, closing, WIP, sub-project columns,
// layouts, typed errors, change events and Undo.

import {
  type ActivityEntry,
  type Attachment,
  type Board,
  type BoardLayout,
  type Card,
  type Column,
  type Comment,
  type CommentChange,
  type ConflictAction,
  type DescriptionSave,
  type Edge,
  type EntityChange,
  type FileChange,
  type Hue,
  type Link,
  type LinkKind,
  type LabelRef,
  type OpError,
  type OpId,
  type OpMap,
  type Project,
  type ProjectChange,
  type ProjectSummary,
  type RevisionInfo,
  type RevisionKind,
  type Settings,
  type SettingsPatch,
  type Task,
  type TaskChange,
  type TaskPart,
  type TaskRef,
  type TypeRef,
} from "../../generated/ops";
import { decodeBase64, materialize, recordedFields, sniff } from "./details";
import { checklistOf, merge3, publicChecklist, renderFake, setChecked, unifiedDiff } from "./markdown";
import { newProject, nextId, SAMPLE_AGENT } from "./seed";
import { type FakeComment, type FakeFile, type FakeProject, type FakeRelation, type FakeRevision, type FakeState, type FakeTask } from "./state";

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

type Handler<K extends OpId> = (input: OpMap[K]["input"]) => OpMap[K]["output"] | Result<OpMap[K]["output"]>;

/** A place among siblings, as inputs give it. */
interface Place {
  before?: string | null | undefined;
  after?: string | null | undefined;
  at?: Edge | null | undefined;
}

/** The engine's layout before anyone changes it. */
const DEFAULT_LAYOUT: Readonly<BoardLayout> = {
  order: [],
  hidden: [],
  collapsed: [],
  sort_mode: "manual",
  column_sort: {},
  hide_done_after_days: 14,
  card_fields: ["type", "priority", "due", "labels", "checklist"],
  include_subprojects: false,
};

/** `{ [key]: value }` when there is a value; nothing otherwise. */
function some<K extends string, V>(key: K, value: V | null | undefined): { [P in K]?: V } {
  return value === null || value === undefined ? {} : ({ [key]: value } as { [P in K]?: V });
}

/** A duration as the engine reads it: `1w 2d 4h 30m`, `90m`, `1.5h`; a day has 8 hours, a week 5 days. */
function minutesOf(text: string): number | null {
  const unit: Record<string, number> = { m: 1, h: 60, d: 480, w: 2400 };
  let total = 0;
  let rest = text.trim().toLowerCase();
  if (rest === "") {
    return null;
  }
  while (rest !== "") {
    const found = /^(\d+(?:[.,]\d+)?)\s*([wdhm])\s*/.exec(rest);
    if (!found) {
      return null;
    }
    total += Number((found[1] ?? "0").replace(",", ".")) * (unit[found[2] ?? "m"] ?? 1);
    rest = rest.slice(found[0].length);
  }
  return Math.round(total);
}

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
    materialize(state, SAMPLE_AGENT, nextId);
    // Every description starts with a revision of its own, as the engine's do.
    for (const task of state.tasks) {
      if (task.description !== "" && !state.revisions.some((revision) => revision.taskId === task.id)) {
        state.revisions.push({ id: nextId("r"), taskId: task.id, body: task.description, kind: "normal", base: null, author: { kind: "user", name: "You" }, createdAt: task.createdAt, resolvedAt: null });
      }
    }
    // Subtasks keep the order they were shown in: by status, then by place in the column.
    for (const parent of state.tasks) {
      const statuses = this.projectOf(parent).statuses.map((status) => status.id);
      state.tasks
        .filter((candidate) => candidate.parentId === parent.id && candidate.subtaskRank === undefined)
        .sort((a, b) => statuses.indexOf(a.statusId) - statuses.indexOf(b.statusId) || a.rank - b.rank)
        .forEach((sub) => {
          sub.subtaskRank = this.lastSubtaskRank(parent.id);
        });
    }
    // One task in progress is claimed by an agent, as agents do while they work.
    const claimed = state.tasks.find((task) => task.closedAt === null && this.statusIn(this.projectOf(task), task.statusId).category === "in_progress");
    if (claimed) {
      state.claims.push({ taskId: claimed.id, agent: SAMPLE_AGENT, session: "session-1", until: new Date(today().getTime() + 45 * 60_000).toISOString() });
    }
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
    this.record(before, op, group, answer.changes);
    this.undoStack.push({ state: before, op, group });
    this.redoStack = [];
    return { output: withGroup(answer.output, group), changes: { group, seq: this.state.seq, changes: answer.changes } };
  }

  /** Adds one command's changes to the activity log. */
  private record(before: FakeState, op: string, group: string, changes: readonly EntityChange[]): void {
    const at = this.today().toISOString();
    for (const change of changes) {
      const project = this.state.projects.find((candidate) => candidate.key === change.project);
      const entry: ActivityEntry = {
        actor: { kind: "user", name: "You" },
        at,
        entity: change.entity,
        fields: recordedFields(before, this.state, change, (task) => this.keyOf(task)),
        group,
        id: change.id,
        op,
        ...some("project", project?.id),
        seq: this.state.seq,
        via: "app",
      };
      this.state.activity.push(entry);
    }
  }

  /** Takes back the latest command, keeping the activity log, which marks it reverted. */
  private revertLatest(): void {
    const step = this.undoStack.pop();
    if (!step) {
      return;
    }
    const { activity, seq } = this.state;
    this.state = structuredClone(step.state);
    this.state.seq = seq;
    // The revert itself gets the next group.
    this.state.activity = activity.map((entry) => (entry.group === step.group ? { ...entry, reverted_by: `g-${String(seq + 1)}` } : entry));
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

  private checklist(task: FakeTask): { done: number; total: number } {
    const items = checklistOf(task.description);
    return { done: items.filter((item) => item.checked).length, total: items.length };
  }

  /** Progress as the engine works it out: closed 100, else the checklist's share, else the subtasks' average, else 0. */
  private progress(task: FakeTask): number {
    if (task.closedAt !== null) {
      return 100;
    }
    const { done, total } = this.checklist(task);
    if (total > 0) {
      return Math.round((done / total) * 100);
    }
    const subtasks = this.subtasksOf(task);
    if (subtasks.length > 0) {
      return Math.round(subtasks.reduce((sum, sub) => sum + this.progress(sub), 0) / subtasks.length);
    }
    return 0;
  }

  // ---------- descriptions ----------

  private revisionsOf(task: FakeTask): FakeRevision[] {
    return this.state.revisions.filter((revision) => revision.taskId === task.id);
  }

  /** The revision the description is at now. */
  private currentRevision(task: FakeTask): FakeRevision | undefined {
    return this.revisionsOf(task)
      .filter((revision) => revision.kind !== "conflict")
      .at(-1);
  }

  private revisionInfo(revision: FakeRevision, current: FakeRevision | undefined): RevisionInfo {
    return {
      id: revision.id,
      author: revision.author,
      ...some("base", revision.base),
      bytes: new TextEncoder().encode(revision.body).length,
      created_at: revision.createdAt,
      current: revision === current,
      kind: revision.kind,
      ...some("resolved_at", revision.resolvedAt),
    };
  }

  private revision(id: string): FakeRevision {
    const revision = this.state.revisions.find((candidate) => candidate.id === id);
    if (!revision) {
      fail("not_found", `There is no description revision ${id}.`);
    }
    return revision;
  }

  /** Writes a new revision and makes it the description. */
  private addRevision(task: FakeTask, body: string, kind: RevisionKind, base: string | null): FakeRevision {
    const revision: FakeRevision = { id: nextId("r"), taskId: task.id, body, kind, base, author: { kind: "user", name: "You" }, createdAt: this.today().toISOString(), resolvedAt: null };
    this.state.revisions.push(revision);
    if (kind !== "conflict") {
      task.description = body;
    }
    return revision;
  }

  // ---------- output shapes ----------

  private open(project: FakeProject): number {
    return this.state.tasks.filter((task) => task.projectId === project.id && task.closedAt === null).length;
  }

  private path(project: FakeProject): string {
    const parent = this.state.projects.find((candidate) => candidate.id === project.parent);
    return parent ? `${this.path(parent)} / ${project.name}` : project.name;
  }

  private summary(project: FakeProject): ProjectSummary {
    const parent = this.state.projects.find((candidate) => candidate.id === project.parent);
    return {
      id: project.id,
      key: project.key,
      name: project.name,
      hue: project.hue,
      ...some("parent", parent?.key),
      depth: this.depth(project) - 1,
      path: this.path(project),
      state: project.state,
      open_tasks: this.open(project),
      open_tasks_with_subprojects: [project, ...this.descendants(project)].reduce((sum, item) => sum + this.open(item), 0),
    };
  }

  private fullProject(project: FakeProject): Project {
    const tasks = this.state.tasks.filter((task) => task.projectId === project.id);
    const summary = this.summary(project);
    return {
      id: project.id,
      key: project.key,
      name: project.name,
      hue: project.hue,
      ...some("parent", summary.parent),
      path: summary.path,
      state: project.state,
      open_tasks: summary.open_tasks,
      total_tasks: tasks.length,
      description: project.description,
      modules: { ...project.modules },
      statuses: project.statuses.map((status) => ({
        id: status.id,
        name: status.name,
        category: status.category,
        hue: status.hue,
        ...some("wip_limit", status.wip_limit),
        tasks: tasks.filter((task) => task.statusId === status.id).length,
      })),
      types: project.types.map((type) => ({ ...type, tasks: tasks.filter((task) => task.typeId === type.id).length })),
      labels: project.labels.map((label) => ({ ...label, tasks: tasks.filter((task) => task.labelIds.includes(label.id)).length })),
      children: this.children(project).map((child) => child.key),
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
  }

  private typeRef(task: FakeTask): TypeRef {
    const project = this.projectOf(task);
    const type = project.types.find((candidate) => candidate.id === task.typeId) ?? project.types[0];
    if (!type) {
      throw new Error(`project ${project.key} has no task types`);
    }
    return { id: type.id, name: type.name, glyph: type.glyph };
  }

  private labelRefs(task: FakeTask): LabelRef[] {
    return this.projectOf(task)
      .labels.filter((label) => task.labelIds.includes(label.id))
      .map((label) => ({ id: label.id, name: label.name, hue: label.hue }));
  }

  /** Whether a task that blocks it is still open. */
  private blocked(task: FakeTask): boolean {
    return this.state.relations.some(
      (relation) =>
        relation.kind === "blocks" &&
        relation.to === task.id &&
        this.state.tasks.some((blocker) => blocker.id === relation.from && blocker.closedAt === null),
    );
  }

  private subtaskCount(task: FakeTask): { done: number; total: number } {
    const subtasks = this.state.tasks.filter((candidate) => candidate.parentId === task.id);
    return { done: subtasks.filter((sub) => sub.closedAt !== null).length, total: subtasks.length };
  }

  private card(task: FakeTask): Card {
    const parent = this.state.tasks.find((candidate) => candidate.id === task.parentId);
    return {
      id: task.id,
      key: this.keyOf(task),
      project: this.projectOf(task).key,
      blocked: this.blocked(task),
      comment_count: this.liveComments(task).length,
      file_count: this.fileCount(task),
      ...some("claim", this.claimOf(task)),
      spent_minutes: task.spentMin,
      title: task.title,
      type: this.typeRef(task),
      priority: task.priority,
      labels: this.labelRefs(task),
      ...some("parent", parent && this.keyOf(parent)),
      ...some("start_date", task.startDate),
      ...some("due_date", task.dueDate),
      ...some("estimate_minutes", task.estimateMin),
      checklist: this.checklist(task),
      progress_percent: this.progress(task),
      subtasks: this.subtaskCount(task),
      rank: String(task.rank),
      status_entered_at: task.statusEnteredAt,
      updated_at: task.updatedAt,
      ...some("closed_at", task.closedAt),
      version: task.version,
    };
  }

  private fullTask(task: FakeTask): Task {
    const project = this.projectOf(task);
    const status = this.statusIn(project, task.statusId);
    const parent = this.state.tasks.find((candidate) => candidate.id === task.parentId);
    return {
      id: task.id,
      key: this.keyOf(task),
      project: project.key,
      title: task.title,
      description: task.description,
      ...some("description_revision", this.currentRevision(task)?.id),
      conflict_copies: this.revisionsOf(task).filter((revision) => revision.kind === "conflict" && revision.resolvedAt === null).length,
      progress_percent: this.progress(task),
      status: { id: status.id, name: status.name, category: status.category },
      type: this.typeRef(task),
      priority: task.priority,
      labels: this.labelRefs(task),
      ...some("parent", parent && this.keyOf(parent)),
      ...some("start_date", task.startDate),
      ...some("due_date", task.dueDate),
      ...some("estimate_minutes", task.estimateMin),
      spent_minutes: task.spentMin,
      checklist: this.checklist(task),
      blocked: this.blocked(task),
      comment_count: this.liveComments(task).length,
      file_count: this.fileCount(task),
      ...some("claim", this.claimOf(task)),
      subtask_count: this.subtaskCount(task),
      rank: String(task.rank),
      created_at: task.createdAt,
      created_by: { kind: "user", name: "You" },
      edited_at: task.editedAt,
      status_entered_at: task.statusEnteredAt,
      updated_at: task.updatedAt,
      ...some("closed_at", task.closedAt),
      version: task.version,
    };
  }

  private board(ref: string, includeInput: boolean | null | undefined, showOlder: boolean): Board {
    const root = this.project(ref);
    const layout = this.layout(root);
    const include = includeInput ?? layout.include_subprojects;
    const onBoard = [root, ...(include ? this.descendants(root).filter((project) => project.state === "active") : [])];
    const base = (status: FakeProject["statuses"][number], project: FakeProject): Column => ({
      id: status.id,
      name: status.name,
      category: status.category,
      hue: status.hue,
      ...some("wip_limit", status.wip_limit),
      statuses: { [project.key]: status.id },
      cards: [],
      count: 0,
      older: 0,
      over_limit: false,
      hidden: false,
      collapsed: false,
      sort_mode: "manual",
    });
    const columns: Column[] = root.statuses.map((status) => base(status, root));
    const extra: Column[] = [];
    for (const project of onBoard.slice(1)) {
      for (const status of project.statuses) {
        const column = [...columns, ...extra].find((candidate) => fold(candidate.name) === fold(status.name));
        if (column) {
          column.statuses[project.key] = status.id;
        } else {
          extra.push(base(status, project));
        }
      }
    }
    const ordered = [...arrange(columns, layout.order), ...arrange(extra, layout.order)];
    const tasks = [...this.state.tasks].sort((a, b) => a.rank - b.rank);
    const cutoff = this.today().getTime() - layout.hide_done_after_days * 86_400_000;
    let hiddenColumns = 0;
    let hiddenTasks = 0;
    for (const column of ordered) {
      const statusIds = new Set(Object.values(column.statuses));
      const all = tasks.filter((task) => statusIds.has(task.statusId));
      const shown = layout.hide_done_after_days > 0 && !showOlder ? all.filter((task) => task.closedAt === null || Date.parse(task.closedAt) >= cutoff) : all;
      column.count = all.length;
      column.older = all.length - shown.length;
      column.over_limit = column.wip_limit !== undefined && all.length > column.wip_limit;
      column.hidden = layout.hidden.includes(column.id);
      column.collapsed = layout.collapsed.includes(column.id);
      column.sort_mode = layout.column_sort[column.id] ?? layout.sort_mode;
      if (column.hidden) {
        hiddenColumns += 1;
        hiddenTasks += all.length;
      } else {
        column.cards = shown.map((task) => this.card(task));
      }
    }
    return {
      project: root.key,
      columns: ordered,
      layout: { ...layout, include_subprojects: include },
      hidden_columns: hiddenColumns,
      hidden_tasks: hiddenTasks,
    };
  }


  // ---------- task details ----------

  private liveComments(task: FakeTask): FakeComment[] {
    return this.state.comments.filter((comment) => comment.taskId === task.id && comment.deletedAt === null);
  }

  /** Files of the task and of its live comments. */
  private liveFiles(task: FakeTask): FakeFile[] {
    const comments = new Set(this.liveComments(task).map((comment) => comment.id));
    return this.state.files.filter((file) => file.taskId === task.id && file.deletedAt === null && (file.commentId === null || comments.has(file.commentId)));
  }

  private fileCount(task: FakeTask): number {
    return this.liveFiles(task).length;
  }

  private claimOf(task: FakeTask) {
    const now = this.today().toISOString();
    const claim = this.state.claims.find((candidate) => candidate.taskId === task.id && candidate.until > now);
    return claim && { agent: claim.agent, session: claim.session, until: claim.until };
  }

  private taskRef(task: FakeTask): TaskRef {
    const status = this.statusIn(this.projectOf(task), task.statusId);
    return { id: task.id, key: this.keyOf(task), title: task.title, closed: task.closedAt !== null, status: { id: status.id, name: status.name, category: status.category } };
  }

  /** Subtasks in their own order, which statuses don't change. */
  private subtasksOf(task: FakeTask): FakeTask[] {
    return this.state.tasks.filter((candidate) => candidate.parentId === task.id).sort((a, b) => (a.subtaskRank ?? 0) - (b.subtaskRank ?? 0));
  }

  /** Whether an activity entry is about the task `taskId`: its own fields, its comments, its files (also on its comments) and its links. */
  private concerns(entry: ActivityEntry, taskId: string): boolean {
    switch (entry.entity) {
      case "task":
        return entry.id === taskId;
      case "comment":
        return this.state.comments.some((comment) => comment.id === entry.id && comment.taskId === taskId);
      case "attachment":
        return this.state.files.some((file) => file.id === entry.id && file.taskId === taskId);
      case "relation": {
        const link = (entry.fields.link?.after ?? entry.fields.link?.before ?? {}) as { from?: string; to?: string };
        return link.from === taskId || link.to === taskId;
      }
      default:
        return false;
    }
  }

  /** The place after the last of `parentId`'s subtasks. */
  private lastSubtaskRank(parentId: string): number {
    return Math.max(0, ...this.state.tasks.filter((candidate) => candidate.parentId === parentId).map((candidate) => candidate.subtaskRank ?? 0)) + 1;
  }

  /** Gives `child` a parent (last among its subtasks) or none. */
  private adopt(child: FakeTask, parentId: string | null): void {
    if (child.parentId === parentId) {
      return;
    }
    child.subtaskRank = parentId === null ? undefined : this.lastSubtaskRank(parentId);
    child.parentId = parentId;
  }

  private linksOf(task: FakeTask): Link[] {
    const backwards: Record<FakeRelation["kind"], LinkKind> = { blocks: "blocked_by", relates: "relates", duplicates: "duplicated_by", clones: "cloned_by" };
    return this.state.relations.flatMap((relation) => {
      const forward = relation.from === task.id;
      if (!forward && relation.to !== task.id) {
        return [];
      }
      const other = this.state.tasks.find((candidate) => candidate.id === (forward ? relation.to : relation.from));
      return other ? [{ id: relation.id, kind: forward ? relation.kind : backwards[relation.kind], task: this.taskRef(other) }] : [];
    });
  }

  private attachment(file: FakeFile): Attachment {
    const task = this.state.tasks.find((candidate) => candidate.id === file.taskId);
    return {
      id: file.id,
      name: file.name,
      kind: file.kind,
      mime: file.mime,
      size: file.size,
      sha256: `sample-${file.id}`,
      task: task ? this.keyOf(task) : "",
      ...some("comment", file.commentId),
      ...some("width", file.width),
      ...some("height", file.height),
      added_by: { kind: "user", name: "You" },
      created_at: file.createdAt,
    };
  }

  private comment(comment: FakeComment, revisions: boolean): Comment {
    const task = this.state.tasks.find((candidate) => candidate.id === comment.taskId);
    const files = this.state.files.filter((file) => file.commentId === comment.id && file.deletedAt === null);
    return {
      id: comment.id,
      task: task ? this.keyOf(task) : "",
      author: comment.author,
      via: comment.via,
      body: comment.body,
      created_at: comment.createdAt,
      ...some("edited_at", comment.editedAt),
      version: comment.version,
      revision_count: comment.revisions.length,
      ...(revisions ? { revisions: comment.revisions } : {}),
      ...some("reply_to", comment.replyTo),
      ...(files.length > 0 ? { files: files.map((file) => this.attachment(file)) } : {}),
    };
  }

  private withParts(task: FakeTask, include: readonly TaskPart[] | undefined): Task {
    const full = this.fullTask(task);
    for (const part of include ?? []) {
      if (part === "subtasks") {
        full.subtasks = this.subtasksOf(task).map((sub) => this.taskRef(sub));
      } else if (part === "links") {
        full.links = this.linksOf(task);
      } else if (part === "comments") {
        full.comments = this.liveComments(task).map((comment) => this.comment(comment, false));
      } else if (part === "checklist") {
        full.checklist_items = publicChecklist(task.description);
      } else if (part === "revisions") {
        const current = this.currentRevision(task);
        full.revisions = this.revisionsOf(task)
          .reverse()
          .map((revision) => this.revisionInfo(revision, current));
      } else {
        full.files = this.state.files.filter((file) => file.taskId === task.id && file.commentId === null && file.deletedAt === null).map((file) => this.attachment(file));
      }
    }
    return full;
  }

  private liveComment(id: string): FakeComment {
    const comment = this.state.comments.find((candidate) => candidate.id === id && candidate.deletedAt === null);
    if (!comment) {
      fail("not_found", `There is no comment ${id}.`);
    }
    return comment;
  }

  private liveFile(id: string): FakeFile {
    const file = this.state.files.find((candidate) => candidate.id === id && candidate.deletedAt === null);
    if (!file) {
      fail("not_found", `There is no attached file ${id}.`);
    }
    return file;
  }

  private commentChange(comment: FakeComment, fields: string[]): Result<CommentChange> {
    const task = this.state.tasks.find((candidate) => candidate.id === comment.taskId);
    return {
      output: { comment: this.comment(comment, false), deleted: comment.deletedAt !== null },
      changes: [{ entity: "comment", id: comment.id, fields, ...some("project", task && this.projectOf(task).key) }],
    };
  }

  private fileChange(file: FakeFile, fields: string[]): Result<FileChange> {
    const task = this.state.tasks.find((candidate) => candidate.id === file.taskId);
    return {
      output: { file: this.attachment(file), deleted: file.deletedAt !== null },
      changes: [{ entity: "attachment", id: file.id, fields, ...some("project", task && this.projectOf(task).key) }],
    };
  }

  /** Whether `ancestor` is a parent, grandparent… of `task`. */
  private isAncestor(ancestor: FakeTask, task: FakeTask): boolean {
    const parent = this.state.tasks.find((candidate) => candidate.id === task.parentId);
    return parent !== undefined && (parent === ancestor || this.isAncestor(ancestor, parent));
  }

  /** Copies `task` (and its subtasks) as a new task under `parent`. */
  private copyTask(task: FakeTask, title: string, statusId: string, parentId: string | null, withSubtasks: boolean): FakeTask {
    const project = this.projectOf(task);
    const now = this.today().toISOString();
    const status = this.statusIn(project, statusId);
    const copy: FakeTask = {
      ...structuredClone(task),
      id: nextId("t"),
      number: project.nextNumber,
      title,
      statusId,
      parentId,
      subtaskRank: parentId === null ? undefined : this.lastSubtaskRank(parentId),
      rank: this.rankAt(statusId, { at: "bottom" }, null),
      spentMin: 0,
      files: 0,
      comments: 0,
      createdAt: now,
      editedAt: now,
      statusEnteredAt: now,
      updatedAt: now,
      updatedBy: { kind: "user", name: "You" },
      closedAt: isClosed(status.category) ? now : null,
      version: 1,
    };
    project.nextNumber += 1;
    this.state.tasks.push(copy);
    for (const file of this.state.files.filter((candidate) => candidate.taskId === task.id && candidate.commentId === null && candidate.deletedAt === null)) {
      this.state.files.push({ ...file, id: nextId("f"), taskId: copy.id, createdAt: now });
    }
    if (withSubtasks) {
      for (const sub of this.subtasksOf(task)) {
        this.copyTask(sub, sub.title, sub.statusId, copy.id, true);
      }
    }
    return copy;
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

  /** The rank for a task placed at `place` among the tasks of `statusId`. */
  private rankAt(statusId: string, place: Place, moving: FakeTask | null): number {
    const column = this.state.tasks.filter((task) => task.statusId === statusId && task !== moving).sort((a, b) => a.rank - b.rank);
    const first = column[0];
    const last = column[column.length - 1];
    const given = [place.before, place.after, place.at].filter((value) => value !== null && value !== undefined).length;
    if (given > 1) {
      fail("invalid", "Give one place: before, after or at.", { details: { path: "before" } });
    }
    const anchorKey = place.before ?? place.after;
    if (anchorKey === null || anchorKey === undefined) {
      if (place.at === "top") {
        return first ? first.rank - 1 : 1;
      }
      return last ? last.rank + 1 : 1;
    }
    const anchor = this.task(anchorKey);
    const all = this.state.tasks.filter((task) => task.statusId === anchor.statusId && task !== moving).sort((a, b) => a.rank - b.rank);
    const at = all.indexOf(anchor);
    if (place.before !== null && place.before !== undefined) {
      const previous = all[at - 1];
      return previous ? (previous.rank + anchor.rank) / 2 : anchor.rank - 1;
    }
    const next = all[at + 1];
    return next ? (anchor.rank + next.rank) / 2 : anchor.rank + 1;
  }

  private taskChange(task: FakeTask, fields: string[]): Result<TaskChange> {
    return {
      output: { task: this.fullTask(task) },
      changes: [{ entity: "task", id: task.id, fields, project: this.projectOf(task).key }],
    };
  }

  private projectChange(project: FakeProject, entity: string, fields: string[]): Result<ProjectChange> {
    return { output: { project: this.fullProject(project) }, changes: [{ entity, id: project.id, fields, project: project.key }] };
  }

  /** Saves a description edited from `base`: as it is, merged with an edit made since, or as a conflict copy. */
  private saveDescription(task: FakeTask, text: string, base: string | null): DescriptionSave {
    const current = this.currentRevision(task);
    if (text === task.description) {
      return { outcome: "unchanged", ...some("revision", current?.id) };
    }
    if (base === null || current === undefined || base === current.id) {
      return { outcome: "saved", revision: this.addRevision(task, text, "normal", current?.id ?? null).id };
    }
    const merged = merge3(this.revision(base).body, text, task.description);
    if (merged !== null) {
      return { outcome: "merged", revision: this.addRevision(task, merged, "merged", base).id };
    }
    const copy = this.addRevision(task, text, "conflict", base);
    return { outcome: "conflict", revision: current.id, conflict_copy: copy.id };
  }

  private resolveConflict(task: FakeTask, copyId: string, action: ConflictAction, text: string | null): void {
    const copy = this.revision(copyId);
    if (copy.taskId !== task.id || copy.kind !== "conflict") {
      fail("invalid", `${copyId} is not a conflict copy of ${this.keyOf(task)}.`, { details: { path: "copy" } });
    }
    if (copy.resolvedAt !== null) {
      fail("conflict", "That conflict copy was resolved already.");
    }
    const current = this.currentRevision(task)?.id ?? null;
    if (action === "use_copy") {
      this.addRevision(task, copy.body, "resolved", current);
    } else if (action === "use_text") {
      if (text === null) {
        fail("invalid", "Give the text to save.", { details: { path: "text" } });
      }
      this.addRevision(task, text, "resolved", current);
    }
    copy.resolvedAt = this.today().toISOString();
  }

  private enterStatus(task: FakeTask, statusId: string): void {
    const project = this.projectOf(task);
    const status = this.statusIn(project, statusId);
    const wasClosed = task.closedAt !== null;
    if (task.statusId !== status.id) {
      task.statusEnteredAt = this.today().toISOString();
    }
    task.statusId = status.id;
    if (isClosed(status.category) && !wasClosed) {
      task.closedAt = this.today().toISOString();
    } else if (!isClosed(status.category)) {
      task.closedAt = null;
    }
  }

  private validHue(hue: Hue | null | undefined): void {
    if (hue !== undefined && hue !== null && !["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey"].includes(hue)) {
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

  private typeIn(project: FakeProject, ref: string, path = "type") {
    const type = project.types.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
    if (!type) {
      fail("not_found", `Project ${project.key} has no task type "${ref}".`, { details: { path } });
    }
    return type;
  }

  private labelIn(project: FakeProject, ref: string, path = "label") {
    const label = project.labels.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
    if (!label) {
      fail("not_found", `Project ${project.key} has no label "${ref}".`, { details: { path } });
    }
    return label;
  }

  /** Moves an item of a project's list (statuses, types) to its place. */
  private place<T extends { id: string; name: string }>(list: T[], item: T, place: Place, what: string): T[] {
    const rest = list.filter((candidate) => candidate !== item);
    const find = (ref: string) => {
      const found = rest.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
      if (!found) {
        fail("not_found", `There is no ${what} "${ref}".`);
      }
      return found;
    };
    let at = rest.length;
    if (place.before !== null && place.before !== undefined) {
      at = rest.indexOf(find(place.before));
    } else if (place.after !== null && place.after !== undefined) {
      at = rest.indexOf(find(place.after)) + 1;
    } else if (place.at === "top") {
      at = 0;
    }
    rest.splice(at, 0, item);
    return rest;
  }

  private checkWip(limit: number | null | undefined): void {
    if (limit !== null && limit !== undefined && (!Number.isInteger(limit) || limit < 1)) {
      fail("invalid", "A WIP limit is a whole number of at least 1.", { details: { path: "wip_limit" } });
    }
  }

  private checkTitle(title: string): string {
    const trimmed = title.replace(/\s+/g, " ").trim();
    if (trimmed === "") {
      fail("invalid", "A task needs a title.", { details: { path: "title" } });
    }
    if (trimmed.length > MAX_TITLE) {
      fail("limit", `A title has at most ${String(MAX_TITLE)} characters.`, { details: { path: "title", max: MAX_TITLE } });
    }
    return trimmed;
  }

  // ---------- the operations ----------

  private readonly handlers: { [K in OpId]?: Handler<K> } = {
    "workspace.get": () => ({
      caller: { actor: { kind: "user", name: "You" }, via: "app" },
      guide: "",
      id: "w-1",
      name: this.state.workspaceName,
      program_version: "0.0.0",
      schema_version: 2,
      settings: { ...this.state.settings },
    }),
    "settings.get": () => ({ ...this.state.settings }),
    "settings.update": (patch: SettingsPatch) => {
      const changed: string[] = [];
      for (const [name, value] of Object.entries(patch)) {
        if (name !== "clear" && value !== null && (this.state.settings as Record<string, unknown>)[name] !== value) {
          (this.state.settings as Record<string, unknown>)[name] = value;
          changed.push(name);
        }
      }
      const settings: Settings = { ...this.state.settings };
      return { output: { changed, settings }, changes: changed.length ? [{ entity: "settings", id: "w-1", fields: changed }] : [] };
    },
    "history.feed": ({ limit, cursor, task: ref }) => {
      const task = ref === null || ref === undefined ? null : this.task(ref);
      const all = [...this.state.activity].reverse().filter((entry) => task === null || this.concerns(entry, task.id));
      const start = cursor ? Number(cursor) : 0;
      const end = start + (limit ?? 50);
      const items = all.slice(start, end);
      return { items, total: all.length, ...(end < all.length ? { next_cursor: String(end) } : {}) };
    },
    "agent.sessions": () => ({ items: [], total: 0 }),
    "project.list": ({ include_archived }) => {
      const ordered: FakeProject[] = [];
      const visit = (parent: string | null) => {
        for (const project of this.state.projects.filter((candidate) => candidate.parent === parent).sort((a, b) => a.rank - b.rank)) {
          if (include_archived === true || project.state === "active") {
            ordered.push(project);
          }
          visit(project.id);
        }
      };
      visit(null);
      return { items: ordered.map((project) => this.summary(project)), total: ordered.length };
    },
    "project.get": ({ project }) => this.fullProject(this.project(project)),
    "project.create": ({ name, key, parent, hue, description }) => {
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
        hue: hue ?? parentProject?.hue ?? "blue",
        parent: parentProject?.id ?? null,
        rank: Math.max(0, ...siblings.map((sibling) => sibling.rank)) + 1,
      });
      project.description = description ?? "";
      if (parentProject) {
        // A sub-project starts with copies of its parent's statuses, types and labels.
        project.statuses = parentProject.statuses.map((status) => ({ ...status, id: nextId("s") }));
        project.types = parentProject.types.map((type) => ({ ...type, id: nextId("ty") }));
        project.labels = parentProject.labels.map((label) => ({ ...label, id: nextId("l") }));
        project.modules = { ...parentProject.modules };
      }
      this.state.projects.push(project);
      return this.projectChange(project, "project", ["*"]);
    },
    // The sample workspace changes the project's own fields here; statuses,
    // types and labels change through their own operations.
    "project.update": ({ project: ref, name, description, hue, modules, archived, clear }) => {
      if (archived === false) {
        return this.handlers["project.unarchive"]?.({ project: ref }) ?? fail("not_found", "No unarchive.");
      }
      const project = this.writable(ref);
      const fields: string[] = [];
      if (name !== null && name !== undefined) {
        const trimmed = name.trim();
        if (trimmed === "") {
          fail("invalid", "A project needs a name.", { details: { path: "name" } });
        }
        project.name = trimmed;
        fields.push("name");
      }
      if (description !== null && description !== undefined) {
        project.description = description;
        fields.push("description");
      }
      if (clear?.includes("description")) {
        project.description = "";
        fields.push("description");
      }
      if (hue !== null && hue !== undefined) {
        this.validHue(hue);
        project.hue = hue;
        fields.push("hue");
      }
      if (modules !== null && modules !== undefined) {
        for (const [module, on] of Object.entries(modules)) {
          if (typeof on === "boolean") {
            project.modules[module as keyof typeof project.modules] = on;
          }
        }
        fields.push("modules");
      }
      if (archived === true) {
        for (const item of [project, ...this.descendants(project)]) {
          item.state = "archived";
        }
        fields.push("state");
      }
      return this.projectChange(project, "project", fields);
    },
    "project.move": ({ project: ref, parent, top_level }) => {
      const project = this.writable(ref);
      const target = parent && top_level !== true ? this.project(parent) : null;
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
      const parent = this.state.projects.find((candidate) => candidate.id === project.parent);
      if (parent?.state === "archived") {
        fail("forbidden", `Its parent ${parent.key} is archived.`, { hint: "Unarchive the parent instead." });
      }
      for (const item of [project, ...this.descendants(project)]) {
        item.state = "active";
      }
      return this.projectChange(project, "project", ["state"]);
    },
    "status.create": ({ project: ref, name, category, hue, wip_limit, before, after, at }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      this.checkWip(wip_limit);
      const status = {
        id: nextId("s"),
        name: this.uniqueName(project.statuses, name, "status"),
        category: category ?? "todo",
        hue: hue ?? "grey",
        wip_limit: wip_limit ?? null,
      };
      project.statuses = this.place([...project.statuses, status], status, { before, after, at }, "status");
      return this.projectChange(project, "status", ["*"]);
    },
    "status.update": ({ project: ref, status: statusRef, name, category, hue, wip_limit, clear }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      const fields: string[] = [];
      if (name !== null && name !== undefined) {
        status.name = this.uniqueName(project.statuses, name, "status", status.id);
        fields.push("name");
      }
      if (category !== null && category !== undefined) {
        status.category = category;
        fields.push("category");
        for (const task of this.state.tasks.filter((candidate) => candidate.statusId === status.id)) {
          this.enterStatus(task, status.id);
        }
      }
      if (hue !== null && hue !== undefined) {
        this.validHue(hue);
        status.hue = hue;
        fields.push("hue");
      }
      if (wip_limit !== null && wip_limit !== undefined) {
        this.checkWip(wip_limit);
        status.wip_limit = wip_limit;
        fields.push("wip_limit");
      }
      if (clear?.includes("wip_limit")) {
        status.wip_limit = null;
        fields.push("wip_limit");
      }
      return this.projectChange(project, "status", fields);
    },
    "status.reorder": ({ project: ref, status: statusRef, before, after, at }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      project.statuses = this.place(project.statuses, status, { before, after, at }, "status");
      return this.projectChange(project, "status", ["rank"]);
    },
    "status.delete": ({ project: ref, status: statusRef, move_tasks_to }) => {
      const project = this.writable(ref);
      const status = this.statusIn(project, statusRef);
      if (project.statuses.length === 1) {
        fail("invalid", "A project keeps at least one status.");
      }
      const tasks = this.state.tasks.filter((candidate) => candidate.statusId === status.id);
      if (tasks.length > 0) {
        const target = move_tasks_to ? this.statusIn(project, move_tasks_to) : null;
        if (!target || target === status) {
          fail("invalid", "Choose another status for its tasks.", { details: { path: "move_tasks_to" } });
        }
        for (const task of tasks) {
          this.enterStatus(task, target.id);
          this.touch(task, false);
        }
      }
      project.statuses = project.statuses.filter((candidate) => candidate !== status);
      return this.projectChange(project, "status", ["*"]);
    },
    "type.create": ({ project: ref, name, glyph, hue, make_default }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      const type = { id: nextId("ty"), name: this.uniqueName(project.types, name, "task type"), glyph: glyph ?? "task", hue: hue ?? "grey", is_default: false };
      project.types.push(type);
      if (make_default === true) {
        project.types.forEach((candidate) => (candidate.is_default = candidate === type));
      }
      return this.projectChange(project, "task_type", ["*"]);
    },
    "type.update": ({ project: ref, type: typeRef, name, glyph, hue, make_default, before, after, at }) => {
      const project = this.writable(ref);
      const type = this.typeIn(project, typeRef);
      const fields: string[] = [];
      if (name !== null && name !== undefined) {
        type.name = this.uniqueName(project.types, name, "task type", type.id);
        fields.push("name");
      }
      if (glyph !== null && glyph !== undefined) {
        type.glyph = glyph;
        fields.push("glyph");
      }
      if (hue !== null && hue !== undefined) {
        this.validHue(hue);
        type.hue = hue;
        fields.push("hue");
      }
      if (make_default === true) {
        project.types.forEach((candidate) => (candidate.is_default = candidate === type));
        fields.push("is_default");
      }
      if ([before, after, at].some((value) => value !== null && value !== undefined)) {
        project.types = this.place(project.types, type, { before, after, at }, "task type");
        fields.push("rank");
      }
      return this.projectChange(project, "task_type", fields);
    },
    "type.delete": ({ project: ref, type: typeRef, move_tasks_to }) => {
      const project = this.writable(ref);
      const type = this.typeIn(project, typeRef);
      if (type.is_default) {
        fail("invalid", "The default task type can't be deleted.", { hint: "Make another type the default first." });
      }
      const target = move_tasks_to ? this.typeIn(project, move_tasks_to, "move_tasks_to") : project.types.find((candidate) => candidate.is_default);
      if (!target || target === type) {
        fail("invalid", "Choose another task type for its tasks.", { details: { path: "move_tasks_to" } });
      }
      for (const task of this.state.tasks.filter((candidate) => candidate.typeId === type.id)) {
        task.typeId = target.id;
        this.touch(task, false);
      }
      project.types = project.types.filter((candidate) => candidate !== type);
      return this.projectChange(project, "task_type", ["*"]);
    },
    "label.create": ({ project: ref, name, hue }) => {
      const project = this.writable(ref);
      this.validHue(hue);
      project.labels.push({ id: nextId("l"), name: this.uniqueName(project.labels, name, "label"), hue: hue ?? "grey" });
      return this.projectChange(project, "label", ["*"]);
    },
    "label.update": ({ project: ref, label: labelRef, name, hue }) => {
      const project = this.writable(ref);
      const label = this.labelIn(project, labelRef);
      const fields: string[] = [];
      if (name !== null && name !== undefined) {
        label.name = this.uniqueName(project.labels, name, "label", label.id);
        fields.push("name");
      }
      if (hue !== null && hue !== undefined) {
        this.validHue(hue);
        label.hue = hue;
        fields.push("hue");
      }
      return this.projectChange(project, "label", fields);
    },
    "label.delete": ({ project: ref, label: labelRef }) => {
      const project = this.writable(ref);
      const label = this.labelIn(project, labelRef);
      for (const task of this.state.tasks.filter((candidate) => candidate.labelIds.includes(label.id))) {
        task.labelIds = task.labelIds.filter((id) => id !== label.id);
        this.touch(task);
      }
      project.labels = project.labels.filter((candidate) => candidate !== label);
      return this.projectChange(project, "label", ["*"]);
    },
    "board.get": ({ project, include_subprojects, show_older }) => this.board(project, include_subprojects, show_older === true),
    "layout.get": ({ project: ref }) => {
      const project = this.project(ref);
      return { project: project.key, layout: this.layout(project) };
    },
    "layout.update": (input) => {
      const project = this.project(input.project);
      const next: BoardLayout = input.reset === true ? structuredClone(DEFAULT_LAYOUT) : this.layout(project);
      const column = (ref: string) => {
        const status = project.statuses.find((candidate) => candidate.id === ref || fold(candidate.name) === fold(ref));
        return status?.id ?? ref;
      };
      const add = (list: string[], refs: readonly string[] | undefined) => {
        for (const id of (refs ?? []).map(column)) {
          if (!list.includes(id)) {
            list.push(id);
          }
        }
      };
      add(next.hidden, input.hide);
      next.hidden = next.hidden.filter((id) => !(input.show ?? []).map(column).includes(id));
      add(next.collapsed, input.collapse);
      next.collapsed = next.collapsed.filter((id) => !(input.expand ?? []).map(column).includes(id));
      if (input.order) {
        next.order = input.order.map(column);
      }
      if (input.sort_mode) {
        next.sort_mode = input.sort_mode;
      }
      const sorts = new Map(Object.entries(next.column_sort));
      for (const [ref, mode] of Object.entries(input.column_sort ?? {})) {
        if (mode === next.sort_mode) {
          sorts.delete(column(ref));
        } else {
          sorts.set(column(ref), mode);
        }
      }
      next.column_sort = Object.fromEntries(sorts);
      if (input.hide_done_after_days !== null && input.hide_done_after_days !== undefined) {
        if (!Number.isInteger(input.hide_done_after_days) || input.hide_done_after_days < 0 || input.hide_done_after_days > 3650) {
          fail("invalid", "Use 0 to 3650 days.", { details: { path: "hide_done_after_days" } });
        }
        next.hide_done_after_days = input.hide_done_after_days;
      }
      if (input.card_fields) {
        next.card_fields = [...new Set(input.card_fields)];
      }
      if (input.include_subprojects !== null && input.include_subprojects !== undefined) {
        next.include_subprojects = input.include_subprojects;
      }
      this.state.layouts[project.id] = next;
      return {
        output: { project: project.key, layout: structuredClone(next) },
        changes: [{ entity: "layout", id: project.id, fields: ["settings"], project: project.key }],
      };
    },
    "task.get": ({ keys, include }) => {
      const items = keys.map((key) => this.withParts(this.task(key), include));
      return { items, total: items.length };
    },
    "task.duplicate": ({ key, title, status, without_subtasks }) => {
      const task = this.task(key);
      const project = this.writable(this.projectOf(task).key);
      const target = status ? this.statusIn(project, status) : (project.statuses.find((candidate) => candidate.category === "todo") ?? project.statuses[0]);
      if (!target) {
        fail("invalid", `Project ${project.key} has no statuses.`);
      }
      const before = new Set(this.state.tasks.map((candidate) => candidate.id));
      const copy = this.copyTask(task, title ? this.checkTitle(title) : task.title, target.id, task.parentId, without_subtasks !== true);
      const relation: FakeRelation = { id: nextId("r"), from: copy.id, to: task.id, kind: "clones" };
      this.state.relations.push(relation);
      const created = this.state.tasks.filter((candidate) => !before.has(candidate.id));
      return {
        output: { task: this.fullTask(copy) },
        changes: [
          ...created.map((made) => ({ entity: "task", id: made.id, fields: ["*"], project: project.key })),
          { entity: "relation", id: relation.id, fields: ["link"], project: project.key },
        ],
      };
    },
    "task.claim": () => fail("forbidden", "Only agents claim tasks.", { hint: "Claims are for agent sessions through MCP." }),
    "task.release": ({ key }) => {
      const task = this.task(key);
      const had = this.state.claims.some((claim) => claim.taskId === task.id);
      this.state.claims = this.state.claims.filter((claim) => claim.taskId !== task.id);
      return had ? this.taskChange(task, ["claim"]) : { output: { task: this.fullTask(task) }, changes: [] };
    },
    "relation.add": ({ key, kind, other: otherKey }) => {
      const task = this.task(key);
      const other = this.task(otherKey);
      this.writable(this.projectOf(task).key);
      if (task === other) {
        fail("invalid", "A task can't be linked to itself.", { details: { path: "other" } });
      }
      if (kind === "subtask_of" || kind === "parent_of") {
        const [child, parent] = kind === "subtask_of" ? [task, other] : [other, task];
        if (child.projectId !== parent.projectId) {
          fail("invalid", "A subtask belongs to its parent's project.", { details: { path: "other" } });
        }
        if (this.isAncestor(child, parent)) {
          fail("invalid", `${this.keyOf(parent)} is a subtask of ${this.keyOf(child)}.`, { details: { path: "other" } });
        }
        this.adopt(child, parent.id);
        this.touch(child);
        return { output: { task: this.fullTask(task) }, changes: [{ entity: "task", id: child.id, fields: ["parent", "subtask_rank"], project: this.projectOf(child).key }] };
      }
      if (this.isAncestor(task, other) || this.isAncestor(other, task)) {
        fail("invalid", "A task can't be linked to its own parent or subtask.", { details: { path: "other" } });
      }
      const forward = kind === "blocks" || kind === "relates" || kind === "duplicates" || kind === "clones";
      const stored = { blocks: "blocks", blocked_by: "blocks", relates: "relates", duplicates: "duplicates", duplicated_by: "duplicates", clones: "clones", cloned_by: "clones" } as const;
      const [from, to] = forward ? [task, other] : [other, task];
      const relationKind = stored[kind];
      const exists = this.state.relations.some(
        (relation) =>
          relation.kind === relationKind &&
          ((relation.from === from.id && relation.to === to.id) || (relationKind === "relates" && relation.from === to.id && relation.to === from.id)),
      );
      if (exists) {
        return { output: { task: this.fullTask(task) }, changes: [] };
      }
      if (relationKind === "blocks" && this.state.relations.some((relation) => relation.kind === "blocks" && relation.from === to.id && relation.to === from.id)) {
        fail("invalid", `${this.keyOf(to)} already blocks ${this.keyOf(from)}.`, { details: { path: "kind" } });
      }
      const relation: FakeRelation = { id: nextId("r"), from: from.id, to: to.id, kind: relationKind };
      this.state.relations.push(relation);
      return { output: { task: this.fullTask(task) }, changes: [{ entity: "relation", id: relation.id, fields: ["link"], project: this.projectOf(task).key }] };
    },
    "relation.remove": ({ key, kind, other: otherKey }) => {
      const task = this.task(key);
      const other = this.task(otherKey);
      this.writable(this.projectOf(task).key);
      if (kind === "subtask_of" || kind === "parent_of") {
        const [child, parent] = kind === "subtask_of" ? [task, other] : [other, task];
        if (child.parentId !== parent.id) {
          return { output: { task: this.fullTask(task) }, changes: [] };
        }
        this.adopt(child, null);
        this.touch(child);
        return { output: { task: this.fullTask(task) }, changes: [{ entity: "task", id: child.id, fields: ["parent", "subtask_rank"], project: this.projectOf(child).key }] };
      }
      const link = this.linksOf(task).find((candidate) => candidate.kind === kind && candidate.task.id === other.id);
      if (!link) {
        return { output: { task: this.fullTask(task) }, changes: [] };
      }
      this.state.relations = this.state.relations.filter((relation) => relation.id !== link.id);
      return { output: { task: this.fullTask(task) }, changes: [{ entity: "relation", id: link.id, fields: ["link"], project: this.projectOf(task).key }] };
    },
    "comment.list": ({ key, revisions }) => {
      const items = this.liveComments(this.task(key)).map((comment) => this.comment(comment, revisions === true));
      return { items, total: items.length };
    },
    "comment.add": ({ key, body, reply_to }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      if (body.trim() === "") {
        fail("invalid", "A comment can't be empty.", { details: { path: "body" } });
      }
      if (reply_to !== null && reply_to !== undefined && this.liveComment(reply_to).taskId !== task.id) {
        fail("invalid", "A reply belongs to the same task.", { details: { path: "reply_to" } });
      }
      const comment: FakeComment = {
        id: nextId("c"),
        taskId: task.id,
        body,
        author: { kind: "user", name: "You" },
        via: "app",
        createdAt: this.today().toISOString(),
        editedAt: null,
        version: 1,
        revisions: [],
        replyTo: reply_to ?? null,
        deletedAt: null,
      };
      this.state.comments.push(comment);
      this.touch(task, false);
      return this.commentChange(comment, ["created"]);
    },
    "comment.update": ({ comment: id, body, expected_version }) => {
      const comment = this.liveComment(id);
      if (expected_version !== null && expected_version !== undefined && expected_version !== comment.version) {
        fail("conflict", "The comment changed since you read it.", { details: { current_version: comment.version } });
      }
      if (body.trim() === "") {
        fail("invalid", "A comment can't be empty.", { details: { path: "body" } });
      }
      if (body === comment.body) {
        return { output: { comment: this.comment(comment, false), deleted: false }, changes: [] };
      }
      const now = this.today().toISOString();
      comment.revisions.unshift({ body: comment.body, replaced_at: now, replaced_by: { kind: "user", name: "You" } });
      comment.body = body;
      comment.editedAt = now;
      comment.version += 1;
      return this.commentChange(comment, ["body"]);
    },
    "comment.delete": ({ comment: id }) => {
      const comment = this.liveComment(id);
      comment.deletedAt = this.today().toISOString();
      return this.commentChange(comment, ["trashed"]);
    },
    "file.get": ({ attachment }) => this.attachment(this.liveFile(attachment)),
    "file.read": ({ attachment }) => {
      const file = this.liveFile(attachment);
      if (file.kind === "image") {
        return { file: this.attachment(file), data_base64: file.base64, truncated: false };
      }
      if (file.kind === "text") {
        const bytes = decodeBase64(file.base64) ?? new Uint8Array();
        return { file: this.attachment(file), text: new TextDecoder().decode(bytes), truncated: false };
      }
      return { file: this.attachment(file), note: "Only text files and images are shown.", truncated: false };
    },
    "file.thumbnail": ({ attachment }) => {
      const file = this.liveFile(attachment);
      if (file.kind !== "image" || file.width === undefined || file.height === undefined) {
        fail("invalid", `${file.name} has no thumbnail.`, { hint: "Only images the engine can read have one." });
      }
      const scale = Math.min(1, 256 / Math.max(file.width, file.height));
      return {
        attachment: file.id,
        path: `cache/thumbs/${file.id.slice(0, 2)}/${file.id}.png`,
        mime: "image/png",
        width: Math.max(1, Math.round(file.width * scale)),
        height: Math.max(1, Math.round(file.height * scale)),
      };
    },
    "file.attach": ({ task: key, comment: commentId, data_base64, name, path }) => {
      if (path !== null && path !== undefined) {
        fail("invalid", "The sample workspace can't read files from disk.", { details: { path: "path" } });
      }
      const comment = commentId === null || commentId === undefined ? null : this.liveComment(commentId);
      const task = comment ? this.state.tasks.find((candidate) => candidate.id === comment.taskId) : key ? this.task(key) : undefined;
      if (!task) {
        fail("invalid", "Name the task or the comment to attach the file to.", { details: { path: "task" } });
      }
      this.writable(this.projectOf(task).key);
      const bytes = decodeBase64(data_base64 ?? "");
      if (!bytes || name === null || name === undefined || name.trim() === "") {
        fail("invalid", "Give the file's content as Base64 and its name.", { details: { path: "data_base64" } });
      }
      const limit = this.state.settings.file_max_mb * 1024 * 1024;
      if (bytes.length > limit) {
        fail("limit", `A file has at most ${String(this.state.settings.file_max_mb)} MB.`, { details: { max: limit } });
      }
      const sniffed = sniff(bytes, name);
      const file: FakeFile = {
        id: nextId("f"),
        taskId: task.id,
        commentId: comment?.id ?? null,
        name: name.trim(),
        size: bytes.length,
        base64: data_base64 ?? "",
        createdAt: this.today().toISOString(),
        deletedAt: null,
        ...sniffed,
      };
      this.state.files.push(file);
      this.touch(task, false);
      return this.fileChange(file, ["created"]);
    },
    "file.delete": ({ attachment }) => {
      const file = this.liveFile(attachment);
      file.deletedAt = this.today().toISOString();
      return this.fileChange(file, ["trashed"]);
    },
    "history.revert": ({ groups, dry_run }) => {
      const reverted: string[] = [];
      const skipped: { group: string; reason: string }[] = [];
      for (const group of groups ?? []) {
        const latest = this.undoStack[this.undoStack.length - 1];
        if (latest?.group === group && dry_run !== true) {
          this.revertLatest();
          reverted.push(group);
        } else if (latest?.group === group) {
          reverted.push(group);
        } else {
          skipped.push({ group, reason: "The sample workspace reverts only its latest change." });
        }
      }
      if (reverted.length === 0 || dry_run === true) {
        return { reverted, skipped };
      }
      return { output: { reverted, skipped }, changes: [{ entity: "workspace", id: "w-1", fields: ["*"] }] };
    },
    "task.create": ({ project: ref, title, status, description, priority, before, after, at, parent: parentKey }) => {
      const project = this.writable(ref);
      const parent = parentKey === null || parentKey === undefined ? null : this.task(parentKey);
      if (parent && parent.projectId !== project.id) {
        fail("invalid", "A subtask belongs to its parent's project.", { details: { path: "parent" } });
      }
      const trimmed = this.checkTitle(title);
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
        description: description ?? "",
        statusId: target.id,
        typeId: type.id,
        priority: priority ?? "none",
        labelIds: [],
        parentId: parent?.id ?? null,
        ...(parent ? { subtaskRank: this.lastSubtaskRank(parent.id) } : {}),
        rank: this.rankAt(target.id, { before, after, at }, null),
        startDate: null,
        dueDate: null,
        estimateMin: null,
        spentMin: 0,
        files: 0,
        comments: 0,
        createdAt: now,
        editedAt: now,
        statusEnteredAt: now,
        updatedAt: now,
        updatedBy: { kind: "user", name: "You" },
        closedAt: isClosed(target.category) ? now : null,
        version: 1,
      };
      project.nextNumber += 1;
      this.state.tasks.push(task);
      if (task.description !== "") {
        this.addRevision(task, task.description, "normal", null);
      }
      return this.taskChange(task, ["*"]);
    },
    "task.update": (input) => {
      const task = this.task(input.key);
      const project = this.writable(this.projectOf(task).key);
      if (input.expected_version !== null && input.expected_version !== undefined && input.expected_version !== task.version) {
        fail("conflict", `${this.keyOf(task)} changed since you read it.`, {
          hint: "Read it again and repeat the change.",
          details: { current_version: task.version },
        });
      }
      const clear = input.clear ?? [];
      const fields: string[] = [];
      if (input.title !== null && input.title !== undefined) {
        task.title = this.checkTitle(input.title);
        fields.push("title");
      }
      let save: DescriptionSave | undefined;
      const text = clear.includes("description") ? "" : input.description;
      if (text !== null && text !== undefined) {
        save = this.saveDescription(task, text, input.base_revision ?? null);
        if (save.outcome === "conflict") {
          fields.push("conflict_copy");
        } else if (save.outcome !== "unchanged") {
          fields.push("description");
        }
      }
      if (input.resolve_conflict) {
        this.resolveConflict(task, input.resolve_conflict.copy, input.resolve_conflict.action, input.resolve_conflict.text ?? null);
        fields.push("description");
      }
      if (input.type !== null && input.type !== undefined) {
        task.typeId = this.typeIn(project, input.type).id;
        fields.push("type");
      }
      if (input.priority !== null && input.priority !== undefined) {
        task.priority = input.priority;
        fields.push("priority");
      }
      const resolve = (refs: readonly string[], path: string) => refs.map((ref) => this.labelIn(project, ref, path).id);
      if (input.labels || input.add_labels || input.remove_labels || clear.includes("labels")) {
        let labels = clear.includes("labels") ? [] : input.labels ? resolve(input.labels, "labels") : [...task.labelIds];
        labels = [...labels, ...resolve(input.add_labels ?? [], "add_labels").filter((id) => !labels.includes(id))];
        const removed = resolve(input.remove_labels ?? [], "remove_labels");
        task.labelIds = labels.filter((id) => !removed.includes(id));
        fields.push("labels");
      }
      const start = clear.includes("start_date") ? null : (input.start_date ?? task.startDate);
      const due = clear.includes("due_date") ? null : (input.due_date ?? task.dueDate);
      if (start !== null && due !== null && start > due) {
        fail("invalid", "The start date is after the due date.", { details: { path: input.start_date ? "start_date" : "due_date" } });
      }
      if (start !== task.startDate) {
        task.startDate = start;
        fields.push("start_date");
      }
      if (due !== task.dueDate) {
        task.dueDate = due;
        fields.push("due_date");
      }
      if (input.estimate !== null && input.estimate !== undefined) {
        const minutes = minutesOf(input.estimate);
        if (minutes === null) {
          fail("invalid", "An estimate is a duration such as 1h 30m.", { details: { path: "estimate" } });
        }
        task.estimateMin = minutes;
        fields.push("estimate");
      }
      if (clear.includes("estimate")) {
        task.estimateMin = null;
        fields.push("estimate");
      }
      if (fields.length === 0) {
        return { output: { task: this.fullTask(task), ...some("description_save", save) }, changes: [] };
      }
      this.touch(task, !(fields.length === 1 && fields[0] === "conflict_copy"));
      const change = this.taskChange(task, fields);
      return { ...change, output: { ...change.output, ...some("description_save", save) } };
    },
    "task.resolve_conflict": ({ key, copy, action, text, base_revision }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      if (base_revision !== null && base_revision !== undefined && base_revision !== this.currentRevision(task)?.id) {
        fail("conflict", "The description changed since you read it.", { hint: "Read it again and resolve the copy once more." });
      }
      this.resolveConflict(task, copy, action, text ?? null);
      this.touch(task);
      return this.taskChange(task, ["description"]);
    },
    "checklist.update": ({ key, comment: commentId, toggle, add, remove }) => {
      if ((add?.length ?? 0) > 0 || (remove?.length ?? 0) > 0) {
        fail("invalid", "The sample workspace only ticks and clears items.");
      }
      const comment = commentId === null || commentId === undefined ? null : this.liveComment(commentId);
      const task = comment ? this.state.tasks.find((candidate) => candidate.id === comment.taskId) : this.task(key ?? "");
      if (!task) {
        fail("not_found", "There is no such task.");
      }
      this.writable(this.projectOf(task).key);
      let text = comment ? comment.body : task.description;
      for (const wanted of toggle ?? []) {
        const items = checklistOf(text);
        const expected = wanted.text.replace(/\s+/g, " ").trim();
        const numbered = items[wanted.number - 1];
        const same = items.filter((item) => item.text === expected);
        const item = numbered?.text === expected ? numbered : same.length === 1 ? same[0] : undefined;
        if (!item) {
          fail("conflict", `The checklist changed; please tick it again. Item ${String(wanted.number)} reads "${numbered?.text ?? ""}", not "${expected}".`, {
            details: { field: "toggle", number: wanted.number },
          });
        }
        text = setChecked(text, item, wanted.checked ?? !item.checked);
      }
      const owner = this.keyOf(task);
      const unchanged = text === (comment ? comment.body : task.description);
      if (comment) {
        if (!unchanged) {
          const now = this.today().toISOString();
          comment.revisions.unshift({ body: comment.body, replaced_at: now, replaced_by: { kind: "user", name: "You" } });
          comment.body = text;
          comment.editedAt = now;
          comment.version += 1;
        }
        const items = publicChecklist(text);
        return {
          output: { checklist: { done: items.filter((item) => item.checked).length, total: items.length }, comment: comment.id, items, key: owner, version: comment.version },
          changes: unchanged ? [] : this.commentChange(comment, ["body"]).changes,
        };
      }
      if (!unchanged) {
        this.addRevision(task, text, "normal", this.currentRevision(task)?.id ?? null);
        this.touch(task);
      }
      const items = publicChecklist(text);
      return {
        output: {
          checklist: this.checklist(task),
          items,
          key: owner,
          ...some("revision", this.currentRevision(task)?.id),
          version: task.version,
        },
        changes: unchanged ? [] : this.taskChange(task, ["description"]).changes,
      };
    },
    "description.history": ({ task: key }) => {
      if (key === null || key === undefined) {
        fail("invalid", "The sample workspace keeps revisions for tasks only.", { details: { path: "task" } });
      }
      const task = this.task(key);
      const current = this.currentRevision(task);
      const items = this.revisionsOf(task)
        .reverse()
        .map((revision) => this.revisionInfo(revision, current));
      return { items, total: items.length };
    },
    "description.revision": ({ revision: id, diff_from }) => {
      const revision = this.revision(id);
      const task = this.state.tasks.find((candidate) => candidate.id === revision.taskId);
      const current = task && this.currentRevision(task);
      const from = diff_from === null || diff_from === undefined ? null : this.revision(diff_from);
      if (from && from.taskId !== revision.taskId) {
        fail("invalid", "Both revisions must belong to the same description.", { details: { path: "diff_from" } });
      }
      return {
        body: revision.body,
        ...some("diff", from && unifiedDiff(from.body, revision.body, from.id, revision.id)),
        owner: task ? this.keyOf(task) : "",
        revision: this.revisionInfo(revision, current),
      };
    },
    "markdown.render": ({ documents }) => {
      if (documents.length === 0 || documents.length > 200) {
        fail("invalid", "Render 1 to 200 texts at once.", { details: { path: "documents" } });
      }
      const known = (key: string) => {
        try {
          this.task(key);
          return true;
        } catch {
          return false;
        }
      };
      const items = documents.map((document) => {
        let text: string;
        let id: string;
        let revision: string | undefined;
        let version: number | undefined;
        if (document.task !== null && document.task !== undefined) {
          const task = this.task(document.task);
          text = task.description;
          id = task.id;
          revision = this.currentRevision(task)?.id;
          version = task.version;
        } else if (document.comment !== null && document.comment !== undefined) {
          const comment = this.liveComment(document.comment);
          text = comment.body;
          id = comment.id;
          version = comment.version;
        } else if (document.revision !== null && document.revision !== undefined) {
          const found = this.revision(document.revision);
          text = found.body;
          id = found.id;
          revision = found.id;
        } else if (document.project !== null && document.project !== undefined) {
          const project = this.project(document.project);
          text = project.description;
          id = project.id;
        } else {
          text = document.source ?? "";
          id = "preview";
        }
        return {
          html: renderFake(text, known),
          checklist: publicChecklist(text),
          document_id: document.document_id ?? id,
          ...some("revision", revision),
          ...some("version", version),
        };
      });
      return { items };
    },
    "task.reorder_subtask": ({ key, before, after, at, expected_version }) => {
      const sub = this.task(key);
      this.writable(this.projectOf(sub).key);
      const parent = this.state.tasks.find((candidate) => candidate.id === sub.parentId);
      if (!parent) {
        fail("invalid", `${this.keyOf(sub)} isn't a subtask.`, { details: { path: "key" } });
      }
      if (expected_version !== null && expected_version !== undefined && expected_version !== sub.version) {
        fail("conflict", `${this.keyOf(sub)} changed since you read it.`, { details: { current_version: sub.version } });
      }
      const order = this.subtasksOf(parent).filter((candidate) => candidate !== sub);
      const sibling = (ref: string, path: string) => {
        const other = this.task(ref);
        const index = order.indexOf(other);
        if (index < 0) {
          fail("invalid", `${this.keyOf(other)} isn't another subtask of ${this.keyOf(parent)}.`, { details: { path } });
        }
        return index;
      };
      const index =
        before !== null && before !== undefined
          ? sibling(before, "before")
          : after !== null && after !== undefined
            ? sibling(after, "after") + 1
            : at === "top"
              ? 0
              : order.length;
      order.splice(index, 0, sub);
      const old = this.subtasksOf(parent);
      if (order.every((candidate, place) => candidate === old[place])) {
        return { output: { task: this.withParts(parent, ["subtasks"]) }, changes: [] };
      }
      const prev = order[index - 1]?.subtaskRank;
      const next = order[index + 1]?.subtaskRank;
      sub.subtaskRank = prev === undefined ? (next ?? 1) - 1 : next === undefined ? prev + 1 : (prev + next) / 2;
      this.touch(sub, false);
      return { output: { task: this.withParts(parent, ["subtasks"]) }, changes: [{ entity: "task", id: sub.id, fields: ["subtask_rank"], project: this.projectOf(sub).key }] };
    },
    "task.move": ({ key, status, project: toProject, expected_version, before, after, at }) => {
      const task = this.task(key);
      const project = this.writable(this.projectOf(task).key);
      if (toProject !== null && toProject !== undefined && this.project(toProject) !== project) {
        fail("invalid", "The sample workspace doesn't move tasks between projects.");
      }
      if (expected_version !== null && expected_version !== undefined && expected_version !== task.version) {
        fail("conflict", `${this.keyOf(task)} changed since you read it.`, { details: { current_version: task.version } });
      }
      const fields: string[] = [];
      const target = status === null || status === undefined ? this.statusIn(project, task.statusId) : this.statusIn(project, status);
      if (target.id !== task.statusId) {
        this.enterStatus(task, target.id);
        fields.push("status");
      }
      const placed = [before, after, at].some((value) => value !== null && value !== undefined);
      if (placed || fields.length > 0) {
        task.rank = this.rankAt(target.id, { before, after, at }, task);
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
  if (typeof output === "object" && output !== null) {
    return { ...output, group };
  }
  return output;
}

/** Columns in the layout's order; a column it doesn't list goes after the one before it in project order. */
function arrange(columns: readonly Column[], order: readonly string[]): Column[] {
  const result = order.map((id) => columns.find((column) => column.id === id)).filter((column): column is Column => column !== undefined);
  columns.forEach((column, index) => {
    if (result.includes(column)) {
      return;
    }
    const previous = columns[index - 1];
    result.splice(previous ? result.indexOf(previous) + 1 : 0, 0, column);
  });
  return result;
}
