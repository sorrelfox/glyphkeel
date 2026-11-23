// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

/** Shown instead of the app when start-up found the window unable to work. */
export function StartupFailure({ reason }: { reason: string }) {
  const t = useT();
  return (
    <main className={styles.failure} role="alert">
      <h1 className={styles.title}>{t("startup-failed-title")}</h1>
      <p className="selectable">{t(reason)}</p>
    </main>
  );
}
