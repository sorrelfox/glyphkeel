// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode, useState } from "react";

import { classes } from "../classes";
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

/**
 * A whole number from `min` to `max`, saved when the field loses the focus
 * or on Enter. Anything else goes back to the stored value. A disabled
 * field shows no number.
 */
export function NumberSetting({
  label,
  value,
  min,
  max,
  unit,
  onSave,
  isDisabled = false,
  wide = false,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  unit: string;
  onSave: (value: number) => void;
  isDisabled?: boolean;
  /** Room for up to seven digits. */
  wide?: boolean;
}) {
  const [text, setText] = useState(String(value));
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    // The stored value changed, here or elsewhere: show it.
    setSeen(value);
    setText(String(value));
  }
  const save = () => {
    const number = Number(text);
    if (text.trim() !== "" && Number.isInteger(number) && number >= min && number <= max) {
      if (number !== value) {
        onSave(number);
      }
    } else {
      setText(String(value));
    }
  };
  return (
    <>
      <span className={classes("field", styles.number, wide && styles.numberWide)} data-disabled={isDisabled || undefined}>
        <input
          type="number"
          className="num"
          aria-label={label}
          min={min}
          max={max}
          step={1}
          disabled={isDisabled}
          value={isDisabled ? "" : text}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              save();
            }
          }}
        />
      </span>
      <span className={classes("muted", styles.unit)}>{unit}</span>
    </>
  );
}
