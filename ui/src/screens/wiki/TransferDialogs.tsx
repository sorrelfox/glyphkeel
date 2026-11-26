// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { FolderInput } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { Segmented } from "../../components/Segmented";
import { useToasts } from "../../components/Toasts";
import { useCall } from "../../data/hooks";
import { skippedReason, useFailureToast } from "../../data/safety";
import { type WikiImportResult } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { chooseWikiFolder, choosesFiles, importWikiFolder, type ImportSource } from "../../ipc/transfer";
import { useExportPage } from "./exportPage";
import styles from "./Wiki.module.css";

type Scope = "page" | "subpages" | "wiki";
type Format = "markdown" | "html";

/** The page an export starts from. */
export interface ExportFrom {
  id: string;
  slug: string;
  title: string;
  project: string;
  /** Whether it holds pages. */
  hasChildren: boolean;
}

/**
 * Export: the page alone, with the pages inside it, or the whole wiki; as
 * Markdown files or as web pages that open in any browser. The file goes
 * where the person chooses in the native file chooser.
 */
export function ExportPageDialog({ page, scope: initial = "page", onClose }: { page: ExportFrom; scope?: Scope; onClose: () => void }) {
  const t = useT();
  const exportPage = useExportPage();
  const [scope, setScope] = useState<Scope>(initial === "subpages" && !page.hasChildren ? "page" : initial);
  const [format, setFormat] = useState<Format>("markdown");
  const start = () => {
    const html = format === "html";
    exportPage(scope === "wiki" ? { project: page.project, html } : { page, subpages: scope === "subpages", html });
    onClose();
  };
  return (
    <Dialog
      title={t("wiki-export-title", { page: page.title })}
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
          <Button className="btn primary" onPress={start}>
            {t(choosesFiles() ? "transfer-export-choose" : "transfer-export-start")}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <Segmented<Scope>
          label={t("wiki-export-scope")}
          value={scope}
          onChange={setScope}
          segments={[
            { id: "page", label: t("wiki-export-scope-page") },
            ...(page.hasChildren ? [{ id: "subpages" as const, label: t("wiki-export-scope-subpages") }] : []),
            { id: "wiki", label: t("wiki-export-scope-wiki") },
          ]}
        />
        <Segmented<Format>
          label={t("wiki-export-format")}
          value={format}
          onChange={setFormat}
          segments={[
            { id: "markdown", label: t("wiki-export-format-markdown") },
            { id: "html", label: t("wiki-export-format-html") },
          ]}
        />
        <p className={classes("faint", styles.small)}>{t(format === "html" ? "wiki-export-html-about" : "wiki-export-markdown-about")}</p>
      </div>
    </Dialog>
  );
}

type Step =
  | { kind: "choose" }
  | { kind: "checking"; source: ImportSource }
  | { kind: "checked"; source: ImportSource; result: WikiImportResult }
  | { kind: "importing"; source: ImportSource; result: WikiImportResult };

/**
 * Import a folder of Markdown files into the wiki, at the top or inside a
 * page: choose the folder in the native folder chooser (in a browser: name
 * it), see what it makes (pages, attached files, titles that were taken,
 * files left out), then import it as one change that Undo takes back.
 */
export function ImportFolderDialog({ project, parent, onClose, onImported }: { project: string; parent?: { id: string; title: string } | undefined; onClose: () => void; onImported: (first: string | undefined) => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const [step, setStep] = useState<Step>({ kind: "choose" });
  const [name, setName] = useState("");
  const target = { project, parent: parent?.id };

  const check = (source: ImportSource) => {
    setStep({ kind: "checking", source });
    importWikiFolder(call, source, target, true).then(
      (result) => {
        setStep({ kind: "checked", source, result });
      },
      (error: unknown) => {
        fail(error);
        setStep({ kind: "choose" });
      },
    );
  };
  const choose = () => {
    chooseWikiFolder().then((source) => {
      if (source !== null) {
        check(source);
      }
    }, fail);
  };
  const run = () => {
    if (step.kind !== "checked") {
      return;
    }
    const { source, result } = step;
    setStep({ kind: "importing", source, result });
    importWikiFolder(call, source, target, false).then(
      (imported) => {
        const group = imported.group;
        toasts.show({
          tone: "info",
          text: t("wiki-imported", { count: imported.pages }),
          action:
            group === undefined
              ? undefined
              : {
                  label: t("toast-undo"),
                  run: () => {
                    call("history.revert", { groups: [group] }).then((reverted) => {
                      const skipped = reverted.skipped[0];
                      if (skipped) {
                        toasts.show({ tone: "error", text: t("history-not-reverted"), detail: skippedReason(t, skipped) });
                      }
                    }, fail);
                  },
                },
        });
        onImported(imported.top?.[0]?.id);
        onClose();
      },
      (error: unknown) => {
        fail(error);
        setStep({ kind: "checked", source, result });
      },
    );
  };
  const busy = step.kind === "checking" || step.kind === "importing";
  const result = step.kind === "checked" || step.kind === "importing" ? step.result : undefined;

  return (
    <Dialog
      title={parent ? t("wiki-import-into-title", { page: parent.title }) : t("wiki-import-title")}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose} isDisabled={busy}>
            {t("dialog-cancel")}
          </Button>
          {result && (
            <Button className="btn primary" onPress={run} isDisabled={busy}>
              {t("wiki-import-start", { count: result.pages })}
            </Button>
          )}
        </>
      }
    >
      <div className={styles.form}>
        {step.kind === "choose" &&
          (choosesFiles() ? (
            <>
              <p className="muted">{t("wiki-import-about")}</p>
              <div>
                <Button className="btn" onPress={choose}>
                  <FolderInput className="ic s" />
                  {t("wiki-import-choose")}
                </Button>
              </div>
            </>
          ) : (
            <form
              className={styles.formRow}
              onSubmit={(event) => {
                event.preventDefault();
                if (name.trim() !== "") {
                  check({ path: name.trim(), name: name.trim() });
                }
              }}
            >
              <span className="field grow">
                <input aria-label={t("wiki-import-name")} placeholder={t("wiki-import-name")} value={name} onChange={(event) => { setName(event.target.value); }} />
              </span>
              <Button type="submit" className="btn" isDisabled={name.trim() === ""}>
                {t("transfer-import-check")}
              </Button>
            </form>
          ))}
        {busy && (
          <div role="status" className={styles.formRow}>
            <div className="meter grow" role="progressbar" aria-label={t(step.kind === "checking" ? "wiki-import-checking" : "wiki-importing")}>
              <i style={{ inlineSize: "100%" }} />
            </div>
          </div>
        )}
        {result && (
          <section aria-label={t("wiki-import-summary")} data-testid="wiki-import-summary">
            <p>{t("wiki-import-makes", { pages: result.pages, files: result.attachments })}</p>
            {(result.renamed ?? []).length > 0 && (
              <>
                <p className="muted">{t("wiki-import-renamed", { count: (result.renamed ?? []).length })}</p>
                <ul className={styles.importList}>
                  {(result.renamed ?? []).map((rename) => (
                    <li key={rename.path}>{t("wiki-import-renamed-one", { path: rename.path, title: rename.title })}</li>
                  ))}
                </ul>
              </>
            )}
            {(result.skipped ?? []).length > 0 && (
              <>
                <p className="muted">{t("wiki-import-skipped", { count: (result.skipped ?? []).length })}</p>
                <ul className={styles.importList}>
                  {(result.skipped ?? []).slice(0, 20).map((path) => (
                    <li key={path} className="selectable">{path}</li>
                  ))}
                </ul>
              </>
            )}
          </section>
        )}
      </div>
    </Dialog>
  );
}
