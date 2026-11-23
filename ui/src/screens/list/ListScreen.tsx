// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { TaskPanel } from "../task/TaskPanel";
import { ListView } from "./ListView";
import styles from "./List.module.css";

const projectRoute = getRouteApi("/shell/projects/$key/list");
const allRoute = getRouteApi("/shell/all");

/** Puts the focus back on the row of a task, after its panel closed. */
function refocus(task: string) {
  requestAnimationFrame(() => {
    document.querySelector<HTMLElement>(`[data-row="${CSS.escape(task)}"]`)?.focus();
  });
}

/** A project's list, with the open task's panel docked on the right. */
export function ProjectListScreen() {
  const { key } = projectRoute.useParams();
  const { task } = projectRoute.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/projects/$key/list", params: { key }, search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <ListView key={key} project={key} openTask={task} onOpen={open} />
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

/** Every task of the workspace, as a list. */
export function AllTasksListScreen() {
  const { task } = allRoute.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/all", search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <ListView project={undefined} openTask={task} onOpen={open} />
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
