// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ActionGroup } from "../actions/actions";
import { useActions } from "../actions/ActionsProvider";
import { parseChord } from "../actions/keymap";
import { Dialog } from "../components/Dialog";
import { Kbd } from "../components/Kbd";
import { useT } from "../i18n/I18n";
import { SCREEN_KEYS, TASK_KEYS, type KeyedScreen, type ScreenKey } from "./screenKeys";
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
  /** The screen on screen, when it has keys of its own. */
  screen?: KeyedScreen | null;
}

/**
 * `?`: every action that has a shortcut, from the same registry as the
 * menus, and the keys the screen on screen handles itself.
 */
export function ShortcutSheet({ isOpen, onOpenChange, screen = null }: ShortcutSheetProps) {
  const t = useT();
  const actions = useActions().filter((action) => action.keys.length > 0);
  const screenKeys = screen === null ? [] : SCREEN_KEYS[screen];
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
        {screenKeys.length > 0 && <KeyGroup title={t("shortcut-group-screen")} entries={screenKeys} />}
        <KeyGroup title={t("shortcut-group-task")} entries={TASK_KEYS} />
      </div>
    </Dialog>
  );
}

/** A group of keys from a table, each with what it does. */
function KeyGroup({ title, entries }: { title: string; entries: readonly ScreenKey[] }) {
  const t = useT();
  return (
    <section className={styles.shortcutGroup}>
      <h2 className="caption">{title}</h2>
      <dl className={styles.shortcutList}>
        {entries.map((entry) => (
          <div key={entry.label} className={styles.shortcutRow}>
            <dt>{t(entry.label)}</dt>
            <dd>
              {entry.keys.map((keys) => (
                <Kbd key={keys} chord={parseChord(keys)} />
              ))}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
