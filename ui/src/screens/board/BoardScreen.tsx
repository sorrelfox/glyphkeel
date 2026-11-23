// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { TaskPanel } from "../task/TaskPanel";
import { Board } from "./Board";
import styles from "./Board.module.css";

const route = getRouteApi("/shell/projects/$key");

/** A project's board, with the open task's panel docked on the right. */
export function BoardScreen() {
  const { key } = route.useParams();
  const { task } = route.useSearch();
  const navigate = useNavigate();
  return (
    <div className={styles.screen}>
      <Board key={key} projectKey={key} openTask={task} />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            void navigate({ to: "/projects/$key", params: { key }, search: {} });
            // The card that opened the panel gets the focus back.
            requestAnimationFrame(() => {
              document.querySelector<HTMLElement>(`[data-card="${CSS.escape(task)}"]`)?.focus();
            });
          }}
          onOpenTask={(other) => void navigate({ to: "/projects/$key", params: { key }, search: { task: other } })}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}
