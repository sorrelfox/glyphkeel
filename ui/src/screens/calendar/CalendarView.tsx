// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { ChevronLeft, ChevronRight, Plus } from "lucide-react";
import { useEffect, useEffectEvent, useId, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { Button, Dialog, Popover, useLocale } from "react-aria-components";

import { keysOf } from "../../actions/actions";
import { classes } from "../../classes";
import { Kbd } from "../../components/Kbd";
import { QueryField, type QueryFieldHandle, type Reading } from "../../components/QueryField/QueryField";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { useCreateTask } from "../../data/board";
import { useCalendarTasks } from "../../data/calendar";
import { useCommand, useUndo } from "../../data/hooks";
import { useListNames, useParsedQuery, useQueryFields } from "../../data/list";
import { useToday } from "../../data/zone";
import { type ListedTask, type QueryProblem } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useNewTask, useScreenActions, useStatus, useTimerTask } from "../../shell/context";
import {
  addDays,
  asDate,
  daysBetween,
  firstDayOfWeek,
  hiddenPerDay,
  layoutWeek,
  shifted,
  spanOf,
  startOfWeek,
  step,
  tasksOn,
  weeksOf,
  type Bar,
  type CalendarMode,
  type Day,
} from "./model";
import styles from "./Calendar.module.css";

/** How long typing pauses before the query runs, as in the list. */
const RUN_DELAY_MS = 250;

/** Lanes of bars a day shows in the month at least; more when the weeks are tall enough. The rest are "+N more". */
const MONTH_LANES = 2;

/** How far the pointer moves before a press on a task becomes a drag. */
const DRAG_THRESHOLD = 4;

interface CalendarViewProps {
  project: string;
  openTask: string | undefined;
  onOpen: (key: string) => void;
}

interface Drag {
  task: ListedTask;
  /** The day under the pointer when the task was taken. */
  grabbed: Day;
  /** The day under the pointer now. */
  over: Day;
  x: number;
  y: number;
  moving: boolean;
}

/** The day cell under a point of the screen. */
function dayAt(x: number, y: number): Day | undefined {
  for (const element of document.elementsFromPoint(x, y)) {
    if (element instanceof HTMLElement && element.dataset.day !== undefined) {
      return element.dataset.day;
    }
  }
  return undefined;
}

/** The engine's message for a query mistake, in the person's language. */
function useProblemText(): (problem: QueryProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

/**
 * A project's tasks on a month or week grid, by start and due date. A task
 * with both dates runs as a bar across its days. Dragging a task, or Alt and
 * an arrow key, moves both of its dates; the change is one Undo step. A
 * day's "+", a double click on it or New task (N) adds a task due that day.
 */
export function CalendarView({ project, openTask, onOpen }: CalendarViewProps) {
  const t = useT();
  const { locale, direction } = useLocale();
  const today = useToday();
  const weekStart = firstDayOfWeek(locale);
  const problemText = useProblemText();
  const hintId = useId();

  const [mode, setMode] = useState<CalendarMode>("month");
  const [anchor, setAnchor] = useState<Day>(today);
  const [focusedDay, setFocusedDay] = useState<Day>(today);
  const [includeSubprojects, setIncludeSubprojects] = useState(false);

  // The query that runs: the text once the engine reads it without a mistake.
  const [text, setText] = useState("");
  const [running, setRunning] = useState("");
  const parsed = useParsedQuery(text);
  const reading = parsed.data && !parsed.isPlaceholderData ? parsed.data : undefined;
  const valid = reading?.valid === true;
  useEffect(() => {
    if (!valid) {
      return;
    }
    const timer = setTimeout(() => {
      setRunning(text);
    }, RUN_DELAY_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [text, valid]);
  const names = useListNames(project);
  const fields = useQueryFields();
  const field = useRef<QueryFieldHandle>(null);

  const weeks = weeksOf(anchor, mode, weekStart);
  const from = weeks[0]?.[0] ?? anchor;
  const to = weeks.at(-1)?.at(-1) ?? anchor;
  const data = useCalendarTasks({ project, includeSubprojects, query: running, from, to });
  const layouts = weeks.map((week) => layoutWeek(week, data.tasks));
  const dayIsShown = (day: Day) => day >= from && day <= to;

  useStatus({ summary: t("calendar-status", { count: data.tasks.length }) });
  useTimerTask(openTask ?? null);
  const goToday = () => {
    setAnchor(today);
    setFocusedDay(today);
  };
  const page = (by: 1 | -1) => {
    setAnchor(step(anchor, mode, by));
  };
  useScreenActions([
    { id: "view.previousPeriod", label: "action-previous-period", group: "navigation", keys: keysOf("view.previousPeriod"), run: () => { page(-1); } },
    { id: "view.nextPeriod", label: "action-next-period", group: "navigation", keys: keysOf("view.nextPeriod"), run: () => { page(1); } },
    { id: "view.today", label: "action-go-to-today", group: "navigation", keys: keysOf("view.today"), run: goToday },
  ]);

  // Formats, in the person's language; days are dates without a time, read in UTC.
  const format = (options: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" });
  const fullDate = format({ weekday: "long", day: "numeric", month: "long", year: "numeric" });
  const shortDate = format({ day: "numeric", month: "short" });
  const dayNumber = format({ day: "numeric" });
  const weekdayShort = format({ weekday: "short" });
  const weekdayLong = format({ weekday: "long" });
  const title =
    mode === "month"
      ? format({ month: "long", year: "numeric" }).format(asDate(anchor))
      : format({ day: "numeric", month: "short", year: "numeric" }).formatRange(asDate(from), asDate(to));

  // A move is one change: Undo in its note, or the app's Undo, takes it back.
  const update = useCommand("task.update");
  const toasts = useToasts();
  const { undo } = useUndo();
  const reschedule = (task: ListedTask, days: number) => {
    const changes = shifted(task, days);
    const span = spanOf(task);
    if (!changes || !span) {
      return;
    }
    const date = fullDate.format(asDate(addDays(span.from, days)));
    update.mutate(
      { key: task.key, ...changes, expected_version: task.version },
      {
        onSuccess: () => {
          toasts.show({ tone: "info", text: t("calendar-moved", { key: task.key, date }), action: { label: t("toast-undo"), run: undo } });
        },
      },
    );
  };

  // ---------- quick add ----------

  /** The day a new task is being added on, with the cell the form opens at. */
  const [adding, setAdding] = useState<Day | null>(null);
  const addAnchor = useRef<HTMLElement | null>(null);
  const createTask = useCreateTask();
  const startAdding = (day: Day) => {
    // The form opens under the day's head, where its "+" is.
    addAnchor.current = gridRef.current?.querySelector<HTMLElement>(`[data-day="${day}"] [data-add]`) ?? null;
    if (addAnchor.current) {
      setFocusedDay(day);
      setAdding(day);
    }
  };
  // New task (N) adds on the focused day, as the "+" of a day and a double click do.
  useNewTask(() => {
    startAdding(dayIsShown(focusedDay) ? focusedDay : from);
  });
  const stopAdding = (day: Day) => {
    setAdding(null);
    requestAnimationFrame(() => {
      gridRef.current?.querySelector<HTMLElement>(`[data-day="${day}"]`)?.focus();
    });
  };
  /** Creates a task due on `day`, through the same command as everywhere else. */
  const add = (day: Day, title: string) => {
    stopAdding(day);
    createTask.mutate(
      { project, title, due_date: day },
      {
        onSuccess: (change) => {
          toasts.show({ tone: "info", text: t("calendar-added", { key: change.task.key, date: fullDate.format(asDate(day)) }), action: { label: t("toast-undo"), run: undo } });
        },
      },
    );
  };

  // ---------- focus ----------

  const gridRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLSpanElement>(null);
  // The month's weeks share the grid's height: as many lanes as fit, a line kept for "+N more".
  const [monthLanes, setMonthLanes] = useState(MONTH_LANES);
  const weekCount = weeks.length;
  useEffect(() => {
    const grid = gridRef.current;
    const probe = probeRef.current;
    if (!grid || !probe || mode !== "month") {
      return;
    }
    const measure = () => {
      const head = grid.querySelector<HTMLElement>("[role=row]")?.offsetHeight ?? 0;
      const number = grid.querySelector<HTMLElement>("[data-day-number]");
      const slot = probe.offsetHeight;
      if (slot <= 0 || !number) {
        return;
      }
      const dayHead = number.offsetHeight + 8;
      const perWeek = (grid.clientHeight - head) / weekCount;
      setMonthLanes(Math.max(MONTH_LANES, Math.floor((perWeek - dayHead - 6) / slot) - 1));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => {
      observer.disconnect();
    };
  }, [mode, weekCount]);
  /** The task to focus once it is drawn again (after a move by keys). */
  const [pendingFocus, setPendingFocus] = useState<{ key: string; day: Day } | null>(null);
  useEffect(() => {
    if (pendingFocus === null) {
      return;
    }
    const bar = gridRef.current?.querySelector<HTMLElement>(`[data-task-bar="${CSS.escape(pendingFocus.key)}"][data-days~="${pendingFocus.day}"]`);
    if (bar) {
      bar.focus();
      setPendingFocus(null);
    }
  }, [pendingFocus, data.tasks]);

  const focusDay = (day: Day) => {
    setFocusedDay(day);
    if (!dayIsShown(day) || (mode === "month" && day.slice(0, 7) !== anchor.slice(0, 7))) {
      setAnchor(day);
    }
    requestAnimationFrame(() => {
      gridRef.current?.querySelector<HTMLElement>(`[data-day="${day}"]`)?.focus();
    });
  };

  /** The tasks of a day, in the order of its lanes, and its "+N more" button. */
  const dayItems = (day: Day): HTMLElement[] => {
    const cell = gridRef.current?.querySelector<HTMLElement>(`[data-day="${day}"]`);
    const week = cell?.closest<HTMLElement>("[data-week]");
    if (!week) {
      return [];
    }
    const bars = [...week.querySelectorAll<HTMLElement>(`[data-task-bar][data-days~="${day}"]`)].sort(
      (a, b) => Number(a.dataset.lane) - Number(b.dataset.lane),
    );
    const more = cell?.querySelector<HTMLElement>("[data-more]");
    return more ? [...bars, more] : bars;
  };

  const forward = direction === "rtl" ? -1 : 1;

  const onCellKey = (event: KeyboardEvent<HTMLElement>, day: Day) => {
    const moves: Record<string, () => Day> = {
      ArrowLeft: () => addDays(day, -forward),
      ArrowRight: () => addDays(day, forward),
      ArrowUp: () => addDays(day, -7),
      ArrowDown: () => addDays(day, 7),
      Home: () => startOfWeek(day, weekStart),
      End: () => addDays(startOfWeek(day, weekStart), 6),
      PageUp: () => step(day, mode, -1),
      PageDown: () => step(day, mode, 1),
    };
    const move = moves[event.key];
    if (move && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      focusDay(move());
    } else if (event.key === "Enter" && !event.altKey && !event.ctrlKey) {
      event.preventDefault();
      dayItems(day)[0]?.focus();
    }
  };

  const onBarKey = (event: KeyboardEvent<HTMLElement>, task: ListedTask, day: Day) => {
    if (event.altKey && !event.ctrlKey && !event.metaKey) {
      const by: Record<string, number> = { ArrowLeft: -forward, ArrowRight: forward, ArrowUp: -7, ArrowDown: 7 };
      const days = by[event.key];
      if (days !== undefined) {
        event.preventDefault();
        event.stopPropagation();
        reschedule(task, days);
        const target = addDays(day, days);
        setFocusedDay(target);
        if (!dayIsShown(target)) {
          setAnchor(target);
        }
        setPendingFocus({ key: task.key, day: target });
      }
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      event.stopPropagation();
      onOpen(task.key);
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      event.stopPropagation();
      const items = dayItems(day);
      const at = items.indexOf(event.currentTarget);
      items[at + (event.key === "ArrowDown" ? 1 : -1)]?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      focusDay(day);
    } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      event.stopPropagation();
      focusDay(addDays(day, (event.key === "ArrowRight" ? 1 : -1) * forward));
    }
  };

  // ---------- dragging ----------

  const [drag, setDrag] = useState<Drag | null>(null);
  /** The drag as the pointer's listeners see it; changed in handlers only. */
  const dragRef = useRef<Drag | null>(null);
  const changeDrag = (next: Drag | null) => {
    dragRef.current = next;
    setDrag(next);
  };
  /** A drag just ended: the click that follows it doesn't open the task. */
  const dropped = useRef(false);
  const drop = useEffectEvent((ended: Drag) => {
    const days = daysBetween(ended.grabbed, ended.over);
    if (days !== 0) {
      reschedule(ended.task, days);
    }
  });

  const dragging = drag !== null;
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const move = (event: PointerEvent) => {
      const now = dragRef.current;
      if (!now) {
        return;
      }
      const moving = now.moving || Math.hypot(event.clientX - now.x, event.clientY - now.y) > DRAG_THRESHOLD;
      const over = dayAt(event.clientX, event.clientY) ?? now.over;
      if (moving !== now.moving || over !== now.over) {
        dragRef.current = { ...now, moving, over };
        setDrag(dragRef.current);
      }
    };
    const up = () => {
      const now = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      if (now?.moving) {
        dropped.current = true;
        requestAnimationFrame(() => {
          dropped.current = false;
        });
        drop(now);
      }
    };
    const cancel = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        dragRef.current = null;
        setDrag(null);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    window.addEventListener("keydown", cancel, true);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      window.removeEventListener("keydown", cancel, true);
    };
  }, [dragging]);

  const startDrag = (event: ReactPointerEvent<HTMLElement>, task: ListedTask) => {
    if (event.button !== 0) {
      return;
    }
    const grabbed = dayAt(event.clientX, event.clientY);
    if (grabbed !== undefined) {
      changeDrag({ task, grabbed, over: grabbed, x: event.clientX, y: event.clientY, moving: false });
    }
  };

  // Where the dragged task would land: its days, moved.
  const dropDays = new Set<Day>();
  if (drag?.moving) {
    const span = spanOf(drag.task);
    const days = daysBetween(drag.grabbed, drag.over);
    if (span) {
      for (let day = addDays(span.from, days); day <= addDays(span.to, days); day = addDays(day, 1)) {
        dropDays.add(day);
      }
    }
  }

  // ---------- drawing ----------

  const previous = mode === "month" ? t("calendar-previous-month") : t("calendar-previous-week");
  const next = mode === "month" ? t("calendar-next-month") : t("calendar-next-week");
  const filterChord = keysOf("view.filter")[0];
  const queryReading: Reading | undefined = reading ? { text, tokens: reading.tokens, error: reading.error } : undefined;
  const problem = reading?.error;
  const tabDay = dayIsShown(focusedDay) ? focusedDay : from;

  const taskLabel = (task: ListedTask) => {
    const span = spanOf(task);
    if (!span) {
      return task.key;
    }
    return span.from === span.to
      ? t("calendar-task-day", { key: task.key, title: task.title, date: fullDate.format(asDate(span.from)) })
      : t("calendar-task-days", { key: task.key, title: task.title, from: fullDate.format(asDate(span.from)), to: fullDate.format(asDate(span.to)) });
  };

  const renderBar = (bar: Bar<ListedTask>, week: readonly Day[]) => {
    const { task } = bar;
    const days = week.slice(bar.column, bar.column + bar.length);
    const hue = names.hues.get(task.status.id) ?? "grey";
    const closed = task.closed_at !== undefined;
    return (
      <button
        key={task.key}
        type="button"
        tabIndex={-1}
        className={classes(styles.bar, closed && styles.closed, openTask === task.key && styles.open)}
        style={{ "--len": bar.length, "--lane": bar.lane } as CSSProperties}
        data-task-bar={task.key}
        data-days={days.join(" ")}
        data-lane={bar.lane}
        data-continues-before={bar.continuesBefore ? "" : undefined}
        data-continues-after={bar.continuesAfter ? "" : undefined}
        data-dragging={drag?.moving === true && drag.task.key === task.key ? "" : undefined}
        aria-label={taskLabel(task)}
        onPointerDown={(event) => {
          startDrag(event, task);
        }}
        onClick={() => {
          if (!dropped.current) {
            onOpen(task.key);
          }
        }}
        onKeyDown={(event) => {
          onBarKey(event, task, focusedDayIn(days));
        }}
        onFocus={() => {
          if (!days.includes(focusedDay) && days[0] !== undefined) {
            setFocusedDay(days[0]);
          }
        }}
      >
        <span className={`dot hue-${hue}`} aria-hidden="true" />
        <span className={styles.barKey}>{task.key}</span>
        <span className={styles.barTitle}>{task.title}</span>
      </button>
    );
  };

  /** The day a bar acts for: the focused day when the bar covers it, else its first. */
  const focusedDayIn = (days: readonly Day[]): Day => (days.includes(focusedDay) ? focusedDay : (days[0] ?? focusedDay));

  return (
    <div className={styles.screen} data-calendar={project} data-mode={mode}>
      <div className={classes("toolbar", styles.toolbar)}>
        <div className={styles.toolbarItem}>
          <Button className="btn" onPress={goToday}>
            {t("calendar-today")}
          </Button>
          <Button className={classes("btn flat icon", styles.flip)} aria-label={previous} onPress={() => {
              page(-1);
            }}>
            <ChevronLeft className="ic" aria-hidden="true" />
          </Button>
          <Button className={classes("btn flat icon", styles.flip)} aria-label={next} onPress={() => {
              page(1);
            }}>
            <ChevronRight className="ic" aria-hidden="true" />
          </Button>
          <h2 className={styles.title} aria-live="polite" data-testid="calendar-title">
            {title}
          </h2>
        </div>
        <div className={styles.toolbarItem}>
          <Segmented
            label={t("calendar-mode")}
            segments={[
              { id: "month", label: t("calendar-mode-month") },
              { id: "week", label: t("calendar-mode-week") },
            ]}
            value={mode}
            onChange={(next) => {
              setMode(next);
              setAnchor(dayIsShown(focusedDay) ? focusedDay : anchor);
            }}
          />
        </div>
        <QueryField
          ref={field}
          value={text}
          onChange={setText}
          reading={queryReading}
          problem={problem ? problemText(problem) : undefined}
          fields={fields}
          names={names}
          label={t("calendar-query-label")}
          placeholder={t("calendar-query-placeholder")}
          onSubmit={() => {
            focusDay(tabDay);
          }}
          onEscape={() => {
            focusDay(tabDay);
          }}
          shortcut={filterChord && <Kbd chord={filterChord} />}
        />
        <div className={styles.toolbarItem}>
          <Switch isSelected={includeSubprojects} onChange={setIncludeSubprojects}>
            {t("calendar-include-subprojects")}
          </Switch>
        </div>
      </div>
      {data.error !== null && (
        <p className={styles.note} role="alert">
          {t("calendar-error", { message: data.error.error.message })}
        </p>
      )}
      {data.total > data.tasks.length && (
        <p className={styles.note} role="status">
          {t("calendar-truncated", { shown: data.tasks.length, total: data.total })}
        </p>
      )}
      <p id={hintId} className={styles.hidden}>
        {t("calendar-keys-hint")}
      </p>
      <span ref={probeRef} className={styles.probe} aria-hidden="true" />
      <div
        ref={gridRef}
        role="grid"
        className={styles.grid}
        aria-label={t("calendar-grid-label", { range: title })}
        aria-describedby={hintId}
        aria-busy={data.pending ? "true" : undefined}
        data-dragging={drag?.moving === true ? "" : undefined}
      >
        <div role="row" className={styles.head}>
          {(weeks[0] ?? []).map((day) => (
            <div key={day} role="columnheader" className={styles.weekday} aria-label={weekdayLong.format(asDate(day))}>
              {weekdayShort.format(asDate(day))}
            </div>
          ))}
        </div>
        {weeks.map((week, index) => {
          const bars = layouts[index] ?? [];
          const lanes = bars.reduce((most, bar) => Math.max(most, bar.lane + 1), 0);
          const limit = mode === "month" ? monthLanes : Number.POSITIVE_INFINITY;
          const hidden = hiddenPerDay(bars, limit);
          const shownLanes = Math.min(lanes, limit);
          return (
            <div
              key={week[0]}
              role="row"
              className={styles.week}
              data-week={index}
              style={{ "--lanes": mode === "month" ? monthLanes : Math.max(lanes, 1), "--more": hidden.some((count) => count > 0) ? 1 : 0 } as CSSProperties}
            >
              {week.map((day, column) => {
                const count = tasksOn(bars, week, day).length;
                const isToday = day === today;
                const date = fullDate.format(asDate(day));
                const more = hidden[column] ?? 0;
                return (
                  <div
                    key={day}
                    role="gridcell"
                    tabIndex={day === tabDay ? 0 : -1}
                    className={styles.day}
                    data-day={day}
                    data-outside={mode === "month" && day.slice(0, 7) !== anchor.slice(0, 7) ? "" : undefined}
                    data-drop={dropDays.has(day) ? "" : undefined}
                    aria-current={isToday ? "date" : undefined}
                    aria-selected={day === focusedDay}
                    aria-label={isToday ? t("calendar-day-today", { date: t("calendar-day-label", { date, count }) }) : t("calendar-day-label", { date, count })}
                    onFocus={(event) => {
                      if (event.target === event.currentTarget) {
                        setFocusedDay(day);
                      }
                    }}
                    onClick={(event) => {
                      if (!(event.target instanceof Element) || event.target.closest("[data-task-bar], [data-more]") === null) {
                        setFocusedDay(day);
                        event.currentTarget.focus();
                      }
                    }}
                    onDoubleClick={(event) => {
                      if (!(event.target instanceof Element) || event.target.closest("[data-task-bar], [data-more], [data-add]") === null) {
                        startAdding(day);
                      }
                    }}
                    onKeyDown={(event) => {
                      if (event.target === event.currentTarget) {
                        onCellKey(event, day);
                      }
                    }}
                  >
                    <div className={styles.dayHead}>
                      <span className={classes(styles.dayNumber, isToday && styles.today)} aria-hidden="true" data-day-number="">
                        {day.endsWith("-01") || (column === 0 && index === 0 && mode === "week") ? shortDate.format(asDate(day)) : dayNumber.format(asDate(day))}
                      </span>
                      <button
                        type="button"
                        tabIndex={-1}
                        className={styles.add}
                        data-add={day}
                        aria-label={t("calendar-add", { date })}
                        title={t("calendar-add", { date })}
                        onClick={() => {
                          startAdding(day);
                        }}
                      >
                        <Plus className="ic s" aria-hidden="true" />
                      </button>
                    </div>
                    <div className={styles.lanes}>{bars.filter((bar) => bar.column === column && bar.lane < shownLanes).map((bar) => renderBar(bar, week))}</div>
                    {more > 0 && (
                      <button
                        type="button"
                        tabIndex={-1}
                        className={styles.more}
                        data-more={more}
                        aria-label={t("calendar-more-label", { count: more, date })}
                        onClick={() => {
                          setMode("week");
                          setAnchor(day);
                          focusDay(day);
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "ArrowUp") {
                            event.preventDefault();
                            const items = dayItems(day);
                            items[items.length - 2]?.focus();
                          } else if (event.key === "Escape") {
                            event.preventDefault();
                            focusDay(day);
                          }
                        }}
                      >
                        {t("calendar-more", { count: more })}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
      {adding !== null && (
        <Popover
          className="pop"
          triggerRef={addAnchor}
          placement="bottom end"
          offset={2}
          isOpen
          onOpenChange={(isOpen) => {
            if (!isOpen) {
              stopAdding(adding);
            }
          }}
        >
          <Dialog className={classes(styles.addDialog)} aria-label={t("calendar-add-title", { date: fullDate.format(asDate(adding)) })}>
            <QuickAdd
              label={t("calendar-add-title", { date: fullDate.format(asDate(adding)) })}
              onAdd={(title) => {
                add(adding, title);
              }}
              onCancel={() => {
                stopAdding(adding);
              }}
            />
          </Dialog>
        </Popover>
      )}
    </div>
  );
}

/** The quick add form of a day: a title, Enter adds and Escape cancels. */
function QuickAdd({ label, onAdd, onCancel }: { label: string; onAdd: (title: string) => void; onCancel: () => void }) {
  const t = useT();
  const [title, setTitle] = useState("");
  return (
    <form
      className={styles.addForm}
      onSubmit={(event) => {
        event.preventDefault();
        if (title.trim() !== "") {
          onAdd(title.trim());
        }
      }}
    >
      <span className="field">
        <input
          autoFocus
          aria-label={label}
          placeholder={t("calendar-add-placeholder")}
          value={title}
          onChange={(event) => {
            setTitle(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              onCancel();
            }
          }}
        />
      </span>
      <Button type="submit" className="btn primary" isDisabled={title.trim() === ""}>
        {t("calendar-add-confirm")}
      </Button>
    </form>
  );
}
