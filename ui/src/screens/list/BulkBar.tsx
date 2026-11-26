// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Trash2, X } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useAskCloseDuplicatesOf } from "../../components/DuplicatePrompt";
import { usePriorityName } from "../../components/Glyphs";
import { Select, type SelectOption } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { PRIORITIES } from "../../data/catalog";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { useFieldText } from "../../data/fields";
import { applyBulk, previewBulk, type ListNames } from "../../data/list";
import { useTrashTasks } from "../../data/safety";
import { type BulkChanges, type BulkResult, type CustomField } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { FieldEditor } from "../task/FieldsSection";
import { MoveTasksDialog } from "../task/MoveTasksDialog";
import styles from "./List.module.css";

interface BulkBarProps {
  /** The selected tasks. */
  keys: readonly string[];
  names: ListNames;
  /** The custom fields of the list's tasks. */
  fields?: readonly CustomField[];
  onClear: () => void;
}

interface Pending {
  changes: BulkChanges;
  /** The change in words: "Status → Review". */
  summary: string;
  preview: BulkResult;
}

const NONE = "";

/**
 * The bar over a selection: change the status, priority, labels or a custom
 * field of every selected task, or move them to another project. Each change
 * is previewed first (how many tasks change), then made as one Undo step. A
 * status that closes them asks first whether to close their open duplicates
 * too.
 */
export function BulkBar({ keys, names, fields = [], onClear }: BulkBarProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const priorityName = usePriorityName();
  const trash = useTrashTasks();
  const askDuplicates = useAskCloseDuplicatesOf();
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  // The project the selection is to move to: the move asks what to do with what it has no match for.
  const [moveTo, setMoveTo] = useState<string | null>(null);
  // The custom field being set for every selected task, and its value so far.
  const [editing, setEditing] = useState<{ field: CustomField; value: unknown } | null>(null);
  const fieldText = useFieldText();

  const preview = (changes: BulkChanges, summary: string) => {
    setBusy(true);
    previewBulk(call, keys, changes).then(
      (result) => {
        setPending({ changes, summary, preview: result });
        setBusy(false);
      },
      (error: unknown) => {
        toasts.show(errorToast(t, error));
        setBusy(false);
      },
    );
  };

  const apply = () => {
    if (!pending) {
      return;
    }
    setBusy(true);
    // Closing tasks asks, as everywhere, whether to close their open duplicates too.
    const status = pending.changes.status;
    const closing = status !== undefined && status !== null && names.closing.has(status);
    (closing ? askDuplicates(keys) : Promise.resolve(undefined))
      .then((close) => applyBulk(call, keys, { ...pending.changes, ...(close === undefined ? {} : { close_duplicates: close }) }, pending.preview.count))
      .then(
      (result) => {
        setPending(null);
        setBusy(false);
        toasts.show({ tone: "info", text: t("list-bulk-done", { count: result.changed }), action: { label: t("toast-undo"), run: undo } });
      },
      (error: unknown) => {
        toasts.show(errorToast(t, error));
        setBusy(false);
      },
    );
  };

  const placeholder = (label: string): SelectOption<string> => ({ id: NONE, label });
  const statusOptions = [placeholder(t("list-bulk-status")), ...names.statuses.map((name) => ({ id: name, label: name }))];
  const priorityOptions = [placeholder(t("list-bulk-priority")), ...PRIORITIES.toReversed().map((priority) => ({ id: priority, label: priorityName(priority) }))];
  const addLabelOptions = [placeholder(t("list-bulk-add-label")), ...names.labels.map((name) => ({ id: name, label: name }))];
  const removeLabelOptions = [placeholder(t("list-bulk-remove-label")), ...names.labels.map((name) => ({ id: name, label: name }))];
  const projectOptions = [placeholder(t("list-bulk-move")), ...names.projects.map((key) => ({ id: key, label: key }))];
  // Field names are the person's own words; several projects' fields of one query name are one choice.
  const fieldChoices = [...new Map(fields.map((field) => [field.query_name, field])).values()];
  const fieldOptions = [placeholder(t("list-bulk-field")), ...fieldChoices.map((field) => ({ id: field.query_name, label: field.name }))];
  // A field is named by its query name, which fields of the same name share across projects.
  const fieldKey = (field: CustomField) => field.query_name.replace(/^cf\./, "");
  const setField = (field: CustomField, value: unknown) => {
    const empty = value === undefined || value === null || (Array.isArray(value) && value.length === 0) || value === "";
    const changes: BulkChanges = { fields: { [fieldKey(field)]: empty ? null : value } };
    setEditing(null);
    preview(
      changes,
      empty ? t("list-bulk-summary-field-clear", { field: field.name }) : t("list-bulk-summary-field", { field: field.name, value: fieldText(field.kind, value) }),
    );
  };

  return (
    <div className={classes("toolbar", styles.bulkBar)} role="toolbar" aria-label={t("list-bulk-label")} data-testid="bulk-bar">
      <span className={styles.bulkCount}>{t("list-selected", { count: keys.length })}</span>
      {pending ? (
        <>
          <span className={styles.bulkPreview} role="status">
            {t("list-bulk-preview", { change: pending.summary, count: pending.preview.changed, total: pending.preview.count })}
          </span>
          {(pending.preview.warnings ?? []).map((warning) => (
            <span key={warning} className="meta">
              {warning}
            </span>
          ))}
          <div className="grow" />
          <Button className="btn sm" onPress={() => { setPending(null); }} isDisabled={busy}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary sm" onPress={apply} isDisabled={busy || pending.preview.changed === 0}>
            {t("list-bulk-apply", { count: pending.preview.changed })}
          </Button>
        </>
      ) : editing ? (
        <>
          <span className={styles.bulkPreview}>{editing.field.name}</span>
          <span className={styles.bulkField} data-testid="bulk-field-editor">
            <FieldEditor
              field={editing.field}
              value={editing.value}
              readOnly={busy}
              onChange={(value) => {
                setEditing({ field: editing.field, value });
              }}
            />
          </span>
          <div className="grow" />
          <Button className="btn sm" onPress={() => { setEditing(null); }} isDisabled={busy}>
            {t("dialog-cancel")}
          </Button>
          {!editing.field.required && (
            <Button className="btn sm" onPress={() => { setField(editing.field, null); }} isDisabled={busy}>
              {t("list-bulk-field-clear")}
            </Button>
          )}
          <Button
            className="btn primary sm"
            onPress={() => { setField(editing.field, editing.value); }}
            isDisabled={busy || editing.value === undefined || editing.value === null}
          >
            {t("list-bulk-field-set")}
          </Button>
        </>
      ) : (
        <>
          <Select
            label={t("list-bulk-status")}
            className={styles.bulkSelect}
            options={statusOptions}
            value={NONE}
            isDisabled={busy}
            onChange={(status) => {
              if (status !== NONE) {
                preview({ status }, t("list-bulk-summary-status", { value: status }));
              }
            }}
          />
          <Select
            label={t("list-bulk-priority")}
            className={styles.bulkSelect}
            options={priorityOptions}
            value={NONE}
            isDisabled={busy}
            onChange={(value) => {
              const priority = PRIORITIES.find((candidate) => candidate === value);
              if (priority !== undefined) {
                preview({ priority }, t("list-bulk-summary-priority", { value: priorityName(priority) }));
              }
            }}
          />
          <Select
            label={t("list-bulk-add-label")}
            className={styles.bulkSelect}
            options={addLabelOptions}
            value={NONE}
            isDisabled={busy || names.labels.length === 0}
            onChange={(label) => {
              if (label !== NONE) {
                preview({ add_labels: [label] }, t("list-bulk-summary-add-label", { value: label }));
              }
            }}
          />
          <Select
            label={t("list-bulk-remove-label")}
            className={styles.bulkSelect}
            options={removeLabelOptions}
            value={NONE}
            isDisabled={busy || names.labels.length === 0}
            onChange={(label) => {
              if (label !== NONE) {
                preview({ remove_labels: [label] }, t("list-bulk-summary-remove-label", { value: label }));
              }
            }}
          />
          {fieldChoices.length > 0 && (
            <Select
              label={t("list-bulk-field")}
              className={styles.bulkSelect}
              options={fieldOptions}
              value={NONE}
              isDisabled={busy}
              onChange={(name) => {
                const field = fieldChoices.find((each) => each.query_name === name);
                if (field !== undefined) {
                  setEditing({ field, value: field.kind === "boolean" ? true : undefined });
                }
              }}
            />
          )}
          <Select
            label={t("list-bulk-move")}
            className={styles.bulkSelect}
            options={projectOptions}
            value={NONE}
            isDisabled={busy}
            onChange={(project) => {
              if (project !== NONE) {
                setMoveTo(project);
              }
            }}
          />
          <div className="grow" />
          <Button className="btn flat sm danger" isDisabled={busy} onPress={() => { trash(keys, onClear); }}>
            <Trash2 className="ic s" />
            {t("trash-move", { count: keys.length })}
          </Button>
        </>
      )}
      <Button className="btn flat icon sm" aria-label={t("list-bulk-clear")} onPress={onClear}>
        <X className="ic s" />
      </Button>
      {moveTo !== null && (
        <MoveTasksDialog
          keys={keys}
          to={moveTo}
          onClose={(moved) => {
            setMoveTo(null);
            if (moved !== undefined) {
              onClear();
            }
          }}
        />
      )}
    </div>
  );
}
