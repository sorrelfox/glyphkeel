// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Trash2, X } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { usePriorityName } from "../../components/Glyphs";
import { Select, type SelectOption } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { PRIORITIES } from "../../data/catalog";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { applyBulk, previewBulk, type ListNames } from "../../data/list";
import { useTrashTasks } from "../../data/safety";
import { type BulkChanges, type BulkResult } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { MoveTasksDialog } from "../task/MoveTasksDialog";
import styles from "./List.module.css";

interface BulkBarProps {
  /** The selected tasks. */
  keys: readonly string[];
  names: ListNames;
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
 * The bar over a selection: change the status, priority or labels of every
 * selected task, or move them to another project. Each change is previewed
 * first (how many tasks change), then made as one Undo step.
 */
export function BulkBar({ keys, names, onClear }: BulkBarProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const priorityName = usePriorityName();
  const trash = useTrashTasks();
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  // The project the selection is to move to: the move asks what to do with what it has no match for.
  const [moveTo, setMoveTo] = useState<string | null>(null);

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
    applyBulk(call, keys, pending.changes, pending.preview.count).then(
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
