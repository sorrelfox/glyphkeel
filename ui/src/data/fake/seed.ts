// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Sample data for the in-memory engine: the same projects and tasks on every
// start for a given day, so screenshots and browser flows can rely on them.

import { type Category as StatusCategory, type Glyph, type Hue, type Priority } from "../../generated/ops";
import { type FakeProject, type FakeState, type FakeTask } from "./state";
import { type FakeSprint } from "./sprints";
import { type FakeVersion } from "./versions";

/** A small deterministic random generator (mulberry32). */
export function random(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A day as `YYYY-MM-DD`, some days from `today`. */
export function dayOffset(today: Date, days: number): string {
  const day = new Date(Date.UTC(today.getFullYear(), today.getMonth(), today.getDate() + days));
  return day.toISOString().slice(0, 10);
}

let ids = 0;
/** IDs that sort in creation order, like the engine's. */
export function nextId(prefix: string): string {
  ids += 1;
  return `${prefix}-${ids.toString(36).padStart(5, "0")}`;
}

/** A new ID in the form of a UUID, for items whose IDs the UI shortens (comments). */
export function nextUuid(): string {
  ids += 1;
  return `00000000-0000-4000-8000-${ids.toString(16).padStart(12, "0")}`;
}

/** A new attachment ID: a lower-case UUID, the one form attachment links take. */
export function nextFileId(): string {
  ids += 1;
  return `00000000-0000-4000-8000-${ids.toString(16).padStart(12, "0")}`;
}

interface StatusSeed {
  name: string;
  category: StatusCategory;
  hue: Hue;
  wip?: number;
}

/** A new project's statuses. */
export const DEFAULT_STATUSES: readonly StatusSeed[] = [
  { name: "Backlog", category: "todo", hue: "grey" },
  { name: "To do", category: "todo", hue: "grey" },
  { name: "In progress", category: "in_progress", hue: "blue" },
  { name: "Review", category: "in_progress", hue: "purple" },
  { name: "Done", category: "done", hue: "green" },
];

/** A new project's task types; the first is the default. */
export const DEFAULT_TYPES: readonly { name: string; glyph: Glyph; hue: Hue }[] = [
  { name: "Task", glyph: "task", hue: "blue" },
  { name: "Bug", glyph: "bug", hue: "red" },
  { name: "Feature", glyph: "feature", hue: "green" },
  { name: "Milestone", glyph: "milestone", hue: "purple" },
];

export function newProject(fields: { key: string; name: string; hue: Hue; parent: string | null; rank: number }): FakeProject {
  return {
    id: nextId("p"),
    ...fields,
    description: "",
    state: "active",
    modules: { board: true, list: true, gantt: true, wiki: true, time: true },
    statuses: DEFAULT_STATUSES.map((status) => ({ id: nextId("s"), name: status.name, category: status.category, hue: status.hue, wip_limit: null })),
    types: DEFAULT_TYPES.map((type, index) => ({ id: nextId("ty"), ...type, is_default: index === 0 })),
    labels: [],
    nextNumber: 1,
  };
}

const VERBS = ["Add", "Fix", "Check", "Write", "Move", "Speed up", "Clean up", "Document", "Test", "Rework", "Split", "Measure"];

const SUBJECTS: Readonly<Record<string, readonly string[]>> = {
  TM: [
    "the board's column order", "hidden columns", "WIP limits per column", "the task panel's fields", "card fields",
    "quick filters", "sort modes", "dark mode switching", "the search index", "backup snapshots", "fractional ranks",
    "the query parser", "keyboard moves between columns", "the project tree", "archived projects", "the filter banner",
  ],
  CLI: ["task list output", "the MCP tool list", "shell completions", "JSON output", "exit codes", "the man page", "claims", "agent sessions"],
  DOC: ["the getting started page", "screenshots", "the keyboard reference", "the install guide", "the FAQ", "the query language page"],
  PER: ["the tax return", "the dentist appointment", "the bike repair", "the holiday plan", "birthday presents", "the garden fence", "the library books"],
  LAB: ["the NAS disks", "router firmware", "the backup job", "the DNS records", "the media server", "TLS certificates", "the UPS battery"],
};

const LABELS: Readonly<Record<string, readonly [string, Hue][]>> = {
  TM: [["board", "blue"], ["desktop", "teal"], ["linux", "orange"], ["theme", "pink"], ["search", "purple"], ["data", "green"], ["markdown", "yellow"], ["query", "grey"]],
  CLI: [["cli", "teal"], ["mcp", "purple"]],
  DOC: [["docs", "pink"]],
  PER: [["home", "green"], ["errand", "orange"], ["money", "yellow"]],
  LAB: [["network", "blue"], ["storage", "orange"], ["security", "red"]],
};

const PRIORITY_WEIGHTS: readonly [Priority, number][] = [["none", 0.25], ["low", 0.2], ["normal", 0.3], ["high", 0.17], ["urgent", 0.08]];

/** The agent whose changes the "Changed by agents" filter finds. */
export const SAMPLE_AGENT = "Build agent";

function pick<T>(rand: () => number, items: readonly T[]): T {
  const item = items[Math.floor(rand() * items.length)];
  if (item === undefined) {
    throw new Error("pick from an empty list");
  }
  return item;
}

function weighted<T>(rand: () => number, items: readonly [T, number][]): T {
  let roll = rand();
  for (const [item, weight] of items) {
    roll -= weight;
    if (roll <= 0) {
      return item;
    }
  }
  const last = items[items.length - 1];
  if (!last) {
    throw new Error("weighted pick from an empty list");
  }
  return last[0];
}

function checklist(rand: () => number): string {
  const total = 2 + Math.floor(rand() * 6);
  const done = Math.floor(rand() * (total + 1));
  const lines = Array.from({ length: total }, (_, index) => `- [${index < done ? "x" : " "}] Step ${String(index + 1)}`);
  return `What has to happen:\n\n${lines.join("\n")}\n`;
}

/** How many tasks each project gets: data set S has 200. */
const TASKS_PER_PROJECT: Readonly<Record<string, number>> = { TM: 80, CLI: 30, DOC: 20, PER: 35, LAB: 35 };

/**
 * Three top-level projects (one with two sub-projects), an archived one, and
 * 200 tasks across every status, with due dates around `today`, checklists,
 * subtasks, blockers and changes by an agent. The In progress column of Task
 * Manager is over its WIP limit; CLI and MCP has a status of its own.
 */
export function seed(today: Date, seedNumber = 7): FakeState {
  ids = 0;
  const rand = random(seedNumber);
  const now = today.getTime();
  const projects: FakeProject[] = [];
  const add = (key: string, name: string, hue: Hue, parent: FakeProject | null) => {
    const project = newProject({ key, name, hue, parent: parent?.id ?? null, rank: projects.length });
    projects.push(project);
    return project;
  };
  const tm = add("TM", "Task Manager", "blue", null);
  add("CLI", "CLI and MCP", "teal", tm);
  add("DOC", "Docs site", "pink", tm);
  add("PER", "Personal", "green", null);
  add("LAB", "Home lab", "orange", null);
  const blog = add("BLOG", "Old blog", "purple", null);
  blog.state = "archived";

  tm.description = "The desktop task manager itself: board, task panel, search and data.";
  const status = (project: FakeProject, name: string) => project.statuses.find((candidate) => candidate.name === name);
  const inProgress = status(tm, "In progress");
  const review = status(tm, "Review");
  if (inProgress) {
    inProgress.wip_limit = 3;
  }
  if (review) {
    review.wip_limit = 4;
  }
  const cli = projects.find((project) => project.key === "CLI");
  if (cli) {
    const waiting = { id: nextId("s"), name: "Waiting", category: "todo" as const, hue: "yellow" as const, wip_limit: null };
    cli.statuses.splice(2, 0, waiting);
  }
  for (const project of projects) {
    project.labels = (LABELS[project.key] ?? []).map(([name, hue]) => ({ id: nextId("l"), name, hue }));
  }

  const tasks: FakeTask[] = [];
  const relations: FakeState["relations"] = [];
  for (const project of projects) {
    const count = TASKS_PER_PROJECT[project.key] ?? 0;
    const subjects = SUBJECTS[project.key] ?? ["something"];
    const ranks = new Map<string, number>();
    for (let index = 0; index < count; index += 1) {
      const roll = rand();
      // Most open work sits in To do; Task Manager crowds In progress.
      const statusIndex =
        roll < 0.18 ? 0 : roll < 0.45 ? 1 : roll < (project.key === "TM" ? 0.6 : 0.55) ? 2 : roll < 0.66 ? 3 : project.statuses.length - 1;
      const taskStatus = project.statuses[Math.min(statusIndex, project.statuses.length - 1)] ?? project.statuses[0];
      if (!taskStatus) {
        continue;
      }
      const closed = taskStatus.category === "done" || taskStatus.category === "cancelled";
      const type = weighted(rand, [[project.types[0], 0.5], [project.types[1], 0.2], [project.types[2], 0.27], [project.types[3], 0.03]]);
      const rank = (ranks.get(taskStatus.id) ?? 0) + 1;
      ranks.set(taskStatus.id, rank);
      const createdDaysAgo = 5 + Math.floor(rand() * 80);
      const updatedDaysAgo = Math.floor(rand() * Math.min(createdDaysAgo, 30));
      const closedDaysAgo = closed ? Math.floor(rand() * 45) : null;
      const dueRoll = rand();
      const due = dueRoll < 0.45 ? dayOffset(today, Math.floor(rand() * 24) - 6) : null;
      const labels = project.labels.filter(() => rand() < 0.22).map((label) => label.id);
      const byAgent = rand() < 0.15;
      const description = rand() < 0.4 ? checklist(rand) : rand() < 0.5 ? "A short note about this task." : "";
      tasks.push({
        id: nextId("t"),
        projectId: project.id,
        number: project.nextNumber,
        title: `${pick(rand, VERBS)} ${pick(rand, subjects)}`,
        description,
        statusId: taskStatus.id,
        typeId: type?.id ?? project.types[0]?.id ?? "",
        priority: weighted(rand, PRIORITY_WEIGHTS),
        labelIds: labels,
        parentId: null,
        rank,
        startDate: due && rand() < 0.4 ? dayOffset(today, -Math.floor(rand() * 10)) : null,
        dueDate: due,
        estimateMin: rand() < 0.4 ? (1 + Math.floor(rand() * 16)) * 30 : null,
        spentMin: taskStatus.category === "todo" ? 0 : Math.floor(rand() * 20) * 15,
        files: rand() < 0.15 ? 1 + Math.floor(rand() * 3) : 0,
        comments: rand() < 0.3 ? 1 + Math.floor(rand() * 5) : 0,
        createdAt: new Date(now - createdDaysAgo * 86_400_000).toISOString(),
        editedAt: new Date(now - updatedDaysAgo * 86_400_000).toISOString(),
        statusEnteredAt: new Date(now - updatedDaysAgo * 86_400_000).toISOString(),
        updatedAt: new Date(now - updatedDaysAgo * 86_400_000 - Math.floor(rand() * 3_600_000)).toISOString(),
        updatedBy: byAgent ? { kind: "agent", name: SAMPLE_AGENT } : { kind: "user", name: "You" },
        closedAt: closedDaysAgo === null ? null : new Date(now - closedDaysAgo * 86_400_000).toISOString(),
        version: 1,
      });
      project.nextNumber += 1;
    }
  }

  // Subtasks: a few parents in Task Manager and the home lab, each with two to four children.
  for (const key of ["TM", "LAB"]) {
    const project = projects.find((candidate) => candidate.key === key);
    const own = tasks.filter((task) => task.projectId === project?.id);
    for (let parentIndex = 0; parentIndex + 4 < own.length && parentIndex < 20; parentIndex += 7) {
      const parent = own[parentIndex];
      const children = own.slice(parentIndex + 1, parentIndex + 2 + Math.floor(rand() * 3));
      for (const child of children) {
        if (parent) {
          child.parentId = parent.id;
        }
      }
    }
  }
  // Blockers: some open tasks wait for others.
  const open = tasks.filter((task) => task.closedAt === null);
  for (let index = 0; index + 1 < open.length; index += 11) {
    const from = open[index];
    const to = open[index + 1];
    if (from && to && from.projectId === to.projectId) {
      relations.push({ id: nextId("r"), from: from.id, to: to.id, kind: "blocks" });
    }
  }
  const first = open[0];
  const second = open[2];
  if (first && second) {
    relations.push({ id: nextId("r"), from: first.id, to: second.id, kind: "relates" });
  }

  // Versions of Task Manager: one released, one on its way (shared with the
  // sub-projects), one later; some of its tasks are planned for them.
  const version = (name: string, status: FakeVersion["status"], dueDays: number | null, sharing: FakeVersion["sharing"], description: string): FakeVersion => ({
    id: nextId("v"),
    projectId: tm.id,
    name,
    description,
    status,
    startDate: null,
    dueDate: dueDays === null ? null : dayOffset(today, dueDays),
    sharing,
    rank: 0,
    createdAt: new Date(now - 60 * 86_400_000).toISOString(),
    updatedAt: new Date(now - 60 * 86_400_000).toISOString(),
  });
  const versions = [
    version("0.9", "closed", -12, "none", "The first preview."),
    version("1.0", "open", 10, "descendants", "Board, list and the task panel, ready for daily use."),
    version("1.1", "open", null, "none", ""),
  ];
  versions.forEach((each, index) => {
    each.rank = index + 1;
  });
  const [released, next, later] = versions;
  tasks
    .filter((task) => task.projectId === tm.id && task.parentId === null)
    .forEach((task, index) => {
      const planned = task.closedAt !== null && index % 3 === 0 ? released : index % 4 === 1 ? next : index % 9 === 2 ? later : undefined;
      if (planned) {
        task.fixVersionId = planned.id;
      }
    });

  // Sprints of Task Manager: one completed, one running (started five days
  // ago, two weeks long) and one planned; open tasks are spread over them.
  const sprint = (name: string, state: FakeSprint["state"], startDays: number, goal: string, rank: number): FakeSprint => ({
    id: nextId("s"),
    projectId: tm.id,
    name,
    goal,
    state,
    startDate: dayOffset(today, startDays),
    endDate: dayOffset(today, startDays + 13),
    startedAt: state === "planned" ? null : new Date(now + startDays * 86_400_000).toISOString(),
    completedAt: state === "closed" ? new Date(now + (startDays + 13) * 86_400_000).toISOString() : null,
    rank,
    createdAt: new Date(now - 40 * 86_400_000).toISOString(),
    updatedAt: new Date(now - 40 * 86_400_000).toISOString(),
  });
  const sprints = [sprint("Sprint 1", "closed", -19, "The board and the list.", 1), sprint("Sprint 2", "active", -5, "The task panel, ready for daily use.", 2), sprint("Sprint 3", "planned", 9, "", 3)];
  const [lastSprint, thisSprint, nextSprint] = sprints;
  tasks
    .filter((task) => task.projectId === tm.id && task.parentId === null)
    .forEach((task, index) => {
      const into = task.closedAt !== null ? (index % 2 === 0 ? lastSprint : index % 3 === 1 ? thisSprint : undefined) : index % 3 === 0 ? thisSprint : index % 5 === 1 ? nextSprint : undefined;
      if (into) {
        task.sprintId = into.id;
      }
    });

  return {
    projects,
    tasks,
    relations,
    versions,
    sprints,
    comments: [],
    files: [],
    claims: [],
    revisions: [],
    activity: [],
    layouts: {},
    timeEntries: [],
    // Kinds of work for logged time, as a workspace would set them up.
    workKinds: ["Development", "Design", "Meeting"].map((name) => ({ id: nextId("wk"), name, archived: false })),
    timer: null,
    reminders: [],
    settings: {
      agent_limits: { writes_per_minute: 120, trash_items_per_call: 20, trash_items_per_hour: 50, read_burst: 60, reads_per_minute: 600, read_mb_per_minute: 8 },
      agent_max_profile: "standard",
      agent_confirmations: true,
      agent_approvals: { read_only: "off", standard: "off", full: "off", expire_days: 14 },
      days_per_week: 5,
      file_max_mb: 100,
      hours_per_day: 8,
      reminder_time: "09:00",
      timer_reminder_hours: 8,
      trash_days: 30,
      week_start: "monday",
    },
    workspaceName: "Sample workspace",
    seq: 0,
  };
}
