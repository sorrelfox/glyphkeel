// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { ChevronDown, Folder, FolderOpen } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import { Button, Tree, TreeItem, TreeItemContent, useDragAndDrop, type Key } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { isMenuKey, useContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { useToasts } from "../../components/Toasts";
import { useProjects } from "../../data/board";
import { type ProjectSummary } from "../../data/catalog";
import { dropMove, folderKey, folderOfKey, groupTopLevel, stepMove, useFolders, type TreeMove } from "../../data/folders";
import { useCommand, useUndo } from "../../data/hooks";
import { type ProjectFolder } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { loadCollapsedFolders, loadCollapsedProjects, saveCollapsedFolders, saveCollapsedProjects } from "../../prefs/store";
import { useMoveProjectEntry } from "./moveProject";
import { type NewProjectRequest } from "./NewProjectDialog";
import styles from "./Project.module.css";

interface ProjectTreeProps {
  /** The open project. */
  current: string | undefined;
  /** The folder picked in the tree (its ID), shown selected instead of the open project. */
  selectedFolder: string | undefined;
  /** A folder row was clicked or focused (its ID), or a project opened from the tree (null). */
  onSelectFolder: (folder: string | null) => void;
  onNewProject: (request: NewProjectRequest) => void;
  onSettings: (key: string) => void;
  /** Opens the export dialog with this project chosen. */
  onExport: (key: string) => void;
  /** Opens the CSV import into this project. */
  onImportCsv: (key: string) => void;
  /** Asks before moving this project and everything under it to the trash. */
  onDelete: (key: string) => void;
  /** Opens the dialog that renames a folder and changes its colour. */
  onRenameFolder: (folder: ProjectFolder) => void;
}

/** The data a dragged tree row carries: a project's key or a folder's tree key. */
const ROW_TYPE = "application/x-glyphkeel-project-row";

/** The keys that move the focused folder or top-level project: Alt with an arrow. */
const STEP_KEYS: Readonly<Record<string, "up" | "down">> = { ArrowUp: "up", ArrowDown: "down" };

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
 *
 * Folders come first, each holding top-level projects (with their
 * sub-projects); the projects in no folder follow. A folder opens and
 * closes like a parent, with its chevron or the arrow keys; clicking or
 * focusing its row selects it, so that a new project goes into it. Its
 * count is its projects' open tasks. A
 * top-level project is dragged onto a folder, or next to another
 * top-level project, and a folder next to another folder; Alt with the up
 * and down arrows moves the focused one a place, and the menus do both.
 */
export function ProjectTree({ current, selectedFolder, onSelectFolder, onNewProject, onSettings, onExport, onImportCsv, onDelete, onRenameFolder }: ProjectTreeProps) {
  const t = useT();
  const navigate = useNavigate();
  const projects = useProjects();
  const folders = useFolders();
  const [collapsed, setCollapsed] = useState<string[]>(loadCollapsedProjects);
  const [closedFolders, setClosedFolders] = useState<string[]>(loadCollapsedFolders);
  const archive = useCommand("project.archive", { undoable: t("toast-project-archived") });
  const unarchive = useCommand("project.unarchive");
  const moveFolder = useCommand("folder.update", { undoable: t("toast-folder-moved") });
  const moveToFolder = useCommand("folder.move", { undoable: t("toast-project-moved-to-folder") });
  const deleteFolder = useCommand("folder.delete");
  const toasts = useToasts();
  const { undo } = useUndo();
  const menu = useContextMenu(t("project-menu"));
  const folderMenu = useContextMenu(t("folder-menu"));
  const moveEntry = useMoveProjectEntry();
  /** The row being dragged, so a drop that can't take it is refused while hovering. */
  const dragging = useRef<string | null>(null);

  const all = projects.data?.items ?? [];
  const active = all.filter((project) => project.state === "active");
  const archived = all.filter((project) => project.state === "archived" && !all.some((other) => other.key === project.parent && other.state === "archived"));
  const children = childrenByParent(active);
  const allFolders = folders.data?.items ?? [];
  const top = groupTopLevel(active, allFolders);
  const open = (key: string) => void navigate({ to: "/projects/$key", params: { key } });

  const run = (move: TreeMove | null) => {
    if (move?.op === "folder.update") {
      moveFolder.mutate(move.input);
    } else if (move?.op === "folder.move") {
      moveToFolder.mutate(move.input);
    }
  };

  const { dragAndDropHooks } = useDragAndDrop({
    getItems: (keys) => [...keys].map((key) => ({ [ROW_TYPE]: String(key), "text/plain": String(key) })),
    onDragStart: (event) => {
      dragging.current = [...event.keys].map(String)[0] ?? null;
    },
    onDragEnd: () => {
      dragging.current = null;
    },
    getDropOperation: (target, _types, allowed) => {
      const dragged = dragging.current;
      if (target.type !== "item" || dragged === null || !allowed.includes("move")) {
        return "cancel";
      }
      return dropMove(top, dragged, String(target.key), target.dropPosition) === null ? "cancel" : "move";
    },
    onMove: (event) => {
      const [dragged] = [...event.keys].map(String);
      if (dragged !== undefined) {
        run(dropMove(top, dragged, String(event.target.key), event.target.dropPosition));
      }
    },
  });

  /** The project menu's "Move to folder": no folder, or one of the folders. */
  const folderEntries = (project: ProjectSummary): MenuEntry[] => {
    if (project.parent !== undefined) {
      return [];
    }
    const up = stepMove(top, project.key, "up");
    const down = stepMove(top, project.key, "down");
    return [
      {
        id: "move-folder",
        label: t("project-move-to-folder"),
        submenu: [
          { id: "move-folder-none", label: t("project-move-no-folder"), disabled: project.folder === undefined, run: () => { moveToFolder.mutate({ project: project.key, none: true }); } },
          ...(allFolders.length > 0 ? (["separator"] as MenuEntry[]) : []),
          ...allFolders.map((folder): MenuEntry => ({
            id: `move-folder-${folder.id}`,
            label: folder.name,
            disabled: project.folder === folder.id,
            run: () => { moveToFolder.mutate({ project: project.key, folder: folder.id }); },
          })),
        ],
      },
      { id: "move-up", label: t("project-move-up"), disabled: up === null, run: () => { run(up); } },
      { id: "move-down", label: t("project-move-down"), disabled: down === null, run: () => { run(down); } },
    ];
  };

  const folderMenuEntries = (folder: ProjectFolder): MenuEntry[] => {
    const up = stepMove(top, folderKey(folder.id), "up");
    const down = stepMove(top, folderKey(folder.id), "down");
    return [
      { id: "new-project", label: t("folder-new-project"), run: () => { onNewProject({ parent: null, folder: folder.id }); } },
      "separator",
      { id: "rename", label: t("folder-rename"), run: () => { onRenameFolder(folder); } },
      { id: "move-up", label: t("folder-move-up"), disabled: up === null, run: () => { run(up); } },
      { id: "move-down", label: t("folder-move-down"), disabled: down === null, run: () => { run(down); } },
      "separator",
      {
        id: "delete",
        label: t("folder-delete"),
        run: () => {
          deleteFolder.mutate(
            { folder: folder.id },
            { onSuccess: (done) => { toasts.show({ tone: "info", text: t("toast-folder-deleted", { count: (done.released ?? []).length }), action: { label: t("toast-undo"), run: undo } }); } },
          );
        },
      },
    ];
  };

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
          ...folderEntries(project),
          "separator",
          { id: "archive", label: t("project-archive"), run: () => { archive.mutate({ project: project.key }); } },
          { id: "export", label: t("project-export"), run: () => { onExport(project.key); } },
          { id: "import-csv", label: t("project-import-csv"), run: () => { onImportCsv(project.key); } },
          { id: "delete", label: t("project-delete"), run: () => { onDelete(project.key); } },
        ];

  const row = (project: ProjectSummary, hasChildren: boolean, isExpanded: boolean, isDropTarget: boolean): ReactNode => {
    // Open tasks only: the engine leaves out done, cancelled and trashed ones.
    const withSubprojects = hasChildren && !isExpanded;
    const count = withSubprojects ? project.open_tasks_with_subprojects : project.open_tasks;
    const countName = t(withSubprojects ? "sidebar-project-open-tasks-with-subprojects" : "sidebar-project-open-tasks", { count });
    return (
      <div
        className={classes("nav", styles.treeRow, project.key === current && selectedFolder === undefined && "on", isDropTarget && styles.dropTarget)}
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
        <span className="count" data-count="project" aria-label={countName} title={countName}>
          {count}
        </span>
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
        <TreeItemContent>{({ isExpanded, isDropTarget }) => row(project, kids.length > 0, isExpanded, isDropTarget === true)}</TreeItemContent>
        {kids.map(item)}
      </TreeItem>
    );
  };

  const folderItem = (folder: ProjectFolder, members: ProjectSummary[]): ReactNode => (
    <TreeItem key={folderKey(folder.id)} id={folderKey(folder.id)} textValue={folder.name} className={classes(styles.treeItem)} data-folder={folder.id}>
      <TreeItemContent>
        {({ isExpanded, isDropTarget }) => {
          const Icon = isExpanded && members.length > 0 ? FolderOpen : Folder;
          return (
            <div
              className={classes("nav", styles.treeRow, styles.folderRow, folder.id === selectedFolder && "on", isDropTarget === true && styles.dropTarget)}
              onContextMenu={(event) => {
                folderMenu.open(event, folderMenuEntries(folder));
              }}
            >
              {members.length > 0 ? (
                <Button slot="chevron" className={classes(styles.chevron, !isExpanded && styles.closed)}>
                  <ChevronDown className="ic s muted" />
                </Button>
              ) : (
                <span className={styles.chevron} />
              )}
              <Icon className={classes("ic s mark", folder.hue !== undefined && `hue-${folder.hue}`)} aria-hidden="true" />
              <span className="truncate">{folder.name}</span>
              <span
                className="count"
                data-count="folder"
                aria-label={t("folder-open-tasks", { count: folder.open_tasks })}
                title={t("folder-open-tasks", { count: folder.open_tasks })}
              >
                {folder.open_tasks}
              </span>
            </div>
          );
        }}
      </TreeItemContent>
      {members.map(item)}
    </TreeItem>
  );

  if (projects.isSuccess && all.length === 0) {
    return <p className={classes("muted", styles.treeEmpty)}>{t("sidebar-no-projects")}</p>;
  }

  const expanded = [
    ...active.filter((project) => children.has(project.key) && !collapsed.includes(project.key)).map((project) => project.key),
    ...allFolders.filter((folder) => !closedFolders.includes(folder.id)).map((folder) => folderKey(folder.id)),
  ];

  return (
    <>
      <div
        onFocus={(event) => {
          // Focusing a folder's row, by keyboard or by clicking, selects the folder.
          const key = (event.target as Element).closest("[data-key]")?.getAttribute("data-key") ?? "";
          const folder = folderOfKey(key);
          if (folder !== undefined && folder !== selectedFolder) {
            onSelectFolder(folder);
          }
        }}
        onKeyDownCapture={(event) => {
          const row = (event.target as Element).closest("[data-key]");
          const key = row?.getAttribute("data-key") ?? "";
          const folder = allFolders.find((candidate) => folderKey(candidate.id) === key);
          const project = active.find((candidate) => candidate.key === key);
          if (!row || (!folder && !project)) {
            return;
          }
          // Shift+F10 or the Menu key: the focused row's menu.
          if (isMenuKey(event)) {
            event.preventDefault();
            event.stopPropagation();
            if (folder) {
              folderMenu.openAt(row, folderMenuEntries(folder));
            } else if (project) {
              menu.openAt(row, entries(project));
            }
            return;
          }
          const direction = event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey ? STEP_KEYS[event.key] : undefined;
          if (direction !== undefined) {
            event.preventDefault();
            event.stopPropagation();
            const move = stepMove(top, key, direction);
            if (move) {
              run(move);
              // The row keeps the focus at its new place, once the tree has it.
              requestAnimationFrame(() => {
                requestAnimationFrame(() => {
                  document.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`)?.focus();
                });
              });
            }
          }
        }}
      >
        <Tree
          aria-label={t("sidebar-projects")}
          className={classes(styles.tree)}
          onAction={(key) => {
            // A folder row selects its folder; its chevron (or the arrow keys) opens and closes it.
            const folder = folderOfKey(String(key));
            onSelectFolder(folder ?? null);
            if (folder === undefined) {
              open(String(key));
            }
          }}
          expandedKeys={expanded}
          onExpandedChange={(keys: Set<Key>) => {
            const next = active.filter((project) => children.has(project.key) && !keys.has(project.key)).map((project) => project.key);
            setCollapsed(next);
            saveCollapsedProjects(next);
            const closed = allFolders.filter((folder) => !keys.has(folderKey(folder.id))).map((folder) => folder.id);
            setClosedFolders(closed);
            saveCollapsedFolders(closed);
          }}
          dragAndDropHooks={dragAndDropHooks}
        >
          {top.folders.map((group) => folderItem(group.folder, group.projects))}
          {top.loose.map(item)}
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
      {folderMenu.menu}
    </>
  );
}
