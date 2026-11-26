// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { suggestHue, type Hue } from "../../data/catalog";
import { useCall, useOp } from "../../data/hooks";
import { useTemplates } from "../../data/recurrence";
import { OpFailure } from "../../data/transport";
import { useT } from "../../i18n/I18n";
import { HuePicker } from "./HuePicker";
import styles from "./Project.module.css";

export interface NewProjectRequest {
  /** The parent's key, for a sub-project. */
  parent: string | null;
}

/** A key suggested from a name: the initials of its words, or the first letters of one word. */
export function suggestKey(name: string): string {
  const words = name
    .normalize("NFKD")
    .toUpperCase()
    .replace(/[^A-Z0-9 ]/g, "")
    .split(/\s+/)
    .filter((word) => /^[A-Z]/.test(word));
  const initials = words.map((word) => word[0] ?? "").join("");
  const key = initials.length >= 2 ? initials : words.join("").slice(0, 3);
  return key.slice(0, 10);
}

/** No template: an empty project. */
const NO_TEMPLATE = "none";

interface NewProjectDialogProps {
  request: NewProjectRequest;
  /** Called with the new project's key, or nothing when cancelled. */
  onClose: (created?: string) => void;
}

/** A name, a key and a colour; a sub-project starts with its parent's statuses, types and labels. */
export function NewProjectDialog({ request, onClose }: NewProjectDialogProps) {
  const t = useT();
  const call = useCall();
  const [name, setName] = useState("");
  const [key, setKey] = useState<string | null>(null);
  // The colour: one its siblings don't use yet, as the engine suggests, until another is picked.
  const [picked, setPicked] = useState<Hue | null>(null);
  const projects = useOp("project.list", { include_archived: true });
  const siblings = (projects.data?.items ?? []).filter((project) => (project.parent ?? null) === request.parent);
  const hue = picked ?? suggestHue(siblings.map((project) => project.hue));
  const [error, setError] = useState<string | null>(null);
  const shownKey = key ?? suggestKey(name);
  const templates = useTemplates("project");
  const [template, setTemplate] = useState(NO_TEMPLATE);
  const toasts = useToasts();
  const { locale } = useLocale();

  const failed = (failure: unknown) => {
    setError(failure instanceof OpFailure ? [failure.error.message, "hint" in failure.error ? failure.error.hint : undefined].filter(Boolean).join(" ") : String(failure));
  };
  const create = () => {
    if (template !== NO_TEMPLATE) {
      // A project template sets up the statuses, types, labels, pages, tasks and task templates.
      call("template.apply", { template, name, key: shownKey, parent_project: request.parent, ...(picked === null ? {} : { hue: picked }) }).then((applied) => {
        const leftOut = applied.left_out ?? [];
        if (leftOut.length > 0) {
          toasts.show({ tone: "info", text: t("template-left-out", { count: leftOut.length, names: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(leftOut) }) });
        }
        onClose(applied.project?.key ?? shownKey);
      }, failed);
      return;
    }
    call("project.create", { name, key: shownKey, parent: request.parent, ...(picked === null ? {} : { hue: picked }) }).then((change) => {
      onClose(change.project.key);
    }, failed);
  };

  return (
    <Dialog
      title={t(request.parent ? "project-new-subproject-title" : "project-new-title", { parent: request.parent ?? "" })}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={() => { onClose(); }}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === ""} onPress={create}>
            {t("project-create")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          create();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("project-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("project-key")}</span>
          <span className="field">
            <input
              className="num"
              value={shownKey}
              maxLength={10}
              onChange={(event) => {
                setKey(event.target.value.toUpperCase());
                setError(null);
              }}
            />
          </span>
        </label>
        <div className={styles.formRow}>
          <span className={styles.formLabel}>{t("project-colour")}</span>
          <HuePicker label={t("project-colour")} value={hue} onChange={setPicked} />
        </div>
        {(templates.data?.items.length ?? 0) > 0 && (
          <div className={styles.formRow}>
            <span className={styles.formLabel}>{t("project-template")}</span>
            <Select
              label={t("project-template")}
              value={template}
              options={[{ id: NO_TEMPLATE, label: t("project-template-none") }, ...(templates.data?.items ?? []).map((each) => ({ id: each.id, label: each.name }))]}
              onChange={setTemplate}
            />
          </div>
        )}
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
