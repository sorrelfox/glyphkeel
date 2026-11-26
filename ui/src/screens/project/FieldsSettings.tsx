// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Fields page of project settings: the custom fields the project's
// tasks have, its own and those of every project, each with its kind,
// options, task types, default and whether it is required. Every change
// applies at once, as on the other pages.

import { ArrowDown, ArrowUp, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { type Project } from "../../data/catalog";
import { FIELD_KINDS, OPTION_KINDS, plainText, useCustomFields } from "../../data/fields";
import { useCommand } from "../../data/hooks";
import { type CustomField, type CustomFieldKind } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Fields.module.css";

/** A text field that saves when it loses the focus or on Enter, if it changed. */
function SaveText({ label, value, placeholder, onSave, className }: { label: string; value: string; placeholder?: string | undefined; onSave: (value: string) => void; className?: string | undefined }) {
  const [text, setText] = useState(value);
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    setSeen(value);
    setText(value);
  }
  const save = () => {
    if (text !== value) {
      onSave(text);
    }
  };
  return (
    <span className={classes("field", className)}>
      <input
        aria-label={label}
        value={text}
        placeholder={placeholder}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            save();
          }
        }}
      />
    </span>
  );
}

export function FieldsSettings({ project }: { project: Project }) {
  const t = useT();
  const fields = useCustomFields(project.key);
  const create = useCommand("field.create", { undoable: t("field-changed") });
  const [name, setName] = useState("");
  const [kind, setKind] = useState<CustomFieldKind>("text");
  const [everywhere, setEverywhere] = useState(false);
  const kinds = FIELD_KINDS.map((each) => ({ id: each, label: t(`field-kind-${each}`) }));
  return (
    <div className={styles.page}>
      {fields.length === 0 && <p className="muted">{t("fields-none")}</p>}
      {fields.map((field) => (
        <FieldBlock key={field.id} field={field} project={project} siblings={fields.filter((each) => (each.project ?? null) === (field.project ?? null))} />
      ))}
      <form
        className={styles.add}
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() === "") {
            return;
          }
          // A select field starts with one option, which the person renames.
          const options = OPTION_KINDS.has(kind) ? [t("field-option-name")] : undefined;
          create.mutate({ ...(everywhere ? {} : { project: project.key }), name, kind, ...(options ? { options } : {}) });
          setName("");
        }}
      >
        <span className="field grow">
          <Plus className="ic" />
          <input aria-label={t("field-add-name")} placeholder={t("field-add-name")} value={name} onChange={(event) => { setName(event.target.value); }} />
        </span>
        <Select className={styles.kind} label={t("field-add-kind")} value={kind} onChange={setKind} options={kinds} />
        <Switch isSelected={everywhere} onChange={setEverywhere}>
          {t("field-add-all-projects")}
        </Switch>
        <button type="submit" className="btn" disabled={name.trim() === ""}>
          {t("field-add")}
        </button>
      </form>
    </div>
  );
}

function FieldBlock({ field, project, siblings }: { field: CustomField; project: Project; siblings: readonly CustomField[] }) {
  const t = useT();
  const update = useCommand("field.update", { undoable: t("field-changed") });
  const remove = useCommand("field.delete");
  const [confirming, setConfirming] = useState(false);
  const [option, setOption] = useState("");
  const ref = { ...(field.project !== undefined ? { project: field.project } : {}), field: field.id };
  const index = siblings.findIndex((each) => each.id === field.id);
  const options = field.options ?? [];
  const types = field.types ?? [];
  const own = field.project !== undefined;
  return (
    <div className={styles.block} data-testid={`field-settings-${field.name}`}>
      <div className={styles.head}>
        <SaveText className={styles.name} label={t("field-name")} value={field.name} onSave={(value) => { update.mutate({ ...ref, name: value }); }} />
        <span className="chip outline">{t(`field-kind-${field.kind}`)}</span>
        <span className="chip outline">{t(own ? "field-scope-project" : "field-scope-all")}</span>
        <Switch
          isSelected={field.required}
          onChange={(required) => {
            update.mutate({ ...ref, required });
          }}
        >
          {t("field-is-required")}
        </Switch>
        <span className={styles.buttons}>
          <Button
            className="btn flat icon sm"
            aria-label={t("field-move-up", { field: field.name })}
            isDisabled={index <= 0}
            onPress={() => {
              const previous = siblings[index - 1];
              if (previous) {
                update.mutate({ ...ref, before: previous.id });
              }
            }}
          >
            <ArrowUp className="ic s" />
          </Button>
          <Button
            className="btn flat icon sm"
            aria-label={t("field-move-down", { field: field.name })}
            isDisabled={index < 0 || index >= siblings.length - 1}
            onPress={() => {
              const next = siblings[index + 1];
              if (next) {
                update.mutate({ ...ref, after: next.id });
              }
            }}
          >
            <ArrowDown className="ic s" />
          </Button>
          <Button className="btn flat icon sm" aria-label={t("field-delete", { field: field.name })} onPress={() => { setConfirming(true); }}>
            <Trash2 className="ic s" />
          </Button>
        </span>
      </div>
      <p className={classes("muted", styles.note)}>{t("field-query-name", { name: field.query_name })}</p>
      {!own && <p className={classes("muted", styles.note)}>{t("field-shared-note")}</p>}
      {confirming && (
        <div className={styles.confirm} role="group" aria-label={t("field-delete", { field: field.name })}>
          <Button className="btn" onPress={() => { setConfirming(false); }}>
            {t("dialog-cancel")}
          </Button>
          <Button
            className="btn danger"
            onPress={() => {
              remove.mutate({ ...ref, expected_task_count: field.tasks });
              setConfirming(false);
            }}
          >
            {t("field-delete-confirm", { field: field.name, count: field.tasks })}
          </Button>
        </div>
      )}
      {OPTION_KINDS.has(field.kind) && (
        <div className={styles.detail}>
          <span className={styles.label}>{t("field-options")}</span>
          <div className={styles.optionList}>
            {options.map((each) => (
              <span key={each} className={styles.optionRow}>
                <SaveText
                  label={t("field-option-name")}
                  value={each}
                  onSave={(value) => {
                    if (value.trim() !== "") {
                      update.mutate({ ...ref, rename_options: [{ from: each, to: value }] });
                    }
                  }}
                />
                <Button
                  className="btn flat icon sm"
                  aria-label={t("field-option-delete", { option: each })}
                  isDisabled={options.length < 2}
                  onPress={() => {
                    update.mutate({ ...ref, options: options.filter((candidate) => candidate !== each) });
                  }}
                >
                  <X className="ic s" />
                </Button>
              </span>
            ))}
            <form
              className={styles.optionRow}
              onSubmit={(event) => {
                event.preventDefault();
                if (option.trim() !== "") {
                  update.mutate({ ...ref, options: [...options, option] });
                  setOption("");
                }
              }}
            >
              <span className="field">
                <Plus className="ic" />
                <input aria-label={t("field-option-add")} placeholder={t("field-option-add")} value={option} onChange={(event) => { setOption(event.target.value); }} />
              </span>
            </form>
          </div>
        </div>
      )}
      {own && (
        <div className={styles.detail}>
          <span className={styles.label}>{t("field-types")}</span>
          <div className={styles.types}>
            {project.types.map((type) => {
              const on = types.length === 0 || types.some((name) => name.localeCompare(type.name, undefined, { sensitivity: "base" }) === 0);
              return (
                <Switch
                  key={type.id}
                  isSelected={on}
                  onChange={(selected) => {
                    const current = types.length === 0 ? project.types.map((each) => each.name) : types;
                    const next = selected ? [...current, type.name] : current.filter((name) => name.localeCompare(type.name, undefined, { sensitivity: "base" }) !== 0);
                    // Every type chosen is the same as none chosen.
                    update.mutate(next.length === 0 || next.length >= project.types.length ? { ...ref, clear: ["types"] } : { ...ref, types: next });
                  }}
                >
                  {type.name}
                </Switch>
              );
            })}
          </div>
        </div>
      )}
      <div className={styles.detail}>
        <span className={styles.label}>{t("field-default")}</span>
        <DefaultValue field={field} onChange={(value) => { update.mutate(value === null ? { ...ref, clear: ["default"] } : { ...ref, default: value }); }} />
      </div>
    </div>
  );
}

const NONE = "\u0000none";

/** The value new tasks get: an option, yes or no, or text the engine reads by the field's kind. */
function DefaultValue({ field, onChange }: { field: CustomField; onChange: (value: unknown) => void }) {
  const t = useT();
  const value = field.default;
  if (field.kind === "boolean" || field.kind === "select") {
    const choices = field.kind === "boolean" ? [{ id: "true", label: t("field-yes") }, { id: "false", label: t("field-no") }] : (field.options ?? []).map((option) => ({ id: option, label: option }));
    return (
      <Select
        className={styles.kind}
        label={t("field-default")}
        value={value === undefined ? NONE : plainText(value)}
        onChange={(chosen) => {
          onChange(chosen === NONE ? null : field.kind === "boolean" ? chosen === "true" : chosen);
        }}
        options={[{ id: NONE, label: t("field-default-none") }, ...choices]}
      />
    );
  }
  const text = value === undefined ? "" : Array.isArray(value) ? value.map(plainText).join(", ") : plainText(value);
  return (
    <SaveText
      label={t("field-default")}
      placeholder={t("field-default-none")}
      value={text}
      onSave={(next) => {
        onChange(next.trim() === "" ? null : field.kind === "multi_select" ? next.split(",").map((each) => each.trim()).filter((each) => each !== "") : next);
      }}
    />
  );
}
