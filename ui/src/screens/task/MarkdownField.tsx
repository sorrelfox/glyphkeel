// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Suspense, type Ref } from "react";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useCall } from "../../data/hooks";
import { useBlockRenderer, useTaskCompletion } from "../../data/markdown";
import { type Attachment } from "../../generated/ops";
import { attachBrowserFiles } from "../../ipc/files";
import { MarkdownEditor, type EditorMode, type MarkdownEditorHandle } from "../../editor";
import { useT } from "../../i18n/I18n";
import styles from "./TaskPanel.module.css";

interface MarkdownFieldProps {
  label: string;
  value: string;
  onChange: (text: string) => void;
  /** Ctrl+Enter. */
  onSubmit: () => void;
  /** Escape, unless the editor used it (closing a completion, say). */
  onCancel?: (() => void) | undefined;
  mode?: EditorMode;
  autoFocus?: boolean;
  readOnly?: boolean;
  /** The composer's one-line look. */
  compact?: boolean;
  className?: string | undefined;
  ref?: Ref<MarkdownEditorHandle>;
  /** The task that files pasted or dropped on the editor are attached to; they go in the text as links. */
  attachTo?: string | undefined;
}

/** Markdown that refers to an attachment: an image shows in the text, another file is a link. */
export function attachmentMarkdown(file: Pick<Attachment, "id" | "name" | "kind">): string {
  const name = file.name.replace(/[[\]\\]/g, "\\$&");
  return `${file.kind === "image" ? "!" : ""}[${name}](gk-file://${file.id})`;
}

/**
 * The Markdown editor as the task panel uses it: tables and callouts drawn
 * by the engine's renderer, task keys completed from the engine, Ctrl+Enter
 * to save or send and Escape to give up. The editor loads on first use.
 */
export function MarkdownField({ label, value, onChange, onSubmit, onCancel, mode = "preview", autoFocus = false, readOnly = false, compact = false, className, ref, attachTo }: MarkdownFieldProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const onFiles = async (files: readonly File[]) => {
    if (attachTo === undefined) {
      return "";
    }
    const attached = await attachBrowserFiles(call, attachTo, files);
    for (const failure of attached.failed) {
      toasts.show({ tone: "error", text: t("files-attach-failed", { name: failure.name }), detail: failure.error.message });
    }
    return attached.files.map((change) => attachmentMarkdown(change.file)).join(" ");
  };
  const renderBlock = useBlockRenderer();
  const completion = useTaskCompletion();
  return (
    <div
      className={classes(styles.markdownField, compact && styles.compactField, className)}
      // Ctrl+Enter is taken before the editor's own binding (a new line below).
      onKeyDownCapture={(event) => {
        if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
          event.preventDefault();
          event.stopPropagation();
          onSubmit();
        }
      }}
      onKeyDown={(event) => {
        if (event.defaultPrevented) {
          return;
        }
        if (event.key === "Escape" && onCancel) {
          event.preventDefault();
          event.stopPropagation();
          onCancel();
        }
      }}
    >
      <Suspense fallback={<p className={classes("faint", styles.small)}>{t("editor-loading")}</p>}>
        <MarkdownEditor
          {...(ref === undefined ? {} : { ref })}
          label={label}
          value={value}
          onChange={onChange}
          mode={mode}
          autoFocus={autoFocus}
          readOnly={readOnly}
          renderBlock={renderBlock}
          completion={completion}
          {...(attachTo === undefined || readOnly ? {} : { onFiles })}
        />
      </Suspense>
    </div>
  );
}
