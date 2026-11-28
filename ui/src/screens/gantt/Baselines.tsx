// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Schedule baselines on the Gantt chart's toolbar: saving the plan as a
// named baseline, choosing the one to compare with, and the tasks it has
// that the chart no longer shows.

import { Check, ChevronDown } from "lucide-react";
import { useState } from "react";
import { Button, Header, Menu, MenuItem, MenuSection, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCall, useUndo } from "../../data/hooks";
import { type Baseline, type ScheduleBaseline } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { asDate } from "../calendar/model";
import styles from "./Gantt.module.css";

/** The choice of no baseline in the menu. */
const NONE = "none";

/** The Baseline menu: save the plan as a baseline, or show one (or none) under the bars. */
export function BaselineMenu({ project, baselines, shown, onShow }: { project: string; baselines: readonly Baseline[]; shown: string | undefined; onShow: (id: string | undefined) => void }) {
  const t = useT();
  const [dialog, setDialog] = useState<"save" | "rename" | "delete" | null>(null);
  const current = baselines.find((baseline) => baseline.id === shown);
  const name = current ? current.name : t("gantt-baseline-none");
  return (
    <>
      <MenuTrigger>
        <Button className="btn flat" aria-label={t("gantt-baseline-button", { name })} data-testid="gantt-baseline-menu">
          <span className="muted hide-narrow">{t("gantt-baseline")}</span> <span>{name}</span>
          <ChevronDown className="ic s muted" aria-hidden="true" />
        </Button>
        <Popover className="pop menuPopover" placement="bottom end" offset={4}>
          <Menu
            className="menu"
            aria-label={t("gantt-baseline")}
            onAction={(id) => {
              if (id === "save" || ((id === "rename" || id === "delete") && current)) {
                setDialog(id);
              }
            }}
          >
            <MenuSection>
              <MenuItem id="save" className="item">
                {t("gantt-baseline-save")}
              </MenuItem>
              <MenuItem id="rename" className="item" isDisabled={!current}>
                {t("gantt-baseline-rename")}
              </MenuItem>
              <MenuItem id="delete" className="item" isDisabled={!current}>
                {t("gantt-baseline-delete")}
              </MenuItem>
            </MenuSection>
            <MenuSection
              selectionMode="single"
              selectedKeys={[shown ?? NONE]}
              onSelectionChange={(keys) => {
                const [id] = keys === "all" ? [] : [...keys];
                onShow(id === undefined || id === NONE ? undefined : String(id));
              }}
              aria-label={t("gantt-baseline-show-label")}
            >
              <Header className={styles.menuHeader}>{t("gantt-baseline-show-label")}</Header>
              <MenuItem id={NONE} className="item" textValue={t("gantt-baseline-show-none")}>
                {({ isSelected }) => <Choice selected={isSelected} text={t("gantt-baseline-show-none")} />}
              </MenuItem>
              {baselines.map((baseline) => (
                <MenuItem key={baseline.id} id={baseline.id} className="item" textValue={baseline.name}>
                  {({ isSelected }) => <Choice selected={isSelected} text={t("gantt-baseline-show", { name: baseline.name, tasks: baseline.tasks })} />}
                </MenuItem>
              ))}
            </MenuSection>
          </Menu>
        </Popover>
      </MenuTrigger>
      {(dialog === "save" || (dialog === "rename" && current)) && (
        <SaveBaselineDialog
          project={project}
          renaming={dialog === "rename" ? current : undefined}
          onClose={(made) => {
            setDialog(null);
            if (made !== undefined) {
              onShow(made);
            }
          }}
        />
      )}
      {dialog === "delete" && current && (
        <DeleteBaselineDialog
          project={project}
          baseline={current}
          onClose={(deleted) => {
            setDialog(null);
            if (deleted) {
              onShow(undefined);
            }
          }}
        />
      )}
    </>
  );
}

/** A menu choice with a tick when it is the one chosen. */
function Choice({ selected, text }: { selected: boolean; text: string }) {
  return (
    <span className={styles.menuChoice}>
      <Check className="ic s" aria-hidden="true" style={{ visibility: selected ? "visible" : "hidden" }} />
      <span>{text}</span>
    </span>
  );
}

/** Deletes a baseline, once the person confirms; the tasks stay as they are. */
function DeleteBaselineDialog({ project, baseline, onClose }: { project: string; baseline: Baseline; onClose: (deleted: boolean) => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const remove = () => {
    setPending(true);
    call("baseline.delete", { project, baseline: baseline.id }).then(
      () => {
        toasts.show({ tone: "info", text: t("gantt-baseline-deleted", { name: baseline.name }), action: { label: t("toast-undo"), run: undo } });
        onClose(true);
      },
      (failure: unknown) => {
        setPending(false);
        const toast = errorToast(t, failure);
        setError([toast.text, toast.detail].filter(Boolean).join(" "));
      },
    );
  };
  return (
    <Dialog
      title={t("gantt-baseline-delete-title", { name: baseline.name })}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose(false);
        }
      }}
      footer={
        <>
          <Button
            className="btn"
            onPress={() => {
              onClose(false);
            }}
          >
            {t("dialog-cancel")}
          </Button>
          <Button className="btn danger" isDisabled={pending} onPress={remove}>
            {t("gantt-baseline-delete-button")}
          </Button>
        </>
      }
    >
      <div className={styles.baselineForm} data-baseline-delete="">
        <p>{t("gantt-baseline-delete-body", { name: baseline.name, count: baseline.tasks })}</p>
        {error !== null && (
          <p className={styles.baselineError} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** Saving the plan as a baseline, or renaming one: one name field. */
function SaveBaselineDialog({ project, renaming, onClose }: { project: string; renaming?: Baseline | undefined; onClose: (made: string | undefined) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const call = useCall();
  const toasts = useToasts();
  const { undo } = useUndo();
  const today = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric" }).format(new Date());
  const [name, setName] = useState(renaming?.name ?? t("gantt-baseline-default-name", { date: today }));
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const submit = () => {
    const trimmed = name.trim();
    if (trimmed === "" || pending) {
      return;
    }
    setPending(true);
    const done = renaming ? call("baseline.rename", { project, baseline: renaming.id, name: trimmed }) : call("baseline.create", { project, name: trimmed });
    done.then(
      (change) => {
        const text = renaming ? t("gantt-baseline-renamed", { name: change.baseline.name }) : t("gantt-baseline-saved", { name: change.baseline.name, count: change.baseline.tasks });
        toasts.show({ tone: "info", text, action: { label: t("toast-undo"), run: undo } });
        onClose(change.baseline.id);
      },
      (failure: unknown) => {
        setPending(false);
        const toast = errorToast(t, failure);
        setError([toast.text, toast.detail].filter(Boolean).join(" "));
      },
    );
  };

  return (
    <Dialog
      title={t(renaming ? "gantt-baseline-rename-title" : "gantt-baseline-save-title")}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose(undefined);
        }
      }}
      footer={
        <>
          <Button
            className="btn"
            onPress={() => {
              onClose(undefined);
            }}
          >
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" isDisabled={name.trim() === "" || pending} onPress={submit}>
            {t(renaming ? "gantt-baseline-rename-button" : "gantt-baseline-save-button")}
          </Button>
        </>
      }
    >
      <form
        className={styles.baselineForm}
        data-baseline-dialog=""
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        {!renaming && <p className="muted">{t("gantt-baseline-save-about")}</p>}
        <label className={styles.baselineField}>
          <span>{t("gantt-baseline-name")}</span>
          <span className="field">
            <input
              autoFocus
              value={name}
              maxLength={100}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
          </span>
        </label>
        {error !== null && (
          <p className={styles.baselineError} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

/** The tasks a baseline has that the chart no longer shows: deleted, moved away, or without dates now. */
export function RemovedPanel({ baseline }: { baseline: ScheduleBaseline }) {
  const t = useT();
  const { locale } = useLocale();
  const removed = baseline.removed ?? [];
  if (baseline.removed_total === 0) {
    return null;
  }
  const day = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
  const datesOf = (start: string | undefined, due: string | undefined) => {
    const first = start ?? due;
    const last = due ?? start;
    if (first === undefined || last === undefined) {
      return "";
    }
    return first === last ? day.format(asDate(first)) : day.formatRange(asDate(first), asDate(last));
  };
  return (
    <details className={styles.removed} data-testid="gantt-removed">
      <summary>{t("gantt-baseline-removed", { count: baseline.removed_total, name: baseline.name })}</summary>
      <ul>
        {removed.map((task) => (
          <li key={task.key} data-removed={task.key}>
            {t(task.deleted ? "gantt-baseline-removed-deleted" : "gantt-baseline-removed-undated", { key: task.key, title: task.title, dates: datesOf(task.start_date, task.due_date) })}
          </li>
        ))}
        {baseline.removed_total > removed.length && <li>{t("gantt-baseline-removed-more", { count: baseline.removed_total - removed.length })}</li>}
      </ul>
    </details>
  );
}
