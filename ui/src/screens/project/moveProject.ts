// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type MenuEntry } from "../../components/ContextMenu";
import { useProjects } from "../../data/board";
import { type ProjectSummary } from "../../data/catalog";
import { useCommand } from "../../data/hooks";
import { useT } from "../../i18n/I18n";

/** Sub-projects go at most three levels deep: depths 0, 1 and 2. */
const DEEPEST = 2;

/** Where a project may move: what it may go under, and whether it may go to the top level. */
export interface MoveTargets {
  topLevel: boolean;
  parents: ProjectSummary[];
}

/**
 * The places `project` may move to among the active `projects`: not under
 * itself or its own sub-projects, not where it is now, and not so deep that
 * it or its sub-projects would pass the third level.
 */
export function moveTargets(project: ProjectSummary, projects: readonly ProjectSummary[]): MoveTargets {
  const under = new Set<string>([project.key]);
  let height = 0;
  for (let grew = true; grew; ) {
    grew = false;
    for (const candidate of projects) {
      if (candidate.parent !== undefined && under.has(candidate.parent) && !under.has(candidate.key)) {
        under.add(candidate.key);
        height = Math.max(height, candidate.depth - project.depth);
        grew = true;
      }
    }
  }
  const parents = projects.filter(
    (candidate) => candidate.state === "active" && !under.has(candidate.key) && candidate.key !== project.parent && candidate.depth + 1 + height <= DEEPEST,
  );
  return { topLevel: project.parent !== undefined, parents };
}

/** The project menu's "Move to" entry: the top level, or under another project. */
export function useMoveProjectEntry(): (project: ProjectSummary | undefined) => MenuEntry {
  const t = useT();
  const projects = useProjects();
  const move = useCommand("project.move", { undoable: t("toast-project-moved") });
  return (project) => {
    const targets = project ? moveTargets(project, projects.data?.items ?? []) : { topLevel: false, parents: [] };
    const key = project?.key ?? "";
    return {
      id: "move",
      label: t("project-move-to"),
      submenu: [
        { id: "move-top-level", label: t("project-move-top-level"), disabled: !targets.topLevel, run: () => { move.mutate({ project: key, top_level: true }); } },
        ...(targets.parents.length > 0 ? (["separator"] as MenuEntry[]) : []),
        ...targets.parents.map((parent): MenuEntry => ({
          id: `move-under-${parent.key}`,
          label: parent.path,
          run: () => { move.mutate({ project: key, parent: parent.key }); },
        })),
      ],
    };
  };
}
