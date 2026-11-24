// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { FileArchive, Folder } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { useCall } from "../../data/hooks";
import { useFailureToast } from "../../data/safety";
import { type ImportResult, type KeyChange } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { chooseImport, choosesFiles, importArchive, type ImportSource } from "../../ipc/transfer";
import { Counts } from "./ExportDialog";
import styles from "./Safety.module.css";

type Step =
  | { kind: "choose" }
  | { kind: "checking"; source: ImportSource }
  | { kind: "checked"; source: ImportSource; result: ImportResult; taken: readonly string[] }
  | { kind: "importing"; source: ImportSource; result: ImportResult; taken: readonly string[] }
  | { kind: "done"; result: ImportResult };

/** The new keys for taken ones, as the import takes them; blank ones are left out. */
export function keyChanges(taken: readonly string[], keys: Readonly<Record<string, string>>): KeyChange[] {
  return taken.flatMap((from) => {
    const to = (keys[from] ?? "").trim().toUpperCase();
    return to === "" ? [] : [{ from, to }];
  });
}

/**
 * Import: choose an archive (a `.zip` file or a folder) in the native file
 * chooser; a check shows what it holds and which project keys are taken
 * here, each with a field for a new key; then the import runs and ends with
 * a summary and Undo, which moves the imported projects to the trash. The
 * import never changes existing data.
 */
export function ImportDialog({ isOpen, onOpenChange }: { isOpen: boolean; onOpenChange: (isOpen: boolean) => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const [step, setStep] = useState<Step>({ kind: "choose" });
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [name, setName] = useState("");

  const check = (source: ImportSource) => {
    setStep({ kind: "checking", source });
    importArchive(call, source, true, []).then(
      (result) => {
        const taken = result.taken_keys ?? [];
        setKeys(Object.fromEntries(taken.map((key) => [key, `${key}2`])));
        setStep({ kind: "checked", source, result, taken });
      },
      (error: unknown) => {
        fail(error);
        setStep({ kind: "choose" });
      },
    );
  };
  const choose = (folder: boolean) => {
    chooseImport(folder).then((source) => {
      if (source !== null) {
        check(source);
      }
    }, fail);
  };
  const run = () => {
    if (step.kind !== "checked") {
      return;
    }
    const { source, result, taken } = step;
    setStep({ kind: "importing", source, result, taken });
    importArchive(call, source, false, keyChanges(taken, keys)).then(
      (imported) => {
        setStep({ kind: "done", result: imported });
        const group = imported.group;
        toasts.show({
          tone: "info",
          text: t("transfer-imported", { count: imported.projects.length }),
          action:
            group === undefined
              ? undefined
              : {
                  label: t("toast-undo"),
                  run: () => {
                    call("history.revert", { groups: [group] }).then((reverted) => {
                      const skipped = reverted.skipped[0];
                      if (skipped) {
                        toasts.show({ tone: "error", text: t("history-not-reverted"), detail: skipped.reason });
                      }
                    }, fail);
                  },
                },
        });
      },
      (error: unknown) => {
        fail(error);
        setStep({ kind: "checked", source, result, taken });
      },
    );
  };
  const close = () => {
    onOpenChange(false);
  };
  const busy = step.kind === "checking" || step.kind === "importing";
  const missingKey = (step.kind === "checked" || step.kind === "importing") && keyChanges(step.taken, keys).length < step.taken.length;

  return (
    <Dialog
      title={t("transfer-import-title")}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      footer={
        step.kind === "done" ? (
          <Button className="btn primary" onPress={close}>
            {t("transfer-done")}
          </Button>
        ) : (
          <>
            <Button className="btn" onPress={close} isDisabled={busy}>
              {t("dialog-cancel")}
            </Button>
            {(step.kind === "checked" || step.kind === "importing") && (
              <Button className="btn primary" onPress={run} isDisabled={busy || missingKey}>
                {t("transfer-import-start", { count: step.result.projects.length })}
              </Button>
            )}
          </>
        )
      }
    >
      <div className={styles.form}>
        {step.kind === "choose" &&
          (choosesFiles() ? (
            <>
              <p className="muted">{t("transfer-import-choose-about")}</p>
              <div className={styles.choice}>
                <Button className="btn" onPress={() => { choose(false); }}>
                  <FileArchive className="ic s" />
                  {t("transfer-import-zip")}
                </Button>
                <Button className="btn" onPress={() => { choose(true); }}>
                  <Folder className="ic s" />
                  {t("transfer-import-folder")}
                </Button>
              </div>
            </>
          ) : (
            <form
              className={styles.choice}
              onSubmit={(event) => {
                event.preventDefault();
                if (name.trim() !== "") {
                  check({ path: name.trim(), name: name.trim() });
                }
              }}
            >
              <span className={classes("field grow")}>
                <input aria-label={t("transfer-import-name")} placeholder={t("transfer-import-name")} value={name} onChange={(event) => { setName(event.target.value); }} />
              </span>
              <Button type="submit" className="btn" isDisabled={name.trim() === ""}>
                {t("transfer-import-check")}
              </Button>
            </form>
          ))}
        {busy && (
          <div role="status" className={styles.choice}>
            <div className="meter grow" role="progressbar" aria-label={t(step.kind === "checking" ? "transfer-checking" : "transfer-importing")}>
              <i style={{ inlineSize: "100%" }} />
            </div>
            <span className="muted">{t(step.kind === "checking" ? "transfer-checking" : "transfer-importing")}</span>
          </div>
        )}
        {(step.kind === "checked" || step.kind === "importing") && (
          <>
            <p>{t("transfer-check-ok", { name: step.source.name, version: step.result.app_version })}</p>
            <Counts counts={step.result.counts} />
            {step.taken.length > 0 && (
              <section aria-label={t("transfer-taken-title")}>
                <p className="muted">{t("transfer-taken-about", { count: step.taken.length })}</p>
                {step.taken.map((key) => (
                  <div key={key} className={styles.keyRow}>
                    <span className="key">{key}</span>
                    <span aria-hidden="true" className="muted">
                      →
                    </span>
                    <span className={classes("field", styles.keyField)}>
                      <input
                        aria-label={t("transfer-new-key", { key })}
                        value={keys[key] ?? ""}
                        disabled={busy}
                        onChange={(event) => {
                          setKeys({ ...keys, [key]: event.target.value.toUpperCase() });
                        }}
                      />
                    </span>
                  </div>
                ))}
              </section>
            )}
          </>
        )}
        {step.kind === "done" && (
          <>
            <p>{t("transfer-imported", { count: step.result.projects.length })}</p>
            <ul aria-label={t("transfer-imported-projects")}>
              {step.result.projects.map((project) => (
                <li key={project.id}>
                  <span className="key">{project.key}</span> {project.name}
                </li>
              ))}
            </ul>
            <Counts counts={step.result.counts} />
          </>
        )}
      </div>
    </Dialog>
  );
}
