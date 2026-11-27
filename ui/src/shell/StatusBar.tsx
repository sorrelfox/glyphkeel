// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { classes } from "../classes";
import { failureText, useOp } from "../data/hooks";
import { useT } from "../i18n/I18n";
import { type StatusInfo } from "./context";
import styles from "./Shell.module.css";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";

interface StatusBarProps {
  /** What the screen reports; nothing for screens without a count of their own. */
  info: StatusInfo | null;
}

/** Counts, selection and the last action's result on the left; the workspace on the right, a menu to switch to another. */
export function StatusBar({ info }: StatusBarProps) {
  const t = useT();
  const workspace = useOp("workspace.get", {});
  return (
    <footer className={classes("statusbar", styles.statusbar)} aria-label={t("status-label")} data-region="status" tabIndex={-1}>
      {info && <span className="num">{info.summary}</span>}
      {info?.alert && (
        <span className="num meta overdue" data-testid="status-alert">
          {info.alert}
        </span>
      )}
      {info?.selection && <span className="num hide-narrow">{info.selection}</span>}
      <span className={classes("truncate", styles.statusNote)} role="status" data-testid="status-note">
        {info?.note}
      </span>
      <div className="grow" />
      <WorkspaceSwitcher
        title={workspace.error ? failureText(t, workspace.error) : undefined}
        name={workspace.data ? t("status-workspace-name", { name: workspace.data.name }) : t(workspace.isError ? "status-workspace-unavailable" : "status-workspace")}
      />
    </footer>
  );
}
