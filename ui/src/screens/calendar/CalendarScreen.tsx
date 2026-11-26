// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { TaskPanel } from "../task/TaskPanel";
import { CalendarView } from "./CalendarView";
import styles from "./Calendar.module.css";

const route = getRouteApi("/shell/projects/$key/calendar");

/** Puts the focus back on a task's bar, after its panel closed. */
function refocus(task: string) {
  requestAnimationFrame(() => {
    document.querySelector<HTMLElement>(`[data-task-bar="${CSS.escape(task)}"]`)?.focus();
  });
}

/** A project's calendar, with the open task's panel docked on the right. */
export function CalendarScreen() {
  const { key } = route.useParams();
  const { task } = route.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/projects/$key/calendar", params: { key }, search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <CalendarView key={key} project={key} openTask={task} onOpen={open} />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            open(undefined);
            refocus(task);
          }}
          onOpenTask={open}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}
