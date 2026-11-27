// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "react-aria-components";

import { useActions } from "../../actions/ActionsProvider";
import { classes } from "../../classes";
import { useToasts } from "../../components/Toasts";
import { failureText, useCall, useOp } from "../../data/hooks";
import { useBytes, useFailureToast, useRefreshBackups } from "../../data/safety";
import { type Backup, type BackupCheck } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { chooseBackupFolder, chosenBackupFolder, choosesFiles, resetBackupFolder } from "../../ipc/transfer";
import { opPrefix } from "../../data/queries";
import { Group, Row } from "../../shell/PreferenceRows";
import { useMomentFormat } from "../task/Activity";
import styles from "./Safety.module.css";

/** Makes a backup now, with a toast; for the page and the app's menu. */
export function useBackUpNow(): { run: () => void; busy: boolean } {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const refresh = useRefreshBackups();
  const [busy, setBusy] = useState(false);
  return {
    busy,
    run: () => {
      setBusy(true);
      call("backup.create", {}).then(
        (backup) => {
          setBusy(false);
          refresh();
          toasts.show({ tone: "info", text: t("backup-made", { name: backup.name }) });
        },
        (error: unknown) => {
          setBusy(false);
          fail(error);
        },
      );
    },
  };
}

/**
 * Preferences, Data and backups: where the backups are, the last one, Back
 * up now, and every backup with its kind, time and size. Each can be
 * checked, restored or deleted; restoring and deleting are confirmed by the
 * app's native dialog. Export and import open their own dialogs.
 */
export function DataPage() {
  const t = useT();
  const moment = useMomentFormat();
  const backups = useOp("backup.list", {});
  const now = useBackUpNow();
  const actions = useActions();
  const exportAction = actions.find((action) => action.id === "app.export");
  const importAction = actions.find((action) => action.id === "app.import");
  const csvAction = actions.find((action) => action.id === "app.importCsv");
  return (
    <>
      <Group title={t("backups-title")} lead={<p className={classes("faint", styles.small)}>{t("backups-about")}</p>}>
        {backups.isError && <p className="faint">{failureText(t, backups.error)}</p>}
        <Row label={t("backups-folder")} about={t("backups-folder-about")}>
          {backups.data ? (
            <span className={classes("selectable", styles.path)} data-testid="backup-folder">
              {backups.data.folder}
            </span>
          ) : null}
          <BackupFolderButtons />
        </Row>
        <Row label={t("backups-last")}>
          <span className="num" data-testid="last-backup">
            {backups.data?.last_backup_at === undefined ? t("backups-never") : moment(backups.data.last_backup_at)}
          </span>
        </Row>
        <Row label={t("action-backup")} about={t("backups-now-about")}>
          <Button className="btn" isDisabled={now.busy} onPress={now.run}>
            {t(now.busy ? "backups-making" : "action-backup")}
          </Button>
        </Row>
      </Group>
      {backups.data && backups.data.items.length > 0 && <BackupTable items={backups.data.items} />}
      <Group title={t("transfer-title")}>
        <Row label={t("action-export")} about={t("transfer-export-about")}>
          <Button className="btn" isDisabled={exportAction?.disabled ?? true} onPress={() => exportAction?.run()}>
            {t("action-export")}
          </Button>
        </Row>
        <Row label={t("action-import")} about={t("transfer-import-about")}>
          <Button className="btn" isDisabled={importAction?.disabled ?? true} onPress={() => importAction?.run()}>
            {t("action-import")}
          </Button>
        </Row>
        <Row label={t("csv-title")} about={t("csv-import-about")}>
          <Button className="btn" isDisabled={csvAction?.disabled ?? true} onPress={() => csvAction?.run()}>
            {t("csv-title")}
          </Button>
        </Row>
      </Group>
    </>
  );
}

/**
 * Change… picks another folder for backups in the native folder chooser
 * (only in the app: the page never names a folder); Use default goes back
 * to the workspace's own. Backups made before stay where they are.
 */
function BackupFolderButtons() {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const refresh = useRefreshBackups();
  const queryClient = useQueryClient();
  const settings = useOp("settings.get", {});
  const chosen = chosenBackupFolder(settings.data);
  const done = (changed: boolean) => {
    if (changed) {
      refresh();
      void queryClient.invalidateQueries({ queryKey: opPrefix("settings.get") });
      toasts.show({ tone: "info", text: t("backups-folder-changed"), detail: t("backups-folder-old-stay") });
    }
  };
  return (
    <>
      {choosesFiles() && (
        <Button className="btn sm" onPress={() => void chooseBackupFolder().then(done, fail)}>
          {t("backups-folder-change")}
        </Button>
      )}
      {chosen !== undefined && (
        <Button className="btn sm" onPress={() => void resetBackupFolder(call).then(() => { done(true); }, fail)}>
          {t("backups-folder-default")}
        </Button>
      )}
    </>
  );
}

function BackupTable({ items }: { items: readonly Backup[] }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const refresh = useRefreshBackups();
  const moment = useMomentFormat();
  const bytes = useBytes();
  const [checks, setChecks] = useState<ReadonlyMap<string, BackupCheck | "checking">>(new Map());
  const [busy, setBusy] = useState<string | null>(null);
  const setCheck = (name: string, check: BackupCheck | "checking" | undefined) => {
    setChecks((was) => {
      const next = new Map(was);
      if (check === undefined) {
        next.delete(name);
      } else {
        next.set(name, check);
      }
      return next;
    });
  };
  const verify = (name: string) => {
    setCheck(name, "checking");
    call("backup.verify", { name }).then(
      (check) => {
        setCheck(name, check);
      },
      (error: unknown) => {
        setCheck(name, undefined);
        fail(error);
      },
    );
  };
  const act = (name: string, work: () => Promise<void>) => {
    setBusy(name);
    work().then(
      () => {
        setBusy(null);
        refresh();
      },
      (error: unknown) => {
        setBusy(null);
        fail(error);
      },
    );
  };
  const restore = (name: string) => {
    act(name, () =>
      call("backup.restore", { name }).then((restored) => {
        toasts.show({ tone: "info", text: t("backup-restored", { name: restored.restored.name }), detail: t("backup-restored-previous", { name: restored.previous.name }) });
      }),
    );
  };
  const remove = (name: string) => {
    act(name, () =>
      call("backup.delete", { name }).then((deleted) => {
        toasts.show({ tone: "info", text: t("backup-deleted", { name: deleted.name }) });
      }),
    );
  };
  return (
    <section aria-label={t("backups-list")}>
      <table className={styles.table} aria-label={t("backups-list")}>
        <thead>
          <tr>
            <th scope="col">{t("backups-column-made")}</th>
            <th scope="col">{t("backups-column-kind")}</th>
            <th scope="col" className={styles.number}>
              {t("backups-column-size")}
            </th>
            <th scope="col">{t("backups-column-check")}</th>
            <th scope="col" aria-label={t("backups-column-actions")} />
          </tr>
        </thead>
        <tbody>
          {items.map((backup) => {
            const check = checks.get(backup.name);
            return (
              <tr key={backup.name} data-backup={backup.name}>
                <td className="num">{moment(backup.created_at)}</td>
                <td>{t(`backup-kind-${backup.kind}`)}</td>
                <td className={classes("num", styles.number)}>
                  {bytes(backup.size)}
                  <div className={classes("faint", styles.small)}>{t("backups-files", { count: backup.files })}</div>
                </td>
                <td className={styles.small} data-testid="backup-check">
                  {check === undefined ? (
                    <span className="faint">{t("backups-unchecked")}</span>
                  ) : check === "checking" ? (
                    <span className="faint">{t("backups-checking")}</span>
                  ) : check.ok ? (
                    <span className={styles.ok}>{t("backups-check-ok", { count: check.files_checked })}</span>
                  ) : (
                    <span className={styles.bad} title={check.problems.map((problem) => problem.detail).join("\n")}>
                      {t("backups-check-problems", { count: check.problems.length })}
                    </span>
                  )}
                </td>
                <td>
                  <div className={styles.actions}>
                    <Button className="btn flat sm" isDisabled={check === "checking"} onPress={() => { verify(backup.name); }}>
                      {t("backups-verify")}
                    </Button>
                    <Button className="btn flat sm" isDisabled={busy !== null} onPress={() => { restore(backup.name); }}>
                      {t("backups-restore")}
                    </Button>
                    <Button className="btn flat sm danger" isDisabled={busy !== null} onPress={() => { remove(backup.name); }}>
                      {t("backups-delete")}
                    </Button>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
