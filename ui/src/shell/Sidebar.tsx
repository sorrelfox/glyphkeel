// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate, useRouterState } from "@tanstack/react-router";
import { ChartArea, Clock, FolderPlus, History, Inbox, LayoutDashboard, List, Plus, ShieldCheck, SlidersHorizontal, SquareKanban, Sun, Trash2 } from "lucide-react";
import { useState } from "react";

import { useAction } from "../actions/ActionsProvider";
import { Kbd } from "../components/Kbd";
import { classes } from "../classes";
import { useCounts, useProjects } from "../data/board";
import { useInboxCount } from "../data/digest";
import { useSavedViews } from "../data/list";
import { usePendingProposals } from "../data/proposals";
import { folderOfProject, useFolders } from "../data/folders";
import { useSidebarCounts } from "../data/today";
import { useT } from "../i18n/I18n";
import { dragRegion, type TitleBar } from "../start";
import { FolderDialog, type FolderRequest } from "../screens/project/FolderDialog";
import { type NewProjectRequest } from "../screens/project/NewProjectDialog";
import { ProjectTree } from "../screens/project/ProjectTree";
import { NO_VIEW, viewOfSearch } from "../screens/mywork/MyWorkScreen";
import { DIGEST_HEADING } from "../screens/today/DigestPanel";
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
 * command palette keeps its own key), Inbox (the digest of what changed
 * while the person was away, at the top of Today) with its count, Today, My
 * work (the board across projects) with its saved views under it, All
 * tasks with their counts, the dashboards, the time report, the reports, the activity, the approvals (with how many changes of agents wait), the project tree (with its folders and the buttons for a new folder and a new project), the
 * timer chip while a timer runs, then Trash and Preferences.
 *
 * A new project goes into the selected folder: the folder row picked in
 * the tree (until another project opens), or else the open project's.
 */
export function Sidebar({ view, project, width, titleBar = "app", onNewProject, onProjectSettings, onExportProject, onDeleteProject, onImportCsv }: SidebarProps) {
  const t = useT();
  const today = useAction("nav.today");
  const myWork = useAction("nav.myWork");
  const allTasks = useAction("nav.allTasks");
  const timeReport = useAction("nav.timeReport");
  const reports = useAction("nav.reports");
  const dashboards = useAction("nav.dashboards");
  const activity = useAction("nav.activity");
  const approvals = useAction("nav.approvals");
  const pendingProposals = usePendingProposals();
  const trash = useAction("nav.trash");
  const preferences = useAction("app.preferences");
  const counts = useCounts();
  const [folderDialog, setFolderDialog] = useState<FolderRequest | null>(null);
  // The folder picked in the tree, with the project open then: opening another project ends it.
  const [picked, setPicked] = useState<{ folder: string; project: string | undefined } | null>(null);
  const folders = useFolders();
  const projects = useProjects();
  const selectedFolder =
    picked !== null && picked.project === project && (folders.data?.items ?? []).some((folder) => folder.id === picked.folder) ? picked.folder : undefined;
  const newProjectFolder = selectedFolder ?? folderOfProject(projects.data?.items ?? [], project);
  const sidebarCounts = useSidebarCounts();
  const inboxCount = useInboxCount();
  const navigate = useNavigate();
  // Inbox opens Today at the digest, whose heading takes the focus.
  const openInbox = () => {
    void navigate({ to: "/", search: {} }).then(() => {
      requestAnimationFrame(() => {
        const heading = document.getElementById(DIGEST_HEADING);
        heading?.scrollIntoView({ block: "start" });
        heading?.focus();
      });
    });
  };
  // My work's saved views, under its entry; the one open is marked.
  const myWorkViews = useSavedViews(undefined, "board_across").views ?? [];
  const search = useRouterState({ select: (state) => state.location.search as { view?: unknown } });
  const openView = view === "my-work" ? viewOfSearch(myWorkViews, typeof search.view === "string" ? search.view : undefined) : undefined;
  // Until Today's counts are in, the project tree's.
  const allCount = sidebarCounts?.open ?? counts?.all;

  return (
    <nav className={classes("sidebar", styles.sidebar)} style={{ flexBasis: width }} aria-label={t("sidebar-label")} data-region="sidebar" tabIndex={-1}>
      <div className={styles.sidebarTop} {...dragRegion(titleBar)}>
        <SidebarSearch />
      </div>

      <div className={styles.navGroup}>
        <button type="button" className="nav" onClick={openInbox}>
          <Inbox className="ic" />
          <span>{t("sidebar-inbox")}</span>
          {inboxCount !== undefined && inboxCount > 0 && (
            <span className="count" data-count="inbox">
              {inboxCount}
            </span>
          )}
        </button>
        <button type="button" className={classes("nav", view === "today" && "on")} aria-current={view === "today" ? "page" : undefined} onClick={today.run}>
          <Sun className="ic" />
          <span>{t("view-today")}</span>
          {sidebarCounts !== undefined && sidebarCounts.today > 0 && (
            <span className="count" data-count="today">
              {sidebarCounts.today}
            </span>
          )}
        </button>
        <button
          type="button"
          className={classes("nav", view === "my-work" && openView === undefined && "on")}
          aria-current={view === "my-work" && openView === undefined ? "page" : undefined}
          onClick={() => {
            if (myWorkViews.some((each) => each.is_default)) {
              // The entry is the whole board; a default view opens from its own entry.
              void navigate({ to: "/my-work", search: { view: NO_VIEW } });
            } else {
              myWork.run();
            }
          }}
        >
          <SquareKanban className="ic" />
          <span>{t("view-my-work")}</span>
        </button>
        {myWorkViews.length > 0 && (
          <ul className={styles.navViews} aria-label={t("sidebar-my-work-views")}>
            {myWorkViews.map((each) => (
              <li key={each.id}>
                <button
                  type="button"
                  className={classes("nav", openView?.id === each.id && "on")}
                  aria-current={openView?.id === each.id ? "page" : undefined}
                  data-view={each.id}
                  onClick={() => {
                    void navigate({ to: "/my-work", search: { view: each.id } });
                  }}
                >
                  {/* The view's name is the person's own words. */}
                  <span className="truncate">{each.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <button type="button" className={classes("nav", view === "all-tasks" && "on")} aria-current={view === "all-tasks" ? "page" : undefined} onClick={allTasks.run}>
          <List className="ic" />
          <span>{t("view-all-tasks")}</span>
          {allCount !== undefined && (
            <span className="count" data-count="all">
              {allCount}
            </span>
          )}
        </button>
        <button type="button" className={classes("nav", view === "dashboards" && "on")} aria-current={view === "dashboards" ? "page" : undefined} onClick={dashboards.run}>
          <LayoutDashboard className="ic" />
          <span>{t("view-dashboards")}</span>
        </button>
        <button type="button" className={classes("nav", view === "time" && "on")} aria-current={view === "time" ? "page" : undefined} onClick={timeReport.run}>
          <Clock className="ic" />
          <span>{t("view-time-report")}</span>
        </button>
        <button type="button" className={classes("nav", view === "reports" && "on")} aria-current={view === "reports" ? "page" : undefined} onClick={reports.run}>
          <ChartArea className="ic" />
          <span>{t("view-reports")}</span>
        </button>
        <button type="button" className={classes("nav", view === "activity" && "on")} aria-current={view === "activity" ? "page" : undefined} onClick={activity.run}>
          <History className="ic" />
          <span>{t("view-activity")}</span>
        </button>
        <button type="button" className={classes("nav", view === "approvals" && "on")} aria-current={view === "approvals" ? "page" : undefined} onClick={approvals.run}>
          <ShieldCheck className="ic" />
          <span>{t("view-approvals")}</span>
          {pendingProposals !== undefined && pendingProposals > 0 && (
            <span className="count" data-count="approvals" title={t("approvals-badge", { count: pendingProposals })}>
              {pendingProposals}
            </span>
          )}
        </button>
      </div>

      <div className={styles.projectsHeader}>
        <span className="caption grow">{t("sidebar-projects")}</span>
        <button
          type="button"
          className="btn flat icon sm"
          aria-label={t("sidebar-new-folder")}
          onClick={() => {
            setFolderDialog({ kind: "new" });
          }}
        >
          <FolderPlus className="ic s" />
        </button>
        <button
          type="button"
          className="btn flat icon sm"
          aria-label={t("sidebar-new-project")}
          onClick={() => {
            onNewProject({ parent: null, folder: newProjectFolder ?? null });
          }}
        >
          <Plus className="ic s" />
        </button>
      </div>
      <div className={styles.projectTree}>
        <ProjectTree current={project}
          selectedFolder={selectedFolder}
          onSelectFolder={(folder) => {
            setPicked(folder === null ? null : { folder, project });
          }}
          onNewProject={onNewProject} onSettings={onProjectSettings} onExport={onExportProject} onImportCsv={onImportCsv}
          onDelete={onDeleteProject}
          onRenameFolder={(folder) => {
            setFolderDialog({ kind: "rename", folder });
          }}
        />
      </div>
      {folderDialog !== null && (
        <FolderDialog
          request={folderDialog}
          onClose={() => {
            setFolderDialog(null);
          }}
        />
      )}

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
