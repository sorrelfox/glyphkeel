// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The board's swimlanes in the fake engine, as the engine groups them:
// priorities, types, open sprints and a select field's options always have
// their lane; labels, parents, projects and completed sprints while a card
// shows them;
// the lane of the cards without a value comes last (a type always has one).
// Types and labels are matched by name across sub-projects.

import { type Column, type LaneBy, type Swimlane } from "../../generated/ops";
import { type FakeField } from "./fields";
import { type FakeSprint } from "./sprints";
import { type FakeProject, type FakeTask } from "./state";

const fold = (text: string) => text.normalize("NFKC").toLowerCase().trim();

/** The ID of the lane of the cards without a value. */
export const EMPTY_LANE = "none";

/** What the lanes need beyond the cards. */
export interface LaneSources {
  by: LaneBy;
  projects: readonly FakeProject[];
  sprints: readonly FakeSprint[];
  field: FakeField | undefined;
  /** The task behind a card, by task ID. */
  taskOf: (id: string) => FakeTask | undefined;
  keyOf: (task: FakeTask) => string;
}

const lane = (id: string, name: string, value: string | undefined): Swimlane => ({
  id,
  name,
  ...(value === undefined ? {} : { value }),
  empty: false,
  settable: true,
  count: 0,
  counts: {},
});

/** Gives every shown card of `columns` its lane, and returns the lanes in order with their counts. */
export function assignLanes(columns: Column[], sources: LaneSources): Swimlane[] {
  const { by } = sources;
  if (by === "none") {
    return [];
  }
  const fixed: Swimlane[] = [];
  const byName = new Map<string, string>();
  if (by === "priority") {
    for (const priority of ["urgent", "high", "normal", "low"]) {
      fixed.push(lane(priority, priority, priority));
    }
  } else if (by === "type") {
    for (const project of sources.projects) {
      for (const type of project.types) {
        if (!byName.has(fold(type.name))) {
          byName.set(fold(type.name), type.id);
          fixed.push({ ...lane(type.id, type.name, type.name), hue: type.hue });
        }
      }
    }
  } else if (by === "sprint") {
    const ids = new Set(sources.projects.map((project) => project.id));
    const open = sources.sprints.filter((sprint) => ids.has(sprint.projectId) && sprint.state !== "closed");
    open.sort((a, b) => (a.state === "active" ? 0 : 1) - (b.state === "active" ? 0 : 1) || a.rank - b.rank);
    for (const sprint of open) {
      fixed.push(lane(sprint.id, sprint.name, sprint.id));
    }
  } else if (by === "field" && sources.field) {
    for (const option of sources.field.options) {
      fixed.push(lane(option, option, option));
    }
  }
  const found: { lane: Swimlane; order: string; named: boolean }[] = [];
  const tally = new Map<string, Record<string, number>>();
  const shown = columns.filter((column) => !column.hidden);
  let hasEmpty = false;
  for (const column of shown) {
    for (const card of column.cards) {
      const task = sources.taskOf(card.id);
      let id: string | undefined;
      let draft: { lane: Swimlane; order: string; named: boolean } | undefined;
      switch (by) {
        case "priority":
          id = card.priority === "none" ? undefined : card.priority;
          break;
        case "type": {
          id = byName.get(fold(card.type.name)) ?? card.type.id;
          break;
        }
        case "label": {
          const first = card.labels[0];
          if (first) {
            id = first.id;
            draft = { lane: { ...lane(first.id, first.name, first.name), hue: first.hue }, order: fold(first.name), named: true };
          }
          break;
        }
        case "sprint": {
          if (card.sprint) {
            id = card.sprint.id;
            draft = { lane: { ...lane(card.sprint.id, card.sprint.name, card.sprint.id), settable: card.sprint.state !== "closed" }, order: card.sprint.end_date ?? "", named: false };
          }
          break;
        }
        case "parent": {
          const parent = task?.parentId === null || task?.parentId === undefined ? undefined : sources.taskOf(task.parentId);
          if (parent) {
            id = parent.id;
            const key = sources.keyOf(parent);
            const project = key.slice(0, key.lastIndexOf("-"));
            const number = key.slice(key.lastIndexOf("-") + 1);
            draft = { lane: { ...lane(parent.id, parent.title, parent.id), key }, order: `${project}-${number.padStart(12, "0")}`, named: false };
          }
          break;
        }
        case "project": {
          // A card changes project by a move of its own, never by a drop.
          const at = sources.projects.findIndex((project) => project.key === card.project);
          const project = sources.projects[at];
          id = card.project;
          draft = { lane: { ...lane(card.project, project?.name ?? card.project, undefined), hue: project?.hue ?? "grey", settable: false }, order: String(at < 0 ? 999_999 : at).padStart(6, "0"), named: false };
          break;
        }
        case "field": {
          const value = sources.field && task?.fields?.[sources.field.id];
          if (typeof value === "string") {
            id = value;
            draft = { lane: lane(value, value, value), order: fold(value), named: false };
          }
          break;
        }
      }
      if (id === undefined) {
        hasEmpty = true;
        id = EMPTY_LANE;
      }
      if (draft && !fixed.some((each) => each.id === draft.lane.id)) {
        const same = found.find((each) => each.lane.id === draft.lane.id || (draft.named && each.named && each.order === draft.order));
        if (same) {
          id = same.lane.id;
        } else {
          found.push(draft);
        }
      }
      const counts = tally.get(id) ?? {};
      counts[column.id] = (counts[column.id] ?? 0) + 1;
      tally.set(id, counts);
      card.lane = id;
    }
  }
  found.sort((a, b) => (a.order < b.order ? -1 : a.order > b.order ? 1 : a.lane.id < b.lane.id ? -1 : 1));
  const lanes = [...fixed, ...found.map((each) => each.lane)];
  if ((by !== "type" && by !== "project") || hasEmpty) {
    lanes.push({ ...lane(EMPTY_LANE, "", undefined), empty: true });
  }
  for (const each of lanes) {
    const counts = tally.get(each.id) ?? {};
    each.counts = Object.fromEntries(shown.map((column) => [column.id, counts[column.id] ?? 0]));
    each.count = Object.values(each.counts).reduce((sum, n) => sum + n, 0);
  }
  return lanes;
}
