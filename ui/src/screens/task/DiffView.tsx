// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { classes } from "../../classes";
import { useT } from "../../i18n/I18n";
import styles from "./TaskPanel.module.css";

export type DiffLineKind = "add" | "remove" | "context" | "hunk";

/** The lines of a unified diff worth showing: hunks and their lines, without the file headers. */
export function diffLines(diff: string): { kind: DiffLineKind; text: string }[] {
  const lines: { kind: DiffLineKind; text: string }[] = [];
  for (const line of diff.split("\n")) {
    if (line.startsWith("--- ") || line.startsWith("+++ ") || line.startsWith("\\") || line === "") {
      continue;
    }
    if (line.startsWith("@@")) {
      lines.push({ kind: "hunk", text: line });
    } else if (line.startsWith("+")) {
      lines.push({ kind: "add", text: line.slice(1) });
    } else if (line.startsWith("-")) {
      lines.push({ kind: "remove", text: line.slice(1) });
    } else {
      lines.push({ kind: "context", text: line.slice(1) });
    }
  }
  return lines;
}

/** A unified diff, line by line: added lines green, removed ones red. */
export function DiffView({ diff, label }: { diff: string; label: string }) {
  const t = useT();
  const lines = diffLines(diff);
  if (lines.length === 0) {
    return <p className={classes("faint", styles.small)}>{t("description-no-differences")}</p>;
  }
  return (
    <div className={styles.diff} role="group" aria-label={label}>
      {lines.map((line, index) => (
        <div key={index} className={classes(styles.diffLine, styles[line.kind])} data-diff={line.kind}>
          <span className={styles.diffSign} aria-hidden="true">
            {line.kind === "add" ? "+" : line.kind === "remove" ? "−" : line.kind === "hunk" ? "" : " "}
          </span>
          {line.kind === "add" ? (
            <ins aria-label={t("description-added-line", { text: line.text })}>{line.text}</ins>
          ) : line.kind === "remove" ? (
            <del aria-label={t("description-removed-line", { text: line.text })}>{line.text}</del>
          ) : (
            <span>{line.kind === "hunk" ? "⋯" : line.text}</span>
          )}
        </div>
      ))}
    </div>
  );
}
