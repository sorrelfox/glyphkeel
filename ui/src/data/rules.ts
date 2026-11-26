// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Status rules and rules for agents, as the page shows them before the
// person acts: which statuses a task can't enter yet and why, and which
// fields can't change in its status. The engine enforces the rules; this
// only reads them, so the page never offers what the engine would refuse.

import type { AgentRules, Card, Priority, Project, StatusRules, Task } from "../generated/ops";

/** The prefix of a custom field in a rule, followed by the field's ID. */
export const CUSTOM_FIELD_PREFIX = "cf:";

/** The fields a status may require before a task enters it. */
export const REQUIRABLE_FIELDS = ["description", "priority", "start_date", "due_date", "estimate", "labels", "parent", "fix_version"] as const;

/** The fields a status may make read-only, and agents may be kept from. */
export const LOCKABLE_FIELDS = ["title", "description", "type", "priority", "start_date", "due_date", "estimate", "progress", "labels", "parent", "fix_version"] as const;

/** The fields a status may clear when a task enters it. */
export const CLEARABLE_FIELDS = ["start_date", "due_date", "estimate", "progress", "fix_version"] as const;

/** A built-in field a rule can name. */
export type RuleField = (typeof LOCKABLE_FIELDS)[number];

/** The rules of status `statusId` in `project`; none when it has none. */
export function rulesOf(project: Project | undefined, statusId: string): StatusRules {
  return project?.statuses.find((status) => status.id === statusId)?.rules ?? {};
}

/** The project's rules for agents; none when it has none. */
export function agentRulesOf(project: Project | undefined): AgentRules {
  return project?.agent_rules ?? {};
}

/** Whether `rules` hold no rule at all. */
export function isEmpty(rules: StatusRules): boolean {
  return (
    (rules.set ?? []).length === 0 &&
    (rules.clear ?? []).length === 0 &&
    (rules.required ?? []).length === 0 &&
    (rules.read_only ?? []).length === 0 &&
    rules.stop_timer !== true &&
    rules.release_claim !== true &&
    rules.agents_may_not_enter !== true
  );
}

/**
 * What the page knows of a task's fields. A field left undefined is not
 * known here (a board card has no description), and never counts as missing.
 */
export interface KnownFields {
  description?: string | undefined;
  priority?: Priority | undefined;
  start_date?: string | null | undefined;
  due_date?: string | null | undefined;
  estimate_minutes?: number | null | undefined;
  labels?: readonly unknown[] | undefined;
  parent?: string | null | undefined;
  fix_version?: unknown;
}

/**
 * The fields `rules` require that the task has no value in, in the rules'
 * order, after what the status sets on the way in. Custom fields and
 * fields the page doesn't know are left to the engine.
 */
export function missingFields(rules: StatusRules, task: KnownFields): RuleField[] {
  const setOnEntry = new Set((rules.set ?? []).map((set) => set.field));
  const out: RuleField[] = [];
  for (const field of rules.required ?? []) {
    if (setOnEntry.has(field)) {
      continue;
    }
    const has = presence(field, task);
    if (has === false) {
      out.push(field as RuleField);
    }
  }
  return out;
}

/** Whether the task has a value in `field`; undefined when the page can't tell. */
function presence(field: string, task: KnownFields): boolean | undefined {
  switch (field) {
    case "description":
      return task.description === undefined ? undefined : task.description.trim() !== "";
    case "priority":
      return task.priority === undefined ? undefined : task.priority !== "none";
    case "start_date":
      return task.start_date === undefined ? undefined : task.start_date !== null;
    case "due_date":
      return task.due_date === undefined ? undefined : task.due_date !== null;
    case "estimate":
      return task.estimate_minutes === undefined ? undefined : task.estimate_minutes !== null;
    case "labels":
      return task.labels === undefined ? undefined : task.labels.length > 0;
    case "parent":
      return task.parent === undefined ? undefined : task.parent !== null;
    case "fix_version":
      return task.fix_version === undefined ? undefined : task.fix_version !== null;
    default:
      return undefined;
  }
}

/** What a board card or a list row tells of its task's fields: all but the description. */
export function fieldsOfCard(card: Pick<Card, "priority" | "start_date" | "due_date" | "estimate_minutes" | "labels" | "parent">): KnownFields {
  return {
    priority: card.priority,
    start_date: card.start_date ?? null,
    due_date: card.due_date ?? null,
    estimate_minutes: card.estimate_minutes ?? null,
    labels: card.labels,
    parent: card.parent ?? null,
  };
}

/** What a loaded task tells of its fields. */
export function fieldsOfTask(task: Task): KnownFields {
  return {
    description: task.description,
    priority: task.priority,
    start_date: task.start_date ?? null,
    due_date: task.due_date ?? null,
    estimate_minutes: task.estimate_minutes ?? null,
    labels: task.labels,
    parent: task.parent ?? null,
    fix_version: task.fix_version ?? null,
  };
}

/** The built-in fields that can't change while a task is in status `statusId`. */
export function readOnlyFields(project: Project | undefined, statusId: string): Set<string> {
  return new Set(rulesOf(project, statusId).read_only ?? []);
}

/** Whether `field` is a custom field. */
export function isCustom(field: string): boolean {
  return field.startsWith(CUSTOM_FIELD_PREFIX);
}

/** How a rule names custom field `id`. */
export function customRuleField(id: string): string {
  return `${CUSTOM_FIELD_PREFIX}${id}`;
}

/** The ID of the custom field a rule names; none for a built-in field. */
export function customFieldId(field: string): string | undefined {
  return isCustom(field) ? field.slice(CUSTOM_FIELD_PREFIX.length) : undefined;
}
