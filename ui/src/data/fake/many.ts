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

/**
 * Gives the added tasks dates around `today` (a few days each) and links
 * `count` pairs of them with `blocks`, for checking that a large Gantt chart
 * stays smooth: `?tasks=500&links=300`.
 */
export function addManyLinks(state: FakeState, count: number, today: Date): void {
  const many = state.tasks.filter((task) => task.id.startsWith("t-many-"));
  if (many.length < 2) {
    return;
  }
  const rand = random(count + many.length);
  const day = (offset: number) => new Date(today.getTime() + offset * 86_400_000).toISOString().slice(0, 10);
  many.forEach((task, index) => {
    const start = Math.floor(index / 4) - 30 + Math.floor(rand() * 10);
    task.startDate = day(start);
    task.dueDate = day(start + 1 + Math.floor(rand() * 6));
  });
  for (let index = 0; index < Math.min(count, MOST); index += 1) {
    const from = Math.floor(rand() * (many.length - 1));
    const to = Math.min(many.length - 1, from + 1 + Math.floor(rand() * 8));
    const a = many[from];
    const b = many[to];
    if (a && b && a !== b) {
      state.relations.push({ id: `r-many-${String(index)}`, from: a.id, to: b.id, kind: "blocks" });
    }
  }
}
