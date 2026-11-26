// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { errorToast, useCall } from "../../data/hooks";
import { type WikiPage } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Wiki.module.css";

/** A new page: its project, the page it goes in (or the top) and maybe a title already, from a link. */
export interface NewPageRequest {
  project: string;
  parent: { id: string; title: string } | null;
  title?: string | undefined;
}

interface TitleDialogProps {
  title: string;
  /** What the title field starts with. */
  initial: string;
  /** The confirming button. */
  action: string;
  /** Saves the title; a refusal shows in the dialog. */
  run: (title: string) => Promise<unknown>;
  onClose: () => void;
  /** A line under the field, such as where the page goes. */
  note?: string | undefined;
}

/** A dialog with one title field, for a new page and a rename. */
function TitleDialog({ title, initial, action, run, onClose, note }: TitleDialogProps) {
  const t = useT();
  const [text, setText] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const submit = () => {
    if (text.trim() === "" || pending) {
      return;
    }
    setPending(true);
    run(text.trim()).then(
      () => {
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
      title={title}
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
          <Button className="btn primary" isDisabled={text.trim() === "" || pending} onPress={submit}>
            {action}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("wiki-title")}</span>
          <span className="field">
            <input
              autoFocus
              value={text}
              maxLength={200}
              onChange={(event) => {
                setText(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        {note !== undefined && <p className="faint">{note}</p>}
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

/** Makes a page, inside another or at the top; answers with the new page's ID. */
export function NewPageDialog({ request, onClose }: { request: NewPageRequest; onClose: (created?: string) => void }) {
  const t = useT();
  const call = useCall();
  const created = useRef<string | undefined>(undefined);
  return (
    <TitleDialog
      title={request.parent ? t("wiki-new-child-title", { parent: request.parent.title }) : t("wiki-new-page")}
      initial={request.title ?? ""}
      action={t("wiki-create")}
      note={request.parent ? t("wiki-new-in", { parent: request.parent.title }) : t("wiki-new-at-top")}
      run={async (title) => {
        const change = await call("wiki.create", { project: request.project, title, ...(request.parent ? { parent: request.parent.id } : {}) });
        created.current = change.page.id;
      }}
      onClose={() => {
        onClose(created.current);
      }}
    />
  );
}

/** A new title; the old one keeps finding the page, so links by it still work. */
export function RenamePageDialog({ page, onClose }: { page: Pick<WikiPage, "id" | "title">; onClose: () => void }) {
  const t = useT();
  const call = useCall();
  return (
    <TitleDialog
      title={t("wiki-rename-title", { page: page.title })}
      initial={page.title}
      action={t("wiki-rename")}
      note={t("wiki-rename-note")}
      run={(title) => (title === page.title ? Promise.resolve() : call("wiki.update", { page: page.id, title }))}
      onClose={onClose}
    />
  );
}
