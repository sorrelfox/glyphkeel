// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The task panel's custom fields: one row per field the task's type shows,
// each edited in place. A field's name and options are the person's own
// words, shown as written.

import { ExternalLink, X } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useUpdateTask } from "../../data/board";
import { plainText, shownFor, useCustomFields } from "../../data/fields";
import { type CustomField, type Task } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { openLink } from "../../ipc/links";
import { customRuleField } from "../../data/rules";
import styles from "./Fields.module.css";

/** The section, when the task's project has fields for its type or the task has values. */
export function FieldsSection({ task, readOnly, locked }: { task: Task; readOnly: boolean; locked?: ReadonlySet<string> }) {
  const t = useT();
  const fields = useCustomFields(task.project);
  const update = useUpdateTask();
  const values = new Map((task.fields ?? []).map((value) => [value.id, value.value]));
  const shown = fields.filter((field) => shownFor(field, task.type.name) || values.has(field.id));
  if (shown.length === 0) {
    return null;
  }
  const set = (field: CustomField, value: unknown) => {
    update.mutate({ key: task.key, fields: { [field.id]: value } });
  };
  return (
    <section className={styles.section} aria-label={t("fields-title")}>
      <span className="caption">{t("fields-title")}</span>
      <div className={styles.grid}>
        {shown.map((field) => (
          <div key={field.id} className={styles.row} data-testid={`field-${field.name}`}>
            <span className={classes("truncate", styles.name)} title={field.name}>
              {field.name}
              {field.required && (
                <span className={styles.required} role="img" aria-label={t("field-required-note", { field: field.name })}>
                  *
                </span>
              )}
            </span>
            <FieldEditor field={field} value={values.get(field.id)} readOnly={readOnly || locked?.has(customRuleField(field.id)) === true} onChange={(value) => { set(field, value); }} />
          </div>
        ))}
      </div>
    </section>
  );
}

export interface EditorProps {
  field: CustomField;
  value: unknown;
  readOnly: boolean;
  /** A new value; null clears it. */
  onChange: (value: unknown) => void;
}

const NONE = "\u0000none";

/** The editor of one custom field's value, by its kind. */
export function FieldEditor({ field, value, readOnly, onChange }: EditorProps) {
  const t = useT();
  switch (field.kind) {
    case "boolean":
      return (
        <Switch
          aria-label={field.name}
          isSelected={value === true}
          isDisabled={readOnly}
          onChange={(on) => {
            onChange(on);
          }}
        >
          {t(value === true ? "field-yes" : "field-no")}
        </Switch>
      );
    case "select": {
      const options = field.options ?? [];
      return (
        <Select
          className={styles.select}
          label={t("field-edit", { field: field.name })}
          value={typeof value === "string" ? value : NONE}
          isDisabled={readOnly}
          onChange={(chosen) => {
            onChange(chosen === NONE ? null : chosen);
          }}
          options={[{ id: NONE, label: t("field-not-set") }, ...options.map((option) => ({ id: option, label: option }))]}
        />
      );
    }
    case "multi_select": {
      const chosen = Array.isArray(value) ? value.map(plainText) : [];
      return (
        <div className={styles.options} role="group" aria-label={field.name}>
          {(field.options ?? []).map((option) => {
            const on = chosen.includes(option);
            return (
              <Button
                key={option}
                className={classes("chip", on ? "hue-blue" : "outline", styles.option)}
                aria-pressed={on}
                isDisabled={readOnly}
                onPress={() => {
                  const next = on ? chosen.filter((each) => each !== option) : [...chosen, option];
                  onChange(next.length === 0 ? null : next);
                }}
              >
                {option}
              </Button>
            );
          })}
        </div>
      );
    }
    default:
      return <TextEditor field={field} value={value} readOnly={readOnly} onChange={onChange} />;
  }
}

/** Text, numbers, dates, links and Markdown: a field that saves when it loses the focus or on Enter. */
function TextEditor({ field, value, readOnly, onChange }: EditorProps) {
  const t = useT();
  const stored = plainText(value);
  const [draft, setDraft] = useState(stored);
  const [seen, setSeen] = useState(stored);
  if (stored !== seen) {
    // The value changed, here or elsewhere: show it.
    setSeen(stored);
    setDraft(stored);
  }
  const save = () => {
    if (draft.trim() === stored.trim()) {
      return;
    }
    onChange(draft.trim() === "" ? null : field.kind === "integer" || field.kind === "decimal" ? Number(draft) : draft);
  };
  const label = t("field-edit", { field: field.name });
  const common = {
    "aria-label": label,
    value: draft,
    disabled: readOnly,
    onChange: (event: { target: { value: string } }) => {
      setDraft(event.target.value);
    },
    onBlur: save,
  };
  return (
    <span className={styles.editor}>
      {field.kind === "markdown" ? (
        <span className={classes("field", styles.grow)}>
          <textarea {...common} rows={3} className={styles.textarea} />
        </span>
      ) : (
        <span className={classes("field", styles.grow)}>
          <input
            {...common}
            type={field.kind === "date" ? "date" : field.kind === "integer" || field.kind === "decimal" ? "number" : field.kind === "url" ? "url" : "text"}
            step={field.kind === "decimal" ? "any" : undefined}
            inputMode={field.kind === "integer" ? "numeric" : field.kind === "decimal" ? "decimal" : undefined}
            placeholder={t("field-not-set")}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                save();
              } else if (event.key === "Escape") {
                setDraft(stored);
              }
            }}
          />
        </span>
      )}
      {field.kind === "url" && stored !== "" && (
        <Button
          className="btn flat icon sm"
          aria-label={t("field-open-link", { link: stored })}
          onPress={() => {
            openLink(stored).catch(() => undefined);
          }}
        >
          <ExternalLink className="ic s" />
        </Button>
      )}
      {stored !== "" && !readOnly && !field.required && (
        <Button
          className="btn flat icon sm"
          aria-label={t("field-clear", { field: field.name })}
          onPress={() => {
            onChange(null);
          }}
        >
          <X className="ic s" />
        </Button>
      )}
    </span>
  );
}
