// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useAction } from "../../actions/ActionsProvider";
import { useT } from "../../i18n/I18n";
import { EmptyState } from "../../shell/EmptyState";

/** Today, until its sections exist: an empty state with New task. */
export function TodayScreen() {
  const t = useT();
  return <EmptyState text={t("empty-today")} action={useAction("task.new")} />;
}

/** All tasks, until the list exists. */
export function AllTasksScreen() {
  const t = useT();
  return <EmptyState text={t("empty-all-tasks")} action={useAction("task.new")} />;
}
