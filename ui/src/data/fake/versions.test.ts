// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type Task, type VersionChange, type VersionList } from "../../generated/ops";
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

function code(run: () => unknown): string {
  try {
    run();
  } catch (error: unknown) {
    if (error instanceof FakeError) {
      return error.error.code;
    }
    throw error;
  }
  throw new Error("expected the call to fail");
}

describe("the sample workspace's versions", () => {
  it("lists Task Manager's versions by due date, shared ones in its sub-projects", () => {
    const ws = workspace();
    const tm = call<VersionList>(ws, "version.list", { project: "TM", include_tasks: true });
    expect(tm.items.map((v) => [v.name, v.status])).toEqual([
      ["0.9", "closed"],
      ["1.0", "open"],
      ["1.1", "open"],
    ]);
    expect(tm.items[1]?.progress.total).toBeGreaterThan(0);
    expect(tm.items[1]?.tasks?.length).toBe(tm.items[1]?.progress.total);
    const cli = call<VersionList>(ws, "version.list", { project: "CLI" });
    expect(cli.items.map((v) => [v.name, v.inherited ?? false])).toEqual([["1.0", true]]);
  });

  it("plans a task for an open version only, and undoes a close that moved its open tasks", () => {
    const ws = workspace();
    const task = call<{ task: Task }>(ws, "task.create", { project: "TM", title: "Next", fix_version: "1.0" }).task;
    expect(task.fix_version?.name).toBe("1.0");
    expect(code(() => ws.call("task.update", { key: task.key, fix_version: "0.9" }))).toBe("conflict");
    const shared = call<{ task: Task }>(ws, "task.create", { project: "CLI", title: "Shared", fix_version: "1.0" }).task;
    expect(code(() => ws.call("task.update", { key: shared.key, fix_version: "1.1" }))).toBe("not_found");
    // 1.1 isn't shared with CLI, so the CLI task can't move there.
    expect(code(() => ws.call("version.update", { version: "1.0", project: "TM", status: "closed", move_open_tasks_to: "1.1" }))).toBe("conflict");
    expect(call<VersionList>(ws, "version.list", { project: "TM", status: ["open"] }).items.map((v) => v.name)).toEqual(["1.0", "1.1"]);
    ws.call("task.update", { key: shared.key, clear: ["fix_version"] });

    const closed = call<VersionChange>(ws, "version.update", { version: "1.0", project: "TM", status: "closed", move_open_tasks_to: "1.1" });
    expect(closed.version.status).toBe("closed");
    expect(closed.tasks_changed).toContain(task.key);
    ws.call("history.undo", {});
    const again = call<{ items: Task[] }>(ws, "task.get", { keys: [task.key] }).items[0];
    expect(again?.fix_version?.name).toBe("1.0");
  });

  it("deletes a version and plans its tasks for none", () => {
    const ws = workspace();
    const made = call<VersionChange>(ws, "version.create", { project: "PER", name: "Summer" });
    const task = call<{ task: Task }>(ws, "task.create", { project: "PER", title: "Trip", fix_version: made.version.id }).task;
    const deleted = call<VersionChange>(ws, "version.delete", { version: "Summer", project: "PER" });
    expect(deleted.deleted).toBe(true);
    expect(deleted.tasks_changed).toEqual([task.key]);
    expect(call<{ items: Task[] }>(ws, "task.get", { keys: [task.key] }).items[0]?.fix_version).toBeUndefined();
    expect(code(() => ws.call("version.create", { project: "TM", name: "1.0" }))).toBe("conflict");
  });
});
