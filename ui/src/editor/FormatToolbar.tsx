// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Bold, Code, Heading, Italic, Link, List, ListChecks, ListOrdered, Quote, SquareCode, Strikethrough, type LucideIcon } from "lucide-react";
import { type ReactNode } from "react";
import { Button, Group, Toolbar } from "react-aria-components";

import { parseChord } from "../actions/keymap";
import { classes } from "../classes";
import { Tooltip } from "../components/Tooltip";
import { useT } from "../i18n/I18n";
import styles from "./FormatToolbar.module.css";
import { FORMAT_SHORTCUTS, type FormatKind } from "./formats";

const ICONS: Readonly<Record<FormatKind, LucideIcon>> = {
  heading: Heading,
  bold: Bold,
  italic: Italic,
  strikethrough: Strikethrough,
  code: Code,
  link: Link,
  "bullet-list": List,
  "numbered-list": ListOrdered,
  checklist: ListChecks,
  quote: Quote,
  "code-block": SquareCode,
};

/** The buttons in groups, as the toolbar shows them in full. */
const FULL: readonly (readonly FormatKind[])[] = [
  ["heading"],
  ["bold", "italic", "strikethrough", "code", "link"],
  ["bullet-list", "numbered-list", "checklist", "quote", "code-block"],
];

/** The comment composer's short set. */
const COMPACT: readonly (readonly FormatKind[])[] = [
  ["bold", "italic", "code", "link"],
  ["bullet-list", "checklist"],
];

interface FormatToolbarProps {
  /** The full set, or the composer's short one. */
  compact?: boolean;
  /** Runs a command on the editor; the text keeps the focus. */
  onFormat: (kind: FormatKind) => void;
  isDisabled?: boolean;
  /** More buttons after the formatting ones, such as links to tasks or pages. */
  children?: ReactNode;
}

/**
 * The formatting toolbar above a Markdown editor: each button changes the
 * text through the editor, as its shortcut does, and its tooltip names the
 * shortcut. The buttons don't take the focus, so the selection stays.
 */
export function FormatToolbar({ compact = false, onFormat, isDisabled = false, children }: FormatToolbarProps) {
  const t = useT();
  return (
    <Toolbar className={classes(styles.toolbar, compact && styles.compact)} aria-label={t("editor-toolbar")}>
      {(compact ? COMPACT : FULL).map((group) => (
        <Group key={group.join(" ")} className={classes(styles.group)}>
          {group.map((kind) => {
            const Icon = ICONS[kind];
            const name = t(`editor-format-${kind}`);
            return (
              <Tooltip key={kind} text={name} chord={parseChord(FORMAT_SHORTCUTS[kind])}>
                <Button
                  className="btn flat sm icon"
                  aria-label={name}
                  data-format={kind}
                  isDisabled={isDisabled}
                  preventFocusOnPress
                  onPress={() => {
                    onFormat(kind);
                  }}
                >
                  <Icon className="ic s" aria-hidden="true" />
                </Button>
              </Tooltip>
            );
          })}
        </Group>
      ))}
      {children}
    </Toolbar>
  );
}
