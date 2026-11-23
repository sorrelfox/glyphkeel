// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { classes } from "../classes";
import { appWindow } from "../ipc/window";
import styles from "./Shell.module.css";

interface HeaderBarProps {
  title: string;
}

/**
 * The window's header bar, drawn by the app instead of the desktop. Empty
 * parts of it move the window, and a double click maximizes it.
 */
export function HeaderBar({ title }: HeaderBarProps) {
  return (
    <header className={classes("headerbar", styles.headerbar)} data-tauri-drag-region>
      <span className={styles.title} data-tauri-drag-region>
        {title}
      </span>
      <div className={styles.spacer} data-tauri-drag-region />
      <div className={styles.windowControls}>
        <button type="button" className="win" aria-label="Minimize" onClick={() => void appWindow.minimize()}>
          <svg className="ic s" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4 8.5h8" />
          </svg>
        </button>
        <button type="button" className="win" aria-label="Maximize" onClick={() => void appWindow.toggleMaximize()}>
          <svg className="ic s" viewBox="0 0 16 16" aria-hidden="true">
            <rect x="4" y="4" width="8" height="8" rx="1" />
          </svg>
        </button>
        <button type="button" className="win" aria-label="Close" onClick={() => void appWindow.close()}>
          <svg className="ic s" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
          </svg>
        </button>
      </div>
    </header>
  );
}
