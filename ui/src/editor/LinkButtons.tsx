// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { BookOpen, Hash, type LucideIcon } from "lucide-react";
import { Button, Group } from "react-aria-components";

import { Tooltip } from "../components/Tooltip";
import { useT } from "../i18n/I18n";
import styles from "./FormatToolbar.module.css";
import type { LinkKind } from "./types";

const BUTTONS: readonly { kind: LinkKind; icon: LucideIcon; label: string; hint: string }[] = [
  { kind: "task", icon: Hash, label: "editor-link-task", hint: "editor-link-task-hint" },
  { kind: "page", icon: BookOpen, label: "editor-link-page", hint: "editor-link-page-hint" },
];

interface LinkButtonsProps {
  /** Opens the search for a link in the text, as typing `#` or `[[` does. */
  onLink: (kind: LinkKind) => void;
  isDisabled?: boolean;
}

/**
 * "Link task" and "Link wiki page", for the formatting toolbar: each types
 * its trigger at the selection and opens the same search as typing it.
 */
export function LinkButtons({ onLink, isDisabled = false }: LinkButtonsProps) {
  const t = useT();
  return (
    <Group className={styles.group ?? ""} aria-label={t("editor-links")}>
      {BUTTONS.map(({ kind, icon: Icon, label, hint }) => (
        <Tooltip key={kind} text={t(hint)}>
          <Button
            className="btn flat sm icon"
            aria-label={t(label)}
            data-link={kind}
            isDisabled={isDisabled}
            preventFocusOnPress
            onPress={() => {
              onLink(kind);
            }}
          >
            <Icon className="ic s" aria-hidden="true" />
          </Button>
        </Tooltip>
      ))}
    </Group>
  );
}
