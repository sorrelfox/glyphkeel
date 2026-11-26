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
import { useTimeZone } from "../data/zone";
import { useT } from "../i18n/I18n";
import { hasQuickAddWindow, showQuickAdd } from "../ipc/quickAdd";
import { appWindow } from "../ipc/window";
import { CommandPalette } from "../screens/palette/CommandPalette";
import { DeleteProjectDialog } from "../screens/project/DeleteProject";
import { NewProjectDialog, type NewProjectRequest } from "../screens/project/NewProjectDialog";
import { ProjectSettings } from "../screens/project/ProjectSettings";
import { useBackUpNow } from "../screens/safety/DataPage";
import { ExportDialog } from "../screens/safety/ExportDialog";
import { ImportDialog } from "../screens/safety/ImportDialog";
import { type TitleBar } from "../start";
import { ShellContext, type ShellHooks, type StatusInfo } from "./context";
import { FirstRun, markFirstRunPending } from "./FirstRun";
import { HeaderBar } from "./HeaderBar";
import { Preferences, type PreferencePage } from "./Preferences";
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
  /** Offers the first-run set-up over the window. */
  firstRun?: boolean | undefined;
}

/** Which view a path shows. */
export function viewOf(pathname: string): View {
  if (pathname.startsWith("/projects/")) {
    return "project";
  }
  if (pathname.startsWith("/time")) {
    return "time";
  }
  if (pathname.startsWith("/search")) {
    return "search";
  }
  if (pathname.startsWith("/trash")) {
    return "trash";
  }
  if (pathname.startsWith("/activity")) {
    return "activity";
  }
  return pathname.startsWith("/all") ? "all-tasks" : "today";
}

/** The header's title of each view but a project's. */
const VIEW_TITLES: Record<Exclude<View, "project">, string> = {
  today: "view-today",
  "all-tasks": "view-all-tasks",
  time: "view-time-report",
  search: "view-search",
  trash: "view-trash",
  activity: "view-activity",
};

/**
 * The window: sidebar with the project tree, header bar, the screen of the
 * current route, status bar.
 */
export function Shell({ titleBar, openGallery, firstRun = false }: ShellProps) {
  const t = useT();
  const { locale } = useLocale();
  const zone = useTimeZone();
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
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [preferencesOpen, setPreferencesOpen] = useState(false);
  const [preferencesPage, setPreferencesPage] = useState<PreferencePage>("appearance");
  const [settingUp, setSettingUp] = useState(firstRun);
  useEffect(() => {
    if (settingUp) {
      markFirstRunPending();
    }
  }, [settingUp]);
  // The projects an export starts with; null while the dialog is closed.
  const [exporting, setExporting] = useState<readonly string[] | null>(null);
  const [importing, setImporting] = useState(false);
  const backUp = useBackUpNow();
  const [newProject, setNewProject] = useState<NewProjectRequest | null>(null);
  const [settingsOf, setSettingsOf] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
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
    openPalette: () => {
      setPaletteOpen(true);
    },
    quickAdd: hasQuickAddWindow()
      ? () => {
          void showQuickAdd();
        }
      : null,
    setView: (next) => {
      void navigate({ to: next === "all-tasks" ? "/all" : next === "time" ? "/time" : next === "trash" ? "/trash" : next === "activity" ? "/activity" : "/" });
    },
    projectView:
      view === "project" && projectKey !== undefined
        ? (next) => {
            void navigate(
              next === "list"
                ? { to: "/projects/$key/list", params: { key: projectKey } }
                : next === "wiki"
                  ? { to: "/projects/$key/wiki", params: { key: projectKey } }
                  : { to: "/projects/$key", params: { key: projectKey } },
            );
          }
        : null,
    toggleSidebar: () => {
      setSidebarOpen((open) => !open);
    },
    showShortcuts: () => {
      setShortcutsOpen(true);
    },
    openPreferences: () => {
      setPreferencesPage("appearance");
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
    backUp: backUp.run,
    exportData: () => {
      setExporting([]);
    },
    importData: () => {
      setImporting(true);
    },
    openGallery,
  });

  const title =
    view === "project"
      ? (project.data?.name ?? projectKey ?? "")
      : t(VIEW_TITLES[view]);
  const subtitle =
    view === "today" ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long", ...(zone === undefined ? {} : { timeZone: zone }) }).format(new Date()) : undefined;

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
                onExportProject={(key) => {
                  setExporting([key]);
                }}
                onDeleteProject={setDeleting}
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
        <CommandPalette isOpen={paletteOpen} onOpenChange={setPaletteOpen} />
        <ShortcutSheet isOpen={shortcutsOpen} onOpenChange={setShortcutsOpen} />
        <Preferences isOpen={preferencesOpen} onOpenChange={setPreferencesOpen} page={preferencesPage} />
        {settingUp && (
          <FirstRun
            onClose={() => {
              setSettingUp(false);
            }}
            onSetUpAgents={() => {
              setSettingUp(false);
              setPreferencesPage("agents");
              setPreferencesOpen(true);
            }}
          />
        )}
        {exporting !== null && (
          <ExportDialog
            isOpen
            projects={exporting}
            onOpenChange={(open) => {
              if (!open) {
                setExporting(null);
              }
            }}
          />
        )}
        {importing && <ImportDialog isOpen onOpenChange={setImporting} />}
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
        {deleting !== null && (
          <DeleteProjectDialog
            projectKey={deleting}
            onClose={() => {
              setDeleting(null);
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
