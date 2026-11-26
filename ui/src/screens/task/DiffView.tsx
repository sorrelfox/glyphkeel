// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { classes } from "../../classes";
import { useT } from "../../i18n/I18n";
import styles from "./TaskPanel.module.css";
import { wordDiff, type WordPart } from "./wordDiff";

export type DiffLineKind = "add" | "remove" | "context" | "hunk";

/** A line of a diff; a changed line paired with its counterpart carries its words, the changed ones marked. */
export interface DiffLine {
  kind: DiffLineKind;
  text: string;
  words?: WordPart[];
}

/**
 * Pairs each run of removed lines with the run of added lines right after
 * it, line by line, and marks the words that differ in each pair.
 */
export function markWords(lines: DiffLine[]): DiffLine[] {
  const out = lines.map((line) => ({ ...line }));
  let index = 0;
  while (index < out.length) {
    if (out[index]?.kind !== "remove") {
      index += 1;
      continue;
    }
    const removedStart = index;
    while (out[index]?.kind === "remove") {
      index += 1;
    }
    const addedStart = index;
    while (out[index]?.kind === "add") {
      index += 1;
    }
    const pairs = Math.min(addedStart - removedStart, index - addedStart);
    for (let pair = 0; pair < pairs; pair += 1) {
      const removed = out[removedStart + pair];
      const added = out[addedStart + pair];
      if (removed === undefined || added === undefined) {
        continue;
      }
      const words = wordDiff(removed.text, added.text);
      if (words !== null) {
        removed.words = words.removed;
        added.words = words.added;
      }
    }
  }
  return out;
}

/** A changed line's text, its changed words marked. */
function Words({ line }: { line: DiffLine }) {
  if (line.words === undefined) {
    return <>{line.text}</>;
  }
  return (
    <>
      {line.words.map((part, index) =>
        part.changed ? (
          <mark key={index} className={styles.diffWord} data-diff-word="">
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </>
  );
}

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

/** A unified diff, line by line: added lines green, removed ones red, and inside a changed line the words that changed. */
export function DiffView({ diff, label }: { diff: string; label: string }) {
  const t = useT();
  const lines = markWords(diffLines(diff));
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
            <ins aria-label={t("description-added-line", { text: line.text })}>
              <Words line={line} />
            </ins>
          ) : line.kind === "remove" ? (
            <del aria-label={t("description-removed-line", { text: line.text })}>
              <Words line={line} />
            </del>
          ) : (
            <span>{line.kind === "hunk" ? "⋯" : line.text}</span>
          )}
        </div>
      ))}
    </div>
  );
}
