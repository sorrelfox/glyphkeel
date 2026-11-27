// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ProjectFolder, type ProjectSummary } from "../generated/ops";
import { dropMove, folderKey, folderOfKey, folderOfProject, groupTopLevel, stepMove } from "./folders";

function project(key: string, fields: Partial<ProjectSummary> = {}): ProjectSummary {
  return { key, name: key, path: key, depth: 0, hue: "blue", state: "active", open_tasks: 0, open_tasks_with_subprojects: 0, id: `id-${key}`, ...fields };
}

function folder(id: string, name: string): ProjectFolder {
  return { id, name, rank: id, projects: [], open_tasks: 0, created_by: { kind: "user", name: "You" }, created_at: "", updated_at: "" };
}

const PROJECTS = [
  project("AA", { folder: "f1" }),
  project("AK", { parent: "AA", depth: 1 }),
  project("BB"),
  project("CC", { folder: "f1" }),
  project("DD"),
  project("EE", { folder: "gone" }),
];
const FOLDERS = [folder("f1", "Work"), folder("f2", "Home")];
const TOP = groupTopLevel(PROJECTS, FOLDERS);

describe("the project tree's folders", () => {
  it("group the top-level projects; an unknown folder counts as none", () => {
    expect(TOP.folders.map((group) => group.projects.map((item) => item.key))).toEqual([["AA", "CC"], []]);
    expect(TOP.loose.map((item) => item.key)).toEqual(["BB", "DD", "EE"]);
    expect(folderOfKey(folderKey("f2"))).toBe("f2");
    expect(folderOfKey("BB")).toBeUndefined();
  });

  it("turn drops into moves", () => {
    expect(dropMove(TOP, "BB", folderKey("f2"), "on")).toEqual({ op: "folder.move", input: { project: "BB", folder: "f2" } });
    expect(dropMove(TOP, "AA", folderKey("f1"), "on")).toBeNull();
    expect(dropMove(TOP, "BB", "CC", "after")).toEqual({ op: "folder.move", input: { project: "BB", folder: "f1", after: "CC" } });
    expect(dropMove(TOP, "AA", "DD", "before")).toEqual({ op: "folder.move", input: { project: "AA", none: true, before: "DD" } });
    expect(dropMove(TOP, folderKey("f2"), folderKey("f1"), "before")).toEqual({ op: "folder.update", input: { folder: "f2", before: "f1" } });
    // Sub-projects follow their parent, and nothing nests by dropping.
    expect(dropMove(TOP, "AK", folderKey("f2"), "on")).toBeNull();
    expect(dropMove(TOP, "BB", "AK", "before")).toBeNull();
    expect(dropMove(TOP, "BB", "DD", "on")).toBeNull();
    expect(dropMove(TOP, folderKey("f1"), "BB", "before")).toBeNull();
  });

  it("move one place up or down within a group", () => {
    expect(stepMove(TOP, "CC", "up")).toEqual({ op: "folder.move", input: { project: "CC", before: "AA" } });
    expect(stepMove(TOP, "CC", "down")).toBeNull();
    expect(stepMove(TOP, "BB", "down")).toEqual({ op: "folder.move", input: { project: "BB", after: "DD" } });
    expect(stepMove(TOP, folderKey("f1"), "down")).toEqual({ op: "folder.update", input: { folder: "f1", after: "f2" } });
    expect(stepMove(TOP, folderKey("f1"), "up")).toBeNull();
    expect(stepMove(TOP, "AK", "up")).toBeNull();
  });

  it("count a sub-project as in its top-level project's folder", () => {
    expect(folderOfProject(PROJECTS, "AA")).toBe("f1");
    expect(folderOfProject(PROJECTS, "AK")).toBe("f1");
    expect(folderOfProject(PROJECTS, "BB")).toBeUndefined();
    expect(folderOfProject(PROJECTS, undefined)).toBeUndefined();
    expect(folderOfProject(PROJECTS, "ZZ")).toBeUndefined();
  });
});
