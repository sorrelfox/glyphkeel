// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Suspense, useEffect, useMemo, useRef, type Ref } from "react";
import { createPortal } from "react-dom";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useCall } from "../../data/hooks";
import { useLinkCompletion } from "../../data/linking";
import { useBlockRenderer } from "../../data/markdown";
import { type Attachment } from "../../generated/ops";
import { attachBrowserFiles, ownerKey, type FileOwner } from "../../ipc/files";
import { MarkdownEditor, type EditorMode, type MarkdownEditorHandle } from "../../editor";
import { FormatToolbar } from "../../editor/FormatToolbar";
import { LinkButtons } from "../../editor/LinkButtons";
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
  /** The composer's one-line look, with the short formatting toolbar. */
  compact?: boolean;
  /** Without the formatting toolbar. */
  plain?: boolean;
  className?: string | undefined;
  ref?: Ref<MarkdownEditorHandle>;
  /** The task or wiki page that files pasted or dropped on the editor are attached to; they go in the text as links. */
  attachTo?: FileOwner | undefined;
  /** The project whose wiki pages `[[` offers first; by default the project of the task `attachTo`. */
  project?: string | undefined;
  /**
   * Where the formatting toolbar goes instead of above the text, such as a
   * page's bar: an element elsewhere in the window (null until it is there).
   */
  toolbarSlot?: HTMLElement | null | undefined;
}

/** Markdown that refers to an attachment: an image shows in the text, another file is a link. */
export function attachmentMarkdown(file: Pick<Attachment, "id" | "name" | "kind">): string {
  const name = file.name.replace(/[[\]\\]/g, "\\$&");
  return `${file.kind === "image" ? "!" : ""}[${name}](gk-file://${file.id})`;
}

/** The editors that take files dropped from the desktop, by their element. */
const dropEditors = new WeakMap<Element, { owner: string; editor: () => MarkdownEditorHandle | null }>();

/**
 * Where the Markdown of files dropped from the desktop at a point of the
 * window goes: the editor under the point, when it takes files for `owner`.
 * In the app, desktop drops reach the page as one event of the window, not
 * as the editor's own drop; the caller attaches the files, then inserts.
 */
export function editorDropAt(x: number, y: number, owner: FileOwner): ((markdown: string) => void) | null {
  const element = document.elementFromPoint(x, y)?.closest("[data-drop-editor]");
  const target = element ? dropEditors.get(element) : undefined;
  const editor = target?.owner === ownerKey(owner) ? target.editor() : null;
  if (!editor) {
    return null;
  }
  const at = editor.positionAt(x, y);
  return (markdown) => {
    editor.insert(markdown, at ?? undefined);
  };
}

/**
 * The Markdown editor as the task panel uses it: tables and callouts drawn
 * by the engine's renderer, links to tasks ("#") and wiki pages ("[[")
 * completed from the engine, also from the toolbar, Ctrl+Enter
 * to save or send and Escape to give up, and the formatting toolbar above
 * the text. The editor loads on first use.
 */
export function MarkdownField({ label, value, onChange, onSubmit, onCancel, mode = "preview", autoFocus = false, readOnly = false, compact = false, plain = false, className, ref, attachTo, project, toolbarSlot }: MarkdownFieldProps) {
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
  // Wiki links point at pages of the task's own project (or the page's).
  const links = useLinkCompletion(project ?? (typeof attachTo === "string" ? attachTo.slice(0, attachTo.lastIndexOf("-")) || undefined : undefined));
  // A wiki page's text also offers its macros.
  const wikiPage = typeof attachTo === "object";
  const completion = useMemo(() => (wikiPage ? { ...links, macros: true } : links), [links, wikiPage]);
  const box = useRef<HTMLDivElement>(null);
  const editor = useRef<MarkdownEditorHandle | null>(null);
  const takesFiles = attachTo !== undefined && !readOnly;
  const owner = attachTo === undefined ? undefined : ownerKey(attachTo);
  useEffect(() => {
    const element = box.current;
    if (!element || owner === undefined || readOnly) {
      return;
    }
    dropEditors.set(element, { owner, editor: () => editor.current });
    return () => {
      dropEditors.delete(element);
    };
  }, [owner, readOnly]);
  const setEditor = (handle: MarkdownEditorHandle | null) => {
    editor.current = handle;
    if (typeof ref === "function") {
      ref(handle);
    } else if (ref) {
      ref.current = handle;
    }
  };
  const toolbar = (
    <FormatToolbar
      compact={compact}
      bare={toolbarSlot !== undefined}
      isDisabled={readOnly}
      onFormat={(kind) => {
        editor.current?.format(kind);
      }}
    >
      <LinkButtons
        isDisabled={readOnly}
        onLink={(kind) => {
          editor.current?.startLink(kind);
        }}
      />
    </FormatToolbar>
  );
  return (
    <div
      ref={box}
      data-drop-editor={takesFiles ? "" : undefined}
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
      {!plain && toolbarSlot === undefined && toolbar}
      {!plain && toolbarSlot && createPortal(toolbar, toolbarSlot)}
      <Suspense fallback={<p className={classes("faint", styles.small)}>{t("editor-loading")}</p>}>
        <MarkdownEditor
          ref={setEditor}
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
