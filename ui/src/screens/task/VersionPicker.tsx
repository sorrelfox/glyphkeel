// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Milestone } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger } from "react-aria-components";

import { FitPopover } from "../../components/FitPopover";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { type Task } from "../../data/catalog";
import { plannable, useVersions } from "../../data/versions";
import { useT } from "../../i18n/I18n";

interface VersionPickerProps {
  task: Task;
  readOnly: boolean;
  /** Plans the task for a version (its ID), or for none. */
  onChange: (version: string | null) => void;
}

/**
 * The version a task is planned for, and the menu that changes it: the open
 * versions of its project and those shared with it, and "No version". A
 * locked or closed version stays shown while the task is in it.
 */
export function VersionPicker({ task, readOnly, onChange }: VersionPickerProps) {
  const t = useT();
  // The versions are read when the menu first opens.
  const [opened, setOpened] = useState(false);
  const versions = useVersions(task.project, { enabled: opened });
  const current = task.fix_version;
  const options = plannable(versions.data?.items ?? []);
  const entries: MenuEntry[] = [
    ...options.map((version) => ({
      id: version.id,
      label: version.inherited === true ? t("prop-version-shared", { name: version.name, project: version.project }) : version.name,
      checked: version.id === current?.id,
      run: () => {
        onChange(version.id);
      },
    })),
    ...(options.length === 0 ? [{ id: "none-open", label: versions.isPending ? t("prop-version-loading") : t("prop-version-none-open"), disabled: true, run: () => undefined }] : []),
    "separator",
    { id: "no-version", label: t("prop-version-clear"), checked: current === undefined, disabled: current === undefined, run: () => { onChange(null); } },
  ];
  return (
    <MenuTrigger
      onOpenChange={(open) => {
        if (open) {
          setOpened(true);
        }
      }}
    >
      {/* The name holds all the text shown: the version and, unless it is open, its status. */}
      <Button
        className="val"
        isDisabled={readOnly}
        aria-label={
          current === undefined
            ? t("prop-version-value", { value: t("prop-none") })
            : current.status === "open"
              ? t("prop-version-value", { value: current.name })
              : t("prop-version-value-status", { value: current.name, status: t(`version-status-${current.status}`) })
        }
      >
        <Milestone className="ic s muted" aria-hidden="true" />
        {current === undefined ? <span className="faint">{t("prop-none")}</span> : <span className="truncate" title={current.name}>{current.name}</span>}
        {/* A space, so the name and the state read as two words. */}
        {current !== undefined && " "}
        {current !== undefined && current.status !== "open" && <span className="chip sm">{t(`version-status-${current.status}`)}</span>}
      </Button>
      <FitPopover className="menuPopover" placement="bottom start" offset={4}>
        <Menu className="menu" aria-label={t("prop-version")}>
          <MenuEntries entries={entries} />
        </Menu>
      </FitPopover>
    </MenuTrigger>
  );
}
