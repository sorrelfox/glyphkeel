// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { useSavedViews } from "../../data/list";
import { type SavedView } from "../../generated/ops";
import { TaskPanel } from "../task/TaskPanel";
import { MyWork } from "./MyWork";
import styles from "../board/Board.module.css";

const route = getRouteApi("/shell/my-work");

/** The `view` of the address that shows no saved view, also when one is the default. */
export const NO_VIEW = "all";

/** The view the address names: by ID, none for {@link NO_VIEW}, else the default view. */
export function viewOfSearch(views: readonly SavedView[], view: string | undefined): SavedView | undefined {
  if (view === NO_VIEW) {
    return undefined;
  }
  return view === undefined ? views.find((each) => each.is_default) : views.find((each) => each.id === view);
}

/** My work: the board across projects, with the open task's panel docked on the right. */
export function MyWorkScreen() {
  const { view, task } = route.useSearch();
  const navigate = useNavigate();
  const { views } = useSavedViews(undefined, "board_across");
  if (views === undefined) {
    return <div className={styles.screen} aria-busy="true" />;
  }
  const active = viewOfSearch(views, view);
  const viewSearch = { view: active?.id ?? NO_VIEW };
  return (
    <div className={styles.screen}>
      <MyWork
        // A view chosen starts the board afresh from what it keeps.
        key={active === undefined ? NO_VIEW : `${active.id}:${active.updated_at}`}
        view={active}
        views={views}
        openTask={task}
        onOpenTask={(key) => void navigate({ to: "/my-work", search: { ...viewSearch, task: key } })}
        onChooseView={(chosen) => void navigate({ to: "/my-work", search: { view: chosen?.id ?? NO_VIEW } })}
      />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            void navigate({ to: "/my-work", search: viewSearch });
            // The card that opened the panel gets the focus back.
            requestAnimationFrame(() => {
              document.querySelector<HTMLElement>(`[data-card="${CSS.escape(task)}"]`)?.focus();
            });
          }}
          onOpenTask={(other) => void navigate({ to: "/my-work", search: { ...viewSearch, task: other } })}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}
