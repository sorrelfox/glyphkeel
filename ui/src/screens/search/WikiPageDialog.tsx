// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { AppWindow, BookOpen } from "lucide-react";
import { useEffect, useRef } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { errorToast, failureText } from "../../data/hooks";
import { useRenderedPage } from "../../data/markdown";
import { useWikiPage, type WikiPageName } from "../../data/wiki";
import { useT } from "../../i18n/I18n";
import { openWikiWindow } from "../../ipc/wikiWindow";
import { usePageOpener } from "../../markdown/pageLinks";
import { MarkdownReader } from "../../markdown/Reader";
import { useMomentFormat } from "../task/Activity";
import styles from "./Search.module.css";

interface WikiPageDialogProps {
  page: WikiPageName;
  /** The heading a link points at; the page scrolls to it. */
  heading?: string | undefined;
  onClose: () => void;
  onOpenTask: (key: string) => void;
}

/**
 * A wiki page, read only, in a dialog: its place in the tree, its text as
 * the reader shows it, and when it last changed. From here it opens in the
 * project's wiki (in the main window) or in a window of its own.
 */
export default function WikiPageDialog({ page, heading, onClose, onOpenTask }: WikiPageDialogProps) {
  const t = useT();
  const found = useWikiPage(page);
  const wiki = found.data ?? null;
  const name = "id" in page ? (wiki?.title ?? "") : page.title;
  const rendered = useRenderedPage(wiki);
  const html = rendered.data?.items[0]?.html;
  const moment = useMomentFormat();
  const body = useRef<HTMLDivElement>(null);
  const opener = usePageOpener();
  const navigate = useNavigate();
  const toasts = useToasts();

  // Scroll to the heading the link names, once the text is in; not again
  // when the text changes meanwhile.
  const scrolled = useRef<string | null>(null);
  const shownId = wiki?.id;
  useEffect(() => {
    const shown = `${shownId ?? ""}\n${heading ?? ""}`;
    if (html === undefined || heading === undefined || scrolled.current === shown) {
      return;
    }
    scrolled.current = shown;
    const wanted = heading.toLocaleLowerCase();
    const target = [...(body.current?.querySelectorAll<HTMLElement>("h1, h2, h3, h4, h5, h6") ?? [])].find((element) => element.textContent.trim().toLocaleLowerCase() === wanted);
    target?.scrollIntoView({ block: "start" });
  }, [html, heading, shownId]);

  return (
    <Dialog
      title={name === "" ? t("wiki-page") : name}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <div ref={body} className={styles.wikiPage} data-wiki-page={wiki?.id}>
        {found.isPending && <p className="faint">{t("wiki-loading")}</p>}
        {found.error && <p role="alert">{failureText(t, found.error)}</p>}
        {found.data === null && <p role="alert">{t("wiki-not-found", { page: name })}</p>}
        {wiki && (
          <>
            <p className={styles.wikiPath}>
              <span className="key">{wiki.project}</span>
              {(wiki.path ?? []).map((parent) => (
                <span key={parent.id}>› {parent.title}</span>
              ))}
              <span className="grow" />
              <span className="faint">{t("wiki-updated", { time: moment(wiki.updated_at) })}</span>
            </p>
            {wiki.body.trim() === "" ? (
              <p className="faint">{t("wiki-empty")}</p>
            ) : (
              html !== undefined && (
                <MarkdownReader
                  html={html}
                  label={t("wiki-text", { page: wiki.title })}
                  onOpenTask={(key) => {
                    onClose();
                    onOpenTask(key);
                  }}
                />
              )
            )}
            <p className={styles.wikiNote}>
              {opener ? (
                <Button
                  className="btn sm"
                  onPress={() => {
                    onClose();
                    opener({ id: wiki.id, project: wiki.project, heading });
                  }}
                >
                  <BookOpen className="ic s" />
                  {t("wiki-open-in-wiki")}
                </Button>
              ) : (
                <Button
                  className="btn sm"
                  onPress={() => {
                    onClose();
                    openWikiWindow(wiki.id, () => void navigate({ to: "/wiki/$id", params: { id: wiki.id }, search: heading === undefined ? {} : { heading } })).catch((error: unknown) => {
                      toasts.show(errorToast(t, error));
                    });
                  }}
                >
                  <AppWindow className="ic s" />
                  {t("wiki-open-window")}
                </Button>
              )}
            </p>
          </>
        )}
      </div>
    </Dialog>
  );
}
