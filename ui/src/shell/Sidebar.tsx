// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { List, Plus, Search, SlidersHorizontal, Sun, Trash2 } from "lucide-react";

import { useAction } from "../actions/ActionsProvider";
import { Kbd } from "../components/Kbd";
import { classes } from "../classes";
import { useT } from "../i18n/I18n";
import { type View } from "./shellActions";
import styles from "./Shell.module.css";

interface SidebarProps {
  view: View;
  width: number;
}

/**
 * The sidebar, top to bottom: the search field that opens the command
 * palette, Today and All tasks, the project tree, then Trash and Preferences.
 * Without a workspace wired in yet, the tree is empty and the actions that
 * need one are dimmed.
 */
export function Sidebar({ view, width }: SidebarProps) {
  const t = useT();
  const palette = useAction("palette.open");
  const today = useAction("nav.today");
  const allTasks = useAction("nav.allTasks");
  const preferences = useAction("app.preferences");

  return (
    <nav className={classes("sidebar", styles.sidebar)} style={{ flexBasis: width }} aria-label={t("sidebar-label")} data-region="sidebar" tabIndex={-1}>
      <div className={styles.sidebarTop} data-tauri-drag-region>
        <button type="button" className={classes("field quiet", styles.search)} aria-label={t(palette.label)} disabled={palette.disabled}>
          <Search className="ic" />
          <span className="grow faint">{t("sidebar-search")}</span>
          {palette.keys[0] && <Kbd chord={palette.keys[0]} />}
        </button>
      </div>

      <div className={styles.navGroup}>
        <button type="button" className={classes("nav", view === "today" && "on")} aria-current={view === "today" ? "page" : undefined} onClick={today.run}>
          <Sun className="ic" />
          <span>{t("view-today")}</span>
        </button>
        <button type="button" className={classes("nav", view === "all-tasks" && "on")} aria-current={view === "all-tasks" ? "page" : undefined} onClick={allTasks.run}>
          <List className="ic" />
          <span>{t("view-all-tasks")}</span>
        </button>
      </div>

      <div className={styles.projectsHeader}>
        <span className="caption grow">{t("sidebar-projects")}</span>
        <button type="button" className="btn flat icon sm" aria-label={t("sidebar-new-project")} disabled>
          <Plus className="ic s" />
        </button>
      </div>
      <div className={styles.projectTree}>
        <p className={classes("muted", styles.treeEmpty)}>{t("sidebar-no-projects")}</p>
      </div>

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
