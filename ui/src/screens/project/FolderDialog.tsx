// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useId, useState } from "react";
import { Button, RadioButton, RadioField, RadioGroup } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { HUES, type Hue } from "../../data/catalog";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { useToasts } from "../../components/Toasts";
import { type ProjectFolder } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Project.module.css";

/** What the folder dialog is for: a new folder, or renaming one. */
export type FolderRequest = { kind: "new" } | { kind: "rename"; folder: ProjectFolder };

/** The choice of no colour in the colour field. */
const NONE = "none";

/**
 * A new folder, or a folder's new name and colour: one name field and the
 * nine hues, with no colour as the first choice.
 */
export function FolderDialog({ request, onClose }: { request: FolderRequest; onClose: () => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const folder = request.kind === "rename" ? request.folder : undefined;
  const [name, setName] = useState(folder?.name ?? "");
  const [hue, setHue] = useState<Hue | typeof NONE>(folder?.hue ?? NONE);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const colourLabel = useId();

  const submit = () => {
    const trimmed = name.trim();
    if (trimmed === "" || pending) {
      return;
    }
    setPending(true);
    const done =
      folder === undefined
        ? call("folder.create", { name: trimmed, ...(hue === NONE ? {} : { hue }) })
        : call("folder.update", { folder: folder.id, name: trimmed, ...(hue === NONE ? (folder.hue === undefined ? {} : { clear: ["hue" as const] }) : { hue }) });
    done.then(
      () => {
        toasts.show({ tone: "info", text: t(folder === undefined ? "toast-folder-created" : "toast-folder-changed"), action: { label: t("toast-undo"), run: undo } });
        onClose();
      },
      (failure: unknown) => {
        setPending(false);
        const toast = errorToast(t, failure);
        setError([toast.text, toast.detail].filter(Boolean).join(" "));
      },
    );
  };

  return (
    <Dialog
      title={t(folder === undefined ? "folder-new-title" : "folder-rename-title")}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === "" || pending} onPress={submit}>
            {t(folder === undefined ? "folder-create" : "folder-save")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        data-folder-dialog=""
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("folder-name")}</span>
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
        <div className={styles.formRow}>
          <span className={styles.formLabel} id={colourLabel}>
            {t("folder-colour")}
          </span>
          <RadioGroup
            aria-labelledby={colourLabel}
            orientation="horizontal"
            className={classes(styles.hues)}
            value={hue}
            onChange={(next) => {
              setHue(next === NONE ? NONE : (HUES.find((candidate) => candidate === next) ?? NONE));
            }}
          >
            <RadioField value={NONE} aria-label={t("folder-no-colour")}>
              <RadioButton className={({ isSelected, isFocusVisible }) => classes(styles.swatch, styles.noHue, isSelected && styles.swatchOn, isFocusVisible && styles.swatchFocus)} />
            </RadioField>
            {HUES.map((candidate) => (
              <RadioField key={candidate} value={candidate} aria-label={t(`hue-${candidate}`)}>
                <RadioButton data-hue={candidate} className={({ isSelected, isFocusVisible }) => classes(styles.swatch, `hue-${candidate}`, isSelected && styles.swatchOn, isFocusVisible && styles.swatchFocus)} />
              </RadioField>
            ))}
          </RadioGroup>
        </div>
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
