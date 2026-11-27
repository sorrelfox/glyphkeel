// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Plus, X } from "lucide-react";
import { Button, useLocale } from "react-aria-components";

import { type QueryChip } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import styles from "./List.module.css";

/** Fields with a name of their own in the message files; others show as typed. */
const NAMED_FIELDS = new Set([
  "key",
  "project",
  "title",
  "text",
  "description",
  "comment",
  "status",
  "category",
  "type",
  "priority",
  "label",
  "sprint",
  "parent",
  "start",
  "due",
  "created",
  "updated",
  "closed",
  "estimate",
  "spent",
  "remaining",
  "progress",
  "is",
]);

const OPERATORS: Record<string, string> = { ":": "is", "=": "is", "<": "lt", "<=": "le", ">": "gt", ">=": "ge", "!=": "is-not" };

/** A field's name in words, or as written when it has none. */
const fieldName = (t: Translate, field: string) => (NAMED_FIELDS.has(field) ? t(`list-field-${field}`) : field);

/** A chip in words: "Status is not Backlog", "Due before +7d", "Text contains login". */
export function chipText(t: Translate, chip: QueryChip, values: string): string {
  if (chip.field === undefined) {
    return t("list-chip-text", { values });
  }
  // The history: "Status was Review", "Due changed after -7d", "Changed by agent".
  const history = /^(was|changed):(.+)$/u.exec(chip.field);
  if (history) {
    const field = fieldName(t, history[2] ?? "");
    if (history[1] === "was") {
      return t(chip.negated ? "list-chip-was-not" : "list-chip-was", { field, values });
    }
    const op = OPERATORS[chip.op ?? ":"] ?? "is";
    return t(chip.negated ? "list-chip-changed-not" : "list-chip-changed", { field, op, values });
  }
  if (chip.field === "changed") {
    return t(chip.negated ? "list-chip-changed-any-not" : "list-chip-changed-any", { values });
  }
  if (chip.field === "changed-by") {
    return t(chip.negated ? "list-chip-changed-by-not" : "list-chip-changed-by", { values });
  }
  const field = fieldName(t, chip.field);
  let operator = OPERATORS[chip.op ?? ":"] ?? "is";
  if (chip.negated) {
    operator = operator === "is" ? "is-not" : operator === "is-not" ? "is" : `not-${operator}`;
  }
  return t(`list-chip-${operator}`, { field, values });
}

interface ChipsProps {
  chips: readonly QueryChip[] | undefined;
  /** The query has OR or parentheses: it is edited as text. */
  advanced: boolean;
  onRemove: (chip: QueryChip) => void;
  onAdd: () => void;
}

/**
 * The query as conditions: one chip per condition of a plain AND, free words
 * as one "Text contains" chip. Removing a chip removes only its text.
 */
export function Chips({ chips, advanced, onRemove, onAdd }: ChipsProps) {
  const t = useT();
  const { locale } = useLocale();
  const list = new Intl.ListFormat(locale, { type: "disjunction" });
  return (
    <div className={styles.chips} role="group" aria-label={t("list-conditions")}>
      {advanced ? (
        <span className="meta">{t("list-advanced")}</span>
      ) : (
        (chips ?? []).map((chip) => {
          const text = chipText(t, chip, list.format(chip.values));
          return (
            <span key={chip.ranges.map((range) => `${String(range.from)}-${String(range.to)}`).join(",")} className={styles.chip}>
              <span className="truncate">{text}</span>
              <Button className="btn flat icon sm" aria-label={t("list-chip-remove", { condition: text })} onPress={() => { onRemove(chip); }}>
                <X className="ic s" />
              </Button>
            </span>
          );
        })
      )}
      <Button className="btn flat sm" onPress={onAdd}>
        <Plus className="ic s" />
        <span>{t("list-add-condition")}</span>
      </Button>
    </div>
  );
}
