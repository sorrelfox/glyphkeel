// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Status rules in the fake engine: the same refusals and actions as the
// engine's, for the person (the fake has no agents moving tasks), with the
// engine's message IDs so the page shows them translated.

import type { OpError, StatusRules } from "../../generated/ops";
import { customFieldId, missingFields } from "../rules";
import { shownFor, type FakeField, type FieldValue } from "./fields";
import type { FakeProject, FakeTask } from "./state";

type Fail = (code: OpError["code"], message: string, extra?: { hint?: string; details?: Record<string, unknown> }) => never;

/** The rules of a status of the fake project. */
export function fakeRules(project: FakeProject, statusId: string): StatusRules {
  return project.statuses.find((status) => status.id === statusId)?.rules ?? {};
}

/** Refuses a change to a field that is read-only in the task's status. */
export function checkReadOnly(project: FakeProject, task: FakeTask, key: string, changed: readonly string[], fail: Fail): void {
  const rules = fakeRules(project, task.statusId);
  const field = (rules.read_only ?? []).find((candidate) => changed.includes(candidate));
  if (field === undefined) {
    return;
  }
  const status = project.statuses.find((candidate) => candidate.id === task.statusId)?.name ?? "";
  fail("forbidden", `The ${field} of ${key} can't change while it is in ${status}.`, {
    hint: "Move the task to another status first, or change the rules of the status in the project's settings.",
    details: { message_id: "rule-read-only", args: { field, name: field, task: key, status }, hint_id: "rule-read-only-hint", path: field },
  });
}

/**
 * A task enters status `statusId`: its required fields (counting what the
 * status sets on the way in), then what the status sets or clears.
 * Returns the fields it changed.
 */
export function enter(project: FakeProject, task: FakeTask, key: string, statusId: string, today: string, fail: Fail, customFields: readonly FakeField[] = [], type = ""): string[] {
  const rules = fakeRules(project, statusId);
  const fieldOf = (rule: string) => {
    const id = customFieldId(rule);
    return id === undefined ? undefined : customFields.find((field) => field.id === id);
  };
  const setOnEntry = new Set((rules.set ?? []).map((set) => set.field));
  // Checked first, counting what the status sets, so a refusal changes nothing.
  const missing: string[] = missingFields(rules, {
    description: task.description,
    priority: task.priority,
    start_date: task.startDate,
    due_date: task.dueDate,
    estimate_minutes: task.estimateMin,
    labels: task.labelIds,
    parent: task.parentId,
    fix_version: task.fixVersionId ?? null,
  });
  for (const rule of rules.required ?? []) {
    const field = fieldOf(rule);
    if (field !== undefined && !setOnEntry.has(rule) && task.fields?.[field.id] === undefined) {
      missing.push(rule);
    }
  }
  const first = missing[0];
  if (first !== undefined) {
    const status = project.statuses.find((candidate) => candidate.id === statusId)?.name ?? "";
    const more = missing.length - 1;
    const custom = fieldOf(first);
    const args = custom === undefined ? { field: first, name: first, task: key, status } : { field: "custom", name: custom.name, task: key, status };
    fail("invalid", `${key} needs a value in ${args.name} before it can enter ${status}.`, {
      hint: "Fill it in first; the project's settings list the rules of each status.",
      details: { message_id: "rule-required", args, hint_id: "rule-required-hint", hint_args: { more }, path: first, missing },
    });
  }
  const changed: string[] = [];
  for (const field of rules.clear ?? []) {
    if (field === "start_date" && task.startDate !== null) {
      task.startDate = null;
      changed.push(field);
    } else if (field === "due_date" && task.dueDate !== null) {
      task.dueDate = null;
      changed.push(field);
    } else if (field === "estimate" && task.estimateMin !== null) {
      task.estimateMin = null;
      changed.push(field);
    } else if (field === "fix_version" && (task.fixVersionId ?? null) !== null) {
      task.fixVersionId = null;
      changed.push(field);
    } else {
      const custom = fieldOf(field);
      if (custom !== undefined && !custom.required && shownFor(custom, type) && task.fields?.[custom.id] !== undefined) {
        task.fields = Object.fromEntries(Object.entries(task.fields).filter(([id]) => id !== custom.id));
        changed.push("fields");
      }
    }
  }
  for (const set of rules.set ?? []) {
    const custom = fieldOf(set.field);
    if (custom !== undefined) {
      const value = ruleValue(custom, set.value, today);
      if (value !== undefined && shownFor(custom, type) && (task.fields?.[custom.id] === undefined || set.only_if_empty !== true)) {
        task.fields = { ...(task.fields ?? {}), [custom.id]: value };
        changed.push("fields");
      }
      continue;
    }
    if (set.field === "start_date" && (task.startDate === null || set.only_if_empty !== true) && (task.dueDate === null || today <= task.dueDate)) {
      task.startDate = today;
      changed.push("start_date");
    } else if (set.field === "due_date" && (task.dueDate === null || set.only_if_empty !== true) && (task.startDate === null || task.startDate <= today)) {
      task.dueDate = today;
      changed.push("due_date");
    } else if (set.field === "priority" && (task.priority === "none" || set.only_if_empty !== true)) {
      task.priority = set.value as FakeTask["priority"];
      changed.push("priority");
    }
  }
  return changed;
}

/** The value a rule sets in a custom field, read as the field reads typed text; none when it doesn't fit. */
function ruleValue(field: FakeField, text: string, today: string): FieldValue | undefined {
  switch (field.kind) {
    case "date":
      return text === "today" ? today : /^\d{4}-\d{2}-\d{2}$/.test(text) ? text : undefined;
    case "integer":
      return /^-?\d+$/.test(text.trim()) ? Number(text) : undefined;
    case "decimal":
      return Number.isFinite(Number(text)) && text.trim() !== "" ? Number(text) : undefined;
    case "boolean":
      return text === "true" ? true : text === "false" ? false : undefined;
    case "select":
      return field.options.find((option) => option.toLowerCase() === text.toLowerCase());
    case "multi_select": {
      const option = field.options.find((each) => each.toLowerCase() === text.toLowerCase());
      return option === undefined ? undefined : [option];
    }
    default:
      return text.trim() === "" ? undefined : text;
  }
}
