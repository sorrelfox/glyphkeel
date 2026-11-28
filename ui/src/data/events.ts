// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type QueryClient } from "@tanstack/react-query";

import { type Changed, type OpId } from "../generated/ops";
import { OP_ROOT, opPrefix } from "./queries";

/**
 * The queries whose answers a change to each kind of item can alter. Events
 * carry no content: the UI re-reads what is on screen, so the engine stays
 * the one source of truth.
 */
/** What links to a page or a task, and what a text links to: any text, page or task can change them. */
const LINKS: readonly OpId[] = ["wiki.backlinks", "wiki.references", "task.backlinks", "task.references"];
/** The flow reports read tasks, statuses, types and labels, and the week's first day. */
const FLOW: readonly OpId[] = ["report.created_closed", "report.cumulative_flow", "report.cycle_time"];

export const ENTITY_QUERIES: Readonly<Record<string, readonly OpId[]>> = {
  // `workspace.get` carries the settings too.
  // Durations are told in the workspace's units, and the timer reminds after its hours.
  // The trash counts its days left by the `trash_days` setting, and dates in words count weeks from `week_start`.
  settings: ["settings.get", "workspace.get", "date.parse", "timer.get", "time.report", "trash.list", ...FLOW],
  // Open-task counts show in the project tree. Rendered texts link task keys
  // only while the task exists, and descriptions keep their revisions.
  // A task's reminder is one of its fields; the time report shows titles.
  // Its cover marks one of its files.
  // Deleting, restoring and purging show in the trash.
  // Its description links to pages and tasks, and its key is a link's target.
  task: ["trash.list", "task.get", "board.get", "project.list", "folder.list", "markdown.render", "description.history", "description.revision", "time.report", "timer.get", "task.search", "today.get", "search.query", "schedule.get", "schedule.preview_move", "version.list", "version.get", "sprint.list", "sprint.get", "sprint.burndown", "field.list", "file.get", ...LINKS, ...FLOW],
  // Statuses, types and labels show on every card and in the task panel;
  // a wiki page's task lists (`{{tasks …}}`) show statuses and match by them.
  project: ["trash.list", "project.list", "folder.list", "project.get", "board.get", "task.get", "layout.get", "list.layout.get", "markdown.render", "time.report", "task.search", "query.parse", "view.list", "today.get", "search.query", "wiki.search", "wiki.get", "wiki.list", "schedule.get", "schedule.preview_move", "gantt.layout.get", "version.list", "version.get", "sprint.list", "sprint.get", "field.list", ...LINKS, ...FLOW],
  status: ["project.get", "board.get", "task.get", "project.list", "folder.list", "task.search", "query.parse", "today.get", "search.query", "schedule.get", "markdown.render", ...FLOW],
  task_type: ["project.get", "board.get", "task.get", "task.search", "query.parse", "schedule.get", "markdown.render", "field.list", ...FLOW],
  label: ["project.get", "board.get", "task.get", "task.search", "query.parse", "schedule.get", "markdown.render", ...FLOW],
  // Custom fields: their list, the values tasks show, the columns and the query field.
  custom_field: ["field.list", "task.get", "task.search", "query.parse", "view.list", ...FLOW],
  // The board leaves out the cards of hidden columns.
  layout: ["layout.get", "board.get"],
  // The order and widths of the list's columns.
  list_layout: ["list.layout.get"],
  // The person's zoom, widths and grouping of the Gantt chart.
  gantt_layout: ["gantt.layout.get"],
  // Cards count comments and files, and show the Blocked badge.
  comment: ["trash.list", "comment.list", "task.get", "board.get", "markdown.render", "task.search", "today.get", "search.query", ...LINKS],
  attachment: ["trash.list", "comment.list", "task.get", "board.get", "file.get", "file.read", "task.search", "search.query", "wiki.get"],
  // The Gantt chart draws blocks links as dependencies.
  relation: ["task.get", "board.get", "task.search", "schedule.get", "schedule.preview_move"],
  // Cards and the task show the time spent; the report adds it up.
  time_entry: ["trash.list", "task.get", "board.get", "time.report", "task.search", "work_kind.list"],
  // Kinds of work: their list, and the entries and reports that name them.
  work_kind: ["work_kind.list", "task.get", "time.report", "timer.get"],
  // The one timer: its chip, the task's timer button, and `workspace.get`, which carries it.
  timer: ["timer.get", "workspace.get", "today.get"],
  // Saved views of the list.
  view: ["view.list"],
  // Versions: the roadmap, the task panel's picker, queries naming them and the Gantt chart's markers.
  version: ["version.list", "version.get", "task.get", "task.search", "query.parse", ...FLOW],
  // Sprints: the sprints page and its burndown, the task panel's picker, cards and queries naming them.
  sprint: ["sprint.list", "sprint.get", "sprint.burndown", "task.get", "task.search", "board.get", "query.parse", "view.list"],
  // Wiki pages: their search, links to them and their text, and their revisions (read like a description's).
  // A recurring task's series: the task carries it, and lists mark its current occurrence.
  series: ["task.get", "task.search", "today.get", "schedule.get", "board.get"],
  // Templates: their lists, and the one shown.
  template: ["template.list", "template.get"],
  // Project folders: their list, and the projects' folders in the tree.
  project_folder: ["folder.list", "project.list"],
  // Dashboards: their list; each widget's value comes with every change (see ALWAYS).
  dashboard: ["dashboard.list"],
  // Automation rules: their list, the one shown and the run log.
  automation_rule: ["automation.list", "automation.get", "automation.runs"],
  // Proposals: the Approvals screen and the sidebar's count.
  proposal: ["proposal.list", "proposal.get"],
  wiki_page: ["trash.list", "wiki.search", "wiki.get", "wiki.list", "wiki.history", "description.revision", "search.query", "markdown.render", ...LINKS],
};

/**
 * Every committed change shows in the activity log and the digest, and
 * may have set automation rules off. A dashboard's widgets read any of the workspace's
 * data, so each re-reads its value: one call per widget on screen. A
 * waiting proposal shows what changed since it was made, which any change
 * can alter.
 */
const ALWAYS: readonly OpId[] = ["history.feed", "agent.sessions", "automation.list", "automation.get", "automation.runs", "dashboard.show", "dashboard.widget", "digest.get", "proposal.list", "proposal.get"];

/**
 * The query prefixes to invalidate for one event. A kind of item without an
 * entry invalidates everything: refetching too much is slow, too little is
 * wrong.
 */
export function invalidationsFor(event: Changed): readonly OpId[] | "all" {
  const ops = new Set<OpId>(ALWAYS);
  for (const change of event.changes) {
    const affected = ENTITY_QUERIES[change.entity];
    if (!affected) {
      return "all";
    }
    for (const op of affected) {
      ops.add(op);
    }
  }
  return [...ops];
}

/** Marks the queries an event affects as stale; those on screen refetch. */
export function applyEvent(queryClient: QueryClient, event: Changed): Promise<void> {
  const ops = invalidationsFor(event);
  if (ops === "all") {
    return queryClient.invalidateQueries({ queryKey: OP_ROOT });
  }
  return Promise.all(ops.map((op) => queryClient.invalidateQueries({ queryKey: opPrefix(op) }))).then(() => undefined);
}

/**
 * After (re)subscribing, events sent while the page wasn't listening are
 * lost: everything is refetched instead.
 */
export function refetchAll(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: OP_ROOT });
}
