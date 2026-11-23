// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Many more tasks for the sample workspace, for checking that long lists stay
// smooth: `?tasks=20000` in a page built with sample data.

import { random } from "./seed";
import { type FakeState } from "./state";

/** The most tasks a page may ask for. */
const MOST = 50_000;

/** Adds `count` plain tasks to the first active project, made from its own tasks. */
export function addManyTasks(state: FakeState, count: number): void {
  const project = state.projects.find((candidate) => candidate.state === "active");
  const templates = state.tasks.filter((task) => task.projectId === project?.id && task.parentId === null);
  if (!project || templates.length === 0) {
    return;
  }
  const rand = random(count);
  for (let index = 0; index < Math.min(count, MOST); index += 1) {
    const template = templates[Math.floor(rand() * templates.length)] ?? templates[0];
    if (!template) {
      return;
    }
    state.tasks.push({
      ...template,
      id: `t-many-${String(index)}`,
      number: project.nextNumber,
      title: `${template.title} ${String(index + 1)}`,
      description: "",
      labelIds: [...template.labelIds],
      parentId: null,
      subtaskRank: undefined,
      rank: 1000 + index,
      files: 0,
      comments: 0,
      spentMin: 0,
    });
    project.nextNumber += 1;
  }
}
