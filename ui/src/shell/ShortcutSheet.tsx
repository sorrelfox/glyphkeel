// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ActionGroup } from "../actions/actions";
import { useActions } from "../actions/ActionsProvider";
import { Dialog } from "../components/Dialog";
import { Kbd } from "../components/Kbd";
import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

const GROUPS: readonly { group: ActionGroup; label: string }[] = [
  { group: "app", label: "shortcut-group-app" },
  { group: "navigation", label: "shortcut-group-navigation" },
  { group: "tasks", label: "shortcut-group-tasks" },
  { group: "window", label: "shortcut-group-window" },
];

interface ShortcutSheetProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
}

/** `?`: every action that has a shortcut, from the same registry as the menus. */
export function ShortcutSheet({ isOpen, onOpenChange }: ShortcutSheetProps) {
  const t = useT();
  const actions = useActions().filter((action) => action.keys.length > 0);
  return (
    <Dialog title={t("shortcuts-title")} isOpen={isOpen} onOpenChange={onOpenChange}>
      <div className={styles.shortcutColumns}>
        {GROUPS.map(({ group, label }) => {
          const inGroup = actions.filter((action) => action.group === group);
          if (inGroup.length === 0) {
            return null;
          }
          return (
            <section key={group} className={styles.shortcutGroup}>
              <h2 className="caption">{t(label)}</h2>
              <dl className={styles.shortcutList}>
                {inGroup.map((action) => (
                  <div key={action.id} className={styles.shortcutRow}>
                    <dt>{t(action.label)}</dt>
                    <dd>
                      {action.keys.map((chord, index) => (
                        <Kbd key={String(index)} chord={chord} />
                      ))}
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          );
        })}
      </div>
    </Dialog>
  );
}
