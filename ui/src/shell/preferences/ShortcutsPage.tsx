// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Search } from "lucide-react";
import { useState } from "react";

import { type ActionGroup } from "../../actions/actions";
import { useActions } from "../../actions/ActionsProvider";
import { classes } from "../../classes";
import { chordLabel, Kbd } from "../../components/Kbd";
import { useT } from "../../i18n/I18n";
import { Group, Row } from "../PreferenceRows";
import styles from "../Preferences.module.css";

const GROUPS: readonly { group: ActionGroup; label: string }[] = [
  { group: "app", label: "shortcut-group-app" },
  { group: "navigation", label: "shortcut-group-navigation" },
  { group: "tasks", label: "shortcut-group-tasks" },
  { group: "window", label: "shortcut-group-window" },
];

/**
 * Every action that has a shortcut, from the same registry as the menus
 * and the `?` sheet, searchable by its name or its keys. Changing keys
 * comes later.
 */
export function ShortcutsPage() {
  const t = useT();
  const [query, setQuery] = useState("");
  const needle = query.trim().toLocaleLowerCase();
  const actions = useActions()
    .filter((action) => action.keys.length > 0)
    .filter((action) => needle === "" || [t(action.label), ...action.keys.map((chord) => chordLabel(chord, t))].some((text) => text.toLocaleLowerCase().includes(needle)));
  return (
    <>
      <label className={classes("field", styles.shortcutSearch)}>
        <Search className="ic s muted" aria-hidden="true" />
        <input
          type="search"
          aria-label={t("preferences-shortcuts-search")}
          placeholder={t("preferences-shortcuts-search")}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </label>
      {actions.length === 0 && <p className={styles.empty}>{t("preferences-shortcuts-none")}</p>}
      {GROUPS.map(({ group, label }) => {
        const inGroup = actions.filter((action) => action.group === group);
        if (inGroup.length === 0) {
          return null;
        }
        return (
          <Group key={group} title={t(label)}>
            {inGroup.map((action) => (
              <Row key={action.id} label={t(action.label)}>
                <span className={styles.shortcutKeys}>
                  {action.keys.map((chord, index) => (
                    <Kbd key={String(index)} chord={chord} />
                  ))}
                </span>
              </Row>
            ))}
          </Group>
        );
      })}
      <p className={classes("faint", styles.empty)}>{t("preferences-shortcuts-later")}</p>
    </>
  );
}
