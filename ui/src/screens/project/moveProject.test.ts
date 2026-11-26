// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type ProjectSummary } from "../../data/catalog";
import { moveTargets } from "./moveProject";

function project(key: string, depth: number, parent?: string, state: ProjectSummary["state"] = "active"): ProjectSummary {
  return {
    id: key,
    key,
    name: key,
    path: key,
    depth,
    hue: "blue",
    state,
    open_tasks: 0,
    open_tasks_with_subprojects: 0,
    ...(parent === undefined ? {} : { parent }),
  };
}

// TM › SITE › DOCS, CLI on its own, OLD archived.
const TM = project("TM", 0);
const SITE = project("SITE", 1, "TM");
const DOCS = project("DOCS", 2, "SITE");
const CLI = project("CLI", 0);
const PROJECTS = [TM, SITE, DOCS, CLI, project("OLD", 0, undefined, "archived")];

describe("where a project may move", () => {
  it("never under itself, its own sub-projects, where it is now or an archived project", () => {
    const site = moveTargets(SITE, PROJECTS);
    expect(site.topLevel).toBe(true);
    // SITE has a sub-project, so it may go one level down at most: under CLI.
    expect(site.parents.map((p) => p.key)).toEqual(["CLI"]);
  });

  it("not so deep that a sub-project passes the third level", () => {
    const tm = moveTargets(TM, PROJECTS);
    expect(tm.topLevel).toBe(false);
    // TM is three levels deep already: it can go nowhere.
    expect(tm.parents).toEqual([]);
    const docs = moveTargets(DOCS, PROJECTS);
    expect(docs.parents.map((p) => p.key)).toEqual(["TM", "CLI"]);
    const cli = moveTargets(CLI, PROJECTS);
    expect(cli.topLevel).toBe(false);
    expect(cli.parents.map((p) => p.key)).toEqual(["TM", "SITE"]);
  });
});
