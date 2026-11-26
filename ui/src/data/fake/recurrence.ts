// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Recurring tasks in the in-memory engine: a series holds the rule and the
// details new occurrences start from. The next occurrence is made when the
// current one closes (in the same command, so Undo takes both back), or,
// made ahead, by `recurrence.run` on its day. Skipping shifts the dates.

import { type EntityChange, type OpMap, type Recurrence, type RecurrenceChange, type RecurrenceRule, type RecurrenceSetInput, type RulePreviewInput, type TaskBlueprint } from "../../generated/ops";
import { type Answer, type FakeHost } from "./host";
import { buildRule, dayNumber, firstDay, nextAfter, nextOccurrence, plusDays, previewDates, RuleProblem, summaryOf, toCore } from "./rules";
import { nextId } from "./seed";
import { type FakeSeries, type FakeTask } from "./state";
import { blueprintOf, makeTask } from "./templates";
import { localDay } from "./time";

const PREVIEW_DEFAULT = 5;
const PREVIEW_MAX = 50;
const CATCH_UP_MAX = 10_000;

const all = (host: FakeHost) => (host.state().series ??= []);

const exhausted = (rule: RecurrenceRule, made: number) => rule.count !== undefined && made >= rule.count;

/** The day an occurrence falls on: its due date, else its start date. */
const occurrenceDay = (task: FakeTask | undefined) => task?.dueDate ?? task?.startDate ?? null;

const taskById = (host: FakeHost, id: string | null) => (id === null ? undefined : host.state().tasks.find((task) => task.id === id));

/** The series `task` is the current occurrence of, while it goes on. */
export function liveSeries(host: FakeHost, task: FakeTask): FakeSeries | undefined {
  if (task.seriesId === undefined) {
    return undefined;
  }
  return all(host).find((series) => series.id === task.seriesId && series.endedAt === null && series.current === task.id);
}

function nextDate(host: FakeHost, series: FakeSeries): string | null {
  if (series.endedAt !== null || exhausted(series.rule, series.made)) {
    return null;
  }
  if (series.rule.create === "ahead") {
    return series.nextDate;
  }
  if (series.rule.from === "completion") {
    return null;
  }
  const last = occurrenceDay(taskById(host, series.current));
  return last === null ? firstDay(toCore(series.rule)) : nextAfter(toCore(series.rule), last);
}

export function recurrenceOf(host: FakeHost, series: FakeSeries): Recurrence {
  const current = taskById(host, series.current);
  const next = nextDate(host, series);
  return {
    series: series.id,
    rule: structuredClone(series.rule),
    summary: summaryOf(series.rule),
    ...(current ? { current: host.keyOf(current) } : {}),
    ...(next === null ? {} : { next_date: next }),
    made: series.made,
    ended: series.endedAt !== null,
    title: series.pattern.title,
  };
}

/** Task `task`'s series, as tasks carry it: only while it is the current occurrence of a live series. */
export function taskRecurrence(host: FakeHost, task: FakeTask): Recurrence | undefined {
  const series = liveSeries(host, task);
  return series ? recurrenceOf(host, series) : undefined;
}

function ruleFrom(host: FakeHost, input: RulePreviewInput, start: string, existing: RecurrenceRule | null): RecurrenceRule {
  try {
    return buildRule(input, start, existing, (field, text) => host.dayInput(text, field));
  } catch (problem) {
    if (problem instanceof RuleProblem) {
      host.fail("invalid", problem.message, { details: { path: problem.field, message_id: problem.messageId, args: problem.args } });
    }
    throw problem;
  }
}

function aheadNext(rule: RecurrenceRule, last: string | null, made: number): string | null {
  if (rule.create !== "ahead" || exhausted(rule, made)) {
    return null;
  }
  return last === null ? firstDay(toCore(rule)) : nextAfter(toCore(rule), last);
}

function patternOf(host: FakeHost, task: FakeTask, anchor: string): TaskBlueprint {
  const bp = blueprintOf(host, task, anchor);
  return bp.due_in_days === undefined && bp.start_in_days === undefined ? { ...bp, due_in_days: 0 } : bp;
}

function openTask(host: FakeHost, key: string): FakeTask {
  const task = host.task(key);
  host.writable(task.projectId);
  if (task.closedAt !== null) {
    host.fail("invalid", `${host.keyOf(task)} is closed; make an open task recurring.`, { details: { path: "key", message_id: "recurrence-task-closed", args: { task: host.keyOf(task) } } });
  }
  return task;
}

const givesRule = (input: RecurrenceSetInput) =>
  [input.text, input.every, input.interval, input.month_day, input.week_of_month, input.month, input.starts, input.until, input.count, input.from, input.create, input.days_ahead].some((value) => value !== null && value !== undefined) || (input.weekdays?.length ?? 0) > 0;

const seriesChange = (host: FakeHost, series: FakeSeries): EntityChange => ({ entity: "series", id: series.id, fields: ["record"], project: host.state().projects.find((project) => project.id === series.projectId)?.key ?? "" });
const taskChange = (host: FakeHost, task: FakeTask, fields: string[]): EntityChange => ({ entity: "task", id: task.id, fields, project: host.projectOf(task).key });

function answer(host: FakeHost, task: FakeTask, series: FakeSeries | undefined, changes: EntityChange[]): Answer<RecurrenceChange> | RecurrenceChange {
  const output: RecurrenceChange = {
    task: host.keyOf(task),
    ...(series && series.endedAt === null ? { recurrence: recurrenceOf(host, series) } : {}),
    ...(task.dueDate === null ? {} : { due_date: task.dueDate }),
  };
  return changes.length === 0 ? output : { output, changes };
}

function shiftDates(task: FakeTask, days: number): void {
  task.startDate = task.startDate === null ? null : plusDays(task.startDate, days);
  task.dueDate = task.dueDate === null ? null : plusDays(task.dueDate, days);
}

/** Makes the next occurrence on `day`; null when its project is archived or gone. */
function makeOccurrence(host: FakeHost, series: FakeSeries, day: string, parentId: string | null, projectId: string): FakeTask | null {
  const project = host.state().projects.find((candidate) => candidate.id === projectId);
  if (!project || project.state === "archived") {
    return null;
  }
  const { task } = makeTask(host, project, series.pattern, null, day, parentId, null, []);
  task.seriesId = series.id;
  series.projectId = project.id;
  series.current = task.id;
  series.made += 1;
  return task;
}

/**
 * After a command: each task it closed that is the current occurrence of a
 * series made on close gets its next occurrence. Returns what changed.
 */
export function afterClosing(host: FakeHost, closed: readonly FakeTask[]): EntityChange[] {
  const changes: EntityChange[] = [];
  const today = host.today();
  for (const task of closed) {
    const series = liveSeries(host, task);
    if (!series || series.rule.create !== "on_close") {
      continue;
    }
    const rule = toCore(series.rule);
    const done = task.closedAt === null ? today : localDay(new Date(task.closedAt));
    const last = occurrenceDay(task) ?? done;
    let next = nextOccurrence(rule, last, done);
    if (rule.from === "schedule") {
      let passed = 0;
      while (next !== null && next < today && passed < CATCH_UP_MAX && !exhausted(series.rule, series.made)) {
        series.made += 1;
        passed += 1;
        next = nextAfter(rule, next);
      }
    }
    if (next === null || exhausted(series.rule, series.made)) {
      series.endedAt = host.now();
      changes.push(seriesChange(host, series));
      continue;
    }
    const parent = taskById(host, task.parentId);
    const made = makeOccurrence(host, series, next, parent?.id ?? null, task.projectId);
    if (made) {
      changes.push(seriesChange(host, series), ...[made, ...host.subtasksOf(made)].map((each) => taskChange(host, each, ["*"])));
    }
  }
  return changes;
}

type Handlers = { [K in "recurrence.set" | "recurrence.clear" | "recurrence.skip" | "recurrence.preview" | "recurrence.run"]: (input: OpMap[K]["input"]) => OpMap[K]["output"] | Answer<OpMap[K]["output"]> };

/** The recurrence operations. */
export function recurrenceHandlers(host: FakeHost): Handlers {
  return {
    "recurrence.set": (input) => {
      const task = openTask(host, input.key);
      const today = host.today();
      const day = occurrenceDay(task);
      const live = liveSeries(host, task);
      if (live) {
        const before = JSON.stringify(live);
        if (givesRule(input)) {
          live.rule = ruleFrom(host, input, day ?? today, live.rule);
        }
        if (input.update_details === true) {
          live.pattern = patternOf(host, task, day ?? today);
        }
        live.nextDate = aheadNext(live.rule, day, live.made);
        return answer(host, task, live, JSON.stringify(live) === before ? [] : [seriesChange(host, live)]);
      }
      const rule = ruleFrom(host, input, day ?? today, null);
      const changes: EntityChange[] = [];
      let anchor = day;
      if (anchor === null) {
        anchor = firstDay(toCore(rule));
        if (anchor === null) {
          host.fail("invalid", "The rule has no dates: it ends before its first one.", { details: { path: "until", message_id: "recurrence-no-dates" } });
        }
        task.dueDate = anchor;
        changes.push(taskChange(host, task, ["due_date"]));
      }
      const series: FakeSeries = {
        id: nextId("se"),
        projectId: task.projectId,
        rule,
        pattern: patternOf(host, task, anchor),
        current: task.id,
        nextDate: aheadNext(rule, anchor, 1),
        made: 1,
        endedAt: null,
        createdAt: host.now(),
      };
      all(host).push(series);
      task.seriesId = series.id;
      changes.push(seriesChange(host, series), taskChange(host, task, ["series"]));
      return answer(host, task, series, changes);
    },
    "recurrence.clear": ({ key }) => {
      const task = host.task(key);
      host.writable(task.projectId);
      const series = liveSeries(host, task);
      if (!series) {
        return answer(host, task, undefined, []);
      }
      series.endedAt = host.now();
      series.nextDate = null;
      return answer(host, task, undefined, [seriesChange(host, series)]);
    },
    "recurrence.skip": ({ key }) => {
      const task = openTask(host, key);
      const series = liveSeries(host, task);
      if (!series) {
        host.fail("invalid", `${host.keyOf(task)} doesn't repeat.`, { details: { path: "key", message_id: "recurrence-not-recurring", args: { task: host.keyOf(task) } } });
      }
      const today = host.today();
      const day = occurrenceDay(task) ?? today;
      const next = series.rule.create === "ahead" ? series.nextDate : nextOccurrence(toCore(series.rule), day, today);
      if (next === null || exhausted(series.rule, series.made)) {
        host.fail("conflict", `The rule of ${host.keyOf(task)} has no more dates.`, { details: { message_id: "recurrence-no-more", args: { task: host.keyOf(task) } } });
      }
      shiftDates(task, dayNumber(next) - dayNumber(day));
      series.made += 1;
      series.nextDate = aheadNext(series.rule, next, series.made);
      return answer(host, task, series, [taskChange(host, task, ["start_date", "due_date"]), seriesChange(host, series)]);
    },
    "recurrence.preview": (input) => {
      const rule = ruleFrom(host, input, host.today(), null);
      const limit = input.limit ?? PREVIEW_DEFAULT;
      if (limit < 1 || limit > PREVIEW_MAX) {
        host.fail("invalid", `List at most ${String(PREVIEW_MAX)} items.`, { details: { path: "limit", message_id: "list-limit", args: { max: PREVIEW_MAX } } });
      }
      return { rule, summary: summaryOf(rule), dates: previewDates(rule, limit) };
    },
    "recurrence.run": () => {
      const today = host.today();
      const created: { key: string; title: string }[] = [];
      const changes: EntityChange[] = [];
      const makingDay = (series: FakeSeries, day: string) => plusDays(day, -(series.rule.days_ahead ?? 0));
      for (const series of all(host).filter((each) => each.endedAt === null && each.nextDate !== null && each.rule.create === "ahead")) {
        let day = series.nextDate ?? today;
        if (makingDay(series, day) > today) {
          continue;
        }
        const rule = toCore(series.rule);
        for (let after = nextAfter(rule, day); after !== null && makingDay(series, after) <= today; after = nextAfter(rule, day)) {
          if (series.rule.count !== undefined && series.made + 1 >= series.rule.count) {
            break;
          }
          series.made += 1;
          day = after;
        }
        if (exhausted(series.rule, series.made)) {
          series.endedAt = host.now();
          series.nextDate = null;
          changes.push(seriesChange(host, series));
          continue;
        }
        const current = taskById(host, series.current);
        const made = makeOccurrence(host, series, day, current?.parentId ?? null, current?.projectId ?? series.projectId);
        if (made) {
          series.nextDate = aheadNext(series.rule, day, series.made);
          created.push({ key: host.keyOf(made), title: made.title });
          changes.push(seriesChange(host, series), taskChange(host, made, ["*"]));
        }
      }
      const output = { created: created.map((each) => host.fullTask(host.task(each.key))).map((task) => ({ id: task.id, key: task.key, title: task.title, status: task.status, closed: task.closed_at !== undefined })) };
      return changes.length === 0 ? output : { output, changes };
    },
  };
}
