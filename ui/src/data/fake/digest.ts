// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's digest ("while you were away"). The sample
// workspace has no history of other programs, so it makes up a small one
// when it opens, on its own tasks: two agent sessions, an automation rule
// with a failed run and a change through the command line, all in the
// hours before now. Reminders and due dates come from the workspace itself.
//
// The person's marker is kept apart from the workspace's data, as the
// engine keeps it: Undo doesn't touch it and moving it is no history.
// Reverting a made-up session or rule marks its changes reverted.

import { type ActivityTask, type AutomationRun, type Digest, type DigestDismissInput, type DigestDismissed, type DigestDueTask, type DigestInput, type DigestRule, type DigestSession, type DueReminder, type OpError } from "../../generated/ops";
import { type FakeProject, type FakeTask } from "./state";

/** What the digest needs of the workspace around it. */
export interface DigestHost {
  now: () => Date;
  /** The person's day, `YYYY-MM-DD`. */
  today: () => string;
  /** A day as typed, `YYYY-MM-DD`; fails on anything else. */
  dayInput: (text: string, path: string) => string;
  tasks: () => FakeTask[];
  projects: () => FakeProject[];
  project: (ref: string) => FakeProject;
  keyOf: (task: FakeTask) => string;
  /** Reminders due up to now, not acknowledged. */
  dueReminders: () => DueReminder[];
  fail: (code: OpError["code"], message: string, path?: string) => never;
}

interface MadeSession {
  id: string;
  name: string;
  client?: string;
  running: boolean;
  startedAt: string;
  lastAt: string;
  changes: number;
  groups: string[];
  taskIds: string[];
  reverted: boolean;
}

interface MadeRule {
  id: string;
  name: string;
  projectId: string;
  runs: { id: string; at: string; result: AutomationRun["result"]; taskId: string; group?: string; changes: number; error?: OpError }[];
  reverted: boolean;
}

interface MadeCli {
  changes: number;
  groups: string[];
  taskIds: string[];
  lastAt: string;
  reverted: boolean;
}

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const DEFAULT_LIMIT = 20;

/** The digest of one workspace, made when it is first read. */
export class FakeDigest {
  private marker: string | undefined;
  private made: { sessions: MadeSession[]; rules: MadeRule[]; cli: MadeCli } | undefined;

  constructor(private readonly host: DigestHost) {}

  private history() {
    if (this.made !== undefined) {
      return this.made;
    }
    const now = this.host.now().getTime();
    const at = (hoursAgo: number) => new Date(now - hoursAgo * HOUR).toISOString();
    const open = this.host.tasks().filter((task) => task.closedAt === null && task.parentId === null);
    const pick = (from: number, count: number) => open.slice(from, from + count).map((task) => task.id);
    const first = open[0];
    this.made = {
      sessions: [
        { id: "digest-session-1", name: "Claude Code", client: "claude-code", running: false, startedAt: at(5), lastAt: at(3), changes: 7, groups: ["digest-group-1", "digest-group-2", "digest-group-3"], taskIds: pick(0, 3), reverted: false },
        { id: "digest-session-2", name: "Planner", running: true, startedAt: at(2), lastAt: at(1), changes: 2, groups: ["digest-group-4"], taskIds: pick(3, 1), reverted: false },
      ],
      rules: first
        ? [
            {
              id: "digest-rule-1",
              name: "Escalate blockers",
              projectId: first.projectId,
              reverted: false,
              runs: [
                { id: "digest-run-1", at: at(4), result: "done", taskId: first.id, group: "digest-group-5", changes: 1 },
                { id: "digest-run-2", at: at(2.5), result: "failed", taskId: open[1]?.id ?? first.id, changes: 0, error: { code: "invalid", message: "The status Blocked doesn't exist in this project." } },
              ],
            },
          ]
        : [],
      cli: { changes: 2, groups: ["digest-group-6"], taskIds: pick(4, 1), lastAt: at(6), reverted: false },
    };
    return this.made;
  }

  /** The projects `ref` covers, with their sub-projects; `undefined`: all. */
  private covered(ref: string | null | undefined): Set<string> | undefined {
    if (ref === null || ref === undefined) {
      return undefined;
    }
    const ids = new Set<string>();
    const add = (id: string) => {
      ids.add(id);
      this.host.projects().filter((each) => each.parent === id).forEach((each) => { add(each.id); });
    };
    add(this.host.project(ref).id);
    return ids;
  }

  private activityTask(id: string): ActivityTask | undefined {
    const task = this.host.tasks().find((each) => each.id === id);
    return task ? { id: task.id, key: this.host.keyOf(task), title: task.title } : undefined;
  }

  get(input: DigestInput): Digest {
    const now = this.host.now();
    const limit = input.limit ?? DEFAULT_LIMIT;
    if (limit < 1 || limit > 100) {
      this.host.fail("invalid", "limit must be from 1 to 100.", "limit");
    }
    let since: string;
    let sinceFrom: Digest["since_from"];
    if (input.since !== null && input.since !== undefined) {
      const parsed = Date.parse(input.since);
      since = Number.isNaN(parsed) || !/\d{2}:\d{2}/.test(input.since) ? new Date(`${this.host.dayInput(input.since, "since")}T00:00:00`).toISOString() : new Date(parsed).toISOString();
      sinceFrom = "given";
    } else if (this.marker !== undefined) {
      since = this.marker;
      sinceFrom = "marker";
    } else {
      since = new Date(now.getTime() - DAY).toISOString();
      sinceFrom = "last_day";
    }
    const until = now.toISOString();
    const inWindow = (at: string) => at >= since && at <= until;
    const projects = this.covered(input.project);
    const projectOfTask = (id: string) => this.host.tasks().find((task) => task.id === id)?.projectId;
    const seen = (taskId: string) => projects === undefined || projects.has(projectOfTask(taskId) ?? "");
    const tasksOf = (ids: string[]) => ids.filter(seen).map((id) => this.activityTask(id)).filter((task) => task !== undefined);
    const made = this.history();

    const agents: DigestSession[] = made.sessions
      .filter((session) => inWindow(session.lastAt) && session.taskIds.some(seen))
      .sort((a, b) => b.lastAt.localeCompare(a.lastAt))
      .map((session) => {
        const tasks = tasksOf(session.taskIds);
        return {
          id: session.id,
          name: session.name,
          ...(session.client === undefined ? {} : { client: session.client }),
          running: session.running,
          started_at: session.startedAt,
          last_at: session.lastAt,
          changes: session.changes,
          reverted: session.reverted ? session.changes : 0,
          groups: session.groups.length,
          tasks: tasks.slice(0, limit),
          task_count: tasks.length,
        };
      });

    const automation: DigestRule[] = [];
    for (const rule of made.rules) {
      if (projects !== undefined && !projects.has(rule.projectId)) {
        continue;
      }
      const runs = rule.runs.filter((run) => inWindow(run.at)).sort((a, b) => b.at.localeCompare(a.at));
      if (runs.length === 0) {
        continue;
      }
      const project = this.host.projects().find((each) => each.id === rule.projectId);
      const tasks = tasksOf([...new Set(runs.map((run) => run.taskId))]);
      const failed = runs.find((run) => run.result === "failed");
      automation.push({
        id: rule.id,
        name: rule.name,
        ...(project ? { project: project.key } : {}),
        runs: runs.length,
        failures: runs.filter((run) => run.result === "failed").length,
        limited: runs.filter((run) => run.result === "limited").length,
        changes: rule.reverted ? 0 : runs.reduce((sum, run) => sum + run.changes, 0),
        last_at: runs[0]?.at ?? until,
        ...(failed?.error ? { last_error: failed.error } : {}),
        groups: rule.reverted ? [] : runs.flatMap((run) => (run.group === undefined ? [] : [run.group])),
        tasks: tasks.slice(0, limit),
        task_count: tasks.length,
        recent: runs.slice(0, limit).map((run) => {
          const task = this.activityTask(run.taskId);
          return {
            id: run.id,
            rule: rule.id,
            rule_name: rule.name,
            at: run.at,
            result: run.result,
            trigger: "comment_added",
            ...(task ? { task: task.key } : {}),
            ...(run.group === undefined ? {} : { group: run.group }),
            changes: run.changes,
            depth: 0,
            ...(run.error ? { error: run.error } : {}),
          };
        }),
      });
    }

    const reminders = this.host.dueReminders().filter((reminder) => (reminder.kind === "reminder" || reminder.kind === "automation") && inWindow(reminder.at) && seen(reminder.task.id));

    // Tasks whose due day began in the window; those already due at its start were seen.
    const today = this.host.today();
    const began = (day: string) => new Date(`${day}T00:00:00`).toISOString() >= since;
    const due = this.host
      .tasks()
      .filter((task) => task.closedAt === null && task.dueDate !== null && began(task.dueDate) && task.dueDate <= today && seen(task.id))
      .sort((a, b) => (a.dueDate ?? "").localeCompare(b.dueDate ?? "") || this.host.keyOf(a).localeCompare(this.host.keyOf(b)));
    const becameDue: DigestDueTask[] = due.slice(0, limit).map((task) => ({ task: { id: task.id, key: this.host.keyOf(task), title: task.title }, due_date: task.dueDate ?? today, overdue: (task.dueDate ?? today) < today }));

    const cliTasks = tasksOf(made.cli.taskIds);
    const cliShown = inWindow(made.cli.lastAt) && cliTasks.length > 0;
    const cli = cliShown
      ? { changes: made.cli.changes, group_count: made.cli.groups.length, groups: made.cli.reverted ? [] : made.cli.groups, tasks: cliTasks.slice(0, limit), task_count: cliTasks.length, last_at: made.cli.lastAt }
      : { changes: 0, group_count: 0, groups: [], tasks: [], task_count: 0 };

    const counts = {
      sessions: agents.length,
      agent_changes: agents.reduce((sum, session) => sum + session.changes, 0),
      rules: automation.length,
      runs: automation.reduce((sum, rule) => sum + rule.runs, 0),
      failures: automation.reduce((sum, rule) => sum + rule.failures, 0),
      reminders: reminders.length,
      became_due: due.length,
      cli_changes: cli.changes,
      // The person's own changes in the app are listed for agents only.
      app_changes: 0,
      items: 0,
    };
    counts.items = counts.sessions + counts.rules + counts.reminders + counts.became_due + (counts.cli_changes > 0 ? 1 : 0);
    return {
      since,
      since_from: sinceFrom,
      until,
      agents: agents.slice(0, limit),
      automation: automation.slice(0, limit),
      reminders: reminders.slice(0, limit),
      became_due: becameDue,
      cli,
      app: { changes: 0, group_count: 0, groups: [], tasks: [], task_count: 0 },
      counts,
    };
  }

  dismiss(input: DigestDismissInput): DigestDismissed {
    const now = this.host.now().toISOString();
    let at = now;
    if (input.at !== null && input.at !== undefined) {
      const parsed = Date.parse(input.at);
      if (Number.isNaN(parsed)) {
        this.host.fail("invalid", `"${input.at}" is neither a moment nor a day.`, "at");
      }
      at = new Date(Math.min(parsed, Date.parse(now))).toISOString();
    }
    if (this.marker === undefined || at > this.marker) {
      this.marker = at;
    }
    return { marker: this.marker };
  }

  /**
   * Reverts a made-up session, rule or command-line change: the groups it
   * names or the session. Answers the groups reverted, or `undefined` when
   * none of them are the digest's own.
   */
  revert(input: { groups?: string[] | null | undefined; session?: string | null | undefined; dry_run?: boolean | null | undefined }): string[] | undefined {
    const made = this.history();
    const reverted: string[] = [];
    const session = made.sessions.find((each) => each.id === input.session);
    if (session) {
      if (!session.reverted) {
        reverted.push(...session.groups);
        if (input.dry_run !== true) {
          session.reverted = true;
        }
      }
      return reverted;
    }
    const groups = input.groups ?? [];
    for (const rule of made.rules) {
      const own = rule.runs.flatMap((run) => (run.group === undefined ? [] : [run.group]));
      if (!rule.reverted && own.length > 0 && own.every((group) => groups.includes(group))) {
        reverted.push(...own);
        if (input.dry_run !== true) {
          rule.reverted = true;
        }
      }
    }
    if (!made.cli.reverted && made.cli.groups.length > 0 && made.cli.groups.every((group) => groups.includes(group))) {
      reverted.push(...made.cli.groups);
      if (input.dry_run !== true) {
        made.cli.reverted = true;
      }
    }
    return reverted.length > 0 ? reverted : undefined;
  }
}
