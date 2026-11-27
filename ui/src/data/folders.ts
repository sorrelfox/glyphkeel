// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type FolderMoveInput, type FolderUpdateInput, type ProjectFolder, type ProjectSummary } from "../generated/ops";
import { useOp } from "./hooks";

/** The project folders, in sidebar order. */
export function useFolders() {
  return useOp("folder.list", {});
}

/** A folder's key in the project tree, apart from the projects' keys. */
const FOLDER_PREFIX = "folder:";

export function folderKey(id: string): string {
  return `${FOLDER_PREFIX}${id}`;
}

/** The folder ID a tree key stands for, if it stands for a folder. */
export function folderOfKey(key: string): string | undefined {
  return key.startsWith(FOLDER_PREFIX) ? key.slice(FOLDER_PREFIX.length) : undefined;
}

/** A folder with its top-level projects, in order. */
export interface FolderGroup {
  folder: ProjectFolder;
  projects: ProjectSummary[];
}

/** The sidebar's top level: the folders with their projects, then the projects in no folder. */
export interface TopLevel {
  folders: FolderGroup[];
  loose: ProjectSummary[];
}

/** Sorts the top-level `projects` (in tree order) into `folders`; a project naming a folder that isn't listed is in none. */
export function groupTopLevel(projects: readonly ProjectSummary[], folders: readonly ProjectFolder[]): TopLevel {
  const known = new Set(folders.map((folder) => folder.id));
  const top = projects.filter((project) => project.parent === undefined);
  return {
    folders: folders.map((folder) => ({ folder, projects: top.filter((project) => project.folder === folder.id) })),
    loose: top.filter((project) => project.folder === undefined || !known.has(project.folder)),
  };
}

/** A change the tree asks for: a project into, out of or within a folder, or a folder among the folders. */
export type TreeMove = { op: "folder.move"; input: FolderMoveInput } | { op: "folder.update"; input: FolderUpdateInput };

/** The group a top-level project is shown in: its folder's projects, or those in no folder. */
function groupOf(top: TopLevel, key: string): { folder: string | undefined; projects: ProjectSummary[] } | undefined {
  for (const group of top.folders) {
    if (group.projects.some((project) => project.key === key)) {
      return { folder: group.folder.id, projects: group.projects };
    }
  }
  return top.loose.some((project) => project.key === key) ? { folder: undefined, projects: top.loose } : undefined;
}

/**
 * The move for dropping the tree item `dragged` at `position` of `target`:
 * a project dropped on a folder goes into it; a top-level project dropped
 * before or after another goes next to it, in that one's folder (or in
 * none); a folder goes before or after another folder. Anything else,
 * sub-projects included, is no move.
 */
export function dropMove(top: TopLevel, dragged: string, target: string, position: "before" | "after" | "on"): TreeMove | null {
  const draggedFolder = folderOfKey(dragged);
  const targetFolder = folderOfKey(target);
  if (dragged === target) {
    return null;
  }
  if (draggedFolder !== undefined) {
    if (targetFolder === undefined || position === "on") {
      return null;
    }
    return { op: "folder.update", input: { folder: draggedFolder, [position]: targetFolder } };
  }
  const from = groupOf(top, dragged);
  if (from === undefined) {
    return null;
  }
  if (targetFolder !== undefined) {
    if (position !== "on" || from.folder === targetFolder) {
      return null;
    }
    return { op: "folder.move", input: { project: dragged, folder: targetFolder } };
  }
  const to = groupOf(top, target);
  if (to === undefined || position === "on") {
    return null;
  }
  const where = to.folder === undefined ? { none: true } : { folder: to.folder };
  return { op: "folder.move", input: { project: dragged, ...where, [position]: target } };
}

/** The move one step up or down, for a folder among the folders or a top-level project within its group; `null` at an end or for anything else. */
export function stepMove(top: TopLevel, key: string, direction: "up" | "down"): TreeMove | null {
  const folder = folderOfKey(key);
  if (folder !== undefined) {
    const at = top.folders.findIndex((group) => group.folder.id === folder);
    const next = top.folders[direction === "up" ? at - 1 : at + 1];
    if (at < 0 || next === undefined) {
      return null;
    }
    return { op: "folder.update", input: { folder, [direction === "up" ? "before" : "after"]: next.folder.id } };
  }
  const group = groupOf(top, key);
  if (group === undefined) {
    return null;
  }
  const at = group.projects.findIndex((project) => project.key === key);
  const next = group.projects[direction === "up" ? at - 1 : at + 1];
  if (next === undefined) {
    return null;
  }
  return { op: "folder.move", input: { project: key, [direction === "up" ? "before" : "after"]: next.key } };
}

/**
 * The folder project `key` counts as in: a top-level project's own folder,
 * or, for a sub-project, its top-level project's; none for a project in no
 * folder or one that isn't listed.
 */
export function folderOfProject(projects: readonly ProjectSummary[], key: string | undefined): string | undefined {
  let project = projects.find((item) => item.key === key);
  const seen = new Set<string>();
  while (project?.parent !== undefined && !seen.has(project.key)) {
    seen.add(project.key);
    const parent: string = project.parent;
    project = projects.find((item) => item.key === parent);
  }
  return project?.folder;
}
