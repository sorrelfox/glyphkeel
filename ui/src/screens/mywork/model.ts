// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The board across projects ("My work"), worked out from the engine's
// answer: the quick toggles as parts of the query, the board in the shape
// the board's columns and lanes draw, and the statuses a card dropped into
// a column can take. Pure functions, so the rules are tested without a page.

import { type AcrossProject, type AcrossStatus, type Board, type BoardAcross, type BoardLayout, type Card, type Category, type LaneBy } from "../../generated/ops";

/** The quick toggles, each a condition of the query language. */
export const TOGGLES = {
  week: "due:this-week",
  sprint: "sprint:active",
  high: "priority>=high",
} as const;
export type Toggle = keyof typeof TOGGLES;
export const TOGGLE_IDS = Object.keys(TOGGLES) as Toggle[];

/** A query with every toggle on that it holds as a whole condition, and the rest as typed. */
export function splitToggles(query: string): { typed: string; toggles: Toggle[] } {
  let typed = ` ${query.trim()} `;
  const toggles: Toggle[] = [];
  // Only a query of conditions joined by spaces holds a toggle as a part of it.
  if (!/\bOR\b|[()]/i.test(query)) {
    for (const id of TOGGLE_IDS) {
      const part = ` ${TOGGLES[id]} `;
      if (typed.includes(part)) {
        toggles.push(id);
        typed = typed.replace(part, " ");
      }
    }
  }
  return { typed: typed.trim().replace(/\s+/g, " "), toggles };
}

/** The query the board reads: what was typed, then the toggles that are on. */
export function joinQuery(typed: string, toggles: readonly Toggle[]): string {
  const text = typed.trim();
  const parts = TOGGLE_IDS.filter((id) => toggles.includes(id)).map((id) => TOGGLES[id]);
  if (parts.length === 0) {
    return text;
  }
  // Conditions joined by spaces must all hold; a typed OR stays one part.
  const own = text === "" ? [] : [/\bOR\b/i.test(text) && !/\bsort:/i.test(text) ? `(${text})` : text];
  return [...own, ...parts].join(" ");
}

/** The layout the board's columns are drawn with: the engine orders the cards, so there is no sorting by hand. */
export function acrossLayout(across: BoardAcross): BoardLayout {
  return {
    order: [],
    hidden: [],
    collapsed: [],
    sort_mode: "priority",
    column_sort: {},
    hide_done_after_days: across.hide_done_after_days,
    card_fields: ["type", "priority", "due", "labels", "checklist"],
    include_subprojects: false,
    colour_by: "none",
    colour_rules: [],
    covers: "off",
    lanes: across.lanes_by,
    lane_field: "",
  };
}

/** The board across projects in the shape of a project's board, for its columns and lanes. */
export function toBoard(across: BoardAcross): Board {
  return {
    project: "",
    columns: across.columns,
    layout: acrossLayout(across),
    hidden_columns: 0,
    hidden_tasks: 0,
    ...(across.lanes === undefined ? {} : { lanes: across.lanes }),
  };
}

/** The statuses of `project` in `category`, in the project's order. */
export function statusesIn(project: AcrossProject | undefined, category: Category): AcrossStatus[] {
  return (project?.statuses ?? []).filter((status) => status.category === category);
}

/** Where a card dropped into a column can go: nothing (it is there already), one status, or a choice. */
export type Landing = { kind: "none" } | { kind: "one"; status: AcrossStatus } | { kind: "choose"; statuses: AcrossStatus[] };

export function landing(card: Card, project: AcrossProject | undefined, from: Category | undefined, to: Category): Landing {
  if (from === to) {
    return { kind: "none" };
  }
  const statuses = statusesIn(project, to).filter((status) => status.id !== card.status?.id);
  if (statuses.length === 0) {
    return { kind: "none" };
  }
  const [first] = statuses;
  return statuses.length === 1 && first !== undefined ? { kind: "one", status: first } : { kind: "choose", statuses };
}

/** The board as it will look once the engine agrees: the card in its new column, with its new status, last. */
export function moveAcross(across: BoardAcross, key: string, column: string, status: AcrossStatus): BoardAcross {
  const found = across.columns.flatMap((each) => each.cards).find((card) => card.key === key);
  if (!found) {
    return across;
  }
  const moved: Card = { ...found, status: { id: status.id, name: status.name, hue: status.hue } };
  const columns = across.columns.map((each) => {
    const had = each.cards.some((card) => card.key === key);
    const cards = each.cards.filter((card) => card.key !== key);
    if (each.id === column) {
      return { ...each, cards: [...cards, moved], count: each.count + (had ? 0 : 1) };
    }
    return had ? { ...each, cards, count: each.count - 1 } : each;
  });
  return { ...across, columns };
}

/** The groupings a saved view can keep; a view without one groups by project. */
export function lanesOf(groupBy: string | undefined): LaneBy {
  const kinds: readonly LaneBy[] = ["project", "none", "parent", "priority", "label", "sprint", "type"];
  return kinds.find((kind) => kind === groupBy) ?? "project";
}
