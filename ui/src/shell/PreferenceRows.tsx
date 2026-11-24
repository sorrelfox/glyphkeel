// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode } from "react";

import styles from "./Preferences.module.css";

/** A titled group of rows in a box, with optional content above the box. */
export function Group({ title, lead, children }: { title: string; lead?: ReactNode; children: ReactNode }) {
  return (
    <section className={styles.group} aria-label={title}>
      <h2 className={styles.groupTitle}>{title}</h2>
      {lead}
      <div className={styles.boxed}>{children}</div>
    </section>
  );
}

/**
 * One setting: its name, an optional line about it, and its control (if
 * any) on the right. When the row is too narrow for both, the control moves below the
 * text instead of running out of the box.
 */
export function Row({ label, about, children }: { label: string; about?: string | undefined; children?: ReactNode }) {
  return (
    <div className={styles.row}>
      <span className={styles.text}>
        <span>{label}</span>
        {about !== undefined && <small>{about}</small>}
      </span>
      {children !== undefined && <span className={styles.control}>{children}</span>}
    </div>
  );
}
