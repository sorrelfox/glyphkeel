// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { HardDrive } from "lucide-react";

import { classes } from "../classes";
import { useOp } from "../data/hooks";
import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

interface StatusBarProps {
  /** How many tasks the view shows. */
  count: number;
}

/** Text only: counts on the left; the workspace on the right. */
export function StatusBar({ count }: StatusBarProps) {
  const t = useT();
  const workspace = useOp("workspace.get", {});
  return (
    <footer className={classes("statusbar", styles.statusbar)} aria-label={t("status-label")} data-region="status" tabIndex={-1}>
      <span className="num">{t("status-tasks", { count })}</span>
      <div className="grow" />
      <span className="meta" data-testid="status-workspace" title={workspace.error ? workspace.error.error.message : undefined}>
        <HardDrive className="ic" />
        {workspace.data
          ? t("status-workspace-name", { name: workspace.data.name })
          : t(workspace.isError ? "status-workspace-unavailable" : "status-workspace")}
      </span>
    </footer>
  );
}
