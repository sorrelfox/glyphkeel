// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { addDays, SPRINT_DAYS } from "../../data/sprints";
import { useToday } from "../../data/zone";
import { type Sprint, type SprintField } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Sprints.module.css";

/** The complete dialog's choice that sends the open tasks back to the backlog. */
const BACKLOG = "*backlog";

/**
 * Runs a sprint command: success shows an Undo toast and closes the dialog
 * (if any); an error stays in the dialog, or shows as a toast without one.
 */
export function useSprintCall(onDone: () => void = () => undefined) {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  const { undo } = useUndo();
  const [error, setError] = useState<string | null>(null);
  const run = (work: Promise<unknown>, done: string, inDialog = true) => {
    work.then(
      () => {
        toasts.show({ tone: "info", text: done, action: { label: t("toast-undo"), run: undo } });
        onDone();
      },
      (failure: unknown) => {
        if (inDialog) {
          setError(errorToast(t, failure).text);
        } else {
          toasts.show(errorToast(t, failure));
        }
      },
    );
  };
  return { call, error, setError, run };
}

function DialogError({ error }: { error: string | null }) {
  return error === null ? null : (
    <p className={styles.error} role="alert">
      {error}
    </p>
  );
}

function DateRow({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className={styles.formRow}>
      <span className={styles.formLabel}>{label}</span>
      <span className="field">
        <input
          type="date"
          value={value}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      </span>
    </label>
  );
}

function GoalRow({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const t = useT();
  return (
    <label className={styles.formRow}>
      <span className={styles.formLabel}>{t("sprints-field-goal")}</span>
      <span className="field">
        <textarea
          rows={3}
          maxLength={4000}
          value={value}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      </span>
    </label>
  );
}

function close(onClose: () => void) {
  return (isOpen: boolean) => {
    if (!isOpen) {
      onClose();
    }
  };
}

interface SprintDialogProps {
  project: string;
  /** The sprint to change; none: a new one. */
  sprint?: Sprint;
  /** The name a new sprint is offered. */
  suggestedName?: string;
  /** The day a new sprint is offered to start on. */
  suggestedStart?: string;
  onClose: () => void;
}

/** Plans a sprint, or changes one: name, goal and dates. */
export function SprintDialog({ project, sprint, suggestedName = "", suggestedStart = "", onClose }: SprintDialogProps) {
  const t = useT();
  const { call, error, setError, run } = useSprintCall(onClose);
  const [name, setName] = useState(sprint?.name ?? suggestedName);
  const [goal, setGoal] = useState(sprint?.goal ?? "");
  const [start, setStart] = useState(sprint?.start_date ?? suggestedStart);
  const [end, setEnd] = useState(sprint?.end_date ?? (suggestedStart === "" ? "" : addDays(suggestedStart, SPRINT_DAYS - 1)));
  const edit = (set: (value: string) => void) => (value: string) => {
    set(value);
    setError(null);
  };
  const save = () => {
    if (name.trim() === "") {
      return;
    }
    if (sprint === undefined) {
      run(
        call("sprint.create", {
          project,
          name,
          ...(goal.trim() === "" ? {} : { goal }),
          ...(start === "" ? {} : { start_date: start }),
          ...(end === "" ? {} : { end_date: end }),
        }),
        t("sprints-created", { name: name.trim() }),
      );
      return;
    }
    const clear: SprintField[] = [];
    if (start === "" && sprint.start_date !== undefined) clear.push("start_date");
    if (end === "" && sprint.end_date !== undefined) clear.push("end_date");
    if (goal.trim() === "" && (sprint.goal ?? "") !== "") clear.push("goal");
    run(
      call("sprint.update", {
        sprint: sprint.id,
        ...(name !== sprint.name ? { name } : {}),
        ...(goal.trim() !== "" && goal !== sprint.goal ? { goal } : {}),
        ...(start !== "" && start !== sprint.start_date ? { start_date: start } : {}),
        ...(end !== "" && end !== sprint.end_date ? { end_date: end } : {}),
        ...(clear.length > 0 ? { clear } : {}),
      }),
      t("sprints-changed", { name: name.trim() }),
    );
  };
  return (
    <Dialog
      title={sprint === undefined ? t("sprints-new-title") : t("sprints-edit-title", { name: sprint.name })}
      isOpen
      onOpenChange={close(onClose)}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === ""} onPress={save}>
            {sprint === undefined ? t("sprints-create") : t("sprints-save")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("sprints-field-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              maxLength={100}
              onChange={(event) => {
                edit(setName)(event.target.value);
              }}
            />
          </span>
        </label>
        <DateRow label={t("sprints-field-start")} value={start} onChange={edit(setStart)} />
        <DateRow label={t("sprints-field-end")} value={end} onChange={edit(setEnd)} />
        <GoalRow value={goal} onChange={setGoal} />
        <DialogError error={error} />
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

/** Starts a planned sprint: its dates (by default from today, two weeks) and its goal. */
export function StartSprintDialog({ sprint, onClose }: { sprint: Sprint; onClose: () => void }) {
  const t = useT();
  const today = useToday();
  const { call, error, setError, run } = useSprintCall(onClose);
  const first = sprint.start_date !== undefined && sprint.start_date >= today ? sprint.start_date : today;
  const [start, setStart] = useState(first);
  const [end, setEnd] = useState(sprint.end_date !== undefined && sprint.end_date >= first ? sprint.end_date : addDays(first, SPRINT_DAYS - 1));
  const [goal, setGoal] = useState(sprint.goal ?? "");
  const begin = () => {
    run(
      call("sprint.start", {
        sprint: sprint.id,
        ...(start === "" ? {} : { start_date: start }),
        ...(end === "" ? {} : { end_date: end }),
        ...(goal.trim() !== (sprint.goal ?? "") && goal.trim() !== "" ? { goal } : {}),
      }),
      t("sprints-started", { name: sprint.name }),
    );
  };
  return (
    <Dialog
      title={t("sprints-start-title", { name: sprint.name })}
      isOpen
      onOpenChange={close(onClose)}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" onPress={begin}>
            {t("sprints-start")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <p>{t("sprints-start-body", { count: sprint.progress.total })}</p>
        <DateRow
          label={t("sprints-field-start")}
          value={start}
          onChange={(value) => {
            setStart(value);
            setError(null);
          }}
        />
        <DateRow
          label={t("sprints-field-end")}
          value={end}
          onChange={(value) => {
            setEnd(value);
            setError(null);
          }}
        />
        <GoalRow value={goal} onChange={setGoal} />
        <DialogError error={error} />
      </div>
    </Dialog>
  );
}

/** Completes the active sprint, and says where its open tasks go: a planned sprint or the backlog. */
export function CompleteSprintDialog({ sprint, planned, onClose }: { sprint: Sprint; planned: readonly Sprint[]; onClose: () => void }) {
  const t = useT();
  const { call, error, run } = useSprintCall(onClose);
  const open = sprint.progress.total - sprint.progress.closed;
  const [target, setTarget] = useState<string>(planned[0]?.id ?? BACKLOG);
  const complete = () => {
    run(
      call("sprint.complete", { sprint: sprint.id, move_open_tasks_to: target === BACKLOG ? "backlog" : target }),
      t("sprints-completed", { name: sprint.name }),
    );
  };
  return (
    <Dialog
      title={t("sprints-complete-title", { name: sprint.name })}
      isOpen
      onOpenChange={close(onClose)}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" onPress={complete}>
            {t("sprints-complete")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <p>{t("sprints-complete-body", { closed: sprint.progress.closed, open })}</p>
        {open > 0 && (
          <div className={styles.formRow}>
            <span className={styles.formLabel}>{t("sprints-complete-move")}</span>
            <Select
              label={t("sprints-complete-move")}
              value={target}
              onChange={setTarget}
              options={[...planned.map((other) => ({ id: other.id, label: other.name })), { id: BACKLOG, label: t("sprints-backlog") }]}
            />
          </div>
        )}
        <DialogError error={error} />
      </div>
    </Dialog>
  );
}

/** Deletes a planned or completed sprint, once the person confirms; its tasks go back to the backlog. */
export function DeleteSprintDialog({ sprint, onClose }: { sprint: Sprint; onClose: () => void }) {
  const t = useT();
  const { call, error, run } = useSprintCall(onClose);
  return (
    <Dialog
      title={t("sprints-delete-title", { name: sprint.name })}
      isOpen
      onOpenChange={close(onClose)}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button
            className="btn danger"
            onPress={() => {
              run(call("sprint.delete", { sprint: sprint.id }), t("sprints-deleted", { name: sprint.name }));
            }}
          >
            {t("sprints-delete")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <p>{t("sprints-delete-body", { count: sprint.progress.total })}</p>
        <DialogError error={error} />
      </div>
    </Dialog>
  );
}
