// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { ChevronDown } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Tree, TreeItem, TreeItemContent, type Key } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { isMenuKey, useContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { useProjects } from "../../data/board";
import { type ProjectSummary } from "../../data/catalog";
import { useCommand } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import { loadCollapsedProjects, saveCollapsedProjects } from "../../prefs/store";
import { useMoveProjectEntry } from "./moveProject";
import { type NewProjectRequest } from "./NewProjectDialog";
import styles from "./Project.module.css";

interface ProjectTreeProps {
  /** The open project. */
  current: string | undefined;
  onNewProject: (request: NewProjectRequest) => void;
  onSettings: (key: string) => void;
  /** Opens the export dialog with this project chosen. */
  onExport: (key: string) => void;
  /** Opens the CSV import into this project. */
  onImportCsv: (key: string) => void;
  /** Asks before moving this project and everything under it to the trash. */
  onDelete: (key: string) => void;
}

/** Projects under each parent key; the top level under "". */
export function childrenByParent(projects: readonly ProjectSummary[]): Map<string, ProjectSummary[]> {
  const map = new Map<string, ProjectSummary[]>();
  for (const project of projects) {
    const parent = project.parent ?? "";
    map.set(parent, [...(map.get(parent) ?? []), project]);
  }
  return map;
}

/**
 * The project tree: an indented tree of active projects. A parent opens and
 * closes with its chevron (or the arrow keys); the closed ones are
 * remembered. Its count is its own open tasks; closed, the count covers
 * everything under it. Archived projects are listed apart, below.
 */
export function ProjectTree({ current, onNewProject, onSettings, onExport, onImportCsv, onDelete }: ProjectTreeProps) {
  const t = useT();
  const navigate = useNavigate();
  const projects = useProjects();
  const [collapsed, setCollapsed] = useState<string[]>(loadCollapsedProjects);
  const archive = useCommand("project.archive", { undoable: t("toast-project-archived") });
  const unarchive = useCommand("project.unarchive");
  const menu = useContextMenu(t("project-menu"));
  const moveEntry = useMoveProjectEntry();

  const all = projects.data?.items ?? [];
  const active = all.filter((project) => project.state === "active");
  const archived = all.filter((project) => project.state === "archived" && !all.some((other) => other.key === project.parent && other.state === "archived"));
  const children = childrenByParent(active);
  const open = (key: string) => void navigate({ to: "/projects/$key", params: { key } });

  const entries = (project: ProjectSummary): MenuEntry[] =>
    project.state === "archived"
      ? [
          { id: "open", label: t("project-open"), run: () => { open(project.key); } },
          { id: "unarchive", label: t("project-unarchive"), run: () => { unarchive.mutate({ project: project.key }); } },
          { id: "export", label: t("project-export"), run: () => { onExport(project.key); } },
          "separator",
          { id: "delete", label: t("project-delete"), run: () => { onDelete(project.key); } },
        ]
      : [
          { id: "open", label: t("project-open"), run: () => { open(project.key); } },
          { id: "subproject", label: t("project-new-subproject"), run: () => { onNewProject({ parent: project.key }); } },
          { id: "settings", label: t("project-settings"), run: () => { onSettings(project.key); } },
          moveEntry(project),
          "separator",
          { id: "archive", label: t("project-archive"), run: () => { archive.mutate({ project: project.key }); } },
          { id: "export", label: t("project-export"), run: () => { onExport(project.key); } },
          { id: "import-csv", label: t("project-import-csv"), run: () => { onImportCsv(project.key); } },
          { id: "delete", label: t("project-delete"), run: () => { onDelete(project.key); } },
        ];

  const row = (project: ProjectSummary, hasChildren: boolean, isExpanded: boolean): ReactNode => {
    const count = hasChildren && !isExpanded ? project.open_tasks_with_subprojects : project.open_tasks;
    return (
      <div
        className={classes("nav", styles.treeRow, project.key === current && "on")}
        onContextMenu={(event) => {
          menu.open(event, entries(project));
        }}
      >
        {hasChildren ? (
          <Button slot="chevron" className={classes(styles.chevron, !isExpanded && styles.closed)}>
            <ChevronDown className="ic s muted" />
          </Button>
        ) : (
          <span className={styles.chevron} />
        )}
        <ProjectMark hue={project.hue} icon={project.icon} />
        <span className="truncate">{project.name}</span>
        <span className="count">{count}</span>
      </div>
    );
  };

  const item = (project: ProjectSummary): ReactNode => {
    const kids = children.get(project.key) ?? [];
    return (
      <TreeItem
        key={project.key}
        id={project.key}
        textValue={project.name}
        className={classes(styles.treeItem)}
      >
        <TreeItemContent>{({ isExpanded }) => row(project, kids.length > 0, isExpanded)}</TreeItemContent>
        {kids.map(item)}
      </TreeItem>
    );
  };

  if (projects.isSuccess && all.length === 0) {
    return <p className={classes("muted", styles.treeEmpty)}>{t("sidebar-no-projects")}</p>;
  }

  const expanded = active.filter((project) => children.has(project.key) && !collapsed.includes(project.key)).map((project) => project.key);

  return (
    <>
      <div
        onKeyDown={(event) => {
          // Shift+F10 or the Menu key: the focused project's menu.
          const row = (event.target as Element).closest("[data-key]");
          const project = active.find((candidate) => candidate.key === row?.getAttribute("data-key"));
          if (row && project && isMenuKey(event)) {
            event.preventDefault();
            menu.openAt(row, entries(project));
          }
        }}
      >
        <Tree
          aria-label={t("sidebar-projects")}
          className={classes(styles.tree)}
          onAction={(key) => {
            open(String(key));
          }}
          expandedKeys={expanded}
          onExpandedChange={(keys: Set<Key>) => {
            const next = active.filter((project) => children.has(project.key) && !keys.has(project.key)).map((project) => project.key);
            setCollapsed(next);
            saveCollapsedProjects(next);
          }}
        >
          {(children.get("") ?? []).map(item)}
        </Tree>
      </div>
      {archived.length > 0 && (
        <div className={styles.archived}>
          <span className="caption">{t("sidebar-archived")}</span>
          {archived.map((project) => (
            <button
              key={project.key}
              type="button"
              className={classes("nav", styles.archivedRow, project.key === current && "on")}
              onClick={() => {
                open(project.key);
              }}
              onContextMenu={(event) => {
                menu.open(event, entries(project));
              }}
            >
              <ProjectMark hue={project.hue} icon={project.icon} />
              <span className="truncate">{project.name}</span>
            </button>
          ))}
        </div>
      )}
      {menu.menu}
    </>
  );
}
