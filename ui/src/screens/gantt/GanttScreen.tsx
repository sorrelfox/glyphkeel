// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { TaskPanel } from "../task/TaskPanel";
import { GanttView } from "./GanttView";
import styles from "./Gantt.module.css";

const route = getRouteApi("/shell/projects/$key/gantt");

/** Puts the focus back on the chart, after a task's panel closed. */
function refocus() {
  requestAnimationFrame(() => {
    document.querySelector<HTMLElement>("[data-gantt] [role=treegrid]")?.focus();
  });
}

/** A project's Gantt chart, with the open task's panel docked on the right. */
export function GanttScreen() {
  const { key } = route.useParams();
  const { task } = route.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/projects/$key/gantt", params: { key }, search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <GanttView key={key} project={key} openTask={task} onOpen={open} />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            open(undefined);
            refocus();
          }}
          onOpenTask={open}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}
