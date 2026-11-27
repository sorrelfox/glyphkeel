// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { useProject } from "../../data/board";
import { type Project } from "../../data/catalog";
import { failureText, useCall, useOp } from "../../data/hooks";
import { useFailureToast, skippedReason } from "../../data/safety";
import { useT } from "../../i18n/I18n";
import styles from "./Project.module.css";

/** Whether project `key` sits somewhere below project `ancestor`. */
export function isBelow(items: readonly { key: string; parent?: string }[], key: string, ancestor: string): boolean {
  let parent = items.find((each) => each.key === key)?.parent;
  for (let depth = 0; parent !== undefined && depth < 8; depth += 1) {
    if (parent === ancestor) {
      return true;
    }
    const above = parent;
    parent = items.find((each) => each.key === above)?.parent;
  }
  return false;
}

/**
 * The question before a project, its sub-projects and everything in them
 * move to the trash, naming how much goes. It can be restored from the
 * trash, so the page asks, not the app's native dialog. Afterwards a toast
 * offers Undo, and the window goes to Today.
 */
export function DeleteQuestion({ project, onCancel, onDeleted }: { project: Project; onCancel: () => void; onDeleted: () => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const navigate = useNavigate();
  const all = useOp("project.list", { include_archived: true });
  const [busy, setBusy] = useState(false);
  const subprojects = (all.data?.items ?? []).filter((each) => isBelow(all.data?.items ?? [], each.key, project.key)).length;
  const remove = () => {
    setBusy(true);
    call("project.delete", { project: project.key }).then(
      (deleted) => {
        setBusy(false);
        toasts.show({
          tone: "info",
          text: t("project-deleted", { name: project.name, subprojects: Math.max(0, deleted.projects.length - 1), tasks: deleted.tasks }),
          action: {
            label: t("toast-undo"),
            run: () => {
              call("history.revert", { groups: [deleted.group] }).then((result) => {
                const skipped = result.skipped[0];
                if (skipped) {
                  toasts.show({ tone: "error", text: t("history-not-reverted"), detail: skippedReason(t, skipped) });
                }
              }, fail);
            },
          },
        });
        onDeleted();
        void navigate({ to: "/" });
      },
      (error: unknown) => {
        setBusy(false);
        fail(error);
      },
    );
  };
  return (
    <div className={classes(styles.confirm, styles.deleteConfirm)} role="group" aria-label={t("project-delete")}>
      <p>{t("project-delete-confirm", { name: project.name, subprojects, tasks: project.total_tasks })}</p>
      <div className={styles.confirmButtons}>
        <Button className="btn" isDisabled={busy} onPress={onCancel}>
          {t("dialog-cancel")}
        </Button>
        <Button className="btn primary" isDisabled={busy} onPress={remove}>
          {t("project-delete-confirm-button")}
        </Button>
      </div>
    </div>
  );
}

/** Delete from the project tree's menu: the same question, in a dialog. */
export function DeleteProjectDialog({ projectKey, onClose }: { projectKey: string; onClose: () => void }) {
  const t = useT();
  const project = useProject(projectKey);
  return (
    <Dialog
      title={t("project-delete-title", { name: project.data?.name ?? projectKey })}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <div className={styles.deleteDialog}>
        {project.isError && <p className="faint">{failureText(t, project.error)}</p>}
        {project.data && <DeleteQuestion project={project.data} onCancel={onClose} onDeleted={onClose} />}
      </div>
    </Dialog>
  );
}
