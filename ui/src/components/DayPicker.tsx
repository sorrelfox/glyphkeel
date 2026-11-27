// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { Button, Dialog, useLocale } from "react-aria-components";

import { classes } from "../classes";
import { useOp } from "../data/hooks";
import { parseNaturalDay } from "../data/naturalDate";
import { useToday } from "../data/zone";
import { useT } from "../i18n/I18n";
import { addDays, addMonths, asDate, firstDayOfWeek, startOfWeek } from "../screens/calendar/model";
import { FitPopover } from "./FitPopover";
import styles from "./DayPicker.module.css";

/** What a day picker answers: a day, null to clear it, or undefined to leave it. */
export type DayChoice = string | null | undefined;

const format = (locale: string, options: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, { timeZone: "UTC", ...options });

/**
 * A day field's editor in a popover anchored to its value: one line for a
 * day in words (`next fri`, `+3d`, `dec 1`, `2026-12-01`) with the day it
 * means shown under it, a month to pick from, Today and Clear. Enter or a
 * click on a day saves; Escape leaves the day as it was; a click outside
 * keeps a day typed in words.
 */
export function DayPopover({
  triggerRef,
  isOpen,
  label,
  value,
  onDone,
}: {
  triggerRef: RefObject<HTMLElement | null>;
  isOpen: boolean;
  label: string;
  value: string | null;
  onDone: (choice: DayChoice) => void;
}) {
  const pending = useRef<DayChoice>(undefined);
  return (
    <FitPopover
      triggerRef={triggerRef}
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          onDone(pending.current);
        }
      }}
      placement="bottom start"
      offset={4}
    >
      <Dialog className={classes(styles.dialog)} aria-label={label}>
        <DayPicker
          label={label}
          value={value}
          onTyped={(day) => {
            pending.current = day ?? undefined;
          }}
          onDone={onDone}
        />
      </Dialog>
    </FitPopover>
  );
}

function DayPicker({ label, value, onTyped, onDone }: { label: string; value: string | null; onTyped: (day: string | null) => void; onDone: (choice: DayChoice) => void }) {
  const t = useT();
  const { locale, direction } = useLocale();
  const today = useToday();
  const settings = useOp("settings.get", {});
  const [words, setWords] = useState("");
  const [invalid, setInvalid] = useState(false);
  const typed = words.trim();
  const parsed = typed === "" ? null : parseNaturalDay(typed, today, settings.data?.week_start ?? "monday");
  const shown = parsed ?? value;
  const [cursor, setCursor] = useState(shown ?? today);
  const focusCursor = useRef(false);
  const grid = useRef<HTMLTableElement>(null);
  const first = firstDayOfWeek(locale);
  const id = useId();

  useEffect(() => {
    if (focusCursor.current) {
      focusCursor.current = false;
      grid.current?.querySelector<HTMLButtonElement>(`button[data-day="${cursor}"]`)?.focus();
    }
  }, [cursor]);

  const month = `${cursor.slice(0, 7)}-01`;
  const start = startOfWeek(month, first);
  const weeks = Array.from({ length: 6 }, (_, row) => Array.from({ length: 7 }, (_, column) => addDays(start, row * 7 + column)));
  const long = format(locale, { weekday: "long", year: "numeric", month: "long", day: "numeric" });
  const dayNumber = format(locale, { day: "numeric" });
  const monthTitle = format(locale, { month: "long", year: "numeric" });
  const weekdayShort = format(locale, { weekday: "narrow" });
  const weekdayLong = format(locale, { weekday: "long" });
  const preview = typed === "" ? null : parsed === null ? t("day-picker-unknown") : long.format(asDate(parsed));

  const move = (to: string) => {
    if (to === cursor) {
      grid.current?.querySelector<HTMLButtonElement>(`button[data-day="${to}"]`)?.focus();
      return;
    }
    focusCursor.current = true;
    setCursor(to);
  };
  const onGridKey = (event: KeyboardEvent<HTMLButtonElement>, day: string) => {
    const forward = direction === "rtl" ? -1 : 1;
    const steps: Record<string, () => string> = {
      ArrowRight: () => addDays(day, forward),
      ArrowLeft: () => addDays(day, -forward),
      ArrowDown: () => addDays(day, 7),
      ArrowUp: () => addDays(day, -7),
      PageDown: () => addMonths(day, event.shiftKey ? 12 : 1),
      PageUp: () => addMonths(day, event.shiftKey ? -12 : -1),
      Home: () => startOfWeek(day, first),
      End: () => addDays(startOfWeek(day, first), 6),
    };
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onDone(day);
      return;
    }
    const step = steps[event.key];
    if (step) {
      event.preventDefault();
      move(step());
    }
  };

  return (
    <div
      className={styles.picker}
      onKeyDown={(event) => {
        // Keys stay in the picker: the list and the panel around it have their own.
        event.stopPropagation();
        if (event.key === "Escape") {
          // Escape leaves the day as it was, whatever was typed.
          event.preventDefault();
          onDone(undefined);
        }
      }}
      onClick={(event) => {
        event.stopPropagation();
      }}
    >
      <div className={styles.top}>
        <span className={classes("field", invalid && styles.invalid)}>
          <input
            type="text"
            aria-label={t("date-words-label", { name: label })}
            aria-describedby={`${id}-preview`}
            aria-invalid={invalid || undefined}
            placeholder={t("date-words-placeholder")}
            autoFocus
            value={words}
            onChange={(event) => {
              setWords(event.target.value);
              setInvalid(false);
              const next = event.target.value.trim();
              const day = next === "" ? null : parseNaturalDay(next, today, settings.data?.week_start ?? "monday");
              onTyped(day);
              if (day !== null) {
                // The month follows the day the words mean.
                setCursor(day);
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                if (typed === "") {
                  onDone(undefined);
                } else if (parsed !== null) {
                  onDone(parsed);
                } else {
                  setInvalid(true);
                }
              } else if (event.key === "ArrowDown") {
                event.preventDefault();
                move(cursor);
              }
            }}
          />
        </span>
        <p id={`${id}-preview`} className={classes(styles.preview, parsed === null && typed !== "" ? styles.problem : "muted")} aria-live="polite" data-testid="day-picker-preview">
          {preview ?? t("day-picker-hint")}
        </p>
      </div>
      <div className={styles.monthBar}>
        <Button
          className={classes("btn flat icon sm", styles.flip)}
          aria-label={t("day-picker-previous-month")}
          onPress={() => {
            setCursor(addMonths(cursor, -1));
          }}
        >
          <ChevronLeft className="ic s" aria-hidden="true" />
        </Button>
        <span id={`${id}-month`} className={styles.month} aria-live="polite">
          {monthTitle.format(asDate(month))}
        </span>
        <Button
          className={classes("btn flat icon sm", styles.flip)}
          aria-label={t("day-picker-next-month")}
          onPress={() => {
            setCursor(addMonths(cursor, 1));
          }}
        >
          <ChevronRight className="ic s" aria-hidden="true" />
        </Button>
      </div>
      <table ref={grid} role="grid" className={styles.grid} aria-labelledby={`${id}-month`} data-testid="day-picker-grid">
        <thead>
          <tr>
            {weeks[0]?.map((day) => (
              <th key={day} scope="col" abbr={weekdayLong.format(asDate(day))} className={styles.weekday}>
                {weekdayShort.format(asDate(day))}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {weeks.map((week) => (
            <tr key={week[0]}>
              {week.map((day) => {
                const selected = day === shown;
                const outside = day.slice(0, 7) !== cursor.slice(0, 7);
                return (
                  <td key={day} role="gridcell" aria-selected={selected}>
                    <button
                      type="button"
                      tabIndex={day === cursor ? 0 : -1}
                      data-day={day}
                      aria-label={long.format(asDate(day))}
                      aria-current={day === today ? "date" : undefined}
                      className={classes(styles.day, selected && styles.selected, day === today && styles.today, outside && styles.outside)}
                      onClick={() => {
                        onDone(day);
                      }}
                      onKeyDown={(event) => {
                        onGridKey(event, day);
                      }}
                    >
                      {dayNumber.format(asDate(day))}
                    </button>
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div className={styles.foot}>
        <Button
          className="btn flat sm"
          onPress={() => {
            onDone(today);
          }}
        >
          {t("day-picker-today")}
        </Button>
        <span className="grow" />
        <Button
          className="btn flat sm"
          isDisabled={value === null}
          onPress={() => {
            onDone(null);
          }}
        >
          {t("day-picker-clear")}
        </Button>
      </div>
    </div>
  );
}
