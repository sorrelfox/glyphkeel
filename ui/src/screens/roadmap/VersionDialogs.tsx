// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { plannable, VERSION_STATUSES } from "../../data/versions";
import { type Version, type VersionField, type VersionSharing, type VersionStatus } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Roadmap.module.css";

/** The choice in the close dialog that leaves the open tasks where they are. */
const KEEP = "*keep";
/** The choice that plans the open tasks for no version. */
const NONE = "*none";

/** Runs a version command: an error stays in the dialog, success closes it with an Undo toast. */
function useVersionCall(onClose: () => void) {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  const { undo } = useUndo();
  const [error, setError] = useState<string | null>(null);
  const run = (work: Promise<unknown>, done: string) => {
    work.then(
      () => {
        toasts.show({ tone: "info", text: done, action: { label: t("toast-undo"), run: undo } });
        onClose();
      },
      (failure: unknown) => {
        setError(errorToast(t, failure).text);
      },
    );
  };
  return { call, error, setError, run };
}

interface VersionDialogProps {
  project: string;
  /** The version to change; none: a new one. */
  version?: Version;
  onClose: () => void;
}

/** Makes a version, or changes one: name, dates, status, sharing and description. */
export function VersionDialog({ project, version, onClose }: VersionDialogProps) {
  const t = useT();
  const { call, error, setError, run } = useVersionCall(onClose);
  const [name, setName] = useState(version?.name ?? "");
  const [description, setDescription] = useState(version?.description ?? "");
  const [start, setStart] = useState(version?.start_date ?? "");
  const [due, setDue] = useState(version?.due_date ?? "");
  const [status, setStatus] = useState<VersionStatus>(version?.status ?? "open");
  const [sharing, setSharing] = useState<VersionSharing>(version?.sharing ?? "none");
  const save = () => {
    if (name.trim() === "") {
      return;
    }
    if (version === undefined) {
      run(
        call("version.create", {
          project,
          name,
          status,
          sharing,
          ...(description.trim() === "" ? {} : { description }),
          ...(start === "" ? {} : { start_date: start }),
          ...(due === "" ? {} : { due_date: due }),
        }),
        t("roadmap-created", { name: name.trim() }),
      );
      return;
    }
    const clear: VersionField[] = [];
    if (start === "" && version.start_date !== undefined) clear.push("start_date");
    if (due === "" && version.due_date !== undefined) clear.push("due_date");
    if (description.trim() === "" && (version.description ?? "") !== "") clear.push("description");
    run(
      call("version.update", {
        version: version.id,
        ...(name !== version.name ? { name } : {}),
        ...(description.trim() !== "" && description !== version.description ? { description } : {}),
        ...(start !== "" && start !== version.start_date ? { start_date: start } : {}),
        ...(due !== "" && due !== version.due_date ? { due_date: due } : {}),
        ...(status !== version.status ? { status } : {}),
        ...(sharing !== version.sharing ? { sharing } : {}),
        ...(clear.length > 0 ? { clear } : {}),
      }),
      t("roadmap-changed", { name: name.trim() }),
    );
  };
  return (
    <Dialog
      title={version === undefined ? t("roadmap-new-title") : t("roadmap-edit-title", { name: version.name })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === ""} onPress={save}>
            {version === undefined ? t("roadmap-create") : t("roadmap-save")}
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
          <span className={styles.formLabel}>{t("roadmap-field-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              maxLength={100}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("roadmap-field-start")}</span>
          <span className="field">
            <input
              type="date"
              value={start}
              onChange={(event) => {
                setStart(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("roadmap-field-due")}</span>
          <span className="field">
            <input
              type="date"
              value={due}
              onChange={(event) => {
                setDue(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        <div className={styles.formRow}>
          <span className={styles.formLabel}>{t("roadmap-field-status")}</span>
          <Select label={t("roadmap-field-status")} value={status} onChange={setStatus} options={VERSION_STATUSES.map((each) => ({ id: each, label: t(`version-status-${each}`) }))} />
        </div>
        <div className={styles.formRow}>
          <span className={styles.formLabel}>{t("roadmap-field-sharing")}</span>
          <Select
            label={t("roadmap-field-sharing")}
            value={sharing}
            onChange={setSharing}
            options={[
              { id: "none", label: t("version-sharing-none") },
              { id: "descendants", label: t("version-sharing-descendants") },
            ]}
          />
        </div>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("roadmap-field-description")}</span>
          <span className="field">
            <textarea
              rows={4}
              maxLength={4000}
              value={description}
              onChange={(event) => {
                setDescription(event.target.value);
              }}
            />
          </span>
        </label>
        {error !== null && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

interface CloseVersionDialogProps {
  project: string;
  version: Version;
  /** The versions the roadmap knows, among which the open tasks can go. */
  others: readonly Version[];
  onClose: () => void;
}

/** Closes a version, and says where its open tasks go: they stay, go to another open version, or to none. */
export function CloseVersionDialog({ version, others, onClose }: CloseVersionDialogProps) {
  const t = useT();
  const { call, error, run } = useVersionCall(onClose);
  const open = (version.tasks ?? []).filter((task) => !task.closed).length;
  const targets = plannable(others).filter((other) => other.id !== version.id);
  const [target, setTarget] = useState<string>(targets[0]?.id ?? KEEP);
  const close = () => {
    run(
      call("version.update", {
        version: version.id,
        status: "closed",
        ...(target === KEEP ? {} : { move_open_tasks_to: target === NONE ? "none" : target }),
      }),
      t("roadmap-closed", { name: version.name }),
    );
  };
  return (
    <Dialog
      title={t("roadmap-close-title", { name: version.name })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" onPress={close}>
            {t("roadmap-close")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <p>{t("roadmap-close-open-tasks", { count: open })}</p>
        {open > 0 && (
          <div className={styles.formRow}>
            <span className={styles.formLabel}>{t("roadmap-close-move")}</span>
            <Select
              label={t("roadmap-close-move")}
              value={target}
              onChange={setTarget}
              options={[
                ...targets.map((other) => ({ id: other.id, label: other.inherited === true ? t("prop-version-shared", { name: other.name, project: other.project }) : other.name })),
                { id: NONE, label: t("roadmap-close-to-none") },
                { id: KEEP, label: t("roadmap-close-keep") },
              ]}
            />
          </div>
        )}
        {error !== null && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** Deletes a version, once the person confirms; its tasks are planned for none. */
export function DeleteVersionDialog({ version, onClose }: { project: string; version: Version; onClose: () => void }) {
  const t = useT();
  const { call, error, run } = useVersionCall(onClose);
  return (
    <Dialog
      title={t("roadmap-delete-title", { name: version.name })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button
            className="btn danger"
            onPress={() => {
              run(call("version.delete", { version: version.id }), t("roadmap-deleted", { name: version.name }));
            }}
          >
            {t("roadmap-delete")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <p>{t("roadmap-delete-body", { count: version.progress.total })}</p>
        {error !== null && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
