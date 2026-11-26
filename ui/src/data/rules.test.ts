// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { FakeError, FakeWorkspace } from "./fake/engine";
import { seed } from "./fake/seed";
import type { CustomFieldChange, Project, ProjectChange, TaskChange, TaskList } from "../generated/ops";
import { customFieldId, customRuleField, fieldsOfCard, isEmpty, missingFields, readOnlyFields, rulesOf } from "./rules";

const TODAY = new Date(2026, 10, 23, 9, 0);

describe("status rules as the page reads them", () => {
  it("names the required fields a task has no value in, after what the status sets", () => {
    const rules = { required: ["estimate", "start_date", "labels", "description"], set: [{ field: "start_date", value: "today" }] };
    const fields = fieldsOfCard({ priority: "none", labels: [] });
    // A card tells nothing of the description, so it never counts as missing there.
    expect(missingFields(rules, fields)).toEqual(["estimate", "labels"]);
    expect(missingFields(rules, { ...fields, estimate_minutes: 30, labels: ["x"], description: " " })).toEqual(["description"]);
    expect(missingFields({}, fields)).toEqual([]);
  });

  it("knows a task's version, and names custom fields by their ID", () => {
    const rules = { required: ["fix_version", customRuleField("f1")] };
    // A card doesn't tell its version; the engine decides then.
    expect(missingFields(rules, fieldsOfCard({ priority: "none", labels: [] }))).toEqual([]);
    expect(missingFields(rules, { fix_version: null })).toEqual(["fix_version"]);
    expect(missingFields(rules, { fix_version: { id: "v", name: "1.0" } })).toEqual([]);
    expect(customFieldId("cf:f1")).toBe("f1");
    expect(customFieldId("estimate")).toBeUndefined();
  });

  it("reads a status's rules from its project", () => {
    const project = { statuses: [{ id: "s1", rules: { read_only: ["title"] } }, { id: "s2" }] } as unknown as Project;
    expect(readOnlyFields(project, "s1")).toEqual(new Set(["title"]));
    expect(isEmpty(rulesOf(project, "s2"))).toBe(true);
    expect(isEmpty({ stop_timer: false, required: [] })).toBe(true);
    expect(isEmpty({ agents_may_not_enter: true })).toBe(false);
  });
});

describe("status rules in the sample workspace", () => {
  // Answers come back untyped; the tests read them as the generated client types them.
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
  const call = <T>(ws: FakeWorkspace, op: string, input: unknown): T => ws.call(op, input).output as T;
  const failure = (run: () => unknown) => {
    try {
      run();
    } catch (error: unknown) {
      if (error instanceof FakeError) {
        return error.error;
      }
      throw error;
    }
    throw new Error("expected the call to fail");
  };

  it("refuses a status whose required field is empty, and locks read-only fields", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const changed = call<ProjectChange>(ws, "rules.update", {
      project: "PER",
      statuses: [{ status: "Done", rules: { required: ["estimate"], read_only: ["title"], set: [{ field: "start_date", value: "today", only_if_empty: true }] } }],
    });
    const done = changed.project.statuses.find((status) => status.name === "Done");
    expect(done?.rules?.required).toEqual(["estimate"]);
    const created = call<TaskChange>(ws, "task.create", { project: "PER", title: "Rules" });
    const key = created.task.key;
    const refused = failure(() => ws.call("task.move", { key, status: "Done" }));
    expect(refused.code).toBe("invalid");
    expect(refused.details?.message_id).toBe("rule-required");
    call(ws, "task.update", { key, estimate: "1h" });
    call(ws, "task.move", { key, status: "Done" });
    const task = call<TaskList>(ws, "task.get", { keys: [key] }).items[0];
    expect(task?.start_date).toBe("2026-11-23");
    expect(failure(() => ws.call("task.update", { key, title: "Renamed" })).details?.message_id).toBe("rule-read-only");
    call(ws, "task.update", { key, priority: "high" });
  });

  it("sets, clears and requires custom fields and the version on entering a status", () => {
    const ws = new FakeWorkspace(seed(TODAY), () => TODAY);
    const points = call<CustomFieldChange>(ws, "field.create", { project: "PER", name: "Points", kind: "integer" }).field;
    const area = call<CustomFieldChange>(ws, "field.create", { project: "PER", name: "Area", kind: "select", options: ["Front end", "Back end"] }).field;
    if (points === undefined || area === undefined) {
      throw new Error("the fields were not created");
    }
    call(ws, "rules.update", {
      project: "PER",
      statuses: [
        { status: "Done", rules: { required: [customRuleField(points.id)], set: [{ field: customRuleField(area.id), value: "back end" }], read_only: [customRuleField(points.id), "fix_version"] } },
        { status: "To do", rules: { clear: [customRuleField(area.id)] } },
      ],
    });
    const key = call<TaskChange>(ws, "task.create", { project: "PER", title: "Fields" }).task.key;
    const refused = failure(() => ws.call("task.move", { key, status: "Done" }));
    expect(refused.details?.message_id).toBe("rule-required");
    expect(refused.details?.args).toMatchObject({ field: "custom", name: "Points" });
    call(ws, "task.update", { key, fields: { Points: 3 } });
    call(ws, "task.move", { key, status: "Done" });
    const done = call<TaskList>(ws, "task.get", { keys: [key] }).items[0];
    expect(done?.fields?.find((value) => value.id === area.id)?.value).toBe("Back end");
    expect(failure(() => ws.call("task.update", { key, fields: { Points: 5 } })).details?.message_id).toBe("rule-read-only");
    call(ws, "task.move", { key, status: "To do" });
    const back = call<TaskList>(ws, "task.get", { keys: [key] }).items[0];
    expect(back?.fields?.find((value) => value.id === area.id)).toBeUndefined();
  });
});
