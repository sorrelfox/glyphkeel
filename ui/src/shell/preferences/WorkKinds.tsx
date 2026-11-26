// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The workspace's kinds of work for logged time (Development, Design,
// Meeting…): add, rename, archive and delete. A kind that entries use is
// archived rather than deleted; it keeps its name on those entries.

import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Switch } from "../../components/Switch";
import { useCommand, useOp } from "../../data/hooks";
import { type WorkKind } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { Group } from "../PreferenceRows";
import styles from "../Preferences.module.css";

/** A name saved when the field loses the focus or on Enter; an empty one goes back. */
function NameField({ kind }: { kind: WorkKind }) {
  const t = useT();
  const update = useCommand("work_kind.update");
  const [text, setText] = useState(kind.name);
  const [seen, setSeen] = useState(kind.name);
  if (kind.name !== seen) {
    setSeen(kind.name);
    setText(kind.name);
  }
  const save = () => {
    const name = text.trim();
    if (name === "") {
      setText(kind.name);
    } else if (name !== kind.name) {
      update.mutate({ kind: kind.id, name });
    }
  };
  return (
    <span className={classes("field", styles.kindName)}>
      <input
        aria-label={t("work-kinds-name", { name: kind.name })}
        value={text}
        maxLength={60}
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
  );
}

function KindRow({ kind }: { kind: WorkKind }) {
  const t = useT();
  const { locale } = useLocale();
  const update = useCommand("work_kind.update");
  const remove = useCommand("work_kind.delete", { undoable: t("work-kinds-deleted", { name: kind.name }) });
  const count = new Intl.NumberFormat(locale);
  return (
    <div className={styles.kindRow} data-work-kind={kind.name}>
      <NameField kind={kind} />
      <span className={classes("faint num", styles.kindUses)}>{t("work-kinds-entries", { count: kind.entries, shown: count.format(kind.entries) })}</span>
      <Switch
        isSelected={kind.archived === true}
        onChange={(archived) => {
          update.mutate({ kind: kind.id, archived });
        }}
      >
        {t("work-kinds-archived")}
      </Switch>
      <Button
        className="btn flat icon sm"
        aria-label={t("work-kinds-delete", { name: kind.name })}
        isDisabled={kind.entries > 0}
        onPress={() => {
          remove.mutate({ kind: kind.id });
        }}
      >
        <Trash2 className="ic s" />
      </Button>
    </div>
  );
}

/** The kinds of work, archived ones included, and a field to add one. */
export function WorkKindsGroup() {
  const t = useT();
  const kinds = useOp("work_kind.list", { include_archived: true });
  const create = useCommand("work_kind.create");
  const [name, setName] = useState("");
  const add = () => {
    const clean = name.trim();
    if (clean !== "") {
      create.mutate({ name: clean }, { onSuccess: () => { setName(""); } });
    }
  };
  return (
    <Group title={t("work-kinds-title")} lead={<p className={classes("muted", styles.lead)}>{t("work-kinds-about")}</p>}>
      {(kinds.data?.items ?? []).map((kind) => (
        <KindRow key={kind.id} kind={kind} />
      ))}
      <div className={styles.kindRow}>
        <span className={classes("field", styles.kindName)}>
          <input
            aria-label={t("work-kinds-new")}
            placeholder={t("work-kinds-new")}
            maxLength={60}
            value={name}
            onChange={(event) => {
              setName(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                add();
              }
            }}
          />
        </span>
        <Button className="btn sm" isDisabled={name.trim() === "" || create.isPending} onPress={add}>
          <Plus className="ic s" />
          {t("work-kinds-add")}
        </Button>
      </div>
    </Group>
  );
}
