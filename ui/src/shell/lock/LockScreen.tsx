// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useEffect, useId, useState, type SyntheticEvent } from "react";

import { classes } from "../../classes";
import { failureText } from "../../data/hooks";
import { useT } from "../../i18n/I18n";
import { unlockApp, useLockMutation, type LockView } from "../../ipc/lock";
import { windowReady } from "../../ipc/ready";
import styles from "./Lock.module.css";

/**
 * Shown instead of the app while it is locked, or while an encrypted
 * workspace waits for its passphrase: a passphrase field and nothing of the
 * workspace's data. The operating system's password is not asked for.
 */
export function LockScreen({ view }: { view: LockView }) {
  const t = useT();
  const titleId = useId();
  const aboutId = useId();
  const errorId = useId();
  const [passphrase, setPassphrase] = useState("");
  const unlock = useLockMutation((given: string) => unlockApp(given));
  const workspace = view.reason === "workspace";
  // An encrypted workspace that waits for its passphrase is the first thing
  // the app shows: the window may appear now, whether or not the app's own
  // first screen got to say so.
  useEffect(() => {
    if (workspace) {
      void windowReady();
    }
  }, [workspace]);
  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    unlock.mutate(
      passphrase,
      {
        onSettled: () => {
          setPassphrase("");
        },
      },
    );
  };
  return (
    <main className={styles.screen} aria-labelledby={titleId}>
      <form className={styles.form} onSubmit={submit} aria-describedby={aboutId}>
        <h1 id={titleId} className={styles.title}>
          {t(workspace ? "lock-workspace-title" : "lock-title")}
        </h1>
        <p id={aboutId} className={styles.about}>
          {t(workspace ? "lock-workspace-about" : "lock-about")}
        </p>
        <label className={styles.label}>
          <span>{t("lock-passphrase")}</span>
          <span className="field">
            <input
              type="password"
              // The lock screen is all there is; the field is where the person types.
              autoFocus
              autoComplete="current-password"
              value={passphrase}
              aria-invalid={unlock.isError || undefined}
              aria-describedby={unlock.isError ? errorId : undefined}
              onChange={(event) => {
                setPassphrase(event.target.value);
              }}
            />
          </span>
        </label>
        {unlock.isError && (
          <p id={errorId} className={styles.error} role="alert">
            {failureText(t, unlock.error)}
          </p>
        )}
        <div className={styles.actions}>
          <button type="submit" className={classes("btn", "primary")} disabled={unlock.isPending}>
            {t(workspace ? "lock-open" : "lock-unlock")}
          </button>
        </div>
      </form>
    </main>
  );
}
