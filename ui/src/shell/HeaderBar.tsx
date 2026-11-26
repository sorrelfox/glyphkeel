// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { BookOpen, CalendarDays, ChartGantt, ChevronDown, Columns3, List, Menu as MenuIcon, Milestone, Minus, PanelLeft, Plus, Square, X } from "lucide-react";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { useAction } from "../actions/ActionsProvider";
import { ActionMenu, SEPARATOR } from "../components/ActionMenu";
import { MenuEntries } from "../components/ContextMenu";
import { useProjects } from "../data/board";
import { type Project } from "../data/catalog";
import { Tooltip } from "../components/Tooltip";
import { classes } from "../classes";
import { ProjectMark } from "../components/ProjectMark";
import { useMoveProjectEntry } from "../screens/project/moveProject";
import { useT } from "../i18n/I18n";
import { appWindow } from "../ipc/window";
import { dragRegion, type TitleBar } from "../start";
import styles from "./Shell.module.css";

interface HeaderBarProps {
  titleBar: TitleBar;
  /** What the window shows, centred. */
  title: string;
  /** A second line under the title, such as today's date. */
  subtitle?: string | undefined;
  /** The open project: the header shows its button and the view switcher instead of the title. */
  project?: Project | undefined;
  onProjectSettings?: (key: string) => void;
  onNewSubproject?: (parent: string) => void;
}

/**
 * The window's header bar. Its empty parts move the window and a double click
 * maximizes it. With the system title bar (always on Windows), the system
 * draws the window buttons and moves the window, so the header bar leaves
 * out both.
 */
export function HeaderBar({ titleBar, title, subtitle, project, onProjectSettings, onNewSubproject }: HeaderBarProps) {
  const t = useT();
  const drag = dragRegion(titleBar);
  const toggleSidebar = useAction("sidebar.toggle");
  const newTask = useAction("task.new");
  const menuItems = [
    useAction("app.undo"),
    useAction("app.redo"),
    SEPARATOR,
    useAction("app.quickAdd"),
    SEPARATOR,
    useAction("app.backup"),
    useAction("app.import"),
    useAction("app.importCsv"),
    useAction("app.export"),
    SEPARATOR,
    useAction("help.shortcuts"),
    useAction("app.preferences"),
    useAction("app.about"),
    SEPARATOR,
    useAction("app.quit"),
  ] as const;

  return (
    <header className={classes("headerbar", styles.headerbar)} data-region="header" tabIndex={-1} {...drag}>
      <Tooltip text={t(toggleSidebar.label)} chord={toggleSidebar.keys[0]}>
        <Button className="btn flat icon" aria-label={t(toggleSidebar.label)} onPress={toggleSidebar.run}>
          <PanelLeft className="ic" />
        </Button>
      </Tooltip>
      {project ? (
        <>
          <ProjectButton project={project} onSettings={onProjectSettings} onNewSubproject={onNewSubproject} />
          <div className={styles.spacer} {...drag} />
          <ViewSwitcher project={project.key} />
          <div className={styles.spacer} {...drag} />
        </>
      ) : (
        <>
          <div className={styles.spacer} {...drag} />
          <div className={styles.viewTitle} {...drag}>
            <h1 className={styles.title} {...drag}>
              {title}
            </h1>
            {subtitle && (
              <span className={classes("muted", styles.subtitle)} {...drag}>
                {subtitle}
              </span>
            )}
          </div>
          <div className={styles.spacer} {...drag} />
        </>
      )}
      <Tooltip text={t(newTask.label)} chord={newTask.keys[0]}>
        <Button className="btn primary" isDisabled={newTask.disabled ?? false} onPress={newTask.run}>
          <Plus className="ic" />
          <span className="hide-narrow">{t("header-new-task")}</span>
        </Button>
      </Tooltip>
      <ActionMenu label={t("header-main-menu")} icon={<MenuIcon className="ic" />} items={menuItems} />
      {titleBar === "app" && (
        <div className={styles.windowControls} role="group" aria-label={t("window-controls")}>
          <Button className="win" aria-label={t("window-minimize")} onPress={() => void appWindow.minimize()}>
            <Minus className="ic s" />
          </Button>
          <Button className="win" aria-label={t("window-maximize")} onPress={() => void appWindow.toggleMaximize()}>
            <Square className="ic s" />
          </Button>
          <Button className="win" aria-label={t("window-close")} onPress={() => void appWindow.close()}>
            <X className="ic s" />
          </Button>
        </div>
      )}
    </header>
  );
}

interface ProjectButtonProps {
  project: Project;
  onSettings?: ((key: string) => void) | undefined;
  onNewSubproject?: ((parent: string) => void) | undefined;
}

/** The project's colour and name (a sub-project with its path, parents muted), opening the project menu. */
function ProjectButton({ project, onSettings, onNewSubproject }: ProjectButtonProps) {
  const t = useT();
  const importCsv = useAction("app.importCsv");
  const projects = useProjects();
  const moveEntry = useMoveProjectEntry();
  const summary = projects.data?.items.find((candidate) => candidate.key === project.key);
  const path: string[] = [];
  let parent = project.parent;
  while (parent !== undefined) {
    const found = projects.data?.items.find((candidate) => candidate.key === parent);
    if (!found) {
      break;
    }
    path.unshift(found.name);
    parent = found.parent;
  }
  return (
    <MenuTrigger>
      <Button className={classes("btn flat", styles.projectButton)} aria-label={t("header-project-menu", { name: project.name })}>
        <ProjectMark hue={project.hue} icon={project.icon} />
        {path.map((name) => (
          <span key={name} className="muted">
            {name} ›
          </span>
        ))}
        <h1 className={styles.projectName}>{project.name}</h1>
        <ChevronDown className="ic s muted" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom start" offset={4}>
        <Menu className="menu">
          <MenuEntries
            entries={[
              { id: "settings", label: t("project-settings"), run: () => onSettings?.(project.key), disabled: !onSettings },
              {
                id: "subproject",
                label: t("project-new-subproject"),
                run: () => onNewSubproject?.(project.key),
                disabled: !onNewSubproject || project.state === "archived",
              },
              { id: "import-csv", label: t("project-import-csv"), run: () => { importCsv.run(); }, disabled: project.state === "archived" },
              ...(project.state === "archived" ? [] : [moveEntry(summary)]),
            ]}
          />
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** Board, List, Calendar, Gantt, Roadmap and Wiki. */
function ViewSwitcher({ project }: { project: string }) {
  const t = useT();
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const current = pathname.endsWith("/list") ? "list" : pathname.endsWith("/calendar") ? "calendar" : pathname.endsWith("/gantt") ? "gantt" : pathname.endsWith("/roadmap") ? "roadmap" : pathname.endsWith("/wiki") ? "wiki" : "board";
  const views = [
    { id: "board", icon: <Columns3 className="ic" />, ready: true },
    { id: "list", icon: <List className="ic" />, ready: true },
    { id: "calendar", icon: <CalendarDays className="ic" />, ready: true },
    { id: "gantt", icon: <ChartGantt className="ic" />, ready: true },
    { id: "roadmap", icon: <Milestone className="ic" />, ready: true },
    { id: "wiki", icon: <BookOpen className="ic" />, ready: true },
  ];
  return (
    <div className="seg" role="group" aria-label={t("header-views")}>
      {views.map((view) => (
        <button
          key={view.id}
          type="button"
          className={view.id === current ? "on" : ""}
          aria-current={view.id === current ? "page" : undefined}
          disabled={!view.ready}
          aria-label={t(`view-${view.id}`)}
          onClick={() => {
            if (view.id === "list") {
              void navigate({ to: "/projects/$key/list", params: { key: project } });
            } else if (view.id === "calendar") {
              void navigate({ to: "/projects/$key/calendar", params: { key: project } });
            } else if (view.id === "gantt") {
              void navigate({ to: "/projects/$key/gantt", params: { key: project } });
            } else if (view.id === "roadmap") {
              void navigate({ to: "/projects/$key/roadmap", params: { key: project } });
            } else if (view.id === "wiki") {
              void navigate({ to: "/projects/$key/wiki", params: { key: project } });
            } else if (view.id === "board") {
              void navigate({ to: "/projects/$key", params: { key: project } });
            }
          }}
        >
          {view.icon}
          <span className="hide-narrow">{t(`view-${view.id}`)}</span>
        </button>
      ))}
    </div>
  );
}
