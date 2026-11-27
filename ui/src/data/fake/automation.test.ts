// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import type { AutomationChange, AutomationList, AutomationRunList, AutomationTestResult, TaskList } from "../../generated/ops";
import { type TaskChange } from "../catalog";
import { FakeError, FakeWorkspace } from "./engine";
import { seed } from "./seed";

const TODAY = new Date(2026, 10, 23, 9, 0);

function workspace() {
  return new FakeWorkspace(seed(TODAY), () => TODAY);
}

// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- a typed view of an untyped answer
function call<T>(ws: FakeWorkspace, op: string, input: unknown = {}): T {
  return ws.call(op, input).output as T;
}

function task(ws: FakeWorkspace, key: string) {
  return call<TaskList>(ws, "task.get", { keys: [key], include: ["comments"] }).items[0];
}

describe("automation rules in the sample workspace", () => {
  it("act on a new task in the same step, as the rule", () => {
    const ws = workspace();
    call<AutomationChange>(ws, "automation.create", {
      project: "TM",
      name: "Triage",
      trigger: { on: "task_created" },
      condition: "title:crash",
      actions: [
        { kind: "set_fields", fields: { priority: "urgent" } },
        { kind: "add_label", label: "board" },
        { kind: "add_comment", text: "{key} triaged by {rule}" },
      ],
    });
    const quiet = call<TaskChange>(ws, "task.create", { project: "TM", title: "Calm" }).task.key;
    expect(task(ws, quiet)?.priority).toBe("none");
    const answer = ws.call("task.create", { project: "TM", title: "A crash" });
    const key = (answer.output as TaskChange).task.key;
    const made = task(ws, key);
    expect(made?.priority).toBe("urgent");
    expect(made?.labels.map((label) => label.name)).toEqual(["board"]);
    expect(made?.comments?.[0]?.body).toBe(`${key} triaged by Triage`);
    expect(answer.changes?.changes.some((change) => change.entity === "comment")).toBe(true);
    const runs = call<AutomationRunList>(ws, "automation.runs", { rule: "Triage", project: "TM" });
    expect(runs.items[0]?.result).toBe("done");
    expect(runs.items[0]?.group).toBe(answer.changes?.group);
    // One Undo takes back the task and what the rule did.
    ws.call("history.undo", {});
    expect(() => ws.call("task.get", { keys: [key] })).toThrow(FakeError);
  });

  it("log a failed run and keep none of its changes", () => {
    const ws = workspace();
    call(ws, "automation.create", {
      project: "TM",
      name: "Broken",
      trigger: { on: "task_created" },
      actions: [{ kind: "add_label", label: "board" }, { kind: "move_to_status", status: "Nowhere" }],
    });
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Fine" }).task.key;
    expect(task(ws, key)?.labels).toEqual([]);
    const list = call<AutomationList>(ws, "automation.list", { project: "TM" });
    expect(list.items[0]?.last_run?.result).toBe("failed");
    expect(list.items[0]?.last_run?.error?.details?.action).toBe(1);
  });

  it("never set a rule off by its own changes", () => {
    const ws = workspace();
    call(ws, "automation.create", { project: "TM", name: "Echo", trigger: { on: "comment_added" }, actions: [{ kind: "add_comment", text: "echo" }] });
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Talk" }).task.key;
    call(ws, "comment.add", { key, body: "hello" });
    expect(task(ws, key)?.comments?.map((comment) => comment.body)).toEqual(["hello", "echo"]);
  });

  it("try a rule without changing anything", () => {
    const ws = workspace();
    const key = call<TaskChange>(ws, "task.create", { project: "TM", title: "Try" }).task.key;
    const result = call<AutomationTestResult>(ws, "automation.test", { task: key, draft: { trigger: { on: "task_created" }, actions: [{ kind: "set_fields", fields: { priority: "high" } }] } });
    expect(result.ok).toBe(true);
    expect(result.changes).toEqual([{ entity: "task", task: key, field: "priority", before: "none", after: "high" }]);
    expect(task(ws, key)?.priority).toBe("none");
  });

  it("refuse a scheduled rule without a condition and a second rule with the same name", () => {
    const ws = workspace();
    const refused = (input: unknown) => {
      try {
        ws.call("automation.create", input);
      } catch (error: unknown) {
        if (error instanceof FakeError) {
          return error.error.details?.message_id;
        }
        throw error;
      }
      return undefined;
    };
    expect(refused({ project: "TM", name: "Nightly", trigger: { on: "schedule", every: "daily", time: "08:00" }, actions: [{ kind: "notify", text: "hi" }] })).toBe("automation-schedule-needs-condition");
    call(ws, "automation.create", { project: "TM", name: "Once", trigger: { on: "task_created" }, actions: [{ kind: "add_label", label: "board" }] });
    expect(refused({ project: "TM", name: "once", trigger: { on: "task_created" }, actions: [{ kind: "add_label", label: "board" }] })).toBe("automation-name-taken");
  });
});
