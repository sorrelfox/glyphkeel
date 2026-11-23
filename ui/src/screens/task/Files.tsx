// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Download, ExternalLink, File, FileCode, FileImage, FileText, MoreHorizontal, Paperclip, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent, type ReactNode } from "react";
import { Button, Menu, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { useToasts } from "../../components/Toasts";
import { type Attachment, type FileKind } from "../../generated/ops";
import { errorToast, useCall, useCommand, useOp } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import {
  attachBrowserFiles,
  attachDrop,
  chooseAndAttach,
  dropTargetAt,
  imageUrl,
  inApp,
  onDesktopDrop,
  openAttachment,
  saveAttachment,
  type Attached,
} from "../../ipc/files";
import styles from "./TaskPanel.module.css";

/** A size in bytes as people read it: "12 KB", "3.4 MB". */
export function useSizeFormat(): (bytes: number) => string {
  const { locale } = useLocale();
  return (bytes) => {
    const units = ["byte", "kilobyte", "megabyte", "gigabyte"] as const;
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }
    return new Intl.NumberFormat(locale, { style: "unit", unit: units[unit], unitDisplay: "short", maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
  };
}

/** Tells what an attach did: a toast for the files attached, one per failure. */
function useReportAttached(): (attached: Attached) => void {
  const t = useT();
  const toasts = useToasts();
  return (attached) => {
    if (attached.files.length > 0) {
      toasts.show({ tone: "info", text: t("files-attached", { count: attached.files.length }) });
    }
    for (const failure of attached.failed) {
      toasts.show({ tone: "error", text: t("files-attach-failed", { name: failure.name }), detail: failure.error.message });
    }
  };
}

/**
 * Attaching files to a task: in the app, the system's file chooser; in a
 * browser, the page's own file field. Returns the button's action and the
 * hidden field to render.
 */
export function useAttach(task: string): { attach: () => void; field: ReactNode; busy: boolean } {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const report = useReportAttached();
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const run = (work: Promise<Attached>) => {
    setBusy(true);
    work.then(report, (error: unknown) => {
      toasts.show(errorToast(t, error));
    }).finally(() => {
      setBusy(false);
    });
  };
  return {
    busy,
    attach: () => {
      if (inApp()) {
        run(chooseAndAttach(task));
      } else {
        input.current?.click();
      }
    },
    field: inApp() ? null : (
      <input
        ref={input}
        type="file"
        multiple
        hidden
        data-attach-field={task}
        aria-label={t("files-attach")}
        onChange={(event) => {
          const files = [...(event.target.files ?? [])];
          event.target.value = "";
          if (files.length > 0) {
            run(attachBrowserFiles(call, task, files));
          }
        }}
      />
    ),
  };
}

/**
 * Files dragged onto the panel: from the desktop through the app's drop
 * handler (the app keeps the paths), or in a browser as the page's own
 * files. Returns whether files are over the panel, and the handlers for a
 * browser.
 */
export function useFileDrop(task: string, enabled: boolean) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const report = useReportAttached();
  const [over, setOver] = useState(false);

  useEffect(() => {
    if (!enabled || !inApp()) {
      return;
    }
    return onDesktopDrop((event) => {
      if (event.kind === "leave") {
        setOver(false);
        return;
      }
      const here = dropTargetAt(event.x, event.y) === task;
      if (event.kind !== "drop") {
        setOver(here);
        return;
      }
      setOver(false);
      if (here) {
        attachDrop(event.id, task).then(report, (error: unknown) => {
          toasts.show(errorToast(t, error));
        });
      }
    });
  }, [enabled, task, t, toasts, report]);

  const hasFiles = (event: DragEvent) => event.dataTransfer.types.includes("Files");
  const handlers =
    enabled && !inApp()
      ? {
          onDragOver: (event: DragEvent) => {
            if (hasFiles(event)) {
              event.preventDefault();
              setOver(true);
            }
          },
          onDragLeave: (event: DragEvent) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              setOver(false);
            }
          },
          onDrop: (event: DragEvent) => {
            if (!hasFiles(event)) {
              return;
            }
            event.preventDefault();
            setOver(false);
            const files = [...event.dataTransfer.files];
            attachBrowserFiles(call, task, files).then(report, (error: unknown) => {
              toasts.show(errorToast(t, error));
            });
          },
        }
      : {};
  return { over, handlers };
}

function KindIcon({ kind }: { kind: FileKind }) {
  switch (kind) {
    case "image":
      return <FileImage className="ic muted" />;
    case "text":
      return <FileText className="ic muted" />;
    case "executable":
      return <FileCode className="ic muted" />;
    default:
      return <File className="ic muted" />;
  }
}

/** An attached image: from the app's `gk-file:` protocol, or in a browser from its content. */
export function AttachedImage({ file, className }: { file: Attachment; className?: string | undefined }) {
  const app = inApp();
  const content = useOp("file.read", { attachment: file.id }, { enabled: !app });
  const source = app ? imageUrl(file.id) : content.data?.data_base64 === undefined ? undefined : `data:${file.mime};base64,${content.data.data_base64}`;
  if (source === undefined) {
    return <span className={classes(styles.thumb, className)} />;
  }
  return <img className={classes(styles.thumb, className)} src={source} alt={file.name} draggable={false} />;
}

/** One attached file: a thumbnail or an icon, its name and size; open, save, remove. */
export function FileRow({ file, readOnly }: { file: Attachment; readOnly: boolean }) {
  const t = useT();
  const toasts = useToasts();
  const size = useSizeFormat();
  const remove = useCommand("file.delete", { undoable: t("files-removed", { name: file.name }) });
  const [preview, setPreview] = useState(false);
  const image = file.kind === "image";
  const app = inApp();
  const fail = (error: unknown) => {
    toasts.show(errorToast(t, error));
  };
  const open = () => {
    if (image) {
      setPreview((shown) => !shown);
    } else if (app && file.kind !== "executable") {
      openAttachment(file.id).catch(fail);
    } else if (app) {
      saveAttachment(file.id).catch(fail);
    }
  };
  return (
    <div className={styles.fileBlock} data-file={file.name}>
      <div className={classes("item", styles.row)}>
        <Button className={classes(styles.rowMain)} aria-label={t("files-open-named", { name: file.name })} onPress={open}>
          {image ? <AttachedImage file={file} /> : <KindIcon kind={file.kind} />}
          <span className="truncate grow">{file.name}</span>
          <span className={classes("num faint", styles.small)}>{size(file.size)}</span>
        </Button>
        <MenuTrigger>
          <Button className="btn flat icon sm" aria-label={t("files-menu", { name: file.name })}>
            <MoreHorizontal className="ic s" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu" aria-label={t("files-menu", { name: file.name })}>
              <MenuEntries
                entries={[
                  {
                    id: "open",
                    label: t("files-open"),
                    icon: <ExternalLink className="ic s" />,
                    disabled: !app || file.kind === "executable",
                    run: () => {
                      openAttachment(file.id).catch(fail);
                    },
                  },
                  {
                    id: "save",
                    label: t("files-save"),
                    icon: <Download className="ic s" />,
                    disabled: !app,
                    run: () => {
                      saveAttachment(file.id).catch(fail);
                    },
                  },
                  "separator",
                  {
                    id: "remove",
                    label: t("files-remove"),
                    icon: <Trash2 className="ic s" />,
                    disabled: readOnly,
                    run: () => {
                      remove.mutate({ attachment: file.id });
                    },
                  },
                ]}
              />
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
      {image && preview && <AttachedImage file={file} className={styles.preview} />}
    </div>
  );
}

/** The task's files, with Attach in the section's header. */
export function FilesSection({ task, files, readOnly }: { task: string; files: readonly Attachment[]; readOnly: boolean }) {
  const t = useT();
  const { attach, field, busy } = useAttach(task);
  return (
    <section className={styles.section} aria-label={t("files-title")}>
      <div className={styles.sectionHead}>
        <span className="caption grow">{t("files-title")}</span>
        <Button className="btn flat icon sm" isDisabled={readOnly || busy} aria-label={t("files-attach")} onPress={attach}>
          <Paperclip className="ic s" />
        </Button>
        {field}
      </div>
      {files.length === 0 ? (
        <p className={classes("faint", styles.small, styles.emptyNote)}>{t("files-none")}</p>
      ) : (
        files.map((file) => <FileRow key={file.id} file={file} readOnly={readOnly} />)
      )}
    </section>
  );
}
