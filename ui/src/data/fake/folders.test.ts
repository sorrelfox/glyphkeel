// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type FolderChange, type FolderList, type ProjectList } from "../../generated/ops";
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

/** The top-level active projects with their folder's name, in tree order. */
function tree(ws: FakeWorkspace): string[] {
  const folders = call<FolderList>(ws, "folder.list").items;
  return call<ProjectList>(ws, "project.list")
    .items.filter((project) => project.depth === 0)
    .map((project) => `${project.key}:${folders.find((folder) => folder.id === project.folder)?.name ?? ""}`);
}

describe("project folders in the fake engine", () => {
  it("are made, ordered and renamed, with names unique", () => {
    const ws = workspace();
    call(ws, "folder.create", { name: "Work", hue: "teal" });
    call(ws, "folder.create", { name: "Home", at: "top" });
    expect(call<FolderList>(ws, "folder.list").items.map((folder) => folder.name)).toEqual(["Home", "Work"]);
    expect(code(() => ws.call("folder.create", { name: "work" }))).toBe("conflict");
    call(ws, "folder.update", { folder: "Home", after: "Work", name: "Personal stuff" });
    expect(call<FolderList>(ws, "folder.list").items.map((folder) => folder.name)).toEqual(["Work", "Personal stuff"]);
    ws.call("history.undo", {});
    expect(call<FolderList>(ws, "folder.list").items.map((folder) => folder.name)).toEqual(["Home", "Work"]);
  });

  it("take top-level projects with their sub-projects, keep their order and let go of them when deleted", () => {
    const ws = workspace();
    call(ws, "folder.create", { name: "Work" });
    const moved = call<FolderChange>(ws, "folder.move", { project: "LAB", folder: "Work" });
    expect(moved.folder?.projects).toEqual(["LAB"]);
    call(ws, "folder.move", { project: "TM", folder: "Work", at: "top" });
    expect(tree(ws)).toEqual(["PER:", "TM:Work", "LAB:Work"]);
    const work = call<FolderList>(ws, "folder.list").items[0];
    expect(work?.projects).toEqual(["TM", "LAB"]);
    // The open tasks of Task Manager and its sub-projects count too.
    const summaries = call<ProjectList>(ws, "project.list").items;
    const sum = summaries.filter((project) => project.key === "TM" || project.key === "LAB").reduce((total, project) => total + project.open_tasks_with_subprojects, 0);
    expect(work?.open_tasks).toBe(sum);
    expect(code(() => ws.call("folder.move", { project: "CLI", folder: "Work" }))).toBe("invalid");
    expect(code(() => ws.call("folder.move", { project: "LAB", before: "PER" }))).toBe("invalid");

    call(ws, "folder.move", { project: "LAB", before: "TM" });
    expect(call<FolderList>(ws, "folder.list").items[0]?.projects).toEqual(["LAB", "TM"]);
    const gone = call<FolderChange>(ws, "folder.delete", { folder: "Work" });
    expect(gone.released).toEqual(["LAB", "TM"]);
    expect(tree(ws)).toEqual(["PER:", "LAB:", "TM:"]);
    ws.call("history.undo", {});
    expect(tree(ws)).toEqual(["PER:", "LAB:Work", "TM:Work"]);
  });

  it("let go of a project that becomes a sub-project", () => {
    const ws = workspace();
    call(ws, "folder.create", { name: "Work" });
    call(ws, "folder.move", { project: "LAB", folder: "Work" });
    call(ws, "project.move", { project: "LAB", parent: "PER" });
    expect(call<FolderList>(ws, "folder.list").items[0]?.projects).toEqual([]);
  });

  it("take a new project straight into a folder, in one change", () => {
    const ws = workspace();
    call(ws, "folder.create", { name: "Work" });
    call(ws, "project.create", { name: "Orchard", key: "ORC", folder: "work" });
    expect(call<FolderList>(ws, "folder.list").items[0]?.projects).toEqual(["ORC"]);
    expect(code(() => ws.call("project.create", { name: "Kid", key: "KID", parent: "PER", folder: "Work" }))).toBe("invalid");
    expect(code(() => ws.call("project.create", { name: "Kid", key: "KID", folder: "Nope" }))).toBe("not_found");
    ws.call("history.undo", {});
    expect(call<FolderList>(ws, "folder.list").items[0]?.projects).toEqual([]);
    expect(tree(ws).some((row) => row.startsWith("ORC"))).toBe(false);
  });
});
