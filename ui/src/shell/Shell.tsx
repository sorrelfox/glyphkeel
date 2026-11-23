// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Outlet, useNavigate, useParams, useRouterState } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";
import { useLocale } from "react-aria-components";

import { ActionsProvider } from "../actions/ActionsProvider";
import { useAppearance } from "../appearance/AppearanceProvider";
import { useProject } from "../data/board";
import { useUndo } from "../data/hooks";
import { useTimerCommands } from "../data/time";
import { useT } from "../i18n/I18n";
import { appWindow } from "../ipc/window";
import { NewProjectDialog, type NewProjectRequest } from "../screens/project/NewProjectDialog";
import { ProjectSettings } from "../screens/project/ProjectSettings";
import { type TitleBar } from "../start";
import { ShellContext, type ShellHooks, type StatusInfo } from "./context";
import { HeaderBar } from "./HeaderBar";
import { Preferences } from "./Preferences";
import { ShortcutSheet } from "./ShortcutSheet";
import { shellActions, type View } from "./shellActions";
import { Sidebar } from "./Sidebar";
import { Splitter } from "./Splitter";
import { StatusBar } from "./StatusBar";
import styles from "./Shell.module.css";

/** The sidebar's width range, and its default in each density, from the design. */
export const SIDEBAR_WIDTH = { min: 180, max: 360, regular: 236, compact: 216 } as const;

interface ShellProps {
  titleBar: TitleBar;
  /** Opens the development gallery; absent in release builds. */
  openGallery?: (() => void) | undefined;
}

/** Which view a path shows. */
export function viewOf(pathname: string): View {
  if (pathname.startsWith("/projects/")) {
    return "project";
  }
  if (pathname.startsWith("/time")) {
    return "time";
  }
  return pathname.startsWith("/all") ? "all-tasks" : "today";
}

/**
 * The window: sidebar with the project tree, header bar, the screen of the
 * current route, status bar.
 */
export function Shell({ titleBar, openGallery }: ShellProps) {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const view = viewOf(pathname);
  const { key: projectKey } = useParams({ strict: false });
  const project = useProject(view === "project" ? projectKey : undefined);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  // Until the user drags the splitter, the width follows the density.
  const { density } = useAppearance().resolved;
  const [chosenWidth, setChosenWidth] = useState<number | null>(null);
  const sidebarWidth = chosenWidth ?? SIDEBAR_WIDTH[density];
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [preferencesOpen, setPreferencesOpen] = useState(false);
  const [newProject, setNewProject] = useState<NewProjectRequest | null>(null);
  const [settingsOf, setSettingsOf] = useState<string | null>(null);
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [newTask, setNewTask] = useState<(() => void) | null>(null);
  const [timerTask, setTimerTask] = useState<string | null>(null);
  const { undo, redo } = useUndo();
  const timer = useTimerCommands();

  const hooks = useMemo<ShellHooks>(
    () => ({
      setStatus,
      setNewTask: (run) => {
        setNewTask(() => run);
      },
      setTimerTask,
    }),
    [],
  );

  const actions = shellActions({
    setView: (next) => {
      void navigate({ to: next === "all-tasks" ? "/all" : next === "time" ? "/time" : "/" });
    },
    toggleSidebar: () => {
      setSidebarOpen((open) => !open);
    },
    showShortcuts: () => {
      setShortcutsOpen(true);
    },
    openPreferences: () => {
      setPreferencesOpen(true);
    },
    newTask,
    toggleTimer:
      timerTask === null
        ? null
        : () => {
            timer.toggle(timerTask);
          },
    undo,
    redo,
    openGallery,
  });

  const title =
    view === "project" ? (project.data?.name ?? projectKey ?? "") : t(view === "today" ? "view-today" : view === "time" ? "view-time-report" : "view-all-tasks");
  const subtitle =
    view === "today" ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(new Date()) : undefined;

  useEffect(() => {
    // Fluent isolates placeables with invisible marks; window titles show them.
    void appWindow.setTitle(t("window-title", { context: title }).replace(/[⁨⁩]/g, ""));
  }, [t, title]);

  return (
    <ShellContext value={hooks}>
      <ActionsProvider actions={actions}>
        <div className={styles.shell}>
          {sidebarOpen && (
            <>
              <Sidebar
                view={view}
                project={view === "project" ? projectKey : undefined}
                width={sidebarWidth}
                onNewProject={setNewProject}
                onProjectSettings={setSettingsOf}
              />
              <Splitter label={t("sidebar-resize")} value={sidebarWidth} min={SIDEBAR_WIDTH.min} max={SIDEBAR_WIDTH.max} onChange={setChosenWidth} />
            </>
          )}
          <div className={styles.main}>
            <HeaderBar
              titleBar={titleBar}
              title={title}
              subtitle={subtitle}
              project={view === "project" ? project.data : undefined}
              onProjectSettings={setSettingsOf}
              onNewSubproject={(parent) => {
                setNewProject({ parent });
              }}
            />
            <main className={styles.content} aria-label={t("content-label")} data-region="content" tabIndex={-1}>
              <Outlet />
            </main>
            <StatusBar info={status} />
          </div>
        </div>
        <ShortcutSheet isOpen={shortcutsOpen} onOpenChange={setShortcutsOpen} />
        <Preferences isOpen={preferencesOpen} onOpenChange={setPreferencesOpen} />
        {newProject && (
          <NewProjectDialog
            request={newProject}
            onClose={(created) => {
              setNewProject(null);
              if (created) {
                void navigate({ to: "/projects/$key", params: { key: created } });
              }
            }}
          />
        )}
        {settingsOf !== null && (
          <ProjectSettings
            projectKey={settingsOf}
            onClose={() => {
              setSettingsOf(null);
            }}
          />
        )}
      </ActionsProvider>
    </ShellContext>
  );
}
