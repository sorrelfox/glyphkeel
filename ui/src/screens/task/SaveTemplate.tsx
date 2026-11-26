// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCall } from "../../data/hooks";
import { type Task } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "../project/Project.module.css";

/** Saves a task with its subtasks as a template of its project: a name and what it is for. */
export function SaveTemplateDialog({ task, onClose }: { task: Task; onClose: () => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const [name, setName] = useState(task.title.slice(0, 100));
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    if (name.trim() === "") {
      return;
    }
    call("template.create", { name: name.trim(), from_task: task.key, ...(description.trim() === "" ? {} : { description: description.trim() }) }).then(
      (made) => {
        toasts.show({ tone: "info", text: t("toast-template-saved", { name: made.template.name }) });
        onClose();
      },
      (failure: unknown) => {
        setError(errorToast(t, failure).text);
      },
    );
  };
  return (
    <Dialog
      title={t("template-save-title")}
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
            {t("template-save")}
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
          <span className={styles.formLabel}>{t("template-name")}</span>
          <span className="field">
            <input autoFocus maxLength={100} value={name} onChange={(event) => { setName(event.target.value); setError(null); }} />
          </span>
        </label>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("template-description")}</span>
          <span className="field">
            <input maxLength={1000} value={description} onChange={(event) => { setDescription(event.target.value); }} />
          </span>
        </label>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
