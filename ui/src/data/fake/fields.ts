// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Custom fields of the in-memory engine: their definitions, a task's
// values in the shapes the engine gives them, and the checks it makes.

import { type CustomField, type CustomFieldKind, type CustomFieldValue } from "../../generated/ops";
import { type FakeProject, type FakeState, type FakeTask } from "./state";

/** A stored field: of one project, or (`projectId` null) of every project. */
export interface FakeField {
  id: string;
  projectId: string | null;
  name: string;
  kind: CustomFieldKind;
  options: string[];
  default?: FieldValue | undefined;
  /** Task type names it shows for; empty for every type. */
  types: string[];
  required: boolean;
}

/** A value as the engine gives it: text, a number, yes/no, or options. */
export type FieldValue = string | number | boolean | string[];

/** An input was refused: the engine's code, message and details. */
export type Fail = (code: "invalid" | "not_found" | "conflict", message: string, extra?: { details?: Record<string, unknown> }) => never;

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();

/** Any input value as text. */
const textOf = (value: unknown): string => (typeof value === "string" ? value : typeof value === "number" || typeof value === "boolean" ? String(value) : JSON.stringify(value));

/** A record without one key. */
function without<V>(record: Readonly<Record<string, V>>, key: string): Record<string, V> {
  return Object.fromEntries(Object.entries(record).filter(([each]) => each !== key));
}

/** The name a field has in queries: folded, words joined by hyphens. */
export function queryName(name: string): string {
  return fold(name)
    .split(/[^\p{L}\p{N}_]+/u)
    .filter((word) => word !== "")
    .join("-");
}

export function fieldsOf(state: FakeState): FakeField[] {
  state.customFields ??= [];
  return state.customFields;
}

/** The fields a project's tasks have: those of every project, then its own. */
export function fieldsFor(state: FakeState, projectId: string | null): FakeField[] {
  const all = fieldsOf(state);
  if (projectId === null) {
    return [...all.filter((field) => field.projectId === null), ...all.filter((field) => field.projectId !== null)];
  }
  return [...all.filter((field) => field.projectId === null), ...all.filter((field) => field.projectId === projectId)];
}

export function fieldOut(state: FakeState, field: FakeField, projects: readonly FakeProject[]): CustomField {
  const project = projects.find((candidate) => candidate.id === field.projectId);
  return {
    id: field.id,
    name: field.name,
    query_name: `cf.${queryName(field.name)}`,
    kind: field.kind,
    ...(project ? { project: project.key } : {}),
    ...(field.options.length > 0 ? { options: field.options } : {}),
    ...(field.default !== undefined ? { default: field.default } : {}),
    ...(field.types.length > 0 ? { types: field.types } : {}),
    required: field.required,
    tasks: state.tasks.filter((task) => task.fields?.[field.id] !== undefined).length,
  };
}

/** The field `ref` names among `fields`: its ID, name or query name. */
export function findField(fields: readonly FakeField[], ref: string, fail: Fail, path = "field"): FakeField {
  const bare = ref.replace(/^cf\./i, "");
  const found = fields.find((field) => field.id === ref || fold(field.name) === fold(ref) || queryName(field.name) === fold(bare));
  if (!found) {
    fail("not_found", `There is no custom field "${ref}" here.`, { details: { path, message_id: "field-not-found", args: { name: ref } } });
  }
  return found;
}

/** Reads an input value for `field`; undefined clears it. */
export function parseValue(field: FakeField, value: unknown, path: string, fail: Fail): FieldValue | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }
  const bad = (id: string, message: string): never => fail("invalid", message, { details: { path, message_id: id, args: { text: textOf(value), field: field.name, choices: field.options.join(", ") } } });
  const option = (text: unknown) => field.options.find((candidate) => fold(candidate) === fold(textOf(text))) ?? bad("value-field-option", `"${textOf(text)}" is not an option of ${field.name}.`);
  switch (field.kind) {
    case "multi_select": {
      const list = Array.isArray(value) ? value : [value];
      const chosen = [...new Set(list.map(option))];
      return chosen.length === 0 ? undefined : field.options.filter((candidate) => chosen.includes(candidate));
    }
    case "select":
      return option(value);
    case "integer": {
      const number = Number(value);
      return Number.isInteger(number) && textOf(value).trim() !== "" ? number : bad("value-field-integer", `"${textOf(value)}" is not a whole number.`);
    }
    case "decimal": {
      const number = Number(value);
      return Number.isFinite(number) && textOf(value).trim() !== "" ? number : bad("value-field-number", `"${textOf(value)}" is not a number.`);
    }
    case "boolean": {
      if (typeof value === "boolean") {
        return value;
      }
      const text = textOf(value).toLowerCase();
      return ["yes", "true", "1", "on"].includes(text) ? true : ["no", "false", "0", "off"].includes(text) ? false : bad("value-field-boolean", `"${textOf(value)}" is not yes or no.`);
    }
    case "date":
      return /^\d{4}-\d{2}-\d{2}$/.test(textOf(value)) ? textOf(value) : bad("value-field-date", `"${textOf(value)}" is not a date.`);
    case "url":
      return /^[a-z][a-z0-9+.-]*:\S+$/i.test(textOf(value).trim()) ? textOf(value).trim() : bad("value-field-url", `"${textOf(value)}" is not a link.`);
    case "markdown":
    case "text": {
      const text = field.kind === "text" ? textOf(value).replace(/\s+/g, " ").trim() : textOf(value);
      return text.trim() === "" ? undefined : text;
    }
  }
}

/** Where a value goes when its task moves: the target's field that takes it and the value there, if any. */
interface Carried {
  field: FakeField;
  target?: { field: FakeField; value: FieldValue } | undefined;
}

const refuse: Fail = () => {
  throw new Error("refused");
};

/** `value` as `field` takes it, or undefined when it doesn't. */
function taken(field: FakeField, value: FieldValue): FieldValue | undefined {
  try {
    return parseValue(field, value, "fields", refuse);
  } catch {
    return undefined;
  }
}

/**
 * What moving `task` to project `projectId` does to its values: a field the
 * target has too keeps it; else it goes to the target's field of the same
 * query name and kind that takes it (options by name), or is left off.
 */
function carryPlan(state: FakeState, task: FakeTask, projectId: string): Carried[] {
  const target = fieldsFor(state, projectId);
  return fieldsFor(state, task.projectId).flatMap((field): Carried[] => {
    const value = task.fields?.[field.id];
    if (value === undefined) {
      return [];
    }
    const same = target.find((candidate) => candidate.id === field.id);
    if (same) {
      return [{ field, target: { field: same, value } }];
    }
    for (const candidate of target.filter((each) => queryName(each.name) === queryName(field.name) && each.kind === field.kind)) {
      const there = taken(candidate, value);
      if (there !== undefined) {
        return [{ field, target: { field: candidate, value: there } }];
      }
    }
    return [{ field }];
  });
}

/** For a move check: each field `task` has a value of, and the target's field that would hold it. */
export function moveMatches(state: FakeState, task: FakeTask, projectId: string): { name: string; target?: string }[] {
  return carryPlan(state, task, projectId).map(({ field, target }) => (target ? { name: field.name, target: target.field.name } : { name: field.name }));
}

/**
 * Moves `task`'s values to the fields of project `projectId` (call before
 * the task changes project). Returns the names of the fields left off.
 */
export function moveValues(state: FakeState, task: FakeTask, projectId: string): string[] {
  const values: Record<string, FieldValue> = {};
  const lost: string[] = [];
  for (const { field, target } of carryPlan(state, task, projectId)) {
    if (target === undefined) {
      lost.push(field.name);
    } else {
      values[target.field.id] = target.value;
    }
  }
  if (task.fields !== undefined) {
    task.fields = values;
  }
  return lost;
}

/** Whether `field` shows for tasks of the type named `type`. */
export function shownFor(field: FakeField, type: string): boolean {
  return field.types.length === 0 || field.types.some((name) => fold(name) === fold(type));
}

/**
 * Sets the values an input gives on a task; for a new task, defaults too,
 * and required fields need a value. Returns whether anything changed.
 */
export function applyValues(state: FakeState, task: FakeTask, type: string, input: Readonly<Record<string, unknown>> | undefined, creating: boolean, fail: Fail): boolean {
  const fields = fieldsFor(state, task.projectId);
  const given = new Set<string>();
  let changed = false;
  let values: Record<string, FieldValue> = { ...(task.fields ?? {}) };
  for (const [ref, raw] of Object.entries(input ?? {})) {
    const path = `fields.${ref}`;
    const field = findField(fields, ref, fail, path);
    given.add(field.id);
    const value = parseValue(field, raw, path, fail);
    if (value !== undefined && !shownFor(field, type)) {
      fail("invalid", `${field.name} is shown only for tasks of the types ${field.types.join(", ")}, not ${type}.`, { details: { path, message_id: "field-not-for-type", args: { field: field.name, types: field.types.join(", "), type } } });
    }
    if (value === undefined && field.required && shownFor(field, type)) {
      fail("invalid", `${field.name} is required; give it a value.`, { details: { path, message_id: "field-required", args: { field: field.name } } });
    }
    if (JSON.stringify(values[field.id]) !== JSON.stringify(value)) {
      changed = true;
    }
    values = value === undefined ? without(values, field.id) : { ...values, [field.id]: value };
  }
  if (creating) {
    for (const field of fields.filter((each) => !given.has(each.id) && shownFor(each, type))) {
      if (field.default !== undefined) {
        values[field.id] = field.default;
        changed = true;
      } else if (field.required) {
        fail("invalid", `${field.name} is required; give it a value.`, { details: { path: `fields.${field.name}`, message_id: "field-required", args: { field: field.name } } });
      }
    }
  }
  task.fields = values;
  return changed;
}

/** A task's values, in the fields' order. */
export function taskValues(state: FakeState, task: FakeTask): CustomFieldValue[] {
  return fieldsFor(state, task.projectId).flatMap((field) => {
    const value = task.fields?.[field.id];
    return value === undefined ? [] : [{ id: field.id, name: field.name, query_name: `cf.${queryName(field.name)}`, kind: field.kind, value }];
  });
}

/** A task's values by query name, as lists show them. */
export function listedValues(state: FakeState, task: FakeTask): Record<string, FieldValue> | undefined {
  const values = taskValues(state, task);
  return values.length === 0 ? undefined : Object.fromEntries(values.map((value) => [value.query_name.slice(3), value.value as FieldValue]));
}

/** The workspace's fields as queries see them, one per query name. */
export function queryFields(state: FakeState): { queryName: string; kind: CustomFieldKind; options: string[] }[] {
  const out: { queryName: string; kind: CustomFieldKind; options: string[] }[] = [];
  for (const field of fieldsFor(state, null)) {
    const name = queryName(field.name);
    const known = out.find((each) => each.queryName === name);
    if (known) {
      known.options = [...new Set([...known.options, ...field.options])];
    } else {
      out.push({ queryName: name, kind: field.kind, options: [...field.options] });
    }
  }
  return out;
}

const HAS_OPTIONS: readonly CustomFieldKind[] = ["select", "multi_select"];

function cleanOptions(kind: CustomFieldKind, options: readonly string[] | undefined, fail: Fail): string[] {
  const list = (options ?? []).map((option) => option.replace(/\s+/g, " ").trim());
  if (!HAS_OPTIONS.includes(kind)) {
    if (list.length > 0) {
      fail("invalid", `A field of kind ${kind} has no options.`, { details: { path: "options", message_id: "field-options-not-allowed", args: { kind } } });
    }
    return [];
  }
  if (list.length === 0) {
    fail("invalid", "A select field needs at least one option.", { details: { path: "options", message_id: "field-needs-options" } });
  }
  list.forEach((option, index) => {
    if (option === "" || list.findIndex((other) => fold(other) === fold(option)) !== index) {
      fail("invalid", `The option "${option}" is empty or listed twice.`, { details: { path: `options[${String(index)}]`, message_id: "field-option-twice", args: { option } } });
    }
  });
  return list;
}

function checkName(state: FakeState, projectId: string | null, name: string, kind: CustomFieldKind, fail: Fail, except?: string): string {
  const trimmed = name.replace(/\s+/g, " ").trim();
  if (queryName(trimmed) === "") {
    fail("invalid", `The field name "${name}" needs at least one letter or digit.`, { details: { path: "name", message_id: "value-field-query-name", args: { text: name } } });
  }
  const others = fieldsOf(state).filter((field) => field.id !== except);
  const clash = others.find((field) => (projectId === null || field.projectId === null || field.projectId === projectId) && (fold(field.name) === fold(trimmed) || queryName(field.name) === queryName(trimmed)));
  if (clash) {
    fail("conflict", `The custom field "${trimmed}" would clash with "${clash.name}".`, { details: { path: "name", message_id: "field-name-taken", args: { name: trimmed, other: clash.name } } });
  }
  const kindClash = others.find((field) => queryName(field.name) === queryName(trimmed) && field.kind !== kind);
  if (kindClash) {
    fail("conflict", `Another project has a custom field "${trimmed}" of kind ${kindClash.kind}.`, { details: { path: "kind", message_id: "field-kind-clash", args: { name: trimmed, kind, other_kind: kindClash.kind } } });
  }
  return trimmed;
}

/** A new field, as `field.create` makes it. */
export function createField(
  state: FakeState,
  input: { project?: FakeProject | null; name: string; kind: CustomFieldKind; options?: string[]; default?: unknown; types?: string[]; required?: boolean },
  id: string,
  fail: Fail,
): FakeField {
  const projectId = input.project?.id ?? null;
  const name = checkName(state, projectId, input.name, input.kind, fail);
  const field: FakeField = { id, projectId, name, kind: input.kind, options: cleanOptions(input.kind, input.options, fail), types: input.types ?? [], required: input.required ?? false };
  const fallback = parseValue(field, input.default, "default", fail);
  if (fallback !== undefined) {
    field.default = fallback;
  }
  fieldsOf(state).push(field);
  return field;
}

/** Changes a field as `field.update` does; returns how many tasks lost a value. */
export function updateField(
  state: FakeState,
  field: FakeField,
  input: { name?: string | null; rename_options?: { from: string; to: string }[]; options?: string[] | null; default?: unknown; types?: string[] | null; required?: boolean | null; clear?: ("default" | "types")[] },
  fail: Fail,
): number {
  if (input.name !== null && input.name !== undefined) {
    field.name = checkName(state, field.projectId, input.name, field.kind, fail, field.id);
  }
  let removed = 0;
  if ((input.rename_options ?? []).length > 0 || (input.options !== null && input.options !== undefined)) {
    let options = [...field.options];
    const renames = new Map<string, string>();
    for (const rename of input.rename_options ?? []) {
      const at = options.findIndex((option) => fold(option) === fold(rename.from));
      if (at < 0) {
        fail("invalid", `"${rename.from}" is not an option of the field.`, { details: { path: "rename_options", message_id: "field-option-unknown", args: { option: rename.from, choices: options.join(", ") } } });
      }
      renames.set(options[at] ?? "", rename.to.trim());
      options[at] = rename.to.trim();
    }
    if (input.options !== null && input.options !== undefined) {
      options = cleanOptions(field.kind, input.options, fail);
    }
    const rename = (value: string) => renames.get(value) ?? options.find((option) => fold(option) === fold(value)) ?? value;
    for (const task of state.tasks) {
      const value = task.fields?.[field.id];
      if (value === undefined || task.fields === undefined) {
        continue;
      }
      const list = (Array.isArray(value) ? value : [textOf(value)]).map(rename);
      const kept = list.filter((option) => options.includes(option));
      if (kept.length < list.length) {
        removed += 1;
      }
      task.fields = kept.length === 0 ? without(task.fields, field.id) : { ...task.fields, [field.id]: field.kind === "multi_select" ? options.filter((option) => kept.includes(option)) : (kept[0] ?? "") };
    }
    field.options = options;
  }
  if (input.default !== null && input.default !== undefined) {
    const value = parseValue(field, input.default, "default", fail);
    field.default = value;
  }
  if (input.clear?.includes("default") === true) {
    field.default = undefined;
  }
  if (input.types !== null && input.types !== undefined) {
    field.types = input.types;
  }
  if (input.clear?.includes("types") === true) {
    field.types = [];
  }
  if (input.required !== null && input.required !== undefined) {
    field.required = input.required;
  }
  return removed;
}

/** Deletes a field and its values; returns how many tasks had one. */
export function deleteField(state: FakeState, field: FakeField): number {
  let count = 0;
  for (const task of state.tasks) {
    if (task.fields?.[field.id] !== undefined) {
      task.fields = without(task.fields, field.id);
      count += 1;
    }
  }
  state.customFields = fieldsOf(state).filter((each) => each !== field);
  return count;
}

/** Moves a field before or after another of its project, or to an end. */
export function placeField(state: FakeState, field: FakeField, place: { before?: string | null; after?: string | null; at?: "top" | "bottom" | null }, fail: Fail): void {
  const group = fieldsOf(state).filter((each) => each.projectId === field.projectId && each !== field);
  let index = group.length;
  if (place.before !== null && place.before !== undefined) {
    index = group.indexOf(findField(group, place.before, fail, "before"));
  } else if (place.after !== null && place.after !== undefined) {
    index = group.indexOf(findField(group, place.after, fail, "after")) + 1;
  } else if (place.at === "top") {
    index = 0;
  } else if (place.at !== "bottom") {
    return;
  }
  group.splice(index, 0, field);
  state.customFields = [...fieldsOf(state).filter((each) => each.projectId !== field.projectId), ...group];
}
