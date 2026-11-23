// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useEffect, useState } from "react";
import { useLocale } from "react-aria-components";

import { ActionsProvider } from "../actions/ActionsProvider";
import { useAppearance } from "../appearance/AppearanceProvider";
import { useT } from "../i18n/I18n";
import { appWindow } from "../ipc/window";
import { type TitleBar } from "../start";
import { EmptyState } from "./EmptyState";
import { HeaderBar } from "./HeaderBar";
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

/**
 * The window: sidebar, header bar, the view, status bar. No workspace is
 * wired in yet, so the views are empty states.
 */
export function Shell({ titleBar, openGallery }: ShellProps) {
  const t = useT();
  const { locale } = useLocale();
  const [view, setView] = useState<View>("today");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  // Until the user drags the splitter, the width follows the density.
  const { density } = useAppearance().resolved;
  const [chosenWidth, setChosenWidth] = useState<number | null>(null);
  const sidebarWidth = chosenWidth ?? SIDEBAR_WIDTH[density];
  const [shortcutsOpen, setShortcutsOpen] = useState(false);

  const actions = shellActions({
    setView,
    toggleSidebar: () => { setSidebarOpen((open) => !open); },
    showShortcuts: () => { setShortcutsOpen(true); },
    openGallery,
  });

  const title = t(view === "today" ? "view-today" : "view-all-tasks");
  const subtitle =
    view === "today" ? new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" }).format(new Date()) : undefined;

  useEffect(() => {
    // Fluent isolates placeables with invisible marks; window titles show them.
    void appWindow.setTitle(t("window-title", { context: title }).replace(/[⁨⁩]/g, ""));
  }, [t, title]);

  const newTask = actions.find((action) => action.id === "task.new");

  return (
    <ActionsProvider actions={actions}>
      <div className={styles.shell}>
        {sidebarOpen && (
          <>
            <Sidebar view={view} width={sidebarWidth} />
            <Splitter label={t("sidebar-resize")} value={sidebarWidth} min={SIDEBAR_WIDTH.min} max={SIDEBAR_WIDTH.max} onChange={setChosenWidth} />
          </>
        )}
        <div className={styles.main}>
          <HeaderBar titleBar={titleBar} title={title} subtitle={subtitle} />
          <main className={styles.content} aria-label={t("content-label")} data-region="content" tabIndex={-1}>
            {newTask && <EmptyState text={t(view === "today" ? "empty-today" : "empty-all-tasks")} action={newTask} />}
          </main>
          <StatusBar count={0} />
        </div>
      </div>
      <ShortcutSheet isOpen={shortcutsOpen} onOpenChange={setShortcutsOpen} />
    </ActionsProvider>
  );
}
