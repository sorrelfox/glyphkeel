// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Menu as MenuIcon, Minus, PanelLeft, Plus, Square, X } from "lucide-react";
import { Button } from "react-aria-components";

import { useAction } from "../actions/ActionsProvider";
import { ActionMenu, SEPARATOR } from "../components/ActionMenu";
import { Tooltip } from "../components/Tooltip";
import { classes } from "../classes";
import { useT } from "../i18n/I18n";
import { appWindow } from "../ipc/window";
import { type TitleBar } from "../start";
import styles from "./Shell.module.css";

interface HeaderBarProps {
  titleBar: TitleBar;
  /** What the window shows, centred. */
  title: string;
  /** A second line under the title, such as today's date. */
  subtitle?: string | undefined;
}

/**
 * The window's header bar. Its empty parts move the window and a double click
 * maximizes it. With the system title bar, the window manager draws the window
 * buttons, so the header bar leaves them out.
 */
export function HeaderBar({ titleBar, title, subtitle }: HeaderBarProps) {
  const t = useT();
  const toggleSidebar = useAction("sidebar.toggle");
  const newTask = useAction("task.new");
  const menuItems = [
    useAction("app.undo"),
    useAction("app.redo"),
    SEPARATOR,
    useAction("app.backup"),
    useAction("app.import"),
    useAction("app.export"),
    SEPARATOR,
    useAction("help.shortcuts"),
    useAction("app.preferences"),
    useAction("app.about"),
    SEPARATOR,
    useAction("app.quit"),
  ] as const;

  return (
    <header className={classes("headerbar", styles.headerbar)} data-region="header" tabIndex={-1} data-tauri-drag-region>
      <Tooltip text={t(toggleSidebar.label)} chord={toggleSidebar.keys[0]}>
        <Button className="btn flat icon" aria-label={t(toggleSidebar.label)} onPress={toggleSidebar.run}>
          <PanelLeft className="ic" />
        </Button>
      </Tooltip>
      <div className={styles.spacer} data-tauri-drag-region />
      <div className={styles.viewTitle} data-tauri-drag-region>
        <h1 className={styles.title} data-tauri-drag-region>
          {title}
        </h1>
        {subtitle && (
          <span className={classes("muted", styles.subtitle)} data-tauri-drag-region>
            {subtitle}
          </span>
        )}
      </div>
      <div className={styles.spacer} data-tauri-drag-region />
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
