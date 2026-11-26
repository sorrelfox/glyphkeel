// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { BookOpen, FileText, MessageSquare } from "lucide-react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { errorToast, useOp } from "../../data/hooks";
import { type Backlink } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { openWikiWindow } from "../../ipc/wikiWindow";
import { usePageOpener } from "../../markdown/pageLinks";
import styles from "./TaskPanel.module.css";

/**
 * "Linked from": the wiki pages, other tasks' descriptions and comments that
 * name this task by its key. A page opens in the project's wiki (or, in a
 * task window, in a window of its own); a task opens as other links do.
 */
export function BacklinksSection({ taskKey, onOpenTask }: { taskKey: string; onOpenTask: (key: string) => void }) {
  const t = useT();
  const toasts = useToasts();
  const navigate = useNavigate();
  const opener = usePageOpener();
  const backlinks = useOp("task.backlinks", { key: taskKey });
  const items = backlinks.data?.items ?? [];

  const open = (link: Backlink) => {
    const page = link.page;
    if (page) {
      if (opener) {
        opener({ id: page.id, project: link.project, heading: link.heading });
        return;
      }
      openWikiWindow(page.id, () => void navigate({ to: "/wiki/$id", params: { id: page.id }, search: {} })).catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
    } else if (link.task !== undefined) {
      onOpenTask(link.task);
    }
  };

  return (
    <section className={styles.section} aria-label={t("backlinks-title")} data-backlinks={taskKey}>
      <div className={styles.sectionHead}>
        <span className="caption grow">{t("backlinks-title")}</span>
        {backlinks.data && <span className="count">{backlinks.data.total}</span>}
      </div>
      {backlinks.isError && <p className={classes("faint", styles.small, styles.emptyNote)}>{backlinks.error.error.message}</p>}
      {backlinks.data && items.length === 0 && <p className={classes("faint", styles.small, styles.emptyNote)}>{t("backlinks-none")}</p>}
      {items.map((link) => {
        const kind = link.page ? "page" : link.comment !== undefined ? "comment" : "task";
        return (
          <div key={`${kind}-${link.page?.id ?? link.comment ?? link.task ?? ""}`} className={classes("item", styles.row)} data-backlink={kind}>
            <Button
              className={classes(styles.rowMain)}
              aria-label={t(`backlinks-open-${kind}`, { title: link.title, key: link.task ?? "" })}
              onPress={() => {
                open(link);
              }}
            >
              {kind === "page" ? <BookOpen className="ic s muted" /> : kind === "comment" ? <MessageSquare className="ic s muted" /> : <FileText className="ic s muted" />}
              {link.task !== undefined && <span className="key">{link.task}</span>}
              <span className="truncate grow">{link.title}</span>
              <span className={classes("faint", styles.small)}>{t(`backlinks-kind-${kind}`)}</span>
            </Button>
          </div>
        );
      })}
    </section>
  );
}
