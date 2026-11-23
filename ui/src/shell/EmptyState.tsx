// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Button } from "react-aria-components";

import { type Action } from "../actions/actions";
import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

interface EmptyStateProps {
  /** The one muted line. */
  text: string;
  /** The one action. */
  action: Action;
}

/** An empty pane: one line of muted text and one action, centred. No illustrations. */
export function EmptyState({ text, action }: EmptyStateProps) {
  const t = useT();
  return (
    <div className={styles.empty}>
      <p className="muted">{text}</p>
      <Button className="btn" isDisabled={action.disabled ?? false} onPress={action.run}>
        {t(action.label)}
      </Button>
    </div>
  );
}
