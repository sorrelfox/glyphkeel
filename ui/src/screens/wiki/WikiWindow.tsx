// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { useToasts } from "../../components/Toasts";
import { useProject } from "../../data/board";
import { errorToast, useCommand, useOp } from "../../data/hooks";
import { type WikiPage } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { inApp } from "../../ipc/files";
import { openTaskWindow } from "../../ipc/taskWindow";
import { appWindow } from "../../ipc/window";
import { NewPageDialog, RenamePageDialog, type NewPageRequest } from "./PageDialogs";
import { PageView } from "./PageView";
import styles from "./Wiki.module.css";

const route = getRouteApi("/wiki/$id");

/**
 * A wiki window: one page filling a window of its own, without the tree
 * and the sidebar. Links to other pages open them in the same window; task
 * keys open the task's own window.
 */
export function WikiWindowScreen() {
  const t = useT();
  const toasts = useToasts();
  const { id } = route.useParams();
  const { heading } = route.useSearch();
  const navigate = useNavigate();
  const page = useOp("wiki.get", { page: id }).data;
  const project = useProject(page?.project);
  const writable = project.data !== undefined && project.data.state === "active" && project.data.modules.wiki;
  const [newPage, setNewPage] = useState<NewPageRequest | null>(null);
  const [renaming, setRenaming] = useState<Pick<WikiPage, "id" | "title"> | null>(null);
  const [fresh, setFresh] = useState<string | null>(null);
  const remove = useCommand("wiki.delete", { undoable: t("wiki-deleted") });

  useEffect(() => {
    if (page) {
      // Fluent isolates placeables with invisible marks; window titles show them.
      void appWindow.setTitle(t("wiki-window-title", { page: page.title, project: page.project }).replace(/[⁨⁩]/g, ""));
    }
  }, [page, t]);

  const close = () => {
    if (inApp()) {
      void appWindow.close();
    } else if (page) {
      // A plain browser shows the wiki window's screen in the page: back to the wiki.
      void navigate({ to: "/projects/$key/wiki", params: { key: page.project }, search: { page: page.id } });
    }
  };

  return (
    <div className={styles.windowScreen}>
      <PageView
        key={id}
        pageId={id}
        writable={writable}
        inWindow
        heading={heading}
        startEditing={fresh === id}
        onNavigate={(other, at) => void navigate({ to: "/wiki/$id", params: { id: other }, search: at === undefined ? {} : { heading: at } })}
        onCreateMissing={(title) => {
          if (writable && page) {
            setNewPage({ project: page.project, parent: null, title });
          } else {
            toasts.show({ tone: "info", text: t("wiki-missing-page", { page: title }) });
          }
        }}
        onOpenTask={(key) => {
          openTaskWindow(key, () => void navigate({ to: "/task/$key", params: { key } })).catch((error: unknown) => {
            toasts.show(errorToast(t, error));
          });
        }}
        onRename={setRenaming}
        onDelete={(target) => {
          remove.mutate({ page: target.id }, { onSuccess: close });
        }}
        onNewChild={(parent) => {
          setNewPage({ project: parent.project, parent: { id: parent.id, title: parent.title } });
        }}
      />
      {newPage && (
        <NewPageDialog
          request={newPage}
          onClose={(created) => {
            setNewPage(null);
            if (created !== undefined) {
              setFresh(created);
              void navigate({ to: "/wiki/$id", params: { id: created }, search: {} });
            }
          }}
        />
      )}
      {renaming && (
        <RenamePageDialog
          page={renaming}
          onClose={() => {
            setRenaming(null);
          }}
        />
      )}
    </div>
  );
}
