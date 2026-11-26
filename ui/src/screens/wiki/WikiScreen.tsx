// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { BookOpen, Plus } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { useProject } from "../../data/board";
import { errorToast, useCommand } from "../../data/hooks";
import { useWikiTree } from "../../data/wiki";
import { type WikiPage, type WikiPageSummary } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { openWikiWindow } from "../../ipc/wikiWindow";
import { useStatus } from "../../shell/context";
import { TaskPanel } from "../task/TaskPanel";
import { NewPageDialog, RenamePageDialog, type NewPageRequest } from "./PageDialogs";
import { PageTree } from "./PageTree";
import { PageView } from "./PageView";
import styles from "./Wiki.module.css";

/** The wiki's place in the window: the page shown, the heading a link named, and a task open beside it. */
export interface WikiSearch {
  page?: string;
  heading?: string;
  task?: string;
}

export function wikiSearch(search: Record<string, unknown>): WikiSearch {
  const out: WikiSearch = {};
  for (const name of ["page", "heading", "task"] as const) {
    const value = search[name];
    if (typeof value === "string" && value !== "") {
      out[name] = value;
    }
  }
  return out;
}

const route = getRouteApi("/shell/projects/$key/wiki");

/**
 * A project's wiki: the page tree on the left, the page with its outline
 * and links, and a task opened from it docked on the right. With the wiki
 * switched off in the project's settings, pages can still be read but not
 * changed.
 */
export function WikiScreen() {
  const t = useT();
  const toasts = useToasts();
  const { key } = route.useParams();
  const { page, heading, task } = route.useSearch();
  const navigate = useNavigate();
  const project = useProject(key);
  const tree = useWikiTree(key);
  const pages = tree.data?.items ?? [];
  const current = page ?? pages[0]?.id;
  const writable = project.data !== undefined && project.data.state === "active" && project.data.modules.wiki;
  const [newPage, setNewPage] = useState<NewPageRequest | null>(null);
  const [renaming, setRenaming] = useState<Pick<WikiPage, "id" | "title"> | null>(null);
  // A page just made opens in the editor.
  const [fresh, setFresh] = useState<string | null>(null);
  const move = useCommand("wiki.move");
  const remove = useCommand("wiki.delete", { undoable: t("wiki-deleted") });

  useStatus({ summary: tree.data ? t("wiki-page-count", { count: tree.data.total }) : "" });

  const show = (next: WikiSearch, replace = false) => {
    void navigate({ to: "/projects/$key/wiki", params: { key }, search: next, replace });
  };
  const openPage = (id: string, at?: string) => {
    show({ page: id, ...(at === undefined ? {} : { heading: at }), ...(task === undefined ? {} : { task }) });
  };
  const openWindow = (target: Pick<WikiPageSummary, "id">) => {
    openWikiWindow(target.id, () => void navigate({ to: "/wiki/$id", params: { id: target.id } })).catch((error: unknown) => {
      toasts.show(errorToast(t, error));
    });
  };
  const trash = (target: Pick<WikiPage, "id" | "title"> & { parent?: string | { id: string } | undefined }) => {
    remove.mutate(
      { page: target.id },
      {
        onSuccess: () => {
          if (target.id === current) {
            const parent = typeof target.parent === "string" ? target.parent : target.parent?.id;
            show(parent === undefined ? {} : { page: parent }, true);
          }
        },
      },
    );
  };

  return (
    <div className={styles.layout}>
      <PageTree
        pages={pages}
        current={current}
        writable={writable}
        onOpen={(id) => {
          openPage(id);
        }}
        onMove={(input) => {
          move.mutate(input);
        }}
        onNew={(parent) => {
          const holder = parent === null ? undefined : pages.find((candidate) => candidate.id === parent);
          setNewPage({ project: key, parent: holder ? { id: holder.id, title: holder.title } : null });
        }}
        onRename={setRenaming}
        onDelete={trash}
        onOpenWindow={openWindow}
      />
      {current !== undefined ? (
        <PageView
          key={current}
          pageId={current}
          writable={writable}
          heading={heading}
          startEditing={fresh === current}
          onNavigate={openPage}
          onCreateMissing={(title) => {
            if (writable) {
              setNewPage({ project: key, parent: null, title });
            } else {
              toasts.show({ tone: "info", text: t("wiki-missing-page", { page: title }) });
            }
          }}
          onOpenTask={(other) => {
            show({ page: current, task: other });
          }}
          onRename={setRenaming}
          onDelete={trash}
          onNewChild={(parent) => {
            setNewPage({ project: key, parent: { id: parent.id, title: parent.title } });
          }}
          onOpenWindow={openWindow}
        />
      ) : (
        tree.isSuccess && (
          <div className={classes(styles.page, styles.empty)}>
            <BookOpen className="ic muted" aria-hidden="true" />
            <p className="muted">{t(writable ? "wiki-empty-text" : "wiki-empty-off")}</p>
            {writable && (
              <Button className="btn primary" onPress={() => { setNewPage({ project: key, parent: null }); }}>
                <Plus className="ic" />
                {t("wiki-new-page")}
              </Button>
            )}
          </div>
        )
      )}
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
          onClose={() => {
            show(current === undefined ? {} : { page: current });
          }}
          onOpenTask={(other) => {
            show({ ...(current === undefined ? {} : { page: current }), task: other });
          }}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
      {newPage && (
        <NewPageDialog
          request={newPage}
          onClose={(created) => {
            setNewPage(null);
            if (created !== undefined) {
              setFresh(created);
              openPage(created);
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
