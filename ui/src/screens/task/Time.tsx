// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Time on a task: the Time tab with its entries, the timer button and the
// quick "+30m" of the action row, and the task's reminder.

import { Bell, BellOff, ChevronDown, MoreHorizontal, Pause, Pencil, Play, Plus, Square, Trash2 } from "lucide-react";
import { useState, type KeyboardEvent } from "react";
import { Button, Dialog, DialogTrigger, Menu, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { formatClock } from "../../data/duration";
import { errorToast, useCall, useCommand, useOp } from "../../data/hooks";
import { isLongRunning, useDurations, useElapsedSeconds, useTimer, useTimerCommands } from "../../data/time";
import { type Reminder, type Task, type TimeEntry, type WorkKind } from "../../generated/ops";
import { dayIn, momentFrom, momentIn, useTimeZone, useToday } from "../../data/zone";
import { useT } from "../../i18n/I18n";
import { useDayFormat } from "../board/CardView";
import styles from "./Time.module.css";

// ---------- the Time tab ----------

interface EntryDraft {
  duration: string;
  date: string;
  note: string;
  /** The kind of work's ID; empty for none. */
  kind: string;
}

/** The list's key for "no kind of work"; no ID has this form. */
const NO_KIND = "-";

/** The kinds of work new time can take, and an archived one an entry keeps (`keep`). */
function useKindChoices(keep?: string): WorkKind[] {
  const kinds = useOp("work_kind.list", { include_archived: true });
  return (kinds.data?.items ?? []).filter((kind) => kind.archived !== true || kind.id === keep);
}

/** The kind of work of an entry or the timer: a picker with "No kind of work". */
function KindPicker({ kinds, value, onChange, className, label }: { kinds: readonly WorkKind[]; value: string; onChange: (id: string) => void; className?: string | undefined; label?: string | undefined }) {
  const t = useT();
  return (
    <Select
      className={className}
      label={label ?? t("time-work-kind")}
      value={value === "" ? NO_KIND : value}
      onChange={(id) => {
        onChange(id === NO_KIND ? "" : id);
      }}
      options={[{ id: NO_KIND, label: t("time-no-work-kind") }, ...kinds.map((kind) => ({ id: kind.id, label: kind.name }))]}
    />
  );
}

/** The fields of a time entry: duration, day, note and kind of work. Enter saves, Escape cancels. */
function EntryForm({
  initial,
  label,
  saveLabel,
  busy,
  onSave,
  onCancel,
}: {
  initial: EntryDraft;
  label: string;
  saveLabel: string;
  busy: boolean;
  onSave: (draft: EntryDraft, minutes: number) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const durations = useDurations();
  const [draft, setDraft] = useState(initial);
  const [invalid, setInvalid] = useState(false);
  const kinds = useKindChoices(initial.kind);
  const save = () => {
    const minutes = durations.parse(draft.duration);
    if (minutes === null || minutes < 1 || minutes > 24 * 60) {
      setInvalid(true);
      return;
    }
    onSave(draft, minutes);
  };
  const keys = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      event.preventDefault();
      save();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
    }
  };
  return (
    <div className={styles.logForm} role="group" aria-label={label}>
      <span className={classes("field", styles.duration, invalid && styles.invalid)}>
        <input
          aria-label={t("time-duration")}
          aria-invalid={invalid}
          autoFocus
          placeholder={t("prop-duration-placeholder")}
          value={draft.duration}
          onChange={(event) => {
            setDraft({ ...draft, duration: event.target.value });
            setInvalid(false);
          }}
          onKeyDown={keys}
        />
      </span>
      <span className={classes("field", styles.day)}>
        <input
          type="date"
          aria-label={t("time-day")}
          value={draft.date}
          required
          onChange={(event) => {
            setDraft({ ...draft, date: event.target.value });
          }}
          onKeyDown={keys}
        />
      </span>
      <span className={classes("field", styles.note)}>
        <input
          aria-label={t("time-note")}
          placeholder={t("time-note-placeholder")}
          maxLength={2000}
          value={draft.note}
          onChange={(event) => {
            setDraft({ ...draft, note: event.target.value });
          }}
          onKeyDown={keys}
        />
      </span>
      {kinds.length > 0 && (
        <KindPicker
          className={styles.kind}
          kinds={kinds}
          value={draft.kind}
          onChange={(kind) => {
            setDraft({ ...draft, kind });
          }}
        />
      )}
      <Button className="btn flat sm" onPress={onCancel}>
        {t("dialog-cancel")}
      </Button>
      <Button className="btn primary sm" isDisabled={busy || draft.duration.trim() === ""} onPress={save}>
        {saveLabel}
      </Button>
      {invalid && (
        <span className={styles.error} role="alert">
          {t("time-duration-invalid")}
        </span>
      )}
    </div>
  );
}

function EntryRow({ entry, readOnly }: { entry: TimeEntry; readOnly: boolean }) {
  const t = useT();
  const day = useDayFormat();
  const durations = useDurations();
  const [editing, setEditing] = useState(false);
  const update = useCommand("time.update");
  const remove = useCommand("time.delete", { undoable: t("time-deleted") });
  const author = entry.author.kind === "user" ? t("comment-me") : entry.author.name;
  if (editing) {
    return (
      <li>
        <EntryForm
          initial={{ duration: durations.format(entry.minutes), date: entry.date, note: entry.note ?? "", kind: entry.work_kind?.id ?? "" }}
          label={t("time-edit-label", { day: day(entry.date) })}
          saveLabel={t("panel-save")}
          busy={update.isPending}
          onCancel={() => {
            setEditing(false);
          }}
          onSave={(draft, minutes) => {
            update.mutate(
              {
                entry: entry.id,
                expected_version: entry.version,
                ...(minutes === entry.minutes ? {} : { duration: `${String(minutes)}m` }),
                ...(draft.date === entry.date || draft.date === "" ? {} : { date: draft.date }),
                ...(draft.note.trim() === (entry.note ?? "") ? {} : { note: draft.note.trim() }),
                ...(draft.kind === (entry.work_kind?.id ?? "") ? {} : { work_kind: draft.kind }),
              },
              { onSuccess: () => { setEditing(false); } },
            );
          }}
        />
      </li>
    );
  }
  return (
    <li className={styles.entry} data-time-entry={entry.id} aria-label={t("time-entry-label", { day: day(entry.date), duration: durations.format(entry.minutes), author })}>
      <span className={classes("num", styles.entryDay)}>{day(entry.date)}</span>
      <span className={classes("num", styles.entryDuration)}>{durations.format(entry.minutes)}</span>
      <span className={classes("selectable", styles.entryNote)}>{entry.note ?? <span className="faint">{author}</span>}</span>
      {entry.work_kind && (
        <span className="chip" data-work-kind={entry.work_kind.name}>
          {entry.work_kind.name}
        </span>
      )}
      {entry.author.kind !== "user" && <span className="chip hue-purple">{entry.author.name}</span>}
      <MenuTrigger>
        <Button className="btn flat icon sm" isDisabled={readOnly} aria-label={t("time-entry-menu")}>
          <MoreHorizontal className="ic s" />
        </Button>
        <Popover className="pop menuPopover" placement="bottom end" offset={4}>
          <Menu className="menu" aria-label={t("time-entry-menu")}>
            <MenuEntries
              entries={[
                { id: "edit", label: t("time-edit"), icon: <Pencil className="ic s" />, run: () => { setEditing(true); } },
                { id: "delete", label: t("time-delete"), icon: <Trash2 className="ic s" />, run: () => { remove.mutate({ entry: entry.id }); } },
              ]}
            />
          </Menu>
        </Popover>
      </MenuTrigger>
    </li>
  );
}

/** The Time tab: the task's entries, newest day first, their total, and "Log time". */
export function TimeEntries({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const durations = useDurations();
  const [logging, setLogging] = useState(false);
  const today = useToday();
  const [requestId, setRequestId] = useState(() => crypto.randomUUID());
  const log = useCommand("time.log");
  const entries = task.time_entries ?? [];
  return (
    <>
      <div className={styles.timeHead}>
        <span className="muted">{t("time-total")}</span>
        <span className={classes("num", styles.total)} data-time-total>
          {durations.format(task.spent_minutes)}
        </span>
        <div className="grow" />
        {!logging && (
          <Button className="btn sm" isDisabled={readOnly} onPress={() => { setLogging(true); }}>
            <Plus className="ic s" />
            {t("time-log")}
          </Button>
        )}
      </div>
      {logging && (
        <EntryForm
          initial={{ duration: "", date: today, note: "", kind: "" }}
          label={t("time-log")}
          saveLabel={t("time-log-save")}
          busy={log.isPending}
          onCancel={() => {
            setLogging(false);
          }}
          onSave={(draft, minutes) => {
            log.mutate(
              {
                key: task.key,
                duration: `${String(minutes)}m`,
                date: draft.date === "" ? "today" : draft.date,
                note: draft.note.trim() === "" ? null : draft.note.trim(),
                ...(draft.kind === "" ? {} : { work_kind: draft.kind }),
                client_request_id: requestId,
              },
              {
                onSuccess: () => {
                  setLogging(false);
                  setRequestId(crypto.randomUUID());
                },
              },
            );
          }}
        />
      )}
      {entries.length === 0 ? (
        <p className="faint">{t("time-none")}</p>
      ) : (
        <ol className={styles.entries} aria-label={t("time-entries")}>
          {entries.map((entry) => (
            <EntryRow key={entry.id} entry={entry} readOnly={readOnly} />
          ))}
        </ol>
      )}
    </>
  );
}

// ---------- the action row ----------

/**
 * The timer button: "Start timer", or, while this task's timer runs, the
 * running time, drawn toggled; pressing it stops the timer and logs the time.
 * A paused timer on this task offers to go on.
 */
export function TimerButton({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const settings = useOp("settings.get", {});
  const { timer, fetchedAt } = useTimer();
  const mine = timer !== undefined && timer.task.key === task.key ? timer : undefined;
  const seconds = useElapsedSeconds(mine, fetchedAt);
  const start = useCommand("timer.start");
  const pause = useCommand("timer.pause");
  const { stopAndLog, pending: stopping } = useTimerCommands();
  const kinds = useKindChoices(mine?.work_kind?.id);
  if (mine === undefined) {
    return (
      <>
        <Button className="btn" isDisabled={readOnly || start.isPending} onPress={() => { start.mutate({ key: task.key }); }}>
          <Play className="ic" />
          {t("timer-start")}
        </Button>
        {kinds.length > 0 && (
          <MenuTrigger>
            <Button className="btn flat icon" isDisabled={readOnly || start.isPending} aria-label={t("timer-start-as")}>
              <ChevronDown className="ic s" />
            </Button>
            <Popover className="pop menuPopover" placement="bottom end" offset={4}>
              <Menu className="menu" aria-label={t("timer-start-as")}>
                <MenuEntries entries={kinds.map((kind) => ({ id: kind.id, label: kind.name, run: () => { start.mutate({ key: task.key, work_kind: kind.id }); } }))} />
              </Menu>
            </Popover>
          </MenuTrigger>
        )}
      </>
    );
  }
  const long = isLongRunning(seconds, settings.data?.timer_reminder_hours);
  const running = mine.state === "running";
  return (
    <>
      <Button
        className={classes("btn", running && "on", styles.timerButton)}
        aria-label={t(running ? "timer-running-label" : "timer-paused-label", { time: formatClock(seconds) })}
        isDisabled={stopping}
        onPress={stopAndLog}
      >
        <Square className="ic" />
        <span className={classes("num", styles.clock, long && styles.long)} data-timer-clock>
          {formatClock(seconds)}
        </span>
      </Button>
      <Button
        className="btn flat icon"
        aria-label={t(running ? "timer-pause" : "timer-resume")}
        onPress={() => {
          if (running) {
            pause.mutate({});
          } else {
            start.mutate({});
          }
        }}
      >
        {running ? <Pause className="ic" /> : <Play className="ic" />}
      </Button>
      {running && kinds.length > 0 && (
        <KindPicker
          kinds={kinds}
          label={t("timer-kind")}
          value={mine.work_kind?.id ?? ""}
          onChange={(kind) => {
            start.mutate({ key: task.key, work_kind: kind });
          }}
        />
      )}
    </>
  );
}

/** "+30m": logs half an hour for today. */
export function QuickLog({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const log = useCommand("time.log", { undoable: t("time-logged-quick", { key: task.key }) });
  return (
    <Button
      className="btn flat"
      isDisabled={readOnly || log.isPending}
      aria-label={t("time-quick-label")}
      onPress={() => {
        log.mutate({ key: task.key, duration: "+30m", date: "today", client_request_id: crypto.randomUUID() });
      }}
    >
      {t("time-quick")}
    </Button>
  );
}

// ---------- the reminder ----------

/** What the reminder says in the panel: "Nov 24, 10:00 AM · 1 day before due"; the year only when it isn't this one. */
function useReminderText(): (reminder: Reminder) => string {
  const t = useT();
  const { locale } = useLocale();
  const zone = useTimeZone();
  const inZone = zone === undefined ? {} : { timeZone: zone };
  const thisYear = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit", ...inZone });
  const otherYear = new Intl.DateTimeFormat(locale, { year: "numeric", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", ...inZone });
  const moment = (at: string) => {
    const date = new Date(at);
    return (dayIn(date, zone).slice(0, 4) === dayIn(new Date(), zone).slice(0, 4) ? thisYear : otherYear).format(date);
  };
  return (reminder) => {
    if (reminder.days_before_due !== undefined) {
      const relative = t("reminder-before-due", { count: reminder.days_before_due });
      return reminder.at === undefined ? relative : `${moment(reminder.at)} · ${relative}`;
    }
    return reminder.at === undefined ? t("prop-none") : moment(reminder.at);
  };
}

/**
 * The task's reminder: none, a moment, or some days before the due date at
 * a time of day. Opens a small form; "Remove" clears it.
 */
export function ReminderValue({ task, readOnly }: { task: Task; readOnly: boolean }) {
  const t = useT();
  const text = useReminderText();
  const reminder = task.reminder;
  const label = reminder === undefined ? t("prop-none") : text(reminder);
  return (
    <DialogTrigger>
      <Button className={classes("val num", styles.reminderValue)} isDisabled={readOnly} aria-label={t("prop-date-value", { name: t("prop-reminder"), value: label })}>
        {reminder === undefined ? (
          <span className="faint">{t("prop-none")}</span>
        ) : (
          <>
            <Bell className={classes("ic s", reminder.acknowledged && "faint")} />
            {label}
          </>
        )}
      </Button>
      <Popover className="pop" placement="bottom start" offset={4}>
        <Dialog className={classes(styles.reminderDialog)} aria-label={t("reminder-title")}>
          {({ close }) => <ReminderForm task={task} close={close} />}
        </Dialog>
      </Popover>
    </DialogTrigger>
  );
}

function ReminderForm({ task, close }: { task: Task; close: () => void }) {
  const t = useT();
  const toasts = useToasts();
  const call = useCall();
  const settings = useOp("settings.get", {});
  const reminder = task.reminder;
  const [mode, setMode] = useState<"at" | "due">(reminder?.days_before_due !== undefined || (reminder === undefined && task.due_date !== undefined) ? "due" : "at");
  const zone = useTimeZone();
  const [at, setAt] = useState(() => momentIn(reminder?.at !== undefined && reminder.days_before_due === undefined ? new Date(reminder.at) : Date.now() + 3_600_000, zone));
  const [days, setDays] = useState(String(reminder?.days_before_due ?? 1));
  const [time, setTime] = useState(reminder?.time ?? "");
  const [note, setNote] = useState(reminder?.note ?? "");
  const run = (promise: Promise<unknown>) => {
    promise.then(close, (error: unknown) => {
      toasts.show(errorToast(t, error));
    });
  };
  const noteOrNull = note.trim() === "" ? null : note.trim();
  return (
    <form
      className={styles.reminderForm}
      onSubmit={(event) => {
        event.preventDefault();
        run(
          call(
            "reminder.set",
            mode === "at"
              ? { key: task.key, at: new Date(momentFrom(at, zone)).toISOString(), note: noteOrNull }
              : { key: task.key, days_before_due: Number(days), time: time === "" ? null : time, note: noteOrNull },
          ),
        );
      }}
    >
      <div className="seg" role="radiogroup" aria-label={t("reminder-kind")}>
        {(["at", "due"] as const).map((each) => (
          <button key={each} type="button" role="radio" aria-checked={mode === each} className={classes(mode === each && "on")} onClick={() => { setMode(each); }}>
            {t(each === "at" ? "reminder-at" : "reminder-due")}
          </button>
        ))}
      </div>
      {mode === "at" ? (
        <span className={classes("field", styles.reminderInput)}>
          <input type="datetime-local" aria-label={t("reminder-moment")} required value={at} onChange={(event) => { setAt(event.target.value); }} />
        </span>
      ) : (
        <div className={styles.reminderRow}>
          <span className={classes("field", styles.daysInput)}>
            <input type="number" min={0} max={365} aria-label={t("reminder-days")} required value={days} onChange={(event) => { setDays(event.target.value); }} />
          </span>
          <span className="muted">{t("reminder-days-before", { count: Number(days) || 0 })}</span>
          <span className={classes("field", styles.reminderInput)}>
            <input type="time" aria-label={t("reminder-time")} title={t("reminder-time-default", { time: settings.data?.reminder_time ?? "09:00" })} value={time} onChange={(event) => { setTime(event.target.value); }} />
          </span>
        </div>
      )}
      {mode === "due" && task.due_date === undefined && <span className="faint">{t("reminder-no-due")}</span>}
      <span className={classes("field", styles.reminderInput)}>
        <input aria-label={t("reminder-note")} placeholder={t("reminder-note-placeholder")} maxLength={500} value={note} onChange={(event) => { setNote(event.target.value); }} />
      </span>
      <div className={styles.reminderButtons}>
        {reminder !== undefined && (
          <Button className="btn flat sm danger" onPress={() => { run(call("reminder.clear", { key: task.key })); }}>
            <BellOff className="ic s" />
            {t("reminder-remove")}
          </Button>
        )}
        <div className="grow" />
        <Button className="btn flat sm" onPress={close}>
          {t("dialog-cancel")}
        </Button>
        <Button type="submit" className="btn primary sm">
          {t("reminder-set")}
        </Button>
      </div>
    </form>
  );
}
