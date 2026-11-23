// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Clock, List, Plus, Search, SlidersHorizontal, Sun, Trash2 } from "lucide-react";
import { useState } from "react";

import { useAction } from "../actions/ActionsProvider";
import { Kbd } from "../components/Kbd";
import { classes } from "../classes";
import { useCounts } from "../data/board";
import { useSidebarCounts } from "../data/today";
import { useT } from "../i18n/I18n";
import { type NewProjectRequest } from "../screens/project/NewProjectDialog";
import { ProjectTree } from "../screens/project/ProjectTree";
import { type View } from "./shellActions";
import { TimerChip } from "./TimerChip";
import styles from "./Shell.module.css";

interface SidebarProps {
  view: View;
  /** The open project's key. */
  project: string | undefined;
  width: number;
  onNewProject: (request: NewProjectRequest) => void;
  onProjectSettings: (key: string) => void;
}

/**
 * The sidebar, top to bottom: the search field (Enter opens the search page
 * with its text; the command palette keeps its own key), Today and All
 * tasks with their counts, the time report, the project tree, the
 * timer chip while a timer runs, then Trash and Preferences.
 */
export function Sidebar({ view, project, width, onNewProject, onProjectSettings }: SidebarProps) {
  const t = useT();
  const palette = useAction("palette.open");
  const today = useAction("nav.today");
  const allTasks = useAction("nav.allTasks");
  const timeReport = useAction("nav.timeReport");
  const preferences = useAction("app.preferences");
  const counts = useCounts();
  const sidebarCounts = useSidebarCounts();
  // Until Today's counts are in, the project tree's.
  const allCount = sidebarCounts?.open ?? counts?.all;
  const navigate = useNavigate();
  const [search, setSearch] = useState("");

  return (
    <nav className={classes("sidebar", styles.sidebar)} style={{ flexBasis: width }} aria-label={t("sidebar-label")} data-region="sidebar" tabIndex={-1}>
      <div className={styles.sidebarTop} data-tauri-drag-region>
        <label className={classes("field quiet", styles.search)} data-search-field>
          <Search className="ic" aria-hidden="true" />
          <input
            type="search"
            aria-label={t("sidebar-search-label")}
            placeholder={t("sidebar-search")}
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && search.trim() !== "") {
                event.preventDefault();
                void navigate({ to: "/search", search: { q: search.trim() } });
                setSearch("");
              } else if (event.key === "Escape") {
                setSearch("");
              }
            }}
          />
          {palette.keys[0] && <Kbd chord={palette.keys[0]} />}
        </label>
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
        <ProjectTree current={project} onNewProject={onNewProject} onSettings={onProjectSettings} />
      </div>

      <TimerChip />
      <div className={styles.navGroup}>
        <button type="button" className="nav" disabled>
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
