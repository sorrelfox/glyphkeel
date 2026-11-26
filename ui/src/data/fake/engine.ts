// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// An engine stand-in that keeps a workspace in memory and answers the
// operations the screens use, with the engine's rules where the screens can
// see them: keys, default statuses, closing, WIP, sub-project columns,
// layouts, typed errors, change events and Undo.

import {
  type ActivityEntry,
  type ActivityTask,
  type Actor,
  type AgentLimit,
  type AgentLimits,
  type Backup,
  type BulkItem,
  type BulkResult,
  type BulkUpdateInput,
  type CustomFieldChange,
  type ListedTask,
  type Attachment,
  type Board,
  type BoardLayout,
  type GanttLayout,
  type ListLayout,
  type Card,
  type Column,
  type Comment,
  type CommentChange,
  type DueReminder,
  type DueReminders,
  type ConflictAction,
  type DescriptionSave,
  type Draft,
  type DraftKind,
  type Edge,
  type EntityChange,
  type TrashEntry,
  type FileChange,
  type Hue,
  type Link,
  type LinkKind,
  type LabelRef,
  type OpError,
  type OpId,
  type OpMap,
  OP_SAFETY,
  type Project,
  type ProjectChange,
  type ProjectIcon,
  type ProjectSummary,
  type Reminder,
  type RevisionInfo,
  type RevisionKind,
  type SavedView,
  type Settings,
  type SettingsPatch,
  type Task,
  type TaskChange,
  type TaskPart,
  type TaskRef,
  type TaskUpdateInput,
  type TimeEntryChange,
  type TimerChange,
  type TimerStatus,
  type TypeRef,
  type Via,
  type WikiPage,
  type WikiPageSummary,
  type Backlink,
  type Reference,
} from "../../generated/ops";
import { PROJECT_ICONS, suggestHue } from "../catalog";
import { formatDuration, parseDuration } from "../duration";
import { decodeBase64, materialize, recordedFields, sniff } from "./details";
import { applyValues, createField, deleteField, fieldOut, fieldsFor, fieldsOf, findField, listedValues, placeField, queryFields, taskValues, updateField } from "./fields";
import { checklistOf, escapeHtml, merge3, publicChecklist, renderFake, setChecked, unifiedDiff } from "./markdown";
import { newProject, nextFileId, nextId, nextUuid, SAMPLE_AGENT } from "./seed";
import { type FakeComment, type FakeFile, type FakeProject, type FakeRelation, type FakeReminder, type FakeRevision, type FakeState, type FakeTask, type FakeTimeEntry, type FakeWikiPage } from "./state";
import { naturalDay, nextDay, searchAll, searchWiki, slugOf, headingsOf } from "./overview";
import { moveDependents, scheduleOf } from "./schedule";

/** The Gantt chart's layout until the person changes it. */
const DEFAULT_GANTT_LAYOUT: GanttLayout = { zoom: "day", widths: {}, group_by: "parent", move_dependents: false };
import { linksIn, placePage, subtree, type TextLink } from "./wiki";
import { createView, deleteView, listViews, matchingKeys, parseQuery, queryError, searchTasks, updateView, type GroupInfo, type QueryNames } from "./query";
import { addDays, csvOf, localDay, MAX_ENTRY_MINUTES, momentOf, reminderAt, reminderOut, sortDue, timeEntryOut, timerElapsedMs, timerOut, unitsOf, weekStart, workDay } from "./time";

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
/** The limits on agent sessions, and the engine's defaults: no limit on new tasks. */
const AGENT_LIMITS: readonly AgentLimit[] = ["writes_per_minute", "new_tasks_per_day", "trash_items_per_call", "trash_items_per_hour"];
const DEFAULT_AGENT_LIMITS: Readonly<AgentLimits> = { writes_per_minute: 120, trash_items_per_call: 20, trash_items_per_hour: 50 };
const MAX_DEPTH = 3;
const PROJECT_ICON_NAMES: ReadonlySet<string> = new Set(PROJECT_ICONS);
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
  /**
   * The person's drafts by kind and target ID. They are kept apart from the
   * workspace's data, as the engine keeps them: Undo doesn't touch them and
   * they are no history.
   */
  private readonly drafts = new Map<string, Draft>();
  /** Whether the running call is an agent's, whose saves leave the person's drafts alone. */
  private byAgent = false;

  constructor(state: FakeState, today: () => Date = () => new Date()) {
    this.state = state;
    this.today = today;
    materialize(state, SAMPLE_AGENT, (prefix) => (prefix === "c" ? nextUuid() : nextId(prefix)));
    // Every description starts with a revision of its own, as the engine's do.
    for (const task of state.tasks) {
      if (task.description !== "" && !state.revisions.some((revision) => revision.taskId === task.id)) {
        state.revisions.push({ id: nextId("r"), taskId: task.id, body: task.description, kind: "normal", base: null, author: { kind: "user", name: "You" }, createdAt: task.createdAt, resolvedAt: null });
      }
    }
    // Time already spent shows as one entry on the day the task was last updated.
    for (const task of state.tasks) {
      if (task.spentMin > 0 && !state.timeEntries.some((entry) => entry.taskId === task.id)) {
        let left = task.spentMin;
        while (left > 0) {
          const minutes = Math.min(left, MAX_ENTRY_MINUTES);
          left -= minutes;
          state.timeEntries.push({ id: nextId("te"), taskId: task.id, date: localDay(new Date(task.updatedAt)), minutes, note: "", author: { kind: "user", name: "You" }, createdAt: task.updatedAt, editedAt: null, version: 1, deletedAt: null });
        }
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
    // One with dates, so the Gantt chart shows it too.
    const working = state.tasks.filter((task) => task.closedAt === null && this.statusIn(this.projectOf(task), task.statusId).category === "in_progress");
    const claimed = working.find((task) => task.dueDate !== null) ?? working[0];
    if (claimed) {
      state.claims.push({ taskId: claimed.id, agent: SAMPLE_AGENT, session: "session-1", until: new Date(today().getTime() + 45 * 60_000).toISOString() });
    }
  }

  /** Runs one operation. Commands record an Undo step and return their changes. */
  /** Runs an operation; `by` records its changes as another actor's (an agent's, through MCP), for tests. */
  call(op: string, input: unknown, by?: { actor: Actor; via: Via }): FakeAnswer {
    if (op === "history.undo" || op === "history.redo") {
      return this.undoOrRedo(op);
    }
    const handler = (this.handlers as Record<string, ((input: unknown) => unknown) | undefined>)[op];
    if (!handler) {
      fail("not_found", `There is no operation "${op}".`);
    }
    this.byAgent = by?.actor.kind === "agent";
    // Reading changes nothing, so it needs no Undo step (copying a large workspace takes long);
    // drafts are not the workspace's data and have none either.
    if ((OP_SAFETY as Record<string, string>)[op] === "read" || op.startsWith("draft.")) {
      return { output: handler(input ?? {}) };
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
    this.record(before, op, group, answer.changes, by);
    this.undoStack.push({ state: before, op, group });
    this.redoStack = [];
    return { output: withGroup(answer.output, group), changes: { group, seq: this.state.seq, changes: answer.changes } };
  }

  /** The settings as the engine answers them, with the default limits for a state saved without them. */
  private settingsNow(): Settings {
    const settings = this.state.settings;
    const saved = (settings as Partial<Settings>).agent_limits;
    return { ...settings, agent_limits: { ...(saved ?? DEFAULT_AGENT_LIMITS) } };
  }

  /** Adds one command's changes to the activity log. */
  private record(before: FakeState, op: string, group: string, changes: readonly EntityChange[], by: { actor: Actor; via: Via } = { actor: { kind: "user", name: "You" }, via: "app" }): void {
    const at = this.today().toISOString();
    for (const change of changes) {
      const project = this.state.projects.find((candidate) => candidate.key === change.project);
      const entry: ActivityEntry = {
        actor: by.actor,
        at,
        entity: change.entity,
        fields: recordedFields(before, this.state, change, (task) => this.keyOf(task)),
        group,
        id: change.id,
        op,
        ...some("project", project?.id),
        seq: this.state.seq,
        via: by.via,
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

  private backup(name: string): Backup {
    const backup = this.state.backups?.find((candidate) => candidate.name === name);
    if (!backup) {
      fail("not_found", `There is no backup "${name}".`);
    }
    return backup;
  }

  /** Every subtask under `id`, at any depth. */
  private subtasksDeep(id: string): FakeTask[] {
    const children = this.state.tasks.filter((task) => task.parentId === id);
    return children.flatMap((child) => [child, ...this.subtasksDeep(child.id)]);
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

  private revisionsOf(task: { id: string }): FakeRevision[] {
    return this.state.revisions.filter((revision) => revision.taskId === task.id);
  }

  /** The revision the description is at now. */
  private currentRevision(task: { id: string }): FakeRevision | undefined {
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
      ...some("message", revision.message),
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
      ...some("icon", project.icon),
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
      ...some("icon", project.icon),
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
      next_label_hue: suggestHue(project.labels.map((label) => label.hue)),
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
      ...(taskValues(this.state, task).length > 0 ? { fields: taskValues(this.state, task) } : {}),
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

  /** The task an activity entry belongs to, as it is now: the task itself, or the task of a comment, time entry or file. */
  private taskOfEntry(entry: ActivityEntry): ActivityTask | undefined {
    const owner = (() => {
      switch (entry.entity) {
        case "task":
          return entry.id;
        case "comment":
          return this.state.comments.find((comment) => comment.id === entry.id)?.taskId;
        case "attachment":
          return this.state.files.find((file) => file.id === entry.id)?.taskId;
        case "time_entry":
          return this.state.timeEntries.find((each) => each.id === entry.id)?.taskId;
        default:
          return undefined;
      }
    })();
    const task = this.state.tasks.find((candidate) => candidate.id === owner);
    return task === undefined ? undefined : { id: task.id, key: this.keyOf(task), title: task.title };
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
      case "time_entry":
        return this.state.timeEntries.some((each) => each.id === entry.id && each.taskId === taskId);
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
    const page = file.pageId === undefined ? undefined : (this.state.wiki ?? []).find((candidate) => candidate.id === file.pageId);
    return {
      ...(page ? { page: page.id, project: this.projectKeyOf(page) } : {}),
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
      } else if (part === "time") {
        const units = unitsOf(this.state.settings);
        full.time_entries = this.entriesOf(task).map((entry) => timeEntryOut(entry, this.keyOf(task), units));
      } else if (part === "reminder") {
        const reminder = this.reminderOf(task);
        if (reminder) {
          full.reminder = reminder;
        }
      } else {
        full.files = this.state.files.filter((file) => file.taskId === task.id && file.commentId === null && file.deletedAt === null).map((file) => this.attachment(file));
      }
    }
    return full;
  }

  /** What a draft is about: the ID of the task, comment, project or page, and the task's key. */
  private draftOwner(kind: DraftKind, target: string): { id: string; task?: string } {
    switch (kind) {
      case "task":
      case "new_comment": {
        const task = this.task(target);
        return { id: task.id, task: this.keyOf(task) };
      }
      case "comment": {
        const comment = this.liveComment(target);
        const task = this.state.tasks.find((candidate) => candidate.id === comment.taskId);
        return { id: comment.id, ...some("task", task && this.keyOf(task)) };
      }
      case "project":
      case "new_wiki_page":
        return { id: this.project(target).id };
      case "wiki_page":
        return { id: this.wikiPage(target, null).id };
    }
  }

  /** The kept draft of `kind` for `id`, unless it is older than 30 days. */
  private keptDraft(kind: DraftKind, id: string): Draft | undefined {
    const draft = this.drafts.get(`${kind}:${id}`);
    return draft !== undefined && this.today().getTime() - Date.parse(draft.updated_at) <= 30 * 86_400_000 ? draft : undefined;
  }

  /** Saving a text forgets its draft, as the engine does. */
  private draftSaved(kind: DraftKind, id: string): void {
    if (this.byAgent) {
      return;
    }
    this.drafts.delete(`${kind}:${id}`);
  }

  private liveComment(id: string): FakeComment {
    const comment = this.state.comments.find((candidate) => candidate.id === id && candidate.deletedAt === null);
    if (!comment) {
      fail("not_found", `There is no comment ${id}.`);
    }
    return comment;
  }

  private liveFile(id: string): FakeFile {
    const pages = new Set((this.state.wiki ?? []).map((page) => page.id));
    const file = this.state.files.find((candidate) => candidate.id === id && candidate.deletedAt === null && (candidate.pageId === undefined || pages.has(candidate.pageId)));
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
    const page = file.pageId === undefined ? undefined : (this.state.wiki ?? []).find((candidate) => candidate.id === file.pageId);
    const project = task ? this.projectOf(task).key : page ? this.projectKeyOf(page) : undefined;
    return {
      output: { file: this.attachment(file), deleted: file.deletedAt !== null },
      changes: [{ entity: "attachment", id: file.id, fields, ...some("project", project) }],
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
      this.state.files.push({ ...file, id: nextFileId(), taskId: copy.id, createdAt: now });
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

  private fieldChange(field: { id: string; projectId: string | null }, fields: string[], removed: number): Result<CustomFieldChange> {
    const stored = fieldsOf(this.state).find((each) => each.id === field.id);
    const project = this.state.projects.find((candidate) => candidate.id === field.projectId);
    return {
      output: { ...(stored ? { field: fieldOut(this.state, stored, this.state.projects) } : {}), ...(removed > 0 ? { values_removed: removed } : {}) },
      changes: [{ entity: "custom_field", id: field.id, fields, ...(project ? { project: project.key } : {}) }],
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

  private validIcon(icon: ProjectIcon | null | undefined): void {
    if (icon !== undefined && icon !== null && !PROJECT_ICON_NAMES.has(icon)) {
      fail("invalid", `"${icon}" is not a project icon.`, { details: { path: "icon" } });
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

  // ---------- time ----------

  /** Whether the running timer's long-running reminder was acknowledged. */
  private timerReminded = false;

  private entryMinutes(text: string, path: string): number {
    const minutes = parseDuration(text, unitsOf(this.state.settings));
    if (minutes === null) {
      fail("invalid", `"${text}" isn't a duration.`, { hint: "For example 1h 30m, 90m, 1.5h or 1:30.", details: { path } });
    }
    if (minutes < 1 || minutes > MAX_ENTRY_MINUTES) {
      fail("invalid", "A time entry holds from 1 minute to 24 hours.", { details: { path } });
    }
    return minutes;
  }

  private entryNote(note: string | null | undefined): string {
    const text = (note ?? "").trim();
    if (text.length > 2000) {
      fail("limit", "A time entry's note has at most 2000 characters.", { details: { path: "note" } });
    }
    return text;
  }

  private liveEntry(id: string): FakeTimeEntry {
    const entry = this.state.timeEntries.find((candidate) => candidate.id === id && candidate.deletedAt === null);
    if (!entry) {
      fail("not_found", `There is no time entry ${id}.`);
    }
    return entry;
  }

  private entriesOf(task: FakeTask): FakeTimeEntry[] {
    return this.state.timeEntries
      .filter((entry) => entry.taskId === task.id && entry.deletedAt === null)
      .sort((a, b) => b.date.localeCompare(a.date) || b.createdAt.localeCompare(a.createdAt));
  }

  private entryOutput(entry: FakeTimeEntry): TimeEntryChange {
    const task = this.task(entry.taskId);
    return { deleted: entry.deletedAt !== null, entry: timeEntryOut(entry, this.keyOf(task), unitsOf(this.state.settings)), spent_minutes: task.spentMin };
  }

  private entryChange(entry: FakeTimeEntry, fields: string[]): Result<TimeEntryChange> {
    const task = this.task(entry.taskId);
    return { output: this.entryOutput(entry), changes: [{ entity: "time_entry", id: entry.id, fields, project: this.projectOf(task).key }] };
  }

  private timerStatus(): TimerStatus {
    const timer = this.state.timer;
    const task = timer && this.state.tasks.find((candidate) => candidate.id === timer.taskId);
    return timer && task ? { timer: timerOut(timer, this.taskRef(task), this.today(), unitsOf(this.state.settings)) } : {};
  }

  private timerEvent(): EntityChange {
    return { entity: "timer", id: "w-1", fields: ["timer"] };
  }

  /** Stops the timer: logs its time (rounded; under half a minute logs nothing) unless discarded. */
  private stopTimer(minutes: number | null, discard: boolean, note?: string): Result<TimerChange> {
    const timer = this.state.timer;
    if (timer === null) {
      return { output: {}, changes: [] };
    }
    this.state.timer = null;
    this.timerReminded = false;
    if (discard) {
      return { output: {}, changes: [] };
    }
    const measured = Math.round(timerElapsedMs(timer, this.today()) / 60_000);
    const logged = Math.min(minutes ?? measured, MAX_ENTRY_MINUTES);
    if (logged < 1) {
      return { output: { warnings: ["Under half a minute: no time was logged."] }, changes: [] };
    }
    const entry: FakeTimeEntry = {
      id: nextId("te"),
      taskId: timer.taskId,
      date: localDay(new Date(timer.startedAt)),
      minutes: logged,
      note: this.entryNote(note ?? timer.note),
      author: { kind: "user", name: "You" },
      createdAt: this.today().toISOString(),
      editedAt: null,
      version: 1,
      deletedAt: null,
    };
    this.state.timeEntries.push(entry);
    const task = this.task(timer.taskId);
    task.spentMin += logged;
    task.updatedAt = entry.createdAt;
    const change = this.entryChange(entry, ["created"]);
    return { output: { entry: change.output.entry }, changes: change.changes };
  }

  private reminderOf(task: FakeTask): Reminder | undefined {
    const reminder = this.state.reminders.find((candidate) => candidate.taskId === task.id);
    return reminder && reminderOut(reminder, this.keyOf(task), reminderAt(reminder, task.dueDate, this.state.settings.reminder_time));
  }

  // ---------- queries, saved views and bulk changes ----------

  /** A task as lists show it. */
  private listed(task: FakeTask): ListedTask {
    const card = this.card(task);
    const status = this.statusIn(this.projectOf(task), task.statusId);
    return {
      blocked: card.blocked,
      checklist: card.checklist,
      ...some("claim", card.claim),
      ...some("closed_at", card.closed_at),
      comment_count: card.comment_count,
      created_at: task.createdAt,
      created_by: { kind: "user", name: "You" },
      ...some("due_date", card.due_date),
      ...some("estimate_minutes", card.estimate_minutes),
      file_count: card.file_count,
      id: card.id,
      key: card.key,
      labels: card.labels,
      ...some("fields", listedValues(this.state, task)),
      ...some("parent", card.parent),
      priority: card.priority,
      progress_percent: card.progress_percent,
      project: card.project,
      rank: card.rank,
      ...some("remaining_minutes", task.estimateMin === null ? null : task.estimateMin - task.spentMin),
      spent_minutes: card.spent_minutes,
      ...some("start_date", card.start_date),
      status: { id: status.id, name: status.name, category: status.category },
      subtask_count: card.subtasks,
      title: card.title,
      type: card.type,
      updated_at: card.updated_at,
      version: card.version,
    };
  }


  /** A date of `task.create` or `task.update`, as `YYYY-MM-DD` or in words. */
  private dayInput(text: string, path: string): string {
    return (
      naturalDay(text, localDay(this.today())) ??
      fail("invalid", `"${text}" is not a date. Write 2026-12-31, or today, fri, next mon, in 3d, +3d.`, { details: { path, message_id: "date-invalid", args: { text } } })
    );
  }

  /** Puts a page (with the pages inside it) under `parentId`, at a place among its siblings; never into itself. */
  private movePage(page: FakeWikiPage, parentId: string | null, place: Place): void {
    const pages = (this.state.wiki ??= []);
    if (parentId !== null && subtree(pages, page).some((inside) => inside.id === parentId)) {
      fail("invalid", "A page can't go inside itself or a page inside it.", { details: { path: "parent", message_id: "wiki-parent-loop", args: {} } });
    }
    const sibling = (ref: string | null | undefined, path: string) => {
      if (ref === null || ref === undefined) {
        return undefined;
      }
      const found = this.wikiPage(ref, this.projectKeyOf(page));
      if (found.parentId !== parentId || found === page) {
        fail("invalid", `"${ref}" is not a page beside it.`, { details: { path, message_id: "wiki-sibling-not-found", args: { page: ref } } });
      }
      return found.id;
    };
    placePage(pages, page, parentId, { before: sibling(place.before, "before"), after: sibling(place.after, "after"), at: place.at ?? undefined });
  }

  private wikiSummary(page: FakeWikiPage): WikiPageSummary {
    const pages = this.state.wiki ?? [];
    let depth = 0;
    for (let parent = page.parentId; parent !== null; parent = pages.find((other) => other.id === parent)?.parentId ?? null) {
      depth += 1;
    }
    return {
      id: page.id,
      title: page.title,
      slug: page.slug,
      project: this.state.projects.find((project) => project.id === page.projectId)?.key ?? "",
      depth,
      children: pages.filter((other) => other.parentId === page.id).length,
      ...some("parent", page.parentId),
      ...(page.agentBrief === true ? { agent_brief: true } : {}),
      ...(page.home === true ? { home: true } : {}),
      ...(page.template === true ? { template: true } : {}),
      updated_at: page.updatedAt,
    };
  }

  private wikiPage(ref: string, projectRef: string | null | undefined): FakeWikiPage {
    const pages = this.state.wiki ?? [];
    const byId = pages.find((page) => page.id === ref);
    if (byId) {
      return byId;
    }
    if (projectRef === null || projectRef === undefined) {
      fail("not_found", `There is no page with the ID ${ref}.`, { details: { message_id: "wiki-page-id-not-found", args: { page: ref } } });
    }
    const project = this.project(projectRef);
    return (
      this.pageIn(project, ref) ??
      fail("not_found", `Project ${project.key} has no page "${ref}".`, { details: { message_id: "wiki-page-not-found", args: { project: project.key, page: ref } } })
    );
  }

  /** The live page of `project` a link or an input names: title (any case), slug or old slug. */
  private pageIn(project: FakeProject, ref: string): FakeWikiPage | undefined {
    const pages = (this.state.wiki ?? []).filter((page) => page.projectId === project.id);
    const wanted = ref.trim();
    const slug = slugOf(wanted, []);
    return (
      pages.find((page) => fold(page.title) === fold(wanted)) ??
      pages.find((page) => page.slug === wanted.toLowerCase() || page.slug === slug) ??
      pages.find((page) => (page.oldSlugs ?? []).includes(slug))
    );
  }

  /**
   * A wiki page's text with its macros expanded, as the engine's renderer
   * writes them: `{{toc}}`, `{{children}}`, `{{include Page}}` (three levels,
   * never inside itself) and `{{tasks <query>}}` (in the page's project
   * unless the query names one; 50 at most).
   */
  private renderPage(text: string, page: FakeWikiPage, known: (key: string) => boolean, stack: string[]): string {
    const lines = text.split(/\r\n|\r|\n/);
    const macros: string[] = [];
    const blank = (line: string | undefined) => line === undefined || line.trim() === "";
    const marked = lines.map((line, index) => {
      const found = /^\s*\{\{\s*(toc|children|include|tasks)(?:\s+([^{}]*?))?\s*\}\}\s*$/i.exec(line);
      if (!found || !blank(lines[index - 1]) || !blank(lines[index + 1])) {
        return line;
      }
      const name = (found[1] ?? "").toLowerCase();
      const argument = (found[2] ?? "").trim();
      if ((name === "toc" || name === "children") !== (argument === "") || (name === "include" && argument === "")) {
        return line;
      }
      macros.push(this.macroHtml(name, argument, text, page, known, stack));
      return `GKMACRO${String(macros.length - 1)}X`;
    });
    let html = renderFake(marked.join("\n"), known);
    macros.forEach((macro, index) => {
      html = html.replace(`<p>GKMACRO${String(index)}X</p>`, macro);
    });
    return html;
  }

  private macroHtml(name: string, argument: string, text: string, page: FakeWikiPage, known: (key: string) => boolean, stack: string[]): string {
    const note = (kind: string, value = "") => `<p data-macro-note="${kind}"${value === "" ? "" : ` data-macro-arg="${escapeHtml(value)}"`}></p>`;
    const link = (target: string, label: string) => `<a data-wiki="${escapeHtml(target)}">${escapeHtml(label)}</a>`;
    const project = this.state.projects.find((candidate) => candidate.id === page.projectId);
    let inner: string;
    if (name === "toc") {
      const headings = headingsOf(text);
      inner = headings.length === 0 ? note("toc-empty") : `<ul>${headings.map((heading) => `<li>${link(`#${heading.text}`, heading.text)}</li>`).join("")}</ul>`;
    } else if (name === "children") {
      const kids = (this.state.wiki ?? []).filter((other) => other.parentId === page.id);
      inner = kids.length === 0 ? note("children-empty") : `<ul>${kids.map((kid) => `<li>${link(kid.title, kid.title)}</li>`).join("")}</ul>`;
    } else if (name === "include") {
      const found = project === undefined ? undefined : this.linkedPage(project, argument);
      inner =
        stack.length > 3
          ? note("include-too-deep", argument)
          : found === undefined
            ? note("include-missing", argument)
            : stack.includes(found.id)
              ? note("include-loop", argument)
              : this.renderPage(found.body, found, known, [...stack, found.id]).replace(/data-item="\d+"/g, 'disabled=""');
    } else {
      const names = this.queryNames();
      const scoped = /(^|\s)-?project:/i.test(argument) || project === undefined ? null : new Set([project, ...this.descendants(project)].map((each) => each.key));
      const all = this.listedTasks().filter((task) => scoped === null || scoped.has(task.project));
      let keys: string[];
      try {
        keys = matchingKeys(all, argument, names, this.archivedKeys());
      } catch (error) {
        // As the engine does: the query's message by its ID and arguments, for the reader to word.
        const problem = queryError(error)?.details;
        const why = problem === undefined ? { id: "wiki-macro-tasks-unreadable", args: {} } : { args: problem.args ?? {}, id: problem.message_id };
        return `<div data-macro="tasks">${note("tasks-error", JSON.stringify(why))}</div>`;
      }
      const shown = keys.slice(0, 50).map((key) => all.find((task) => task.key === key)).filter((task) => task !== undefined);
      const done = (task: ListedTask) => task.status.category === "done" || task.status.category === "cancelled";
      inner =
        shown.length === 0
          ? note("tasks-empty")
          : `<ul>${shown
              .map((task) => {
                const title = done(task) ? `<del>${escapeHtml(task.title)}</del>` : escapeHtml(task.title);
                return `<li><a data-ref="${task.key}">${task.key}</a> ${title} <span data-macro="status">${escapeHtml(task.status.name)}</span></li>`;
              })
              .join("")}</ul>${keys.length > shown.length ? note("tasks-more", String(keys.length - shown.length)) : ""}`;
    }
    return `<div data-macro="${name}">${inner}</div>`;
  }

  /** The page a link in a text of `project` names: in that project, or `KEY:Page` in another. */
  private linkedPage(project: FakeProject, target: string): FakeWikiPage | undefined {
    const own = this.pageIn(project, target);
    if (own) {
      return own;
    }
    const colon = target.indexOf(":");
    const other = colon < 0 ? undefined : this.state.projects.find((candidate) => candidate.key === target.slice(0, colon).trim().toUpperCase());
    return other ? this.pageIn(other, target.slice(colon + 1)) : undefined;
  }

  /** A task named by a link, or nothing. */
  private linkedTask(key: string): FakeTask | undefined {
    try {
      return this.task(key);
    } catch {
      return undefined;
    }
  }

  /** A project whose wiki pages may change: active, with the wiki switched on. */
  private wikiWritable(ref: string): FakeProject {
    const project = this.writable(ref);
    if (!project.modules.wiki) {
      fail("forbidden", `The wiki is switched off in project ${project.key}, so its pages can't be changed.`, {
        hint: "Switch the wiki on in the project's settings first.",
        details: { message_id: "project-wiki-off", args: { project: project.key }, hint_id: "project-wiki-off-hint", hint_args: {} },
      });
    }
    return project;
  }

  private projectKeyOf(page: FakeWikiPage): string {
    return this.state.projects.find((project) => project.id === page.projectId)?.key ?? "";
  }

  /** Writes a new revision of a page and makes it the text. */
  private addPageRevision(page: FakeWikiPage, body: string, kind: RevisionKind, base: string | null): FakeRevision {
    const revision: FakeRevision = { id: nextId("r"), taskId: page.id, body, kind, base, author: this.byAgent ? { kind: "agent", name: SAMPLE_AGENT } : { kind: "user", name: "You" }, createdAt: this.today().toISOString(), resolvedAt: null };
    this.state.revisions.push(revision);
    page.body = body;
    page.updatedAt = revision.createdAt;
    return revision;
  }

  /** Every live text with its links: pages, task descriptions, comments. */
  private linkingTexts(): { source: Backlink; projectId: string; order: number; taskId?: string; pageId?: string; links: TextLink[] }[] {
    const archived = this.archivedKeys();
    const texts: { source: Backlink; projectId: string; order: number; taskId?: string; pageId?: string; links: TextLink[] }[] = [];
    for (const page of this.state.wiki ?? []) {
      const project = this.projectKeyOf(page);
      if (!archived.has(project)) {
        texts.push({ source: { project, title: page.title, page: { id: page.id, slug: page.slug, title: page.title } }, projectId: page.projectId, order: 0, pageId: page.id, links: linksIn(page.body) });
      }
    }
    for (const task of this.state.tasks) {
      const project = this.projectOf(task);
      if (!archived.has(project.key)) {
        texts.push({ source: { project: project.key, title: task.title, task: this.keyOf(task) }, projectId: project.id, order: 1, taskId: task.id, links: linksIn(task.description) });
      }
    }
    for (const comment of this.state.comments) {
      const task = this.state.tasks.find((candidate) => candidate.id === comment.taskId);
      if (comment.deletedAt === null && task && !archived.has(this.projectOf(task).key)) {
        texts.push({ source: { project: this.projectOf(task).key, title: task.title, task: this.keyOf(task), comment: comment.id }, projectId: task.projectId, order: 2, taskId: task.id, links: linksIn(comment.body) });
      }
    }
    return texts;
  }

  /** The texts that link to a page or a task: pages by title, then descriptions by key, then comments. */
  private backlinksTo(matches: (link: TextLink, projectId: string) => boolean, except: { pageId?: string; taskId?: string }): Backlink[] {
    const found: { order: number; source: Backlink }[] = [];
    for (const text of this.linkingTexts()) {
      if ((except.pageId !== undefined && text.pageId === except.pageId) || (except.taskId !== undefined && text.taskId === except.taskId)) {
        continue;
      }
      const first = text.links.find((link) => matches(link, text.projectId));
      if (first) {
        found.push({ order: text.order, source: { ...text.source, ...some("heading", first.heading) } });
      }
    }
    const byKey = (key: string | undefined) => {
      const [project = "", number = "0"] = (key ?? "").split("-");
      return [project, Number(number)] as const;
    };
    return found
      .sort((a, b) => {
        if (a.order !== b.order) {
          return a.order - b.order;
        }
        if (a.order === 0) {
          return a.source.title.localeCompare(b.source.title);
        }
        if (a.order === 1) {
          const [ap, an] = byKey(a.source.task);
          const [bp, bn] = byKey(b.source.task);
          return ap.localeCompare(bp) || an - bn;
        }
        return 0;
      })
      .map((entry) => entry.source);
  }

  /** What one text's links point at now. */
  private referencesOf(links: readonly TextLink[], project: FakeProject, comment?: string): Reference[] {
    return links.map((link): Reference => {
      if (link.kind === "task") {
        const task = this.linkedTask(link.target);
        return { kind: "task", target: link.target, found: task !== undefined, ...some("comment", comment), ...(task ? { task: this.keyOf(task), title: task.title, project: this.projectOf(task).key } : {}) };
      }
      const page = this.linkedPage(project, link.target);
      return {
        kind: "page",
        target: link.target,
        found: page !== undefined,
        ...some("heading", link.heading),
        ...some("comment", comment),
        ...(page ? { page: { id: page.id, slug: page.slug, title: page.title }, title: page.title, project: this.projectKeyOf(page) } : {}),
      };
    });
  }

  private pageBacklinks(page: FakeWikiPage): Backlink[] {
    return this.backlinksTo((link, projectId) => {
      const project = this.state.projects.find((candidate) => candidate.id === projectId);
      return link.kind === "page" && project !== undefined && this.linkedPage(project, link.target)?.id === page.id;
    }, { pageId: page.id });
  }

  private pageReferences(page: FakeWikiPage): Reference[] {
    const project = this.state.projects.find((candidate) => candidate.id === page.projectId);
    return project ? this.referencesOf(linksIn(page.body), project) : [];
  }

  private wikiOut(page: FakeWikiPage, include: readonly string[] = []): WikiPage {
    const pages = this.state.wiki ?? [];
    const ref = (other: FakeWikiPage) => ({ id: other.id, slug: other.slug, title: other.title });
    const path: FakeWikiPage[] = [];
    for (let parentId = page.parentId; parentId !== null; ) {
      const parent = pages.find((other) => other.id === parentId);
      if (!parent) {
        break;
      }
      path.unshift(parent);
      parentId = parent.parentId;
    }
    const parent = path[path.length - 1];
    return {
      id: page.id,
      title: page.title,
      slug: page.slug,
      project: this.state.projects.find((project) => project.id === page.projectId)?.key ?? "",
      body: page.body,
      current: true,
      conflict_copies: 0,
      headings: headingsOf(page.body),
      ...(parent ? { parent: ref(parent) } : {}),
      path: path.map(ref),
      children: pages.filter((other) => other.parentId === page.id).map(ref),
      ...some("revision", this.currentRevision(page)?.id),
      ...(include.includes("backlinks") ? { backlinks: this.pageBacklinks(page) } : {}),
      ...(include.includes("references") ? { references: this.pageReferences(page) } : {}),
      ...(include.includes("checklist") ? { checklist_items: publicChecklist(page.body) } : {}),
      ...(include.includes("files") ? { files: this.state.files.filter((file) => file.pageId === page.id && file.deletedAt === null).map((file) => this.attachment(file)) } : {}),
      ...(page.agentBrief === true ? { agent_brief: true } : {}),
      ...(page.home === true ? { home: true } : {}),
      ...(page.template === true ? { template: true } : {}),
      created_at: page.createdAt,
      updated_at: page.updatedAt,
    };
  }

  /** Every task as lists show it, worked out once per state of the workspace (lists may hold 20,000 tasks). */
  private searchCache = new WeakMap<FakeState, { seq: string; results: Map<string, { found: ListedTask[]; order: string[] }> }>();
  private listedCache = new WeakMap<FakeState, { seq: string; items: ListedTask[] }>();

  /** The state the caches were made for: the last change, and the claims still on (a claim ends without a change). */
  private cacheStamp(): string {
    const now = this.today().toISOString();
    return `${String(this.state.seq)}:${String(this.state.claims.filter((claim) => claim.until > now).length)}`;
  }

  private listedTasks(): ListedTask[] {
    const stamp = this.cacheStamp();
    const cached = this.listedCache.get(this.state);
    if (cached?.seq === stamp) {
      return cached.items;
    }
    const children = new Map<string, FakeTask[]>();
    for (const task of this.state.tasks) {
      if (task.parentId !== null) {
        children.set(task.parentId, [...(children.get(task.parentId) ?? []), task]);
      }
    }
    const open = new Set(this.state.tasks.filter((task) => task.closedAt === null).map((task) => task.id));
    const blocked = new Set(this.state.relations.filter((relation) => relation.kind === "blocks" && open.has(relation.from)).map((relation) => relation.to));
    const count = (ids: readonly string[]) => {
      const counts = new Map<string, number>();
      for (const id of ids) {
        counts.set(id, (counts.get(id) ?? 0) + 1);
      }
      return counts;
    };
    const liveComments = this.state.comments.filter((comment) => comment.deletedAt === null);
    const commentIds = new Set(liveComments.map((comment) => comment.id));
    const comments = count(liveComments.map((comment) => comment.taskId));
    const files = count(this.state.files.filter((file) => file.deletedAt === null && (file.commentId === null || commentIds.has(file.commentId))).map((file) => file.taskId));
    const progress = (task: FakeTask): number => {
      if (task.closedAt !== null) {
        return 100;
      }
      const { done, total } = this.checklist(task);
      if (total > 0) {
        return Math.round((done / total) * 100);
      }
      const subtasks = children.get(task.id) ?? [];
      return subtasks.length > 0 ? Math.round(subtasks.reduce((sum, sub) => sum + progress(sub), 0) / subtasks.length) : 0;
    };
    const byId = new Map(this.state.tasks.map((task) => [task.id, task]));
    const items = this.state.tasks.map((task): ListedTask => {
      const project = this.projectOf(task);
      const status = this.statusIn(project, task.statusId);
      const parent = task.parentId === null ? undefined : byId.get(task.parentId);
      const subtasks = children.get(task.id) ?? [];
      return {
        blocked: blocked.has(task.id),
        checklist: this.checklist(task),
        ...some("claim", this.claimOf(task)),
        ...some("closed_at", task.closedAt),
        comment_count: comments.get(task.id) ?? 0,
        created_at: task.createdAt,
        created_by: { kind: "user", name: "You" },
        ...some("due_date", task.dueDate),
        ...some("estimate_minutes", task.estimateMin),
        file_count: files.get(task.id) ?? 0,
        id: task.id,
        key: `${project.key}-${String(task.number)}`,
        labels: this.labelRefs(task),
        ...some("fields", listedValues(this.state, task)),
        ...some("parent", parent && this.keyOf(parent)),
        priority: task.priority,
        progress_percent: progress(task),
        project: project.key,
        rank: String(task.rank),
        ...some("remaining_minutes", task.estimateMin === null ? null : task.estimateMin - task.spentMin),
        spent_minutes: task.spentMin,
        ...some("start_date", task.startDate),
        status: { id: status.id, name: status.name, category: status.category },
        subtask_count: { done: subtasks.filter((sub) => sub.closedAt !== null).length, total: subtasks.length },
        title: task.title,
        type: this.typeRef(task),
        updated_at: task.updatedAt,
        version: task.version,
      };
    });
    this.listedCache.set(this.state, { seq: stamp, items });
    return items;
  }

  /** The names queries may use: statuses, types and labels of every project. */
  private queryNames(): QueryNames {
    const unique = (names: string[]) => [...new Set(names)];
    return {
      custom: queryFields(this.state),
      statuses: unique(this.state.projects.flatMap((project) => project.statuses.map((status) => status.name))),
      types: unique(this.state.projects.flatMap((project) => project.types.map((type) => type.name))),
      labels: unique(this.state.projects.flatMap((project) => project.labels.map((label) => label.name))),
      projects: this.state.projects.map((project) => project.key),
      today: localDay(this.today()),
    };
  }

  private archivedKeys(): Set<string> {
    return new Set(this.state.projects.filter((project) => project.state === "archived").map((project) => project.key));
  }

  /** Runs a query; a query that doesn't read fails as the engine's `invalid`. */
  private runQuery<T>(run: () => T): T {
    try {
      return run();
    } catch (error: unknown) {
      const problem = queryError(error);
      if (problem) {
        fail("invalid", problem.message, { details: problem.details });
      }
      throw error;
    }
  }

  /** `task.bulk_update`: every change through the single-task commands, in one Undo step; a preview is rolled back. */
  private bulk(input: BulkUpdateInput): BulkResult | Result<BulkResult> {
    const keys =
      input.keys && input.keys.length > 0
        ? input.keys
        : input.query !== null && input.query !== undefined
          ? this.runQuery(() => matchingKeys(this.listedTasks(), input.query ?? "", this.queryNames(), this.archivedKeys()))
          : fail("invalid", "Give the tasks' keys or a query.");
    const dryRun = input.dry_run !== false;
    if (!dryRun && (input.expected_count === null || input.expected_count === undefined)) {
      fail("invalid", "A bulk change needs expected_count from its preview.", { details: { path: "expected_count" } });
    }
    if (!dryRun && input.expected_count !== keys.length) {
      fail("conflict", `The change is for ${String(keys.length)} tasks now, not ${String(input.expected_count)}.`, { hint: "Preview it again." });
    }
    const saved = structuredClone(this.state);
    const changes: EntityChange[] = [];
    const items: BulkItem[] = [];
    const take = (answer: unknown) => {
      if (isResult(answer)) {
        changes.push(...answer.changes);
      }
    };
    const shown = (task: ListedTask) => ({
      status: task.status.name,
      priority: task.priority,
      labels: task.labels.map((label) => label.name).join(", "),
      due_date: task.due_date ?? "",
      start_date: task.start_date ?? "",
      estimate: task.estimate_minutes === undefined ? "" : formatDuration(task.estimate_minutes),
      type: task.type.name,
      project: task.project,
    });
    const { changes: change } = input;
    try {
      for (const key of keys) {
        const before = shown(this.listed(this.task(key)));
        if ((change.status !== null && change.status !== undefined) || (change.project !== null && change.project !== undefined)) {
          take(this.handlers["task.move"]?.({ key, ...some("status", change.status), ...some("project", change.project) }));
        }
        const update: TaskUpdateInput = {
          key,
          ...some("priority", change.priority),
          ...some("labels", change.labels),
          ...some("due_date", change.due_date),
          ...some("start_date", change.start_date),
          ...some("estimate", change.estimate),
          ...some("type", change.type),
          ...some("parent", change.parent),
          ...(change.add_labels && change.add_labels.length > 0 ? { add_labels: change.add_labels } : {}),
          ...(change.remove_labels && change.remove_labels.length > 0 ? { remove_labels: change.remove_labels } : {}),
          ...(change.clear && change.clear.length > 0 ? { clear: change.clear } : {}),
        };
        if (Object.keys(update).length > 1) {
          take(this.handlers["task.update"]?.(update));
        }
        const task = this.listed(this.task(key));
        const after = shown(task);
        const fields = (Object.keys(after) as (keyof typeof after)[]).filter((field) => after[field] !== before[field]);
        items.push({
          key: task.key,
          title: task.title,
          changes: fields.map((field) => ({ field, ...some("before", before[field] || null), ...some("after", after[field] || null) })),
        });
      }
    } finally {
      if (dryRun) {
        this.state = saved;
      }
    }
    const output: BulkResult = { changed: items.filter((item) => item.changes.length > 0).length, count: keys.length, dry_run: dryRun, items };
    return dryRun ? output : { output, changes };
  }

  /** The change event of a saved view. */
  private viewChange(view: SavedView, fields: string[]): EntityChange[] {
    return [{ entity: "view", id: view.id, fields, ...some("project", view.project) }];
  }

  // ---------- the operations ----------

  private readonly handlers: { [K in OpId]?: Handler<K> } = {
    "workspace.get": () => {
      const info = {
        caller: { actor: { kind: "user" as const, name: "You" }, via: "app" as const },
        guide: "",
        id: "w-1",
        name: this.state.workspaceName,
        program_version: __VERSION__,
        schema_version: 2,
        settings: this.settingsNow(),
        // The limits on agent sessions enforced now; a lifted limit is left out.
        agent_limits: this.settingsNow().agent_limits,
      };
      return info;
    },
    "settings.get": () => this.settingsNow(),
    "settings.update": (patch: SettingsPatch) => {
      const changed: string[] = [];
      for (const [name, value] of Object.entries(patch)) {
        if (name !== "clear" && value !== null && (this.state.settings as Record<string, unknown>)[name] !== value) {
          (this.state.settings as Record<string, unknown>)[name] = value;
          changed.push(name);
        }
      }
      const settings: Settings = this.settingsNow();
      return { output: { changed, settings }, changes: changed.length ? [{ entity: "settings", id: "w-1", fields: changed }] : [] };
    },
    "history.feed": ({ limit, cursor, task: ref, actor_kind, project: projectRef, from: fromText, to: toText }) => {
      const task = ref === null || ref === undefined ? null : this.task(ref);
      // A project's changes include its sub-projects'.
      const projects = new Set<string>();
      if (projectRef !== null && projectRef !== undefined) {
        const add = (id: string) => {
          projects.add(id);
          this.state.projects.filter((each) => each.parent === id).forEach((each) => { add(each.id); });
        };
        add(this.project(projectRef).id);
      }
      // Whole days in the caller's zone, the last one included.
      const from = fromText === null || fromText === undefined ? null : this.dayInput(fromText, "from");
      const to = toText === null || toText === undefined ? null : this.dayInput(toText, "to");
      if (from !== null && to !== null && to < from) {
        fail("invalid", "The last day comes before the first.", { details: { path: "to", message_id: "activity-to-before-from" } });
      }
      const all = [...this.state.activity].reverse().filter((entry) => {
        const day = localDay(new Date(entry.at));
        return (
          (task === null || this.concerns(entry, task.id)) &&
          (actor_kind === null || actor_kind === undefined || entry.actor.kind === actor_kind) &&
          (projects.size === 0 || (entry.project !== undefined && projects.has(entry.project))) &&
          (from === null || day >= from) &&
          (to === null || day <= to)
        );
      });
      const start = cursor ? Number(cursor) : 0;
      const end = start + (limit ?? 50);
      const items = all.slice(start, end).map((entry) => ({ ...entry, ...some("task", this.taskOfEntry(entry)) }));
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
    "project.create": ({ name, key, parent, hue, icon, description }) => {
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
      this.validIcon(icon);
      const project = newProject({
        key,
        name: trimmed,
        // Without a colour: one its siblings don't use yet, as the engine does.
        hue: hue ?? suggestHue(siblings.map((sibling) => sibling.hue)),
        parent: parentProject?.id ?? null,
        rank: Math.max(0, ...siblings.map((sibling) => sibling.rank)) + 1,
      });
      project.description = description ?? "";
      project.icon = icon ?? undefined;
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
    "project.update": ({ project: ref, name, description, hue, icon, modules, archived, clear }) => {
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
      if (icon !== null && icon !== undefined) {
        this.validIcon(icon);
        project.icon = icon;
        fields.push("icon");
      }
      if (clear?.includes("icon")) {
        project.icon = undefined;
        fields.push("icon");
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
      project.labels.push({ id: nextId("l"), name: this.uniqueName(project.labels, name, "label"), hue: hue ?? suggestHue(project.labels.map((label) => label.hue)) });
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
    "field.list": ({ project: ref }) => {
      const project = ref === null || ref === undefined ? null : this.project(ref);
      return { fields: fieldsFor(this.state, project?.id ?? null).map((field) => fieldOut(this.state, field, this.state.projects)) };
    },
    "field.create": (input) => {
      const project = input.project === null || input.project === undefined ? null : this.writable(input.project);
      const field = createField(this.state, { ...input, project }, nextId("cf"), fail);
      placeField(this.state, field, input, fail);
      return this.fieldChange(field, ["*"], 0);
    },
    "field.update": (input) => {
      const project = input.project === null || input.project === undefined ? null : this.project(input.project);
      const field = findField(fieldsFor(this.state, project?.id ?? null), input.field, fail);
      const removed = updateField(this.state, field, input, fail);
      placeField(this.state, field, input, fail);
      return this.fieldChange(field, ["*"], removed);
    },
    "field.delete": (input) => {
      const project = input.project === null || input.project === undefined ? null : this.project(input.project);
      const field = findField(fieldsFor(this.state, project?.id ?? null), input.field, fail);
      const count = this.state.tasks.filter((task) => task.fields?.[field.id] !== undefined).length;
      if (input.expected_task_count !== null && input.expected_task_count !== undefined && input.expected_task_count !== count) {
        fail("conflict", `"${field.name}" has a value on ${String(count)} tasks, not ${String(input.expected_task_count)}.`, { details: { message_id: "field-count-differs" } });
      }
      const removed = deleteField(this.state, field);
      return this.fieldChange(field, ["deleted"], removed);
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
        id: nextUuid(),
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
      this.draftSaved("new_comment", task.id);
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
      this.draftSaved("comment", comment.id);
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
    "task.delete": ({ keys }) => {
      if (keys.length === 0 || keys.length > 500) {
        fail("invalid", "Name 1 to 500 tasks.");
      }
      const at = this.today().toISOString();
      const purgeAt = new Date(this.today().getTime() + this.state.settings.trash_days * 86_400_000).toISOString();
      const items: TrashEntry[] = [];
      const changes: EntityChange[] = [];
      for (const key of keys) {
        const task = this.task(key);
        this.writable(this.projectOf(task).key);
        const gone = [task, ...this.subtasksDeep(task.id)];
        const project = this.projectOf(task);
        const entry: TrashEntry = {
          contains: gone.length - 1,
          deleted_at: at,
          deleted_by: { kind: "user", name: "You" },
          group: "",
          id: task.id,
          key: this.keyOf(task),
          kind: "task",
          project: project.key,
          purge_at: purgeAt,
          title: task.title,
        };
        (this.state.trash ??= []).push({ entry, tasks: gone });
        this.state.tasks = this.state.tasks.filter((candidate) => !gone.includes(candidate));
        items.push(entry);
        changes.push(...gone.map((each) => ({ entity: "task", id: each.id, fields: ["trashed"], project: project.key })));
      }
      return { output: { items, total: changes.length }, changes };
    },
    "trash.list": ({ kinds, project, limit, cursor }) => {
      const all = [...(this.state.trash ?? [])]
        .reverse()
        .map((deletion) => deletion.entry)
        .filter((entry) => (kinds === undefined || kinds.length === 0 || kinds.includes(entry.kind)) && (project === null || project === undefined || entry.project === project.toUpperCase()));
      const start = cursor ? Number(cursor) : 0;
      const end = start + (limit ?? 50);
      return { items: all.slice(start, end), total: all.length, trash_days: this.state.settings.trash_days, ...(end < all.length ? { next_cursor: String(end) } : {}) };
    },
    "trash.restore": ({ items: ids }) => {
      const items: TrashEntry[] = [];
      const changes: EntityChange[] = [];
      for (const id of ids) {
        const deletion = this.state.trash?.find((candidate) => candidate.entry.id === id);
        if (!deletion) {
          fail("not_found", `There is nothing in the trash with the ID ${id}.`);
        }
        if (deletion.pages) {
          const root = deletion.pages[0];
          const holder = root?.parentId ?? null;
          if (holder !== null && !(this.state.wiki ?? []).some((page) => page.id === holder)) {
            fail("conflict", "This page can't come back on its own: the page that held it is in the trash too.", { details: { message_id: "trash-restore-page-first", args: {} } });
          }
          const clash = deletion.pages.find((page) => (this.state.wiki ?? []).some((other) => other.projectId === page.projectId && fold(other.title) === fold(page.title)));
          if (clash) {
            fail("conflict", `Another page is called "${clash.title}" now.`, { hint: "Rename that page first." });
          }
          (this.state.wiki ??= []).push(...deletion.pages);
          this.state.trash = (this.state.trash ?? []).filter((candidate) => candidate !== deletion);
          items.push(deletion.entry);
          changes.push(...deletion.pages.map((page) => ({ entity: "wiki_page", id: page.id, fields: ["trashed"], project: deletion.entry.project ?? "" })));
          continue;
        }
        const parent = deletion.tasks[0]?.parentId;
        if (parent !== null && parent !== undefined && !this.state.tasks.some((task) => task.id === parent)) {
          fail("conflict", "This can't come back on its own: its task is in the trash too.", { hint: "Restore what holds it first; that brings it back too." });
        }
        this.state.tasks.push(...deletion.tasks);
        this.state.trash = (this.state.trash ?? []).filter((candidate) => candidate !== deletion);
        items.push(deletion.entry);
        changes.push(...deletion.tasks.map((task) => ({ entity: "task", id: task.id, fields: ["trashed"], project: deletion.entry.project ?? "" })));
      }
      return { output: { items, total: changes.length }, changes };
    },
    "trash.purge": ({ items: ids, all }) => {
      const chosen = (this.state.trash ?? []).filter((deletion) => all === true || (ids ?? []).includes(deletion.entry.id));
      this.state.trash = (this.state.trash ?? []).filter((deletion) => !chosen.includes(deletion));
      const changes = chosen.flatMap((deletion) => [
        ...deletion.tasks.map((task) => ({ entity: "task", id: task.id, fields: ["purged"], project: deletion.entry.project ?? "" })),
        ...(deletion.pages ?? []).map((page) => ({ entity: "wiki_page", id: page.id, fields: ["purged"], project: deletion.entry.project ?? "" })),
      ]);
      return { output: { items: chosen.map((deletion) => deletion.entry), total: changes.length }, changes };
    },
    "agent.limits.update": (patch) => {
      // Like the engine: only the app reaches it; numbers from 1 to 1000000,
      // lifted limits left out, and a reset brings back the default.
      const before = this.settingsNow().agent_limits;
      const values = new Map<AgentLimit, number | undefined>(AGENT_LIMITS.map((name) => [name, before[name]]));
      for (const name of AGENT_LIMITS) {
        const value = patch[name];
        if (value === null || value === undefined) {
          continue;
        }
        if (!Number.isInteger(value) || value < 1 || value > 1_000_000) {
          fail("invalid", `${name} must be from 1 to 1000000; got ${String(value)}.`, { details: { path: name } });
        }
        values.set(name, value);
      }
      for (const name of patch.no_limit ?? []) {
        values.set(name, undefined);
      }
      for (const name of patch.reset ?? []) {
        values.set(name, DEFAULT_AGENT_LIMITS[name]);
      }
      const limits: AgentLimits = {};
      for (const [name, value] of values) {
        if (value !== undefined) {
          limits[name] = value;
        }
      }
      const same = AGENT_LIMITS.every((name) => limits[name] === before[name]);
      if (!same) {
        this.state.settings.agent_limits = limits;
      }
      const changed = same ? [] : ["agent_limits"];
      return { output: { changed, settings: this.settingsNow() }, changes: changed.length ? [{ entity: "settings", id: "w-1", fields: changed }] : [] };
    },
    "backup.folder.update": ({ folder }) => {
      const settings = this.state.settings;
      const chosen = folder === null || folder === undefined || folder === "" ? undefined : folder;
      if (chosen !== undefined && !chosen.startsWith("/")) {
        fail("invalid", "The backup folder must be an absolute path.", { details: { path: "folder" } });
      }
      const changed = settings.backup_folder === chosen ? [] : ["backup_folder"];
      if (chosen === undefined) {
        delete settings.backup_folder;
      } else {
        settings.backup_folder = chosen;
      }
      return { output: { changed, settings: { ...settings } }, changes: changed.length ? [{ entity: "settings", id: "w-1", fields: changed }] : [] };
    },
    "backup.list": () => {
      const items = this.state.backups ?? [];
      return { folder: "/sample/backups", items, ...some("last_backup_at", items[0]?.created_at) };
    },
    "backup.create": () => {
      const at = this.today();
      const backup: Backup = {
        app_version: __VERSION__,
        created_at: at.toISOString(),
        files: this.state.files.length,
        files_size: this.state.files.reduce((sum, file) => sum + file.size, 0),
        kind: "manual",
        name: `manual-${at.toISOString().replace(/[:.]/g, "-")}-${String((this.state.backups ?? []).length + 1)}`,
        schema_version: 1,
        size: 4096 * (1 + this.state.tasks.length),
      };
      this.state.backups = [backup, ...(this.state.backups ?? [])];
      return backup;
    },
    "backup.verify": ({ name }) => ({ backup: this.backup(name), files_checked: this.backup(name).files, ok: true, problems: [] }),
    "backup.delete": ({ name }) => {
      this.backup(name);
      this.state.backups = (this.state.backups ?? []).filter((backup) => backup.name !== name);
      return { name, files_removed: 0 };
    },
    "backup.restore": ({ name }) => {
      const restored = this.backup(name);
      const previous = this.handlers["backup.create"]?.({}) as Backup;
      return { restored, previous };
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
    "file.attach": ({ task: key, comment: commentId, page: pageRef, project: projectRef, data_base64, name, path }) => {
      if (path !== null && path !== undefined) {
        fail("invalid", "The sample workspace can't read files from disk.", { details: { path: "path" } });
      }
      const page = pageRef === null || pageRef === undefined ? null : this.wikiPage(pageRef, projectRef);
      const comment = commentId === null || commentId === undefined ? null : this.liveComment(commentId);
      const task = page ? undefined : comment ? this.state.tasks.find((candidate) => candidate.id === comment.taskId) : key ? this.task(key) : undefined;
      if (!task && !page) {
        fail("invalid", "Name the task, the comment or the wiki page to attach the file to.", { details: { path: "task" } });
      }
      if (page) {
        this.wikiWritable(this.projectKeyOf(page));
      } else if (task) {
        this.writable(this.projectOf(task).key);
      }
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
        id: nextFileId(),
        taskId: task?.id ?? "",
        ...(page ? { pageId: page.id } : {}),
        commentId: comment?.id ?? null,
        name: name.trim(),
        size: bytes.length,
        base64: data_base64 ?? "",
        createdAt: this.today().toISOString(),
        deletedAt: null,
        ...sniffed,
      };
      this.state.files.push(file);
      if (task) {
        this.touch(task, false);
      }
      return this.fileChange(file, ["created"]);
    },
    "file.delete": ({ attachment }) => {
      const file = this.liveFile(attachment);
      file.deletedAt = this.today().toISOString();
      return this.fileChange(file, ["trashed"]);
    },
    "today.get": ({ project: ref, limit }) => {
      const day = localDay(this.today());
      const scope = ref === null || ref === undefined ? null : new Set([this.project(ref), ...this.descendants(this.project(ref))].map((project) => project.key));
      const archived = this.archivedKeys();
      const visible = (key: string) => (scope === null ? !archived.has(key) : scope.has(key));
      const all = this.listedTasks().filter((task) => visible(task.project));
      const open = all.filter((task) => task.closed_at === undefined);
      const priority: Record<string, number> = { urgent: 0, high: 1, normal: 2, low: 3, none: 4 };
      const overdue = open.filter((task) => task.due_date !== undefined && task.due_date < day).sort((a, b) => (a.due_date ?? "").localeCompare(b.due_date ?? ""));
      const dueToday = open.filter((task) => task.due_date === day).sort((a, b) => (priority[a.priority] ?? 4) - (priority[b.priority] ?? 4));
      const listed = new Set([...overdue, ...dueToday].map((task) => task.key));
      const inProgress = open.filter((task) => task.status.category === "in_progress" && !listed.has(task.key));
      const reminders = (this.handlers["reminder.due"]?.({ until: `${nextDay(day)}T00:00:00Z`, limit: 200 }) as DueReminders | undefined)?.items.filter((item) => item.kind === "reminder" && visible(item.task.key.slice(0, item.task.key.lastIndexOf("-")))) ?? [];
      const claimed = all.filter((task) => task.claim !== undefined);
      const since = this.today().getTime() - 86_400_000;
      const recent = all
        .filter((task) => Date.parse(task.updated_at) >= since)
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
        .map((task) => ({ task, changed_at: task.updated_at, changed_by: this.state.tasks.find((candidate) => candidate.id === task.id)?.updatedBy ?? { kind: "user" as const, name: "You" } }));
      const cap = Math.min(Math.max(limit ?? 50, 1), 200);
      const timer = this.timerStatus().timer;
      return {
        date: day,
        overdue: overdue.slice(0, cap),
        due_today: dueToday.slice(0, cap),
        in_progress: inProgress.slice(0, cap),
        reminders: reminders.slice(0, cap),
        claimed: claimed.slice(0, cap),
        recent: recent.slice(0, cap),
        ...some("timer", timer),
        counts: {
          overdue: overdue.length,
          due_today: dueToday.length,
          in_progress: inProgress.length,
          reminders: reminders.length,
          claimed: claimed.length,
          recent: recent.length,
          today: overdue.length + dueToday.length + inProgress.length,
          open: open.length,
        },
      };
    },
    "schedule.get": (input) => {
      if ((input.project === null || input.project === undefined) && (input.query === null || input.query === undefined)) {
        fail("invalid", "Give a `project`, a `query`, or both.", { details: { message_id: "schedule-scope-needed" } });
      }
      const scope = input.project === null || input.project === undefined ? null : this.project(input.project);
      const keys = scope === null ? null : new Set([scope, ...(input.include_subprojects === true ? this.descendants(scope) : [])].map((project) => project.key));
      const archived = this.archivedKeys();
      const filter = input.query === null || input.query === undefined || input.query.trim() === "" ? null : new Set(this.runQuery(() => matchingKeys(this.listedTasks(), input.query ?? "", this.queryNames(), archived)));
      const tasks = this.listedTasks().filter((task) => (keys === null || keys.has(task.project)) && (filter === null || filter.has(task.key)));
      const milestoneTypes = new Set(this.state.projects.flatMap((project) => project.types.filter((type) => type.glyph === "milestone").map((type) => type.id)));
      const links = this.state.relations.filter((relation) => relation.kind === "blocks").map((relation) => ({ id: relation.id, from: relation.from, to: relation.to }));
      return scheduleOf(tasks, links, (task) => milestoneTypes.has(task.type.id), localDay(this.today()), archived);
    },
    "search.query": (input) => {
      const scope = input.project === null || input.project === undefined ? null : this.project(input.project);
      const keys = scope === null ? null : new Set([scope, ...(input.include_subprojects === true ? this.descendants(scope) : [])].map((project) => project.key));
      const archived = this.archivedKeys();
      const filter = input.query === null || input.query === undefined || input.query.trim() === "" ? null : new Set(this.runQuery(() => matchingKeys(this.listedTasks(), input.query ?? "", this.queryNames(), archived)));
      const listed = new Map(this.listedTasks().filter((task) => filter === null || filter.has(task.key)).map((task) => [task.id, task]));
      const result = searchAll(
        {
          tasks: this.state.tasks.flatMap((task) => {
            const item = listed.get(task.id);
            return item ? [{ task: item, description: task.description, projectActive: !archived.has(item.project) || keys !== null }] : [];
          }),
          comments: this.state.comments.flatMap((comment) => {
            const task = listed.get(comment.taskId);
            return task && comment.deletedAt === null && !archived.has(task.project) ? [{ id: comment.id, task, body: comment.body, updatedAt: comment.editedAt ?? comment.createdAt }] : [];
          }),
          files: this.state.files.flatMap((file) => {
            const task = listed.get(file.taskId);
            return task && file.deletedAt === null && !archived.has(task.project) ? [{ id: file.id, task, name: file.name, createdAt: file.createdAt }] : [];
          }),
          pages: (this.state.wiki ?? []).map((page) => ({ page, project: this.state.projects.find((project) => project.id === page.projectId)?.key ?? "" })).filter((entry) => !archived.has(entry.project)),
          projects: this.state.projects.map((project) => ({ id: project.id, key: project.key, name: project.name, active: project.state === "active" })),
          inScope: (key) => keys === null || keys.has(key),
        },
        input,
      );
      return result === "empty" ? fail("invalid", "Type something to search for.", { details: { path: "text", message_id: "search-empty", args: {} } }) : result;
    },
    "wiki.list": ({ project: ref }) => {
      const project = this.project(ref);
      const pages = (this.state.wiki ?? []).filter((page) => page.projectId === project.id);
      const ordered: FakeWikiPage[] = [];
      const visit = (parent: string | null) => {
        for (const page of pages.filter((candidate) => candidate.parentId === parent)) {
          ordered.push(page);
          visit(page.id);
        }
      };
      visit(null);
      return { items: ordered.map((page) => this.wikiSummary(page)), total: ordered.length };
    },
    "wiki.search": (input) => {
      const scope = input.project === null || input.project === undefined ? null : this.project(input.project);
      const archived = this.archivedKeys();
      const pages = (this.state.wiki ?? [])
        .map((page) => ({ page, summary: this.wikiSummary(page) }))
        .filter(({ page, summary }) => (scope === null ? !archived.has(summary.project) : page.projectId === scope.id));
      const hits = searchWiki(pages, input);
      return { items: hits.slice(0, Math.min(Math.max(input.limit ?? 20, 1), 200)), total: hits.length };
    },
    "wiki.get": ({ page: ref, project, revision: revisionId, include }) => {
      const page = this.wikiPage(ref, project);
      const out = this.wikiOut(page, include ?? []);
      if (revisionId === null || revisionId === undefined) {
        return out;
      }
      const revision = this.revision(revisionId);
      if (revision.taskId !== page.id) {
        fail("invalid", `Revision ${revisionId} is not one of this page's.`, { details: { path: "revision" } });
      }
      return { ...out, body: revision.body, headings: headingsOf(revision.body), revision: revision.id, current: revision === this.currentRevision(page) };
    },
    "wiki.history": ({ page: ref, project }) => {
      const page = this.wikiPage(ref, project);
      const current = this.currentRevision(page);
      const items = this.revisionsOf(page)
        .reverse()
        .map((revision) => this.revisionInfo(revision, current));
      return { items, total: items.length };
    },
    "wiki.backlinks": ({ page: ref, project }) => {
      const items = this.pageBacklinks(this.wikiPage(ref, project));
      return { items, total: items.length };
    },
    "wiki.references": ({ page: ref, project }) => {
      const items = this.pageReferences(this.wikiPage(ref, project));
      return { items, total: items.length };
    },
    "task.backlinks": ({ key }) => {
      const task = this.task(key);
      const items = this.backlinksTo((link) => link.kind === "task" && this.linkedTask(link.target)?.id === task.id, { taskId: task.id });
      return { items, total: items.length };
    },
    "task.references": ({ key }) => {
      const task = this.task(key);
      const project = this.projectOf(task);
      const items = [
        ...this.referencesOf(linksIn(task.description), project),
        ...this.state.comments.filter((comment) => comment.taskId === task.id && comment.deletedAt === null).flatMap((comment) => this.referencesOf(linksIn(comment.body), project, comment.id)),
      ];
      return { items, total: items.length };
    },
    "wiki.update": ({ page: ref, project: projectRef, title, body, edits, parent: parentRef, clear, before, after, at, base_revision, agent_brief, home, template, message }) => {
      const page = this.wikiPage(ref, projectRef);
      const project = this.wikiWritable(this.projectKeyOf(page));
      const pages = (this.state.wiki ??= []);
      if (body !== null && body !== undefined && edits !== undefined && edits.length > 0) {
        fail("invalid", "Give a new text or edits, not both.", { details: { path: "edits", message_id: "wiki-body-and-edits", args: {} } });
      }
      if (parentRef !== null && parentRef !== undefined && (clear ?? []).includes("parent")) {
        fail("invalid", "Give a parent or clear it, not both.", { details: { path: "clear", message_id: "wiki-parent-and-clear", args: {} } });
      }
      const fields: string[] = [];
      if (title !== null && title !== undefined && title.trim() !== page.title) {
        const trimmed = title.trim();
        if (trimmed === "") {
          fail("invalid", "A page needs a title.", { details: { path: "title", message_id: "wiki-title-empty", args: {} } });
        }
        const clash = pages.find((other) => other !== page && other.projectId === project.id && fold(other.title) === fold(trimmed));
        if (clash) {
          fail("conflict", `Project ${project.key} already has a page called "${clash.title}".`, { details: { field: "title", message_id: "wiki-title-taken", args: { project: project.key, title: clash.title } } });
        }
        const slug = slugOf(trimmed, pages.filter((other) => other !== page && other.projectId === project.id).map((other) => other.slug));
        if (slug !== page.slug) {
          page.oldSlugs = [...(page.oldSlugs ?? []).filter((old) => old !== slug), page.slug];
          for (const other of pages) {
            if (other !== page && other.projectId === project.id) {
              other.oldSlugs = (other.oldSlugs ?? []).filter((old) => old !== slug);
            }
          }
          page.slug = slug;
        }
        page.title = trimmed;
        fields.push("title");
      }
      if (agent_brief !== null && agent_brief !== undefined && agent_brief !== (page.agentBrief === true)) {
        for (const other of pages) {
          if (other.projectId === project.id) {
            other.agentBrief = other === page ? agent_brief : agent_brief ? false : other.agentBrief === true;
          }
        }
        fields.push("agent_brief");
      }
      if (home !== null && home !== undefined && home !== (page.home === true)) {
        for (const other of pages) {
          if (other.projectId === project.id) {
            other.home = other === page ? home : home ? false : other.home === true;
          }
        }
        fields.push("home");
      }
      if (template !== null && template !== undefined && template !== (page.template === true)) {
        page.template = template;
        fields.push("template");
      }
      let text = body ?? undefined;
      if (edits !== undefined && edits.length > 0) {
        text = page.body;
        for (const edit of edits) {
          if (text.split(edit.old_text).length !== 2) {
            fail("invalid", `"${edit.old_text}" must occur exactly once.`, { details: { path: "edits" } });
          }
          text = text.replace(edit.old_text, edit.new_text);
        }
      }
      let saved: FakeRevision | undefined;
      if (text !== undefined && text !== page.body) {
        saved = this.addPageRevision(page, text, "normal", base_revision ?? this.currentRevision(page)?.id ?? null);
        const note = message?.trim() ?? "";
        if (note !== "") {
          saved.message = note;
        }
        fields.push("body");
      }
      const given = (value: unknown) => value !== null && value !== undefined;
      const moves = given(parentRef) || (clear ?? []).includes("parent") || given(before) || given(after) || given(at);
      if (moves) {
        const parent = parentRef === null || parentRef === undefined ? ((clear ?? []).includes("parent") ? null : page.parentId) : this.wikiPage(parentRef, project.key).id;
        this.movePage(page, parent, { before, after, at });
        fields.push("parent");
      }
      if (fields.length === 0) {
        return { page: this.wikiOut(page), save: { outcome: "unchanged" as const } };
      }
      page.updatedAt = this.today().toISOString();
      return {
        output: { page: this.wikiOut(page), ...(saved ? { save: { outcome: "saved" as const, revision: saved.id } } : {}) },
        changes: [{ entity: "wiki_page", id: page.id, fields, project: project.key }],
      };
    },
    "wiki.move": ({ page: ref, project: projectRef, parent: parentRef, top_level, before, after, at }) => {
      const page = this.wikiPage(ref, projectRef);
      const project = this.wikiWritable(this.projectKeyOf(page));
      const hasParent = parentRef !== null && parentRef !== undefined;
      if (hasParent && top_level === true) {
        fail("invalid", "Give a parent or move it to the top, not both.", { details: { path: "top_level", message_id: "wiki-parent-and-top", args: {} } });
      }
      const placed = (before !== null && before !== undefined) || (after !== null && after !== undefined) || (at !== null && at !== undefined);
      if (!hasParent && top_level !== true && !placed) {
        fail("invalid", "Say where to move the page.", { details: { path: "parent", message_id: "wiki-move-nowhere", args: {} } });
      }
      const parent = hasParent ? this.wikiPage(parentRef, project.key).id : top_level === true ? null : page.parentId;
      this.movePage(page, parent, { before, after, at: at ?? (parent !== page.parentId && !placed ? "bottom" : undefined) });
      return { output: { page: this.wikiOut(page) }, changes: [{ entity: "wiki_page", id: page.id, fields: ["parent", "rank"], project: project.key }] };
    },
    "wiki.export": ({ page: ref, project: projectRef, subpages, path, format }) => {
      // The sample workspace writes nothing: it says what an export would hold.
      if (ref === null || ref === undefined) {
        const project = this.project(projectRef ?? "");
        const pages = (this.state.wiki ?? []).filter((page) => page.projectId === project.id);
        return { path: `exports/${path ?? `${project.key.toLowerCase()}-wiki.zip`}`, format: format ?? "zip", pages: pages.length, attachments: 0, size: pages.reduce((sum, one) => sum + one.body.length, 0) };
      }
      const page = this.wikiPage(ref, projectRef);
      const pages = subpages === true ? subtree(this.state.wiki ?? [], page) : [page];
      const name = path ?? `${page.slug}.zip`;
      return { path: `exports/${name}`, format: format ?? "zip", pages: pages.length, attachments: 0, size: pages.reduce((sum, one) => sum + one.body.length, 0) };
    },
    "wiki.import": ({ project: ref, path, parent: parentRef, dry_run }) => {
      // The sample workspace reads no folders: it knows one, "notes", with two
      // Markdown files and a picture.
      const project = this.wikiWritable(ref);
      if (path.replace(/[/\\]+$/, "").split(/[/\\]/).at(-1) !== "notes") {
        fail("invalid", "The sample workspace can import only its own folder \"notes\".", { details: { path: "path" } });
      }
      const pages = (this.state.wiki ??= []);
      const parent = parentRef === null || parentRef === undefined ? null : this.wikiPage(parentRef, project.key);
      const free = (title: string) => {
        let name = title;
        for (let n = 2; pages.some((page) => page.projectId === project.id && fold(page.title) === fold(name)); n += 1) {
          name = `${title} (${String(n)})`;
        }
        return name;
      };
      const first = free("Meeting notes");
      const second = free("Ideas");
      const renamed = [
        ...(first === "Meeting notes" ? [] : [{ path: "Meeting notes.md", title: first }]),
        ...(second === "Ideas" ? [] : [{ path: "Meeting notes/Ideas.md", title: second }]),
      ];
      const summary = { dry_run: dry_run === true, pages: 2, attachments: 1, skipped: ["notes.txt"], ...(renamed.length > 0 ? { renamed } : {}) };
      if (dry_run === true) {
        return { ...summary, top: [{ id: "", slug: slugOf(first, []), title: first }] };
      }
      const now = this.today().toISOString();
      const make = (title: string, under: string | null, body: string): FakeWikiPage => {
        const page: FakeWikiPage = {
          id: nextId("w"),
          projectId: project.id,
          title,
          slug: slugOf(title, pages.filter((other) => other.projectId === project.id).map((other) => other.slug)),
          parentId: under,
          body: "",
          createdAt: now,
          updatedAt: now,
        };
        this.movePage(page, under, { at: "bottom" });
        this.addPageRevision(page, body, "normal", null);
        return page;
      };
      const top = make(first, parent?.id ?? null, `# Meeting notes\n\nSee [[${second}]].\n`);
      const child = make(second, top.id, "Ideas for later.\n");
      return {
        output: { ...summary, top: [{ id: top.id, slug: top.slug, title: top.title }] },
        changes: [top, child].map((page) => ({ entity: "wiki_page", id: page.id, fields: ["created"], project: project.key })),
      };
    },
    "wiki.delete": ({ page: ref, project: projectRef }) => {
      const page = this.wikiPage(ref, projectRef);
      const project = this.wikiWritable(this.projectKeyOf(page));
      const pages = (this.state.wiki ??= []);
      const gone = subtree(pages, page);
      const at = this.today().toISOString();
      const entry: TrashEntry = {
        contains: gone.length - 1,
        deleted_at: at,
        deleted_by: { kind: "user", name: "You" },
        group: "",
        id: page.id,
        kind: "wiki_page",
        project: project.key,
        purge_at: new Date(this.today().getTime() + this.state.settings.trash_days * 86_400_000).toISOString(),
        title: page.title,
      };
      (this.state.trash ??= []).push({ entry, tasks: [], pages: gone });
      this.state.wiki = pages.filter((candidate) => !gone.includes(candidate));
      return { output: { items: [entry], total: gone.length }, changes: gone.map((each) => ({ entity: "wiki_page", id: each.id, fields: ["trashed"], project: project.key })) };
    },
    "wiki.create": ({ project: ref, title, body: given, template, parent: parentRef, before, after, at }) => {
      const project = this.wikiWritable(ref);
      if (template !== null && template !== undefined && given !== null && given !== undefined) {
        fail("invalid", "Give either a text or a template to start from, not both.", { details: { path: "template", message_id: "wiki-template-and-body", args: {} } });
      }
      const source = template === null || template === undefined ? undefined : ((this.state.wiki ?? []).find((candidate) => candidate.id === template && candidate.projectId === project.id) ?? this.linkedPage(project, template));
      if (template !== null && template !== undefined && source === undefined) {
        fail("not_found", `There is no page “${template}” to start from.`, { details: { field: "template", message_id: "wiki-template-not-found", args: { page: template } } });
      }
      const body = source?.body ?? given;
      const pages = (this.state.wiki ??= []);
      const trimmed = title.trim();
      if (trimmed === "") {
        fail("invalid", "A page needs a title.", { details: { path: "title", message_id: "wiki-title-empty", args: {} } });
      }
      if (pages.some((page) => page.projectId === project.id && fold(page.title) === fold(trimmed))) {
        fail("conflict", `Project ${project.key} already has a page called "${trimmed}".`, { details: { message_id: "wiki-title-taken", args: { project: project.key, title: trimmed } } });
      }
      const parent = parentRef === null || parentRef === undefined ? null : this.wikiPage(parentRef, project.key);
      const now = this.today().toISOString();
      const page: FakeWikiPage = {
        id: nextId("w"),
        projectId: project.id,
        title: trimmed,
        slug: slugOf(trimmed, pages.filter((other) => other.projectId === project.id).map((other) => other.slug)),
        parentId: parent?.id ?? null,
        body: "",
        createdAt: now,
        updatedAt: now,
      };
      this.movePage(page, parent?.id ?? null, { before, after, at });
      if ((body ?? "") !== "") {
        this.addPageRevision(page, body ?? "", "normal", null);
        page.updatedAt = now;
      }
      return { output: { page: this.wikiOut(page) }, changes: [{ entity: "wiki_page", id: page.id, fields: ["created"], project: project.key }] };
    },
    "query.parse": ({ query, fields }) => parseQuery(query, this.queryNames(), fields === true),
    "task.search": (input) => {
      const scope = input.project === null || input.project === undefined ? null : this.project(input.project);
      const projects = scope === null ? null : new Set([scope, ...(input.include_subprojects === true ? this.descendants(scope) : [])].map((project) => project.key));
      const tasks = this.listedTasks().filter((task) => projects === null || projects.has(task.project));
      const order = (task: ListedTask) => {
        const project = this.state.projects.find((candidate) => candidate.key === task.project);
        return project ? project.statuses.findIndex((status) => status.id === task.status.id) : 0;
      };
      const stamp = this.cacheStamp();
      let cache = this.searchCache.get(this.state);
      if (cache?.seq !== stamp) {
        cache = { seq: stamp, results: new Map() };
        this.searchCache.set(this.state, cache);
      }
      const results = cache.results;
      const groupInfo: GroupInfo = {
        statusHue: (task) => this.state.projects.find((candidate) => candidate.key === task.project)?.statuses.find((status) => status.id === task.status.id)?.hue,
        titleOf: (key) => tasks.find((task) => task.key === key)?.title ?? this.listedTasks().find((task) => task.key === key)?.title,
        projectName: (key) => this.state.projects.find((candidate) => candidate.key === key)?.name,
        weekStart: (day) => weekStart(day, this.state.settings.week_start),
      };
      return this.runQuery(() => searchTasks(tasks, input, this.queryNames(), { archived: this.archivedKeys(), scoped: scope !== null, statusOrder: order, cache: results, groupInfo }));
    },
    "draft.get": ({ kind, target }) => {
      const owner = this.draftOwner(kind, target);
      return { ...some("draft", this.keptDraft(kind, owner.id)) };
    },
    "draft.list": ({ task: ref }) => {
      const task = ref === null || ref === undefined ? null : this.task(ref);
      const items = [...this.drafts.values()]
        .filter((draft) => this.keptDraft(draft.kind, draft.target) !== undefined)
        .filter((draft) => task === null || draft.task === this.keyOf(task))
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at));
      return { items };
    },
    "draft.save": ({ kind, target, body, title, base_revision }) => {
      const owner = this.draftOwner(kind, target);
      const id = `${kind}:${owner.id}`;
      if (body === "" && (title ?? "") === "") {
        this.drafts.delete(id);
        return {};
      }
      const draft: Draft = { kind, target: owner.id, ...some("task", owner.task), body, ...some("title", title), ...some("base_revision", base_revision), updated_at: this.today().toISOString() };
      this.drafts.delete(id);
      this.drafts.set(id, draft);
      return { draft };
    },
    "draft.discard": ({ kind, target }) => {
      const owner = this.draftOwner(kind, target);
      return { discarded: this.drafts.delete(`${kind}:${owner.id}`) };
    },
    "schedule.preview_move": ({ key, start_date, due_date }) => {
      const task = this.task(key);
      const day = /^\d{4}-\d{2}-\d{2}$/;
      for (const [field, value] of [["start_date", start_date], ["due_date", due_date]] as const) {
        if (value !== null && value !== undefined && !day.test(value)) {
          fail("invalid", `${field} must be a date written as YYYY-MM-DD.`, { details: { path: field, message_id: "schedule-preview-date", args: { field } } });
        }
      }
      const archived = this.archivedKeys();
      const dates = new Map(
        this.state.tasks.map((each) => [each.id, { start: each.startDate, due: each.dueDate, closed: each.closedAt !== null || archived.has(this.projectOf(each).key) }]),
      );
      dates.set(task.id, { start: start_date ?? null, due: due_date ?? null, closed: false });
      const links = this.state.relations.filter((relation) => relation.kind === "blocks");
      const moved = moveDependents(task.id, dates, links).flatMap((id) => {
        const other = this.state.tasks.find((candidate) => candidate.id === id);
        const now = dates.get(id);
        if (!other || !now) {
          return [];
        }
        const was = other.startDate ?? other.dueDate;
        const is = now.start ?? now.due;
        const days = was === null || is === null ? 0 : Math.round((Date.parse(is) - Date.parse(was)) / 86_400_000);
        return [{ days, id: other.id, key: this.keyOf(other), ...some("start_date", now.start), ...some("due_date", now.due) }];
      });
      return { moved };
    },
    "gantt.layout.get": ({ project: ref }) => {
      const project = this.project(ref);
      return { project: project.key, layout: structuredClone(this.state.ganttLayouts?.[project.id] ?? DEFAULT_GANTT_LAYOUT) };
    },
    "gantt.layout.update": ({ project: ref, reset, zoom, table, widths, group_by, move_dependents }) => {
      const project = this.project(ref);
      this.state.ganttLayouts ??= {};
      const before = reset === true ? DEFAULT_GANTT_LAYOUT : (this.state.ganttLayouts[project.id] ?? DEFAULT_GANTT_LAYOUT);
      if (table !== null && table !== undefined && (!Number.isInteger(table) || table < 200 || table > 1200)) {
        fail("invalid", "The table is 200 to 1200 pixels wide.", { details: { path: "table", message_id: "gantt-layout-table", args: { min: 200, max: 1200 } } });
      }
      for (const [column, width] of Object.entries(widths ?? {})) {
        if (!["key", "title", "claim", "start", "due", "progress"].includes(column)) {
          fail("invalid", "The chart's table has only these columns: key, title, claim, start, due, progress.", { details: { path: `widths.${column}`, message_id: "gantt-layout-column", args: { columns: "key, title, claim, start, due, progress" } } });
        }
        if (!Number.isInteger(width) || width < 24 || width > 600) {
          fail("invalid", "A column is 24 to 600 pixels wide.", { details: { path: `widths.${column}`, message_id: "gantt-layout-width", args: { min: 24, max: 600 } } });
        }
      }
      const next: GanttLayout = {
        zoom: zoom ?? before.zoom,
        ...(table === null || table === undefined ? (before.table === undefined ? {} : { table: before.table }) : { table }),
        widths: { ...before.widths, ...widths },
        group_by: group_by ?? before.group_by,
        move_dependents: move_dependents ?? before.move_dependents,
      };
      const was = this.state.ganttLayouts[project.id] ?? DEFAULT_GANTT_LAYOUT;
      this.state.ganttLayouts[project.id] = next;
      return {
        output: { project: project.key, layout: structuredClone(next) },
        changes: JSON.stringify(was) === JSON.stringify(next) ? [] : [{ entity: "gantt_layout", id: project.id, fields: ["settings"], project: project.key }],
      };
    },
    "list.layout.get": ({ project: ref }) => {
      const project = ref === null || ref === undefined ? undefined : this.project(ref);
      return { ...some("project", project?.key), layout: structuredClone(this.state.listLayouts?.[project?.id ?? "all"] ?? { columns: [], widths: {} }) };
    },
    "list.layout.update": ({ project: ref, reset, columns, widths, collapsed }) => {
      const project = ref === null || ref === undefined ? undefined : this.project(ref);
      const scope = project?.id ?? "all";
      this.state.listLayouts ??= {};
      const before = reset === true ? { columns: [], widths: {} } : (this.state.listLayouts[scope] ?? { columns: [], widths: {} });
      for (const [column, width] of Object.entries(widths ?? {})) {
        if (!Number.isInteger(width) || width < 40 || width > 1200) {
          fail("invalid", "A column is 40 to 1200 pixels wide.", { details: { path: `widths.${column}`, message_id: "list-layout-width", args: { min: 40, max: 1200 } } });
        }
      }
      const folded = new Map<string, string[]>(Object.entries(before.collapsed ?? {}));
      for (const [grouping, values] of Object.entries(collapsed ?? {})) {
        if (!["status", "priority", "label", "parent", "due_week", "project"].includes(grouping)) {
          fail("invalid", "Groups can be folded only for status, priority, label, parent, due week or project.", { details: { path: `collapsed.${grouping}`, message_id: "list-layout-grouping" } });
        }
        if (values.length === 0) {
          folded.delete(grouping);
        } else {
          folded.set(grouping, [...new Set(values)]);
        }
      }
      const next: ListLayout = {
        columns: columns ? [...new Set(columns)] : [...before.columns],
        widths: { ...before.widths, ...widths },
        ...(folded.size > 0 ? { collapsed: Object.fromEntries(folded) } : {}),
      };
      this.state.listLayouts[scope] = next;
      return {
        output: { ...some("project", project?.key), layout: structuredClone(next) },
        changes: [{ entity: "list_layout", id: project?.id ?? "00000000-0000-0000-0000-000000000000", fields: ["settings"], ...some("project", project?.key) }],
      };
    },
    "task.bulk_update": (input) => this.bulk(input),
    "view.list": (input) => {
      const items = listViews(this.state.views ?? [], { ...input, ...some("project", input.project && this.project(input.project).key) });
      return { items: structuredClone(items), total: items.length };
    },
    "view.create": (input) => {
      if (input.query.trim() !== "") {
        this.runQuery(() => searchTasks([], { query: input.query }, this.queryNames(), { archived: new Set(), scoped: false, statusOrder: () => 0 }));
      }
      this.state.views ??= [];
      const view = createView(this.state.views, { ...input, ...some("project", input.project && this.project(input.project).key) }, this.today().toISOString());
      if ("code" in view) {
        fail(view.code, view.message);
      }
      return { output: { view: structuredClone(view) }, changes: this.viewChange(view, ["created"]) };
    },
    "view.update": (input) => {
      this.state.views ??= [];
      const view = updateView(this.state.views, input, this.today().toISOString());
      if ("code" in view) {
        fail(view.code, view.message);
      }
      return { output: { view: structuredClone(view) }, changes: this.viewChange(view, ["record"]) };
    },
    "view.delete": (input) => {
      this.state.views ??= [];
      const view = deleteView(this.state.views, input);
      if ("code" in view) {
        fail(view.code, view.message);
      }
      return { output: { view, deleted: true }, changes: this.viewChange(view, ["deleted"]) };
    },
    "history.revert": ({ groups, dry_run }) => {
      const reverted: string[] = [];
      const skipped: { group: string; reason: string; reason_id: string; reason_args: Record<string, string> }[] = [];
      for (const group of groups ?? []) {
        const latest = this.undoStack[this.undoStack.length - 1];
        if (latest?.group === group && dry_run !== true) {
          this.revertLatest();
          reverted.push(group);
        } else if (latest?.group === group) {
          reverted.push(group);
        } else {
          skipped.push({ group, reason: "The sample workspace reverts only its latest change.", reason_id: "revert-skipped-not-latest", reason_args: {} });
        }
      }
      if (reverted.length === 0 || dry_run === true) {
        return { reverted, skipped };
      }
      return { output: { reverted, skipped }, changes: [{ entity: "workspace", id: "w-1", fields: ["*"] }] };
    },
    "task.create": ({ project: ref, title, status, description, priority, before, after, at, parent: parentKey, start_date, due_date, fields }) => {
      const project = this.writable(ref);
      const startDate = start_date === null || start_date === undefined ? null : this.dayInput(start_date, "start_date");
      const dueDate = due_date === null || due_date === undefined ? null : this.dayInput(due_date, "due_date");
      if (startDate !== null && dueDate !== null && startDate > dueDate) {
        fail("invalid", "The start date is after the due date.", { details: { path: "start_date" } });
      }
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
        startDate,
        dueDate,
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
      applyValues(this.state, task, type.name, fields, true, fail);
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
        this.draftSaved("task", task.id);
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
      const typed = (text: string | null | undefined, path: string) => (text === null || text === undefined ? null : this.dayInput(text, path));
      const start = clear.includes("start_date") ? null : (typed(input.start_date, "start_date") ?? task.startDate);
      const due = clear.includes("due_date") ? null : (typed(input.due_date, "due_date") ?? task.dueDate);
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
      if (input.fields !== undefined && applyValues(this.state, task, this.typeRef(task).name, input.fields, false, fail)) {
        fields.push("fields");
      }
      if (fields.length === 0) {
        return { output: { task: this.fullTask(task), ...some("description_save", save) }, changes: [] };
      }
      this.touch(task, !(fields.length === 1 && fields[0] === "conflict_copy"));
      const change = this.taskChange(task, fields);
      if (input.move_dependents === true && (fields.includes("start_date") || fields.includes("due_date"))) {
        const dates = new Map(this.state.tasks.map((each) => [each.id, { start: each.startDate, due: each.dueDate, closed: each.closedAt !== null }]));
        const links = this.state.relations.filter((relation) => relation.kind === "blocks");
        const movedIds = moveDependents(task.id, dates, links);
        const moved = [];
        for (const id of movedIds) {
          const other = this.state.tasks.find((candidate) => candidate.id === id);
          const now = dates.get(id);
          if (!other || !now) {
            continue;
          }
          const was = other.startDate ?? other.dueDate;
          const is = now.start ?? now.due;
          other.startDate = now.start;
          other.dueDate = now.due;
          this.touch(other);
          const days = was === null || is === null ? 0 : Math.round((Date.parse(is) - Date.parse(was)) / 86_400_000);
          moved.push({ days, id: other.id, key: this.keyOf(other), ...some("start_date", other.startDate), ...some("due_date", other.dueDate) });
          change.changes.push({ entity: "task", id: other.id, fields: ["start_date", "due_date"], project: this.projectOf(other).key });
        }
        return { ...change, output: { ...change.output, ...some("description_save", save), ...(moved.length > 0 ? { moved } : {}) } };
      }
      return { ...change, output: { ...change.output, ...some("description_save", save) } };
    },
    "task.resolve_conflict": ({ key, copy, action, text, base_revision }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      if (base_revision !== null && base_revision !== undefined && base_revision !== this.currentRevision(task)?.id) {
        fail("conflict", "The description changed since you read it.", { hint: "Read it again and resolve the copy once more." });
      }
      this.resolveConflict(task, copy, action, text ?? null);
      this.draftSaved("task", task.id);
      this.touch(task);
      return this.taskChange(task, ["description"]);
    },
    "checklist.update": ({ key, comment: commentId, page: pageRef, project: projectRef, toggle, add, remove }) => {
      if ((add?.length ?? 0) > 0 || (remove?.length ?? 0) > 0) {
        fail("invalid", "The sample workspace only ticks and clears items.");
      }
      const ticked = (start: string) => {
        let text = start;
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
        return text;
      };
      if (pageRef !== null && pageRef !== undefined) {
        const page = this.wikiPage(pageRef, projectRef);
        const project = this.wikiWritable(this.projectKeyOf(page));
        const text = ticked(page.body);
        const unchanged = text === page.body;
        if (!unchanged) {
          this.addPageRevision(page, text, "normal", this.currentRevision(page)?.id ?? null);
        }
        const items = publicChecklist(text);
        return {
          output: {
            checklist: { done: items.filter((item) => item.checked).length, total: items.length },
            items,
            key: project.key,
            page: page.id,
            ...some("revision", this.currentRevision(page)?.id),
            version: 0,
          },
          changes: unchanged ? [] : [{ entity: "wiki_page", id: page.id, fields: ["body"], project: project.key }],
        };
      }
      const comment = commentId === null || commentId === undefined ? null : this.liveComment(commentId);
      const task = comment ? this.state.tasks.find((candidate) => candidate.id === comment.taskId) : this.task(key ?? "");
      if (!task) {
        fail("not_found", "There is no such task.");
      }
      this.writable(this.projectOf(task).key);
      const text = ticked(comment ? comment.body : task.description);
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
      const page = (this.state.wiki ?? []).find((candidate) => candidate.id === revision.taskId);
      const current = task ? this.currentRevision(task) : page && this.currentRevision(page);
      const from = diff_from === null || diff_from === undefined ? null : this.revision(diff_from);
      if (from && from.taskId !== revision.taskId) {
        fail("invalid", "Both revisions must belong to the same description.", { details: { path: "diff_from" } });
      }
      return {
        body: revision.body,
        ...some("diff", from && unifiedDiff(from.body, revision.body, from.id, revision.id)),
        owner: task ? this.keyOf(task) : (page?.title ?? ""),
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
        const page = document.page === null || document.page === undefined ? undefined : this.wikiPage(document.page, undefined);
        if (page !== undefined && document.source === undefined && document.revision === undefined) {
          text = page.body;
          id = `wiki-${page.id}`;
        }
        return {
          html: page === undefined ? renderFake(text, known) : this.renderPage(text, page, known, [page.id]),
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

    // ---------- time ----------

    "time.log": ({ key, duration, date, note }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      const minutes = this.entryMinutes(duration, "duration");
      const day = workDay(date, this.today()) ?? fail("invalid", `"${date ?? ""}" isn't a day.`, { details: { path: "date" } });
      const entry: FakeTimeEntry = {
        id: nextId("te"),
        taskId: task.id,
        date: day,
        minutes,
        note: this.entryNote(note),
        author: { kind: "user", name: "You" },
        createdAt: this.today().toISOString(),
        editedAt: null,
        version: 1,
        deletedAt: null,
      };
      this.state.timeEntries.push(entry);
      task.spentMin += minutes;
      task.updatedAt = this.today().toISOString();
      return this.entryChange(entry, ["created"]);
    },
    "time.update": ({ entry: id, date, duration, note, expected_version }) => {
      const entry = this.liveEntry(id);
      const task = this.task(entry.taskId);
      this.writable(this.projectOf(task).key);
      if (expected_version !== null && expected_version !== undefined && expected_version !== entry.version) {
        fail("conflict", "The time entry changed since you read it.", { details: { current_version: entry.version } });
      }
      const fields: string[] = [];
      if (duration !== null && duration !== undefined) {
        const minutes = this.entryMinutes(duration, "duration");
        if (minutes !== entry.minutes) {
          task.spentMin += minutes - entry.minutes;
          entry.minutes = minutes;
          fields.push("minutes");
        }
      }
      if (date !== null && date !== undefined) {
        const day = workDay(date, this.today()) ?? fail("invalid", `"${date}" isn't a day.`, { details: { path: "date" } });
        if (day !== entry.date) {
          entry.date = day;
          fields.push("date");
        }
      }
      if (note !== null && note !== undefined && this.entryNote(note) !== entry.note) {
        entry.note = this.entryNote(note);
        fields.push("note");
      }
      if (fields.length === 0) {
        return { output: this.entryOutput(entry), changes: [] };
      }
      entry.version += 1;
      entry.editedAt = this.today().toISOString();
      task.updatedAt = entry.editedAt;
      return this.entryChange(entry, fields);
    },
    "time.delete": ({ entry: id }) => {
      const entry = this.liveEntry(id);
      const task = this.task(entry.taskId);
      this.writable(this.projectOf(task).key);
      entry.deletedAt = this.today().toISOString();
      task.spentMin -= entry.minutes;
      task.updatedAt = entry.deletedAt;
      return this.entryChange(entry, ["trashed"]);
    },
    "time.report": ({ from, to, group_by, project, include_subprojects, task: taskRef, person, entries, csv }) => {
      const now = this.today();
      const weekFirst = (day: string) => weekStart(day, this.state.settings.week_start);
      const first = from === null || from === undefined ? weekFirst(localDay(now)) : (workDay(from, now) ?? fail("invalid", `"${from}" isn't a day.`, { details: { path: "from" } }));
      const last = to === null || to === undefined ? addDays(first, 6) : (workDay(to, now) ?? fail("invalid", `"${to}" isn't a day.`, { details: { path: "to" } }));
      if (last < first) {
        fail("invalid", "The last day comes before the first.", { details: { path: "to" } });
      }
      const scope = project === null || project === undefined ? null : this.project(project);
      const projects = scope === null ? null : new Set([scope, ...(include_subprojects === false ? [] : this.descendants(scope))].map((each) => each.id));
      const root = taskRef === null || taskRef === undefined ? null : this.task(taskRef);
      const within = (task: FakeTask): boolean => root === null || task === root || this.isAncestor(root, task);
      const who = person?.trim().toLowerCase();
      if (who === "") {
        fail("invalid", "Name a person: me, agent, or a name.", { details: { path: "person" } });
      }
      const loggedBy = (entry: FakeTimeEntry): boolean =>
        who === undefined || (who === "me" ? entry.author.kind === "user" : who === "agent" ? entry.author.kind === "agent" : entry.author.name.toLowerCase() === who);
      const units = unitsOf(this.state.settings);
      const chosen = this.state.timeEntries
        .filter((entry) => entry.deletedAt === null && entry.date >= first && entry.date <= last && loggedBy(entry))
        .flatMap((entry) => {
          const task = this.state.tasks.find((candidate) => candidate.id === entry.taskId);
          return task && (projects === null || projects.has(task.projectId)) && within(task) ? [{ entry, task }] : [];
        })
        .sort((a, b) => a.entry.date.localeCompare(b.entry.date) || a.entry.createdAt.localeCompare(b.entry.createdAt));
      const grouping = group_by ?? "task";
      const groups = new Map<string, { key: string; title: string; person?: Actor; minutes: number; count: number }>();
      for (const { entry, task } of chosen) {
        const project = this.projectOf(task);
        const week = weekFirst(entry.date);
        const [key, title] =
          grouping === "day"
            ? [entry.date, new Date(`${entry.date}T12:00:00`).toLocaleDateString("en", { weekday: "long" })]
            : grouping === "week"
              ? [week, `${week}..${addDays(week, 6)}`]
              : grouping === "month"
                ? [entry.date.slice(0, 7), entry.date.slice(0, 7)]
                : grouping === "person"
                  ? [`${entry.author.kind}:${entry.author.name}`, entry.author.name]
                  : grouping === "project"
                    ? [project.key, project.name]
                    : [this.keyOf(task), task.title];
        const group = groups.get(key) ?? { key, title, ...(grouping === "person" ? { person: entry.author } : {}), minutes: 0, count: 0 };
        group.minutes += entry.minutes;
        group.count += 1;
        groups.set(key, group);
      }
      const rows = [...groups.values()];
      const inOrder = grouping === "day" || grouping === "week" || grouping === "month";
      rows.sort(inOrder ? (a, b) => a.key.localeCompare(b.key) : (a, b) => b.minutes - a.minutes || a.key.localeCompare(b.key));
      const total = chosen.reduce((sum, { entry }) => sum + entry.minutes, 0);
      const items = rows.map((row) => ({ duration: formatDuration(row.minutes, units), entry_count: row.count, key: row.key, minutes: row.minutes, ...(row.person ? { person: row.person } : {}), title: row.title }));
      const csvText =
        csv === "groups"
          ? csvOf([grouping, "title", "minutes", "duration", "entry_count"], items.map((row) => [row.key, row.title, String(row.minutes), row.duration, String(row.entry_count)]))
          : csv === "entries"
            ? csvOf(
                ["date", "person_kind", "person", "project", "task", "title", "minutes", "duration", "note"],
                chosen.map(({ entry, task }) => [entry.date, entry.author.kind, entry.author.name, this.projectOf(task).key, this.keyOf(task), task.title, String(entry.minutes), formatDuration(entry.minutes, units), entry.note]),
              )
            : undefined;
      return {
        ...(csvText === undefined ? {} : { csv: csvText }),
        days_per_week: units.daysPerWeek,
        ...(entries === true ? { entries: chosen.map(({ entry, task }) => timeEntryOut(entry, this.keyOf(task), units)) } : {}),
        from: first,
        group_by: grouping,
        hours_per_day: units.hoursPerDay,
        items,
        to: last,
        total_duration: formatDuration(total, units),
        total_minutes: total,
      };
    },

    // ---------- timer ----------

    "timer.get": () => this.timerStatus(),
    "timer.start": ({ key, note }) => {
      const current = this.state.timer;
      const now = this.today().toISOString();
      if (key === null || key === undefined) {
        if (current === null) {
          fail("not_found", "There is no paused timer to start again.", { hint: "Name the task to time." });
        }
        if (current.runningSince !== null) {
          return { output: this.timerStatus(), changes: [] };
        }
        current.runningSince = now;
        return { output: this.timerStatus(), changes: [this.timerEvent()] };
      }
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      if (current !== null && current.taskId === task.id) {
        if (current.runningSince === null) {
          current.runningSince = now;
          return { output: this.timerStatus(), changes: [this.timerEvent()] };
        }
        return { output: this.timerStatus(), changes: [] };
      }
      const changes = current === null ? [] : this.stopTimer(null, false).changes;
      this.state.timer = { taskId: task.id, startedAt: now, runningSince: now, elapsedMs: 0, note: note ?? "" };
      return { output: this.timerStatus(), changes: [...changes, this.timerEvent()] };
    },
    "timer.pause": () => {
      const current = this.state.timer;
      if (current === null) {
        fail("not_found", "No timer runs.");
      }
      if (current.runningSince === null) {
        return { output: this.timerStatus(), changes: [] };
      }
      current.elapsedMs = timerElapsedMs(current, this.today());
      current.runningSince = null;
      return { output: this.timerStatus(), changes: [this.timerEvent()] };
    },
    "timer.stop": ({ discard, duration, note }) => {
      if (this.state.timer === null) {
        fail("not_found", "No timer runs.");
      }
      const minutes = duration === null || duration === undefined ? null : this.entryMinutes(duration, "duration");
      const stopped = this.stopTimer(minutes, discard === true, note ?? undefined);
      return { output: stopped.output, changes: [...stopped.changes, this.timerEvent()] };
    },

    // ---------- reminders ----------

    "reminder.set": ({ key, at, days_before_due, time, note }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      if ((at === null || at === undefined) === (days_before_due === null || days_before_due === undefined)) {
        fail("invalid", "Give either a moment or the days before the due date.", { details: { path: "at" } });
      }
      const moment = at === null || at === undefined ? null : (momentOf(at) ?? fail("invalid", `"${at}" isn't a moment.`, { details: { path: "at" } }));
      if (days_before_due !== null && days_before_due !== undefined && (days_before_due < 0 || days_before_due > 365)) {
        fail("invalid", "Days before the due date go from 0 to 365.", { details: { path: "days_before_due" } });
      }
      if (time !== null && time !== undefined && !/^([01]\d|2[0-3]):[0-5]\d$/.test(time)) {
        fail("invalid", `"${time}" isn't a time of day (HH:MM).`, { details: { path: "time" } });
      }
      if ((note ?? "").length > 500) {
        fail("limit", "A reminder's text has at most 500 characters.", { details: { path: "note" } });
      }
      this.state.reminders = this.state.reminders.filter((reminder) => reminder.taskId !== task.id);
      const reminder: FakeReminder = {
        id: nextId("rm"),
        taskId: task.id,
        at: moment,
        daysBeforeDue: days_before_due ?? null,
        time: moment === null ? (time ?? null) : null,
        note: note ?? "",
        createdAt: this.today().toISOString(),
        acknowledged: false,
      };
      this.state.reminders.push(reminder);
      task.updatedAt = reminder.createdAt;
      return {
        output: { task: this.keyOf(task), reminder: this.reminderOf(task) ?? fail("storage", "The reminder went missing.") },
        changes: [{ entity: "task", id: task.id, fields: ["reminder"], project: this.projectOf(task).key }],
      };
    },
    "reminder.clear": ({ key }) => {
      const task = this.task(key);
      this.writable(this.projectOf(task).key);
      const had = this.state.reminders.some((reminder) => reminder.taskId === task.id);
      this.state.reminders = this.state.reminders.filter((reminder) => reminder.taskId !== task.id);
      return { output: { task: this.keyOf(task) }, changes: had ? [{ entity: "task", id: task.id, fields: ["reminder"], project: this.projectOf(task).key }] : [] };
    },
    "reminder.due": ({ until, since, limit }) => {
      const now = this.today();
      const end = until === null || until === undefined ? now.toISOString() : (momentOf(until) ?? fail("invalid", `"${until}" isn't a moment.`, { details: { path: "until" } }));
      const missedBefore = since === null || since === undefined ? new Date(now.getTime() - 5 * 60_000).toISOString() : (momentOf(since) ?? fail("invalid", `"${since}" isn't a moment.`, { details: { path: "since" } }));
      const items: DueReminder[] = [];
      let next: string | undefined;
      const consider = (at: string, item: Omit<DueReminder, "at" | "missed">) => {
        if (at <= end) {
          items.push({ ...item, at, missed: at < missedBefore });
        } else if (next === undefined || at < next) {
          next = at;
        }
      };
      for (const reminder of this.state.reminders.filter((candidate) => !candidate.acknowledged)) {
        const task = this.state.tasks.find((candidate) => candidate.id === reminder.taskId);
        if (!task || (reminder.at === null && task.closedAt !== null)) {
          continue;
        }
        const at = reminderAt(reminder, task.dueDate, this.state.settings.reminder_time);
        if (at !== null) {
          consider(at, { id: `reminder:${reminder.id}`, kind: "reminder", task: this.taskRef(task), ...some("note", reminder.note === "" ? null : reminder.note), ...some("due_date", task.dueDate) });
        }
      }
      const timer = this.state.timer;
      const hours = this.state.settings.timer_reminder_hours;
      if (timer !== null && hours > 0 && !this.timerReminded) {
        const task = this.state.tasks.find((candidate) => candidate.id === timer.taskId);
        const remaining = hours * 3_600_000 - timerElapsedMs(timer, now);
        if (task && (timer.runningSince !== null || remaining <= 0)) {
          consider(new Date(now.getTime() + Math.max(0, remaining)).toISOString(), { id: `timer:${String(Date.parse(timer.startedAt))}`, kind: "timer", task: this.taskRef(task) });
        }
      }
      sortDue(items);
      return { items: items.slice(0, limit ?? 100), total: items.length, until: end, ...some("next_at", next) };
    },
    "reminder.acknowledge": ({ ids }) => {
      const acknowledged: string[] = [];
      const unknown: string[] = [];
      for (const id of ids) {
        const reminder = this.state.reminders.find((candidate) => `reminder:${candidate.id}` === id && !candidate.acknowledged);
        if (reminder) {
          reminder.acknowledged = true;
          acknowledged.push(id);
        } else if (id.startsWith("timer:") && this.state.timer !== null && !this.timerReminded) {
          this.timerReminded = true;
          acknowledged.push(id);
        } else {
          unknown.push(id);
        }
      }
      return { acknowledged, ...(unknown.length > 0 ? { unknown } : {}) };
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
