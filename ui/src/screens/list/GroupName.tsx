// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { PriorityGlyph, usePriorityName } from "../../components/Glyphs";
import { PRIORITIES } from "../../data/catalog";
import { type CustomField, type ListGroup } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useDayFormat } from "./cells";
import { isFieldGrouping, type Grouping } from "./model";

/** The custom field a grouping is by, among `fields`. */
export function groupingField(grouping: Grouping | undefined, fields: readonly CustomField[]): CustomField | undefined {
  return isFieldGrouping(grouping) ? fields.find((field) => field.query_name === grouping) : undefined;
}

/** A group's name as text, as its header shows it (without the priority's glyph). */
export function useGroupText(fields: readonly CustomField[] = []): (grouping: Grouping, group: ListGroup) => string {
  const t = useT();
  const priorityName = usePriorityName();
  const day = useDayFormat();
  return (grouping, group) => {
    const value = group.value;
    if (isFieldGrouping(grouping)) {
      const field = groupingField(grouping, fields);
      if (value === undefined) {
        return t("list-group-none-field", { field: field?.name ?? grouping });
      }
      // Options are the person's own words; yes and no are translated.
      return field?.kind === "boolean" ? t(value === "yes" ? "field-yes" : "field-no") : value;
    }
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
export function GroupName({ grouping, group, fields = [] }: { grouping: Grouping; group: ListGroup; fields?: readonly CustomField[] }) {
  const t = useT();
  const priorityName = usePriorityName();
  const day = useDayFormat();
  const text = useGroupText(fields);
  const value = group.value;
  if (isFieldGrouping(grouping)) {
    return <span className="truncate">{text(grouping, group)}</span>;
  }
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
