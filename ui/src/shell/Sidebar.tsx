// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Clock, History, List, Plus, SlidersHorizontal, Sun, Trash2 } from "lucide-react";

import { useAction } from "../actions/ActionsProvider";
import { Kbd } from "../components/Kbd";
import { classes } from "../classes";
import { useCounts } from "../data/board";
import { useSidebarCounts } from "../data/today";
import { useT } from "../i18n/I18n";
import { dragRegion, type TitleBar } from "../start";
import { type NewProjectRequest } from "../screens/project/NewProjectDialog";
import { ProjectTree } from "../screens/project/ProjectTree";
import { type View } from "./shellActions";
import { SidebarSearch } from "./SidebarSearch";
import { TimerChip } from "./TimerChip";
import styles from "./Shell.module.css";

interface SidebarProps {
  view: View;
  /** The open project's key. */
  project: string | undefined;
  width: number;
  /** With the system title bar the sidebar's top doesn't move the window. */
  titleBar?: TitleBar;
  onNewProject: (request: NewProjectRequest) => void;
  onProjectSettings: (key: string) => void;
  onExportProject: (key: string) => void;
  onDeleteProject: (key: string) => void;
  /** Opens the CSV import into this project. */
  onImportCsv: (key: string) => void;
}

/**
 * The sidebar, top to bottom: the search field (typing suggests tasks,
 * projects and actions; Enter opens the search page with its text; the
 * command palette keeps its own key), Today and All
 * tasks with their counts, the time report, the activity, the project tree, the
 * timer chip while a timer runs, then Trash and Preferences.
 */
export function Sidebar({ view, project, width, titleBar = "app", onNewProject, onProjectSettings, onExportProject, onDeleteProject, onImportCsv }: SidebarProps) {
  const t = useT();
  const today = useAction("nav.today");
  const allTasks = useAction("nav.allTasks");
  const timeReport = useAction("nav.timeReport");
  const activity = useAction("nav.activity");
  const trash = useAction("nav.trash");
  const preferences = useAction("app.preferences");
  const counts = useCounts();
  const sidebarCounts = useSidebarCounts();
  // Until Today's counts are in, the project tree's.
  const allCount = sidebarCounts?.open ?? counts?.all;

  return (
    <nav className={classes("sidebar", styles.sidebar)} style={{ flexBasis: width }} aria-label={t("sidebar-label")} data-region="sidebar" tabIndex={-1}>
      <div className={styles.sidebarTop} {...dragRegion(titleBar)}>
        <SidebarSearch />
      </div>

      <div className={styles.navGroup}>
        <button type="button" className={classes("nav", view === "today" && "on")} aria-current={view === "today" ? "page" : undefined} onClick={today.run}>
          <Sun className="ic" />
          <span>{t("view-today")}</span>
          {sidebarCounts !== undefined && sidebarCounts.today > 0 && (
            <span className="count" data-count="today">
              {sidebarCounts.today}
            </span>
          )}
        </button>
        <button type="button" className={classes("nav", view === "all-tasks" && "on")} aria-current={view === "all-tasks" ? "page" : undefined} onClick={allTasks.run}>
          <List className="ic" />
          <span>{t("view-all-tasks")}</span>
          {allCount !== undefined && (
            <span className="count" data-count="all">
              {allCount}
            </span>
          )}
        </button>
        <button type="button" className={classes("nav", view === "time" && "on")} aria-current={view === "time" ? "page" : undefined} onClick={timeReport.run}>
          <Clock className="ic" />
          <span>{t("view-time-report")}</span>
        </button>
        <button type="button" className={classes("nav", view === "activity" && "on")} aria-current={view === "activity" ? "page" : undefined} onClick={activity.run}>
          <History className="ic" />
          <span>{t("view-activity")}</span>
        </button>
      </div>

      <div className={styles.projectsHeader}>
        <span className="caption grow">{t("sidebar-projects")}</span>
        <button
          type="button"
          className="btn flat icon sm"
          aria-label={t("sidebar-new-project")}
          onClick={() => {
            onNewProject({ parent: null });
          }}
        >
          <Plus className="ic s" />
        </button>
      </div>
      <div className={styles.projectTree}>
        <ProjectTree current={project} onNewProject={onNewProject} onSettings={onProjectSettings} onExport={onExportProject} onImportCsv={onImportCsv} onDelete={onDeleteProject} />
      </div>

      <TimerChip />
      <div className={styles.navGroup}>
        <button type="button" className={classes("nav", view === "trash" && "on")} aria-current={view === "trash" ? "page" : undefined} onClick={trash.run}>
          <Trash2 className="ic" />
          <span>{t("sidebar-trash")}</span>
        </button>
        <button type="button" className="nav" disabled={preferences.disabled} onClick={preferences.run}>
          <SlidersHorizontal className="ic" />
          <span>{t("sidebar-preferences")}</span>
          {preferences.keys[0] && (
            <span className={styles.navHint}>
              <Kbd chord={preferences.keys[0]} />
            </span>
          )}
        </button>
      </div>
    </nav>
  );
}
