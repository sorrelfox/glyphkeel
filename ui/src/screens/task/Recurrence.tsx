// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Repeat, RepeatOff, SkipForward, Copy } from "lucide-react";
import { useState } from "react";
import { Button, Dialog, DialogTrigger, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCommand, useOp } from "../../data/hooks";
import { longDay, monthName, ruleSummary, weekdayName, WEEKDAYS } from "../../data/recurrence";
import { type CreateWhen, type Frequency, type RepeatFrom, type RulePreviewInput, type Task, type Weekday } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Recurrence.module.css";

type MonthBy = "day" | "last" | "weekday";
type Ends = "never" | "on" | "after";

/** The editor's choices, as strings where a field holds them. */
interface Draft {
  every: Frequency;
  interval: string;
  weekdays: Weekday[];
  monthBy: MonthBy;
  monthDay: string;
  weekOfMonth: string;
  weekday: Weekday;
  month: string;
  ends: Ends;
  until: string;
  count: string;
  from: RepeatFrom;
  create: CreateWhen;
  daysAhead: string;
  text: string;
}

const UNITS: readonly Frequency[] = ["day", "week", "month", "year"];
const NTHS: readonly string[] = ["1", "2", "3", "4", "5", "-1"];
const MONTHS: readonly number[] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/** The day a task's occurrence falls on: its due date, else its start date, else today. */
const dayOfTask = (task: Task) => task.due_date ?? task.start_date ?? new Date().toISOString().slice(0, 10);

function weekdayOf(day: string): Weekday {
  return WEEKDAYS[(new Date(`${day}T00:00:00Z`).getUTCDay() + 6) % 7] ?? "mon";
}

function draftOf(task: Task): Draft {
  const rule = task.recurrence?.rule;
  const day = rule?.starts ?? dayOfTask(task);
  const [, month = 1, date = 1] = day.split("-").map(Number);
  return {
    every: rule?.every ?? "week",
    interval: String(rule?.interval ?? 1),
    weekdays: rule?.weekdays && rule.weekdays.length > 0 ? [...rule.weekdays] : [weekdayOf(day)],
    monthBy: rule?.week_of_month !== undefined ? "weekday" : rule?.month_day === -1 ? "last" : "day",
    monthDay: String(rule?.month_day !== undefined && rule.month_day > 0 ? rule.month_day : date),
    weekOfMonth: String(rule?.week_of_month ?? Math.min(4, Math.ceil(date / 7))),
    weekday: rule?.weekday ?? weekdayOf(day),
    month: String(rule?.month ?? month),
    ends: rule?.until !== undefined ? "on" : rule?.count !== undefined ? "after" : "never",
    until: rule?.until ?? "",
    count: String(rule?.count ?? 10),
    from: rule?.from ?? "schedule",
    create: rule?.create ?? "on_close",
    daysAhead: String(rule?.days_ahead ?? 0),
    text: "",
  };
}

const number = (text: string, fallback: number) => {
  const n = Number(text);
  return Number.isFinite(n) && text.trim() !== "" ? Math.trunc(n) : fallback;
};

/** The rule the editor's choices describe, as `recurrence.set` and `recurrence.preview` take it. */
function ruleInput(draft: Draft, starts: string): RulePreviewInput {
  const ahead = draft.from === "schedule" && draft.create === "ahead";
  const timing: RulePreviewInput = { from: draft.from, create: ahead ? "ahead" : "on_close", ...(ahead ? { days_ahead: number(draft.daysAhead, 0) } : {}) };
  const ends: RulePreviewInput = draft.ends === "on" && draft.until !== "" ? { until: draft.until } : draft.ends === "after" ? { count: number(draft.count, 1) } : {};
  if (draft.text.trim() !== "") {
    return { text: draft.text.trim(), starts, create: timing.create ?? null, ...(ahead ? { days_ahead: timing.days_ahead ?? null } : {}) };
  }
  const base: RulePreviewInput = { every: draft.every, interval: number(draft.interval, 1), starts, ...timing, ...ends };
  if (draft.from === "completion") {
    return base;
  }
  const onDay: RulePreviewInput =
    draft.monthBy === "weekday" ? { week_of_month: number(draft.weekOfMonth, 1), weekday: draft.weekday } : { month_day: draft.monthBy === "last" ? -1 : number(draft.monthDay, 1) };
  switch (draft.every) {
    case "week":
      return { ...base, weekdays: draft.weekdays };
    case "month":
      return { ...base, ...onDay };
    case "year":
      return { ...base, month: number(draft.month, 1), ...onDay };
    default:
      return base;
  }
}

/** The task panel's Repeats value: the rule in words, and the editor in a popover. */
export function RepeatValue({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const rule = task.recurrence?.rule;
  const label = rule === undefined ? t("repeat-none") : ruleSummary(t, locale, rule);
  return (
    <DialogTrigger>
      <Button className={classes("val", styles.value)} isDisabled={readOnly || task.closed_at !== undefined} aria-label={t("repeat-value", { value: label })}>
        {rule === undefined ? (
          <span className="faint">{t("repeat-none")}</span>
        ) : (
          <>
            <Repeat className="ic s" />
            <span className="truncate">{label}</span>
          </>
        )}
      </Button>
      <Popover className="pop" placement="bottom start" offset={4}>
        <Dialog className={classes(styles.dialog)} aria-label={t("repeat-title")}>
          {({ close }) => <RepeatForm task={task} close={close} />}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

function RepeatForm({ task, close }: { task: Task; close: () => void }) {
  const t = useT();
  const { locale } = useLocale();
  const [draft, setDraft] = useState(() => draftOf(task));
  const set = useCommand("recurrence.set", { undoable: t("toast-repeat-set") });
  const starts = task.recurrence?.rule.starts ?? dayOfTask(task);
  const input = ruleInput(draft, starts);
  const preview = useOp("recurrence.preview", { ...input, limit: 5 });
  const change = (patch: Partial<Draft>) => {
    setDraft({ ...draft, ...patch });
  };
  const typed = draft.text.trim() !== "";
  const pattern = !typed && draft.from === "schedule";
  const unitOptions = UNITS.map((unit) => ({ id: unit, label: t("repeat-unit-option", { unit, count: number(draft.interval, 1) }) }));
  const onDayOptions = [
    { id: "day" as const, label: t("repeat-by-day") },
    { id: "last" as const, label: t("repeat-by-last-day") },
    { id: "weekday" as const, label: t("repeat-by-weekday") },
  ];
  return (
    <form
      className={styles.form}
      onSubmit={(event) => {
        event.preventDefault();
        set.mutate({ key: task.key, ...input }, { onSuccess: close });
      }}
    >
      <fieldset className={styles.group} disabled={typed}>
        <div className={styles.row}>
          <span className={styles.label}>{t("repeat-every")}</span>
          <span className={classes("field", styles.number)}>
            <input type="number" min={1} max={999} aria-label={t("repeat-interval")} value={draft.interval} onChange={(event) => { change({ interval: event.target.value }); }} />
          </span>
          <Select label={t("repeat-unit")} value={draft.every} options={unitOptions} onChange={(every) => { change({ every }); }} isDisabled={typed} />
        </div>
        {pattern && draft.every === "week" && (
          <div className={styles.row} role="group" aria-label={t("repeat-weekdays")}>
            {WEEKDAYS.map((day) => {
              const on = draft.weekdays.includes(day);
              return (
                <button
                  key={day}
                  type="button"
                  className={classes("btn sm", styles.day, on && styles.dayOn)}
                  aria-pressed={on}
                  aria-label={weekdayName(locale, day)}
                  onClick={() => {
                    const next = on ? draft.weekdays.filter((each) => each !== day) : WEEKDAYS.filter((each) => each === day || draft.weekdays.includes(each));
                    if (next.length > 0) {
                      change({ weekdays: next });
                    }
                  }}
                >
                  {weekdayName(locale, day, "short")}
                </button>
              );
            })}
          </div>
        )}
        {pattern && draft.every === "year" && (
          <div className={styles.row}>
            <span className={styles.label}>{t("repeat-month")}</span>
            <Select label={t("repeat-month")} value={draft.month} options={MONTHS.map((month) => ({ id: String(month), label: monthName(locale, month) }))} onChange={(month) => { change({ month }); }} isDisabled={typed} />
          </div>
        )}
        {pattern && (draft.every === "month" || draft.every === "year") && (
          <div className={styles.row}>
            <span className={styles.label}>{t("repeat-monthly-by")}</span>
            <Select label={t("repeat-monthly-by")} value={draft.monthBy} options={onDayOptions} onChange={(monthBy) => { change({ monthBy }); }} isDisabled={typed} />
            {draft.monthBy === "day" && (
              <span className={classes("field", styles.number)}>
                <input type="number" min={1} max={31} aria-label={t("repeat-day-of-month")} value={draft.monthDay} onChange={(event) => { change({ monthDay: event.target.value }); }} />
              </span>
            )}
            {draft.monthBy === "weekday" && (
              <>
                <Select label={t("repeat-week-of-month")} value={draft.weekOfMonth} options={NTHS.map((nth) => ({ id: nth, label: t("repeat-nth-option", { nth: Number(nth) }) }))} onChange={(weekOfMonth) => { change({ weekOfMonth }); }} isDisabled={typed} />
                <Select label={t("repeat-weekday")} value={draft.weekday} options={WEEKDAYS.map((day) => ({ id: day, label: weekdayName(locale, day) }))} onChange={(weekday) => { change({ weekday }); }} isDisabled={typed} />
              </>
            )}
          </div>
        )}
        <div className={styles.row}>
          <span className={styles.label}>{t("repeat-ends")}</span>
          <Select
            label={t("repeat-ends")}
            value={draft.ends}
            options={[
              { id: "never" as const, label: t("repeat-ends-never") },
              { id: "on" as const, label: t("repeat-ends-on") },
              { id: "after" as const, label: t("repeat-ends-after") },
            ]}
            onChange={(ends) => { change({ ends }); }}
            isDisabled={typed}
          />
          {draft.ends === "on" && (
            <span className="field">
              <input type="date" aria-label={t("repeat-until")} value={draft.until} onChange={(event) => { change({ until: event.target.value }); }} />
            </span>
          )}
          {draft.ends === "after" && (
            <>
              <span className={classes("field", styles.number)}>
                <input type="number" min={1} max={10000} aria-label={t("repeat-count")} value={draft.count} onChange={(event) => { change({ count: event.target.value }); }} />
              </span>
              <span className="muted">{t("repeat-count-suffix", { count: number(draft.count, 1) })}</span>
            </>
          )}
        </div>
        <div className={styles.row}>
          <span className={styles.label}>{t("repeat-from")}</span>
          <div className="seg" role="radiogroup" aria-label={t("repeat-from")}>
            {(["schedule", "completion"] as const).map((each) => (
              <button key={each} type="button" role="radio" aria-checked={draft.from === each} className={classes(draft.from === each && "on")} onClick={() => { change({ from: each, ...(each === "completion" ? { create: "on_close" } : {}) }); }}>
                {t(each === "schedule" ? "repeat-from-schedule" : "repeat-from-completion")}
              </button>
            ))}
          </div>
        </div>
      </fieldset>
      <div className={styles.row}>
        <span className={styles.label}>{t("repeat-create")}</span>
        <div className="seg" role="radiogroup" aria-label={t("repeat-create")}>
          {(["on_close", "ahead"] as const).map((each) => (
            <button
              key={each}
              type="button"
              role="radio"
              aria-checked={draft.create === each}
              disabled={each === "ahead" && draft.from === "completion" && !typed}
              className={classes(draft.create === each && "on")}
              onClick={() => { change({ create: each }); }}
            >
              {t(each === "on_close" ? "repeat-create-on-close" : "repeat-create-ahead")}
            </button>
          ))}
        </div>
        {draft.create === "ahead" && (
          <>
            <span className={classes("field", styles.number)}>
              <input type="number" min={0} max={365} aria-label={t("repeat-days-ahead")} value={draft.daysAhead} onChange={(event) => { change({ daysAhead: event.target.value }); }} />
            </span>
            <span className="muted">{t("repeat-days-ahead-suffix", { count: number(draft.daysAhead, 0) })}</span>
          </>
        )}
      </div>
      <label className={styles.textRow}>
        <span className={styles.label}>{t("repeat-text")}</span>
        <span className="field">
          <input aria-label={t("repeat-text")} placeholder={t("repeat-text-placeholder")} value={draft.text} onChange={(event) => { change({ text: event.target.value }); }} />
        </span>
        <span className="faint">{t("repeat-text-hint")}</span>
      </label>
      <div className={styles.preview} aria-live="polite">
        {preview.isError ? (
          <p className={styles.problem} role="alert">
            {errorToast(t, preview.error).text}
          </p>
        ) : preview.data ? (
          <>
            <p className={styles.summary} data-testid="repeat-preview-summary">
              {ruleSummary(t, locale, preview.data.rule)}
            </p>
            <span className="meta">{t("repeat-preview")}</span>
            {preview.data.dates.length === 0 ? (
              <span className="faint">{t("repeat-preview-none")}</span>
            ) : (
              <ul className={styles.dates} aria-label={t("repeat-preview")}>
                {preview.data.dates.map((day) => (
                  <li key={day} className="num">
                    {longDay(locale, day)}
                  </li>
                ))}
              </ul>
            )}
          </>
        ) : null}
      </div>
      <div className={styles.buttons}>
        <div className="grow" />
        <Button className="btn flat sm" onPress={close}>
          {t("dialog-cancel")}
        </Button>
        <Button type="submit" className="btn primary sm" isDisabled={set.isPending || preview.isError}>
          {t("repeat-save")}
        </Button>
      </div>
    </form>
  );
}

/** What a recurring task's panel says about its series, and its actions. */
export function RecurrenceNote({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const toasts = useToasts();
  const skip = useCommand("recurrence.skip");
  const details = useCommand("recurrence.set", { undoable: t("toast-repeat-details") });
  const stop = useCommand("recurrence.clear", { undoable: t("toast-repeat-stopped") });
  const series = task.recurrence;
  if (series === undefined) {
    return null;
  }
  const next = series.next_date === undefined ? t(series.rule.from === "completion" ? "repeat-next-after-done" : "repeat-next-none") : t("repeat-next", { date: longDay(locale, series.next_date) });
  return (
    <section className={styles.note} aria-label={t("prop-repeats")} data-testid="recurrence-note">
      <div className={styles.noteHead}>
        <Repeat className="ic s" aria-hidden="true" />
        <span className={styles.summary}>{ruleSummary(t, locale, series.rule)}</span>
      </div>
      <div className={styles.facts}>
        <span className="meta" data-testid="recurrence-next">
          {next}
        </span>
        <span className="meta">{t("repeat-made", { count: series.made })}</span>
        {series.title !== task.title && <span className="meta">{t("repeat-series-title", { title: series.title })}</span>}
      </div>
      <p className={classes("faint", styles.explain)}>{t("repeat-note")}</p>
      {!readOnly && (
        <div className={styles.actions}>
          <Button
            className="btn sm"
            isDisabled={skip.isPending}
            onPress={() => {
              skip.mutate(
                { key: task.key },
                {
                  onSuccess: (changed) => {
                    if (changed.due_date !== undefined) {
                      toasts.show({ tone: "info", text: t("toast-repeat-skipped", { date: longDay(locale, changed.due_date) }) });
                    }
                  },
                },
              );
            }}
          >
            <SkipForward className="ic s" />
            {t("repeat-skip")}
          </Button>
          <Button className="btn sm" isDisabled={details.isPending} onPress={() => { details.mutate({ key: task.key, update_details: true }); }}>
            <Copy className="ic s" />
            {t("repeat-update-details")}
          </Button>
          <Button className="btn flat sm danger" isDisabled={stop.isPending} onPress={() => { stop.mutate({ key: task.key }); }}>
            <RepeatOff className="ic s" />
            {t("repeat-stop")}
          </Button>
        </div>
      )}
    </section>
  );
}

