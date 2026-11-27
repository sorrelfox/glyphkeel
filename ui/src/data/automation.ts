// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import type { ActionKind, AutomationAction, AutomationTrigger, RelationKind, ScheduleEvery, TriggerKind, Weekday } from "../generated/ops";
import type { Translate } from "../i18n/I18n";

/** Every trigger, in the order the editor offers them. */
export const TRIGGERS = ["task_created", "field_changed", "moved_to_status", "comment_added", "due_date_reached", "overdue", "schedule"] as const satisfies readonly TriggerKind[];

/** Every action, in the order the editor offers them. */
export const ACTIONS = ["set_fields", "add_label", "remove_label", "move_to_status", "add_comment", "create_subtask", "create_linked_task", "notify"] as const satisfies readonly ActionKind[];

/** The task fields a `field_changed` trigger can watch, as the engine names them. */
export const WATCHABLE = ["title", "description", "status", "type", "priority", "labels", "start_date", "due_date", "estimate", "progress", "fix_version", "parent"] as const;

/** The task fields a `set_fields` action can set, as the engine names them. */
export const SETTABLE = ["priority", "due_date", "start_date", "estimate", "progress", "type", "title", "description", "fix_version"] as const;
export type Settable = (typeof SETTABLE)[number];

/** The fields a `set_fields` action may clear. */
export const CLEARABLE: readonly Settable[] = ["description", "start_date", "due_date", "estimate", "progress", "fix_version"];

export const EVERY = ["daily", "weekly"] as const satisfies readonly ScheduleEvery[];
export const WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const satisfies readonly Weekday[];

/** The links a linked task can have, as read from the task the rule acts on. */
export const LINKS = ["relates", "blocks", "blocked_by", "duplicates", "duplicated_by", "clones", "cloned_by"] as const satisfies readonly RelationKind[];

/** The placeholders texts may hold. */
export const PLACEHOLDERS = ["key", "title", "status", "priority", "due_date", "project", "rule", "actor", "today"] as const;

/** A rule as the editor holds it while it is being written. */
export interface Draft {
  name: string;
  /** The project's key, or empty for the whole workspace. */
  project: string;
  enabled: boolean;
  trigger: AutomationTrigger;
  condition: string;
  actions: AutomationAction[];
}

/** A new rule's starting point. */
export function newDraft(project: string): Draft {
  return { name: "", project, enabled: true, trigger: { on: "task_created" }, condition: "", actions: [newAction("add_label")] };
}

/** An action of `kind` with nothing filled in yet. */
export function newAction(kind: ActionKind): AutomationAction {
  switch (kind) {
    case "set_fields":
      return { kind, fields: { priority: "high" } };
    case "create_linked_task":
      return { kind, text: "", link: "relates" };
    default:
      return { kind };
  }
}

/** A trigger of another kind, keeping what still fits. */
export function withTriggerKind(trigger: AutomationTrigger, on: TriggerKind): AutomationTrigger {
  switch (on) {
    case "field_changed":
      return { on, fields: trigger.fields ?? [] };
    case "moved_to_status":
      return { on, ...(trigger.status === undefined ? {} : { status: trigger.status }) };
    case "schedule":
      return { on, every: trigger.every ?? "daily", time: trigger.time ?? "09:00", ...(trigger.every === "weekly" ? { weekday: trigger.weekday ?? "mon" } : {}) };
    default:
      return { on };
  }
}

/** The trigger as it is sent: a weekly schedule names its day, a daily one doesn't. */
export function cleanTrigger(trigger: AutomationTrigger): AutomationTrigger {
  if (trigger.on !== "schedule") {
    return trigger;
  }
  const { weekday, ...rest } = trigger;
  return rest.every === "weekly" ? { ...rest, weekday: weekday ?? "mon" } : rest;
}

/** The action as it is sent: empty texts and templates left out. */
export function cleanAction(action: AutomationAction): AutomationAction {
  const out: AutomationAction = { kind: action.kind };
  if (action.fields !== undefined && Object.keys(action.fields).length > 0) {
    out.fields = action.fields;
  }
  for (const key of ["label", "status", "text", "template"] as const) {
    const value = action[key]?.trim();
    if (value !== undefined && value !== "") {
      out[key] = value;
    }
  }
  if (action.link !== undefined && action.kind === "create_linked_task") {
    out.link = action.link;
  }
  return out;
}

/** The one field a `set_fields` action sets in the editor, with its value. */
export function setField(action: AutomationAction): { field: Settable; value: unknown } {
  const [field, value] = Object.entries(action.fields ?? {})[0] ?? ["priority", "high"];
  return { field: (SETTABLE as readonly string[]).includes(field) ? (field as Settable) : "priority", value };
}

/** How the page names things for a summary: a list in the person's language, a weekday, a custom field. */
export interface Naming {
  list: (items: readonly string[]) => string;
  weekday: (day: Weekday) => string;
  custom?: ReadonlyMap<string, string>;
}

/** What a trigger is, in a few words. */
export function triggerSummary(t: Translate, trigger: AutomationTrigger, naming: Naming): string {
  switch (trigger.on) {
    case "field_changed": {
      const fields = trigger.fields ?? [];
      return fields.length === 0 ? t("automation-when-any-field") : t("automation-when-fields", { fields: naming.list(fields.map((field) => fieldName(t, field, naming.custom))), count: fields.length });
    }
    case "moved_to_status":
      return trigger.status === undefined ? t("automation-when-moved-any") : t("automation-when-moved", { status: trigger.status });
    case "schedule":
      return trigger.every === "weekly" ? t("automation-when-weekly", { day: naming.weekday(trigger.weekday ?? "mon"), time: trigger.time ?? "" }) : t("automation-when-daily", { time: trigger.time ?? "" });
    default:
      return t(`automation-trigger-${trigger.on}`);
  }
}

/** A field's name as people read it; a custom field (`cf:<id>`) by the name given. */
export function fieldName(t: Translate, field: string, custom: ReadonlyMap<string, string> = new Map()): string {
  if (field.startsWith("cf:")) {
    return custom.get(field.slice(3)) ?? t("rule-field-custom");
  }
  return t(`automation-field-${field}`);
}

/** What a rule does, for its row. */
export function actionsSummary(t: Translate, actions: readonly AutomationAction[], naming: Naming): string {
  return naming.list(actions.map((action) => t(`automation-action-${action.kind}`)));
}

/** The day of 2024-01-01 (a Monday) plus `offset`, for naming weekdays with the locale's formatter. */
export function weekdayDate(day: Weekday): Date {
  return new Date(Date.UTC(2024, 0, 1 + WEEKDAYS.indexOf(day), 12));
}
