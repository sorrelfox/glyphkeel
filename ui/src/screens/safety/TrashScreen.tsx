// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { useCall, useOp, useUndo } from "../../data/hooks";
import { daysLeft, useBytes, useFailureToast, useTrashList } from "../../data/safety";
import { useDurations } from "../../data/time";
import { type TrashEntry, type TrashKind } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useMomentFormat } from "../task/Activity";
import styles from "./Safety.module.css";

/** The kinds the filter offers, in the order people think of them. */
export const TRASH_KINDS: readonly TrashKind[] = ["task", "project", "wiki_page", "comment", "file", "time_entry"];

const ALL = "*";

/** Items purged within this many days are marked. */
const SOON_DAYS = 3;

/**
 * The trash: deleted items, newest deletion first, with who deleted them,
 * when, and the days left before they are purged. Selected items can be
 * restored (with everything deleted with them) or deleted for good; the
 * whole trash can be emptied. Deleting for good is confirmed by the app's
 * native dialog, never by the page.
 */
export function TrashScreen() {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const { undo } = useUndo();
  const projects = useOp("project.list", { include_archived: true });
  const [kind, setKind] = useState<TrashKind | typeof ALL>(ALL);
  const [project, setProject] = useState<string>(ALL);
  const trash = useTrashList({ kinds: kind === ALL ? [] : [kind], project: project === ALL ? undefined : project });
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const entries = trash.data?.pages.flatMap((page) => page.items) ?? [];
  const total = trash.data?.pages[0]?.total ?? 0;
  const trashDays = trash.data?.pages[0]?.trash_days;
  const chosen = entries.filter((entry) => selected.has(entry.id)).map((entry) => entry.id);

  const toggle = (id: string, on: boolean) => {
    setSelected((was) => {
      const next = new Set(was);
      if (on) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  };
  const run = (work: Promise<unknown>) => {
    setBusy(true);
    work.then(
      () => {
        setSelected(new Set());
        setBusy(false);
      },
      (error: unknown) => {
        fail(error);
        setBusy(false);
      },
    );
  };
  const restore = (ids: readonly string[]) => {
    run(
      call("trash.restore", { items: [...ids] }).then((change) => {
        toasts.show({ tone: "info", text: t("trash-restored", { count: change.items.length, total: change.total }), action: { label: t("toast-undo"), run: undo } });
      }),
    );
  };
  const purge = (input: { items: string[] } | { all: true }) => {
    run(
      call("trash.purge", input).then((change) => {
        toasts.show({ tone: "info", text: t("trash-purged", { count: change.items.length, total: change.total }) });
      }),
    );
  };

  return (
    <section className={styles.screen} aria-label={t("view-trash")}>
      <div className={classes("toolbar", styles.toolbar)} role="toolbar" aria-label={t("trash-toolbar")}>
        <Select
          className={styles.select}
          label={t("trash-filter-kind")}
          value={kind}
          onChange={(next) => {
            setKind(next);
            setSelected(new Set());
          }}
          options={[{ id: ALL, label: t("trash-kind-all") }, ...TRASH_KINDS.map((each) => ({ id: each, label: t(`trash-kind-${each}`) }))]}
        />
        <Select
          className={styles.select}
          label={t("trash-filter-project")}
          value={project}
          onChange={(next) => {
            setProject(next);
            setSelected(new Set());
          }}
          options={[
            { id: ALL, label: t("report-all-projects") },
            ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> })),
          ]}
        />
        <div className="grow" />
        {chosen.length > 0 && <span className="muted">{t("list-selected", { count: chosen.length })}</span>}
        <Button className="btn" isDisabled={busy || chosen.length === 0} onPress={() => { restore(chosen); }}>
          <RotateCcw className="ic s" />
          {t("trash-restore")}
        </Button>
        <Button className="btn danger" isDisabled={busy || chosen.length === 0} onPress={() => { purge({ items: chosen }); }}>
          <Trash2 className="ic s" />
          {t("trash-purge")}
        </Button>
        <Button className="btn flat danger" isDisabled={busy || total === 0} onPress={() => { purge({ all: true }); }}>
          {t("trash-empty")}
        </Button>
      </div>
      {trashDays !== undefined && <p className={classes("faint", styles.note, styles.small)}>{t("trash-about", { days: trashDays })}</p>}
      {trash.isError && <p className={classes("faint", styles.note)}>{trash.error.error.message}</p>}
      {trash.data && entries.length === 0 && <p className={classes("muted", styles.note)}>{t("trash-none")}</p>}
      {entries.length > 0 && (
        <div className={styles.scroll}>
          <div role="table" aria-label={t("view-trash")} aria-rowcount={total + 1}>
            <div role="row" className={classes("tr head", styles.trashRow)}>
              <span role="columnheader" className="th">
                <input
                  type="checkbox"
                  className="check"
                  aria-label={t("trash-select-all")}
                  checked={chosen.length > 0 && chosen.length === entries.length}
                  onChange={(event) => {
                    setSelected(event.target.checked ? new Set(entries.map((entry) => entry.id)) : new Set());
                  }}
                />
              </span>
              <span role="columnheader" className="th">{t("trash-column-item")}</span>
              <span role="columnheader" className="th">{t("trash-column-kind")}</span>
              <span role="columnheader" className="th">{t("trash-column-project")}</span>
              <span role="columnheader" className="th">{t("trash-column-deleted")}</span>
              <span role="columnheader" className="th">{t("trash-column-purged")}</span>
            </div>
            {entries.map((entry) => (
              <TrashRow key={entry.id} entry={entry} selected={selected.has(entry.id)} onSelect={(on) => { toggle(entry.id, on); }} />
            ))}
          </div>
          {trash.hasNextPage && (
            <div className={styles.more}>
              <Button className="btn flat sm" isDisabled={trash.isFetchingNextPage} onPress={() => void trash.fetchNextPage()}>
                {t("trash-more")}
              </Button>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function TrashRow({ entry, selected, onSelect }: { entry: TrashEntry; selected: boolean; onSelect: (on: boolean) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const bytes = useBytes();
  const durations = useDurations();
  const days = daysLeft(entry.purge_at, new Date());
  const who = entry.deleted_by === undefined ? undefined : entry.deleted_by.kind === "user" ? t("comment-me") : entry.deleted_by.name;
  return (
    <div role="row" className={classes("tr", styles.trashRow, selected && "on")} data-trash={entry.id} aria-selected={selected}>
      <span role="cell" className={styles.cell}>
        <input type="checkbox" className="check" aria-label={t("trash-select", { title: entry.title })} checked={selected} onChange={(event) => { onSelect(event.target.checked); }} />
      </span>
      <span role="cell" className={styles.cell}>
        {entry.key !== undefined && <span className="key">{entry.key}</span>}
        <span className={styles.title}>{entry.title === "" ? t("trash-untitled") : entry.title}</span>
        {entry.task !== undefined && entry.kind !== "task" && <span className={classes("faint", styles.small)}>{t("trash-on-task", { key: entry.task })}</span>}
        {entry.minutes !== undefined && <span className={classes("faint num", styles.small)}>{durations.format(entry.minutes)}</span>}
        {entry.size !== undefined && <span className={classes("faint num", styles.small)}>{bytes(entry.size)}</span>}
        {entry.contains > 0 && <span className={classes("faint", styles.small)}>{t("trash-contains", { count: entry.contains })}</span>}
      </span>
      <span role="cell" className={classes(styles.cell, "muted")}>{t(`trash-kind-${entry.kind}`)}</span>
      <span role="cell" className={classes(styles.cell, "muted")}>{entry.project ?? ""}</span>
      <span role="cell" className={classes(styles.cell, styles.small)}>
        {who === undefined ? moment(entry.deleted_at) : t("trash-deleted-by", { actor: who, time: moment(entry.deleted_at) })}
      </span>
      <span role="cell" className={classes(styles.cell, styles.small, "num", days <= SOON_DAYS && styles.soon)}>
        {t("trash-days-left", { days })}
      </span>
    </div>
  );
}
