// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type CustomFieldChange, type CustomFieldList, type TaskChange, type TaskList, type TaskSearchResult } from "../../generated/ops";
import { FakeError, FakeWorkspace } from "./engine";
import { queryName } from "./fields";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function failure(run: () => unknown) {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

describe("custom fields in the sample workspace", () => {
  it("names fields in queries by their words joined by hyphens", () => {
    expect(queryName("Story points")).toBe("story-points");
    expect(queryName("Área / Zone")).toBe("área-zone");
  });

  it("keeps values on tasks, lists them and finds tasks by them", () => {
    const ws = workspace();
    const created = call<CustomFieldChange>(ws, "field.create", { project: "TM", name: "Story points", kind: "integer" });
    expect(created.field?.query_name).toBe("cf.story-points");
    call(ws, "field.create", { project: "TM", name: "Area", kind: "select", options: ["Front end", "Back end"], default: "front end" });
    const task = call<TaskChange>(ws, "task.create", { project: "TM", title: "Sized", fields: { "Story points": "5" } }).task;
    expect(task.fields?.map((value) => [value.name, value.value])).toEqual([
      ["Story points", 5],
      ["Area", "Front end"],
    ]);
    const changed = call<TaskChange>(ws, "task.update", { key: task.key, fields: { area: "Back end", "cf.story-points": null } }).task;
    expect(changed.fields?.map((value) => value.value)).toEqual(["Back end"]);
    expect(call<TaskList>(ws, "task.get", { keys: [task.key] }).items[0]?.fields?.[0]?.name).toBe("Area");
    const found = call<TaskSearchResult>(ws, "task.search", { query: 'cf.area:"back end"' });
    expect(found.items.map((each) => each.key)).toEqual([task.key]);
    expect(found.items[0]?.fields).toEqual({ area: "Back end" });
    expect(failure(() => call(ws, "task.update", { key: task.key, fields: { area: "Middle" } })).details?.message_id).toBe("value-field-option");
    const list = call<CustomFieldList>(ws, "field.list", { project: "TM" });
    expect(list.fields.map((field) => [field.name, field.tasks])).toEqual([
      ["Story points", 0],
      ["Area", 1],
    ]);
  });

  it("renames options on the tasks too and deletes a field with its values", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "TM", name: "Area", kind: "select", options: ["UI", "Back end"] });
    const task = call<TaskChange>(ws, "task.create", { project: "TM", title: "Styled", fields: { Area: "UI" } }).task;
    call(ws, "field.update", { project: "TM", field: "Area", rename_options: [{ from: "ui", to: "Front end" }] });
    expect(call<TaskList>(ws, "task.get", { keys: [task.key] }).items[0]?.fields?.[0]?.value).toBe("Front end");
    const removed = call<CustomFieldChange>(ws, "field.delete", { project: "TM", field: "Area", expected_task_count: 1 });
    expect(removed.values_removed).toBe(1);
    expect(removed.field).toBeUndefined();
    expect(call<TaskList>(ws, "task.get", { keys: [task.key] }).items[0]?.fields).toBeUndefined();
  });

  it("asks a required field of new tasks", () => {
    const ws = workspace();
    call(ws, "field.create", { project: "TM", name: "Customer", kind: "text", required: true });
    expect(failure(() => call(ws, "task.create", { project: "TM", title: "Without" })).details?.message_id).toBe("field-required");
    expect(call<TaskChange>(ws, "task.create", { project: "TM", title: "With", fields: { Customer: "ACME" } }).task.fields?.[0]?.value).toBe("ACME");
  });
});
