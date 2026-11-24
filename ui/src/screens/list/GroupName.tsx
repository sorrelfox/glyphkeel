// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { PriorityGlyph, usePriorityName } from "../../components/Glyphs";
import { PRIORITIES } from "../../data/catalog";
import { type ListGroup, type ListGrouping } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useDayFormat } from "./cells";

/** A group's name as text, as its header shows it (without the priority's glyph). */
export function useGroupText(): (grouping: ListGrouping, group: ListGroup) => string {
  const t = useT();
  const priorityName = usePriorityName();
  const day = useDayFormat();
  return (grouping, group) => {
    const value = group.value;
    if (value === undefined) {
      return t(grouping === "label" || grouping === "parent" || grouping === "due_week" ? `list-group-none-${grouping}` : "list-none");
    }
    switch (grouping) {
      case "priority":
        return priorityName(PRIORITIES.find((candidate) => candidate === value) ?? "none");
      case "parent":
      case "project":
        return t("list-group-titled", { key: value, title: group.title ?? "" });
      case "due_week":
        return /^\d{4}-\d{2}-\d{2}$/.test(value) ? t("list-group-week", { day: day(value) }) : value;
      case "status":
      case "label":
        return value;
    }
  };
}

/**
 * A group's name on its header: the status, label or project, the
 * priority with its glyph, the parent's key and title, or the due week;
 * the tasks without a value get "No label", "No parent" or "No due date".
 */
export function GroupName({ grouping, group }: { grouping: ListGrouping; group: ListGroup }) {
  const t = useT();
  const priorityName = usePriorityName();
  const day = useDayFormat();
  const value = group.value;
  if (value === undefined) {
    const none = grouping === "label" || grouping === "parent" || grouping === "due_week" ? `list-group-none-${grouping}` : "list-none";
    return <span className="truncate">{t(none)}</span>;
  }
  switch (grouping) {
    case "priority": {
      const priority = PRIORITIES.find((candidate) => candidate === value) ?? "none";
      return (
        <>
          <PriorityGlyph priority={priority} />
          <span className="truncate">{priorityName(priority)}</span>
        </>
      );
    }
    case "parent":
    case "project":
      return <span className="truncate">{t("list-group-titled", { key: value, title: group.title ?? "" })}</span>;
    case "due_week":
      // While another grouping's rows still show, their values aren't days.
      return <span className="truncate">{/^\d{4}-\d{2}-\d{2}$/.test(value) ? t("list-group-week", { day: day(value) }) : value}</span>;
    case "status":
    case "label":
      return <span className="truncate">{value}</span>;
  }
}
