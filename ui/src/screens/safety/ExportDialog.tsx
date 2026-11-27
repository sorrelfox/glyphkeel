// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Fragment, useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { ProjectMark } from "../../components/ProjectMark";
import { Segmented } from "../../components/Segmented";
import { useToasts } from "../../components/Toasts";
import { useCall, useOp } from "../../data/hooks";
import { useBytes, useFailureToast } from "../../data/safety";
import { useToday } from "../../data/zone";
import { type ArchiveCounts, type ExportResult } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useLock } from "../../ipc/lock";
import { choosesFiles, exportArchive } from "../../ipc/transfer";
import styles from "./Safety.module.css";

type Scope = "all" | "some";
type Format = "zip" | "folder";

/** What an archive holds, as a short list of counts. */
export function Counts({ counts }: { counts: ArchiveCounts }) {
  const t = useT();
  const shown = ["projects", "tasks", "comments", "attachments", "wiki_pages", "time_entries"] as const;
  return (
    <dl className={styles.counts}>
      {shown.map((what) => (
        <Fragment key={what}>
          <dt>{t(`transfer-count-${what}`)}</dt>
          <dd className="num">{counts[what]}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

/**
 * Export: the whole workspace or chosen projects (each with its
 * sub-projects), to a `.zip` file or a folder that the person picks in the
 * native file chooser. While it runs the dialog shows that it is working;
 * it ends with what was written.
 */
export function ExportDialog({ isOpen, onOpenChange, projects: initial = [] }: { isOpen: boolean; onOpenChange: (isOpen: boolean) => void; projects?: readonly string[] }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const bytes = useBytes();
  const today = useToday();
  const list = useOp("project.list", { include_archived: true });
  const [scope, setScope] = useState<Scope>(initial.length > 0 ? "some" : "all");
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set(initial));
  const [format, setFormat] = useState<Format>("zip");
  // An encrypted workspace encrypts its exports unless the person turns it off.
  const encrypted = useLock().data?.encryption?.encrypted === true;
  const [plain, setPlain] = useState(false);
  const encrypt = encrypted && !plain;
  const folder = format === "folder" && !encrypt;
  const [running, setRunning] = useState(false);
  const [done, setDone] = useState<ExportResult | null>(null);
  const projects = scope === "all" ? [] : [...chosen];
  const close = () => {
    onOpenChange(false);
  };
  const start = () => {
    setRunning(true);
    const name = t("transfer-export-name", { date: today, format: folder ? "folder" : "zip" });
    exportArchive(call, { projects, folder, name, ...(encrypted ? { encrypt } : {}) }).then(
      (result) => {
        setRunning(false);
        if (result !== null) {
          setDone(result);
          toasts.show({ tone: "info", text: t("transfer-exported", { count: result.projects.length }), detail: result.path });
        }
      },
      (error: unknown) => {
        setRunning(false);
        fail(error);
      },
    );
  };
  return (
    <Dialog
      title={t("transfer-export-title")}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      footer={
        done ? (
          <Button className="btn primary" onPress={close}>
            {t("transfer-done")}
          </Button>
        ) : (
          <>
            <Button className="btn" onPress={close} isDisabled={running}>
              {t("dialog-cancel")}
            </Button>
            <Button className="btn primary" onPress={start} isDisabled={running || (scope === "some" && chosen.size === 0)}>
              {t(choosesFiles() ? "transfer-export-choose" : "transfer-export-start")}
            </Button>
          </>
        )
      }
    >
      <div className={styles.form}>
        {done ? (
          <>
            <p>{t("transfer-exported", { count: done.projects.length })}</p>
            <p className={classes("selectable", styles.path)} data-testid="export-path">
              {done.path}
            </p>
            <p className="muted num">{bytes(done.size)}</p>
            {done.encrypted === true && <p className="muted">{t("transfer-exported-encrypted")}</p>}
            <Counts counts={done.counts} />
          </>
        ) : (
          <>
            <Segmented<Scope>
              label={t("transfer-export-scope")}
              value={scope}
              onChange={setScope}
              segments={[
                { id: "all", label: t("transfer-scope-all") },
                { id: "some", label: t("transfer-scope-some") },
              ]}
            />
            {scope === "some" && (
              <div className={styles.projects} role="group" aria-label={t("transfer-export-projects")}>
                {(list.data?.items ?? []).map((project) => (
                  <label key={project.key} className={styles.check}>
                    <input
                      type="checkbox"
                      className="check"
                      checked={chosen.has(project.key)}
                      onChange={(event) => {
                        const next = new Set(chosen);
                        if (event.target.checked) {
                          next.add(project.key);
                        } else {
                          next.delete(project.key);
                        }
                        setChosen(next);
                      }}
                    />
                    <ProjectMark hue={project.hue} icon={project.icon} />
                    <span>{project.path}</span>
                  </label>
                ))}
                <p className={classes("faint", styles.small)}>{t("transfer-export-subprojects")}</p>
              </div>
            )}
            {encrypted && (
              <label className={styles.check}>
                <input
                  type="checkbox"
                  className="check"
                  checked={encrypt}
                  onChange={(event) => {
                    setPlain(!event.target.checked);
                  }}
                />
                <span>{t("transfer-encrypt")}</span>
              </label>
            )}
            {encrypt ? (
              <p className={classes("faint", styles.small)}>{t("transfer-encrypt-about")}</p>
            ) : (
              <Segmented<Format>
                label={t("transfer-format-label")}
                value={format}
                onChange={setFormat}
                segments={[
                  { id: "zip", label: t("transfer-format-zip") },
                  { id: "folder", label: t("transfer-format-folder") },
                ]}
              />
            )}
            {!choosesFiles() && <p className={classes("faint", styles.small)}>{t("transfer-export-browser")}</p>}
            {running && (
              <div role="status" className={styles.choice}>
                <div className="meter grow" role="progressbar" aria-label={t("transfer-exporting")}>
                  <i style={{ inlineSize: "100%" }} />
                </div>
                <span className="muted">{t("transfer-exporting")}</span>
              </div>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}
