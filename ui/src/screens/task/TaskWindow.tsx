// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";

import { keysOf, type Action } from "../../actions/actions";
import { ActionsProvider } from "../../actions/ActionsProvider";
import { useTaskDetails } from "../../data/task";
import { useTimerCommands } from "../../data/time";
import { useOp } from "../../data/hooks";
import { inApp } from "../../ipc/files";
import { appWindow } from "../../ipc/window";
import { projectOfKey } from "../../start";
import { TaskPanel } from "./TaskPanel";
import styles from "./TaskPanel.module.css";

const route = getRouteApi("/task/$key");

/**
 * A task window: the task panel filling a window of its own, without the
 * board. Opening another task from it shows that task in the same window.
 */
export function TaskWindowScreen() {
  const { key } = route.useParams();
  const navigate = useNavigate();
  const task = useTaskDetails(key).data?.items[0];
  const timer = useTimerCommands();
  useEffect(() => {
    if (task) {
      void appWindow.setTitle(`${task.key} · ${task.title}`);
    }
  }, [task]);
  const actions: Action[] = [
    {
      id: "timer.toggle",
      label: "action-timer-toggle",
      group: "tasks",
      keys: keysOf("timer.toggle"),
      run: () => {
        timer.toggle(key);
      },
    },
  ];
  return (
    <ActionsProvider actions={actions}>
      <div className={styles.windowScreen}>
        <TaskPanel
          key={key}
          taskKey={key}
          mode="window"
          onClose={() => {
            if (inApp()) {
              void appWindow.close();
            } else {
              // A plain browser shows the task window's screen in the page: back to the board.
              void navigate({ to: "/projects/$key", params: { key: task?.project ?? projectOfKey(key) }, search: { task: key } });
            }
          }}
          onOpenTask={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      </div>
    </ActionsProvider>
  );
}

const openRoute = getRouteApi("/shell/open/$key");

/** A `glyphkeel://open/<KEY>` link: the task's board with the task open beside it. */
export function OpenTaskScreen() {
  const { key } = openRoute.useParams();
  const navigate = useNavigate();
  const task = useOp("task.get", { keys: [key] });
  const project = task.data?.items[0]?.project;
  // The link may name the task by its ID; the panel shows it by its key.
  const taskKey = task.data?.items[0]?.key ?? key;
  const failed = task.isError;
  useEffect(() => {
    if (project !== undefined) {
      void navigate({ to: "/projects/$key", params: { key: project }, search: { task: taskKey }, replace: true });
    } else if (failed) {
      void navigate({ to: "/", replace: true });
    }
  }, [project, failed, taskKey, navigate]);
  return null;
}
