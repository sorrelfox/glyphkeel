// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { IterationCw } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger } from "react-aria-components";

import { FitPopover } from "../../components/FitPopover";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { type Task } from "../../data/catalog";
import { joinable, useSprints } from "../../data/sprints";
import { useT } from "../../i18n/I18n";

interface SprintPickerProps {
  task: Task;
  readOnly: boolean;
  /** Puts the task in a sprint (its ID), or back in the backlog. */
  onChange: (sprint: string | null) => void;
}

/**
 * The sprint a task is in, and the menu that changes it: the active and
 * planned sprints of its project, and "Backlog". A completed sprint stays
 * shown while the task is in it.
 */
export function SprintPicker({ task, readOnly, onChange }: SprintPickerProps) {
  const t = useT();
  // The sprints are read when the menu first opens.
  const [opened, setOpened] = useState(false);
  const sprints = useSprints(task.project, { enabled: opened });
  const current = task.sprint;
  const options = joinable(sprints.data?.items ?? []);
  const entries: MenuEntry[] = [
    ...options.map((sprint) => ({
      id: sprint.id,
      label: sprint.state === "active" ? t("prop-sprint-active", { name: sprint.name }) : sprint.name,
      checked: sprint.id === current?.id,
      run: () => {
        onChange(sprint.id);
      },
    })),
    ...(options.length === 0 ? [{ id: "none-open", label: sprints.isPending ? t("prop-sprint-loading") : t("prop-sprint-none-open"), disabled: true, run: () => undefined }] : []),
    "separator",
    { id: "backlog", label: t("prop-sprint-clear"), checked: current === undefined, disabled: current === undefined, run: () => { onChange(null); } },
  ];
  return (
    <MenuTrigger
      onOpenChange={(open) => {
        if (open) {
          setOpened(true);
        }
      }}
    >
      {/* The name holds all the text shown: the sprint and, when it runs or ended, its state. */}
      <Button
        className="val"
        isDisabled={readOnly}
        aria-label={
          current === undefined
            ? t("prop-sprint-value", { value: t("prop-sprint-clear") })
            : current.state === "planned"
              ? t("prop-sprint-value", { value: current.name })
              : t("prop-sprint-value-state", { value: current.name, state: t(`sprint-state-${current.state}`) })
        }
      >
        <IterationCw className="ic s muted" aria-hidden="true" />
        {current === undefined ? <span className="faint">{t("prop-sprint-clear")}</span> : <span className="truncate">{current.name}</span>}
        {/* A space, so the name and the state read as two words. */}
        {current !== undefined && " "}
        {current !== undefined && current.state !== "planned" && <span className="chip sm">{t(`sprint-state-${current.state}`)}</span>}
      </Button>
      <FitPopover className="menuPopover" placement="bottom start" offset={4}>
        <Menu className="menu" aria-label={t("prop-sprint")}>
          <MenuEntries entries={entries} />
        </Menu>
      </FitPopover>
    </MenuTrigger>
  );
}
