// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The in-memory engine's project folders: the same rules as the engine's,
// kept small. A folder groups top-level projects in the sidebar; a project
// keeps its place among the top-level projects, in a folder or not.

import {
  type Actor,
  type Edge,
  type EntityChange,
  type FolderChange,
  type FolderCreateInput,
  type FolderMoveInput,
  type FolderRef,
  type FolderRenameInput,
  type FolderUpdateInput,
  type Hue,
  type OpError,
  type ProjectFolder,
} from "../../generated/ops";
import { type FakeProject } from "./state";

export interface FakeFolder {
  id: string;
  name: string;
  hue?: Hue | undefined;
  /** Order among the folders. */
  rank: number;
  createdAt: string;
  createdBy: Actor;
  updatedAt: string;
}

/** What the folders need of the workspace around them. */
export interface FolderHost {
  folders: () => FakeFolder[];
  projects: () => FakeProject[];
  project: (ref: string) => FakeProject;
  /** Open tasks of an active project and its active sub-projects. */
  openBelow: (project: FakeProject) => number;
  actor: () => Actor;
  now: () => string;
  newId: () => string;
  fail: (code: OpError["code"], message: string, path?: string) => never;
}

type Answer = { output: FolderChange; changes: EntityChange[] };

const NAME_MAX = 100;

const fold = (text: string) => text.trim().toLowerCase();

function ordered(host: FolderHost): FakeFolder[] {
  return [...host.folders()].sort((a, b) => a.rank - b.rank);
}

/** The top-level projects, in order. */
function topLevel(host: FolderHost): FakeProject[] {
  return host
    .projects()
    .filter((project) => project.parent === null)
    .sort((a, b) => a.rank - b.rank);
}

export function folderOut(host: FolderHost, folder: FakeFolder): ProjectFolder {
  const inside = topLevel(host).filter((project) => project.folder === folder.id);
  return {
    id: folder.id,
    name: folder.name,
    ...(folder.hue === undefined ? {} : { hue: folder.hue }),
    rank: String(folder.rank),
    projects: inside.map((project) => project.key),
    open_tasks: inside.reduce((sum, project) => sum + host.openBelow(project), 0),
    created_by: folder.createdBy,
    created_at: folder.createdAt,
    updated_at: folder.updatedAt,
  };
}

export function listFolders(host: FolderHost) {
  const items = ordered(host).map((folder) => folderOut(host, folder));
  return { items, total: items.length };
}

function find(host: FolderHost, text: string, path = "folder"): FakeFolder {
  const found = host.folders().find((folder) => folder.id === text.trim()) ?? host.folders().find((folder) => fold(folder.name) === fold(text));
  return found ?? host.fail("not_found", `There is no folder "${text.trim()}".`, path);
}

function checkName(host: FolderHost, name: string, except?: FakeFolder): string {
  const trimmed = name.trim();
  if (trimmed === "" || Array.from(trimmed).length > NAME_MAX) {
    host.fail("invalid", `A folder's name is 1 to ${String(NAME_MAX)} characters long.`, "name");
  }
  if (host.folders().some((folder) => folder !== except && fold(folder.name) === fold(trimmed))) {
    host.fail("conflict", `There is already a folder named "${trimmed}".`, "name");
  }
  return trimmed;
}

/** Puts `moving` into `list` at the place asked for; returns the new list, or `null` when no place is asked for. */
function placeIn<T>(list: readonly T[], moving: T, before: T | undefined, after: T | undefined, at: Edge | null | undefined): T[] | null {
  const rest = list.filter((item) => item !== moving);
  let index: number;
  if (before !== undefined) {
    index = rest.indexOf(before);
  } else if (after !== undefined) {
    index = rest.indexOf(after) + 1;
  } else if (at === "top") {
    index = 0;
  } else if (at === "bottom") {
    index = rest.length;
  } else {
    return null;
  }
  rest.splice(index, 0, moving);
  return rest;
}

function given(value: string | null | undefined): value is string {
  return value !== null && value !== undefined;
}

function reorderFolders(host: FolderHost, folder: FakeFolder, input: { before?: string | null; after?: string | null; at?: Edge | null }): boolean {
  const other = (text: string, path: string) => {
    const found = find(host, text, path);
    return found === folder ? host.fail("invalid", "A folder can't be placed next to itself.", path) : found;
  };
  const before = given(input.before) ? other(input.before, "before") : undefined;
  const after = given(input.after) ? other(input.after, "after") : undefined;
  const next = placeIn(ordered(host), folder, before, after, input.at);
  if (next === null) {
    return false;
  }
  next.forEach((item, index) => {
    item.rank = index + 1;
  });
  return true;
}

function answer(host: FolderHost, folder: FakeFolder | undefined, changes: EntityChange[], extra: Partial<FolderChange> = {}): Answer {
  return { output: { ...(folder ? { folder: folderOut(host, folder) } : {}), ...extra }, changes };
}

const folderChange = (folder: FakeFolder): EntityChange => ({ entity: "project_folder", id: folder.id, fields: ["record"] });
const projectChange = (project: FakeProject, fields: string[]): EntityChange => ({ entity: "project", id: project.id, fields, project: project.key });

export function createFolder(host: FolderHost, input: FolderCreateInput): Answer {
  const folder: FakeFolder = {
    id: host.newId(),
    name: checkName(host, input.name),
    ...(input.hue ? { hue: input.hue } : {}),
    rank: Math.max(0, ...host.folders().map((item) => item.rank)) + 1,
    createdAt: host.now(),
    createdBy: host.actor(),
    updatedAt: host.now(),
  };
  host.folders().push(folder);
  reorderFolders(host, folder, input);
  return answer(host, folder, [folderChange(folder)]);
}

export function updateFolder(host: FolderHost, input: FolderUpdateInput): Answer {
  const folder = find(host, input.folder);
  if (input.hue && input.clear?.includes("hue") === true) {
    host.fail("invalid", "hue is both given and cleared.", "clear");
  }
  if (given(input.name)) {
    folder.name = checkName(host, input.name, folder);
  }
  if (input.hue) {
    folder.hue = input.hue;
  }
  if (input.clear?.includes("hue") === true) {
    folder.hue = undefined;
  }
  reorderFolders(host, folder, input);
  folder.updatedAt = host.now();
  return answer(host, folder, [folderChange(folder)]);
}

export function renameFolder(host: FolderHost, input: FolderRenameInput): Answer {
  return updateFolder(host, { folder: input.folder, name: input.name });
}

export function deleteFolder(host: FolderHost, input: FolderRef): Answer {
  const folder = find(host, input.folder);
  const out = folderOut(host, folder);
  const members = topLevel(host).filter((project) => project.folder === folder.id);
  for (const project of host.projects().filter((item) => item.folder === folder.id)) {
    project.folder = undefined;
  }
  const all = host.folders();
  all.splice(all.indexOf(folder), 1);
  return {
    output: { folder: out, deleted: true, ...(members.length > 0 ? { released: members.map((project) => project.key) } : {}) },
    changes: [...members.map((project) => projectChange(project, ["folder"])), folderChange(folder)],
  };
}

export function moveToFolder(host: FolderHost, input: FolderMoveInput): Answer {
  const project = host.project(input.project);
  if (project.parent !== null) {
    host.fail("invalid", `${project.key} is a sub-project; only top-level projects go into folders, and sub-projects follow their parent.`, "project");
  }
  if (input.none === true && given(input.folder)) {
    host.fail("invalid", "folder is both given and cleared.", "none");
  }
  const target = input.none === true ? undefined : given(input.folder) ? find(host, input.folder).id : project.folder;
  const tops = topLevel(host);
  const neighbour = (text: string, path: string) => {
    const other = host.project(text);
    if (other === project || other.parent !== null || other.folder !== target) {
      host.fail("invalid", `${other.key} is not another project of the folder ${project.key} goes to.`, path);
    }
    return other;
  };
  const before = given(input.before) ? neighbour(input.before, "before") : undefined;
  const after = given(input.after) ? neighbour(input.after, "after") : undefined;
  const moves = target !== project.folder;
  project.folder = target;
  const others = tops.filter((item) => item !== project && item.folder === target);
  const edge = input.at ?? (moves && before === undefined && after === undefined ? "bottom" : undefined);
  let next: FakeProject[] | null = null;
  if (before !== undefined || after !== undefined) {
    next = placeIn(tops, project, before, after, undefined);
  } else if (edge === "top" && others[0] !== undefined) {
    next = placeIn(tops, project, others[0], undefined, undefined);
  } else if (edge === "bottom" && others.length > 0) {
    next = placeIn(tops, project, undefined, others[others.length - 1], undefined);
  }
  const fields = moves ? ["folder"] : [];
  if (next !== null) {
    next.forEach((item, index) => {
      item.rank = index + 1;
    });
    fields.push("rank");
  }
  const folder = target === undefined ? undefined : host.folders().find((item) => item.id === target);
  return answer(host, folder, fields.length > 0 ? [projectChange(project, fields)] : [], { project: project.key });
}
