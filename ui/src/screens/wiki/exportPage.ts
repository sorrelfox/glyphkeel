// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useToasts } from "../../components/Toasts";
import { errorToast, useCall } from "../../data/hooks";
import { type WikiPageRef } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { exportWikiPage } from "../../ipc/transfer";

/** What to export: a page, alone or with the pages inside it, or a project's whole wiki; as web pages when `html`. */
export type ExportTarget = ({ page: Pick<WikiPageRef, "id" | "slug">; subpages: boolean } | { project: string }) & { html?: boolean };

/**
 * Exports a page, alone or with the pages inside it, or a whole wiki, as
 * Markdown files or web pages with their attachments in a `.zip` file where
 * the person chooses (in a browser: the workspace's exports folder), and
 * says how it went.
 */
export function useExportPage(): (target: ExportTarget) => void {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  return (target) => {
    const request =
      "page" in target
        ? { page: target.page.id, subpages: target.subpages, name: `${target.page.slug}.zip` }
        : // Fluent isolates placeables with invisible marks, which don't belong in a file name.
          { project: target.project, name: `${t("wiki-export-file", { project: target.project.toLowerCase() }).replace(/[⁨⁩]/g, "")}.zip` };
    const html = target.html === true;
    exportWikiPage(call, { ...request, html }).then(
      (done) => {
        if (done) {
          toasts.show({ tone: "info", text: t(html ? "wiki-exported-html" : "wiki-exported", { count: done.pages }), detail: done.path });
        }
      },
      (error: unknown) => {
        toasts.show(errorToast(t, error));
      },
    );
  };
}
