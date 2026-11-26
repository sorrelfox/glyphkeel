// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart itself: one scroll container holding a table that sticks
// to the start edge and a timeline beside it. The timeline's shapes are SVG
// drawn left to right and mirrored for right-to-left languages; its text is
// HTML placed with logical properties, so it reads the right way round.
// Only the rows and the stretch of timeline on screen are drawn.

import { useLocalization } from "@fluent/react";
import { ChevronDown, ChevronRight, TriangleAlert } from "lucide-react";
import {
  memo,
  useCallback,
  useEffect,
  useEffectEvent,
  useId,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type Ref,
} from "react";
import { useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { type Schedule, type ScheduleDependency, type ScheduleProblem, type ScheduledTask, type UnscheduledTask } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { addDays, asDate, daysBetween, firstDayOfWeek, weekday, type Day } from "../calendar/model";
import { COLUMN_MAX, COLUMN_MIN, SIZED, TABLE_MAX, TABLE_MIN, type Column } from "./layout";
import {
  ROW,
  buildRows,
  columnWindow,
  datesOf,
  draggedDates,
  elbow,
  endsLater,
  geometry,
  isWarning,
  linkBetween,
  problemsByTask,
  rowWindow,
  sameDates,
  visibleDependencies,
  type Dates,
  type DragMode,
  type Geometry,
  type Row,
} from "./model";
import { PX_PER_DAY, UNITS, centreOf, dayAt, dayX, headerCells, scrollFor, visibleDays, weekendDays, weekendsBetween, zoomStep, type HeaderCell, type Zoom } from "./scale";
import styles from "./Gantt.module.css";

/** The header's height: two rows. */
const HEAD = 48;
/** How far the pointer moves before a press becomes a drag. */
const DRAG_THRESHOLD = 4;
/** Rows Page Up and Page Down move by. */
const PAGE_ROWS = 15;
/** The table's width when the person hasn't set one. */
export const DEFAULT_TABLE = 440;

/** A task whose dates the chart can change. */
export type Movable = Pick<ScheduledTask, "key" | "version" | "start_date" | "due_date"> & { dependents?: number };

export interface GanttHandle {
  /** Scrolls so that today is in view. */
  goToday: () => void;
  /** Scrolls a screen's width earlier (-1) or later (1). */
  page: (by: -1 | 1) => void;
}

export interface GanttProps {
  schedule: Schedule;
  origin: Day;
  days: number;
  zoom: Zoom;
  table: number;
  widths: Partial<Record<Column, number>>;
  moveDependents: boolean;
  openTask: string | undefined;
  /** Changes a task's dates; true once the change is made. */
  onDates: (task: Movable, next: Dates) => Promise<boolean>;
  onLink: (from: string, to: string) => Promise<boolean>;
  onUnlink: (dependency: ScheduleDependency) => Promise<boolean>;
  onOpen: (key: string) => void;
  onZoom: (zoom: Zoom) => void;
  onTable: (width: number) => void;
  onWidth: (column: Column, width: number) => void;
  onRange: (range: { from: Day; to: Day }) => void;
  ref?: Ref<GanttHandle>;
}

interface Drag {
  /** When it began, which tells one drag from the next. */
  id: number;
  key: string;
  mode: DragMode | "link" | "schedule";
  clientX: number;
  clientY: number;
  moving: boolean;
  days: number;
  /** The pointer in the timeline: logical pixels from its start, and from its top. */
  x: number;
  y: number;
  /** For a link, the task under the pointer. */
  target?: string | undefined;
  /** For a task from the Unscheduled group: the day under the pointer, when over the timeline. */
  day?: Day | undefined;
}

interface Editing {
  key: string;
  field: "start" | "due";
}

/** Day formats, in the person's language; days are dates without a time, read in UTC. */
function useFormats(locale: string, today: Day) {
  return useMemo(() => {
    const format = (options: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" });
    const short = format({ day: "numeric", month: "short" });
    const shortYear = format({ day: "numeric", month: "short", year: "numeric" });
    const thisYear = today.slice(0, 4);
    return {
      cell: (day: Day) => (day.startsWith(thisYear) ? short : shortYear).format(asDate(day)),
      full: format({ day: "numeric", month: "long", year: "numeric" }),
      day: format({ day: "numeric" }),
      week: short,
      month: format({ month: "short" }),
      monthYear: format({ month: "long", year: "numeric" }),
      year: format({ year: "numeric" }),
      percent: new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 }),
    };
  }, [locale, today]);
}

type Formats = ReturnType<typeof useFormats>;

/** Puts the focus on the chart that holds `element`, where its keys work. */
function focusChart(element: Element) {
  element.closest<HTMLElement>("[role=treegrid]")?.focus({ preventScroll: true });
}

/** The engine's message for a schedule problem, in the person's language. */
function useProblemText(): (problem: ScheduleProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

export const Gantt = memo(function Gantt({
  schedule,
  origin,
  days,
  zoom,
  table,
  widths: savedWidths,
  moveDependents,
  openTask,
  onDates,
  onLink,
  onUnlink,
  onOpen,
  onZoom,
  onTable,
  onWidth,
  onRange,
  ref,
}: GanttProps) {
  const t = useT();
  const { locale, direction } = useLocale();
  const rtl = direction === "rtl";
  const today = schedule.today;
  const ppd = PX_PER_DAY[zoom];
  const width = Math.round(days * ppd);
  const fmt = useFormats(locale, today);
  const weekStart = firstDayOfWeek(locale);
  const weekend = useMemo(() => weekendDays(locale), [locale]);
  const problemText = useProblemText();
  const ids = useId();
  const rowId = (key: string) => `${ids}-row-${key}`;

  const scroller = useRef<HTMLDivElement>(null);
  const timeline = useRef<HTMLDivElement>(null);

  // ---------- table sizes: the person's, or live while a handle is dragged ----------

  const [liveTable, setLiveTable] = useState<number | null>(null);
  const [liveWidths, setLiveWidths] = useState<Partial<Record<Column, number>>>({});
  const tableW = liveTable ?? table;
  const widths = { ...savedWidths, ...liveWidths };

  // ---------- rows ----------

  const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set());
  const { rows, index } = useMemo(() => buildRows(schedule.tasks, schedule.unscheduled, folded), [schedule.tasks, schedule.unscheduled, folded]);
  const problems = useMemo(() => problemsByTask(schedule.problems), [schedule.problems]);
  const byKey = useMemo(() => new Map(schedule.tasks.map((task) => [task.key, task])), [schedule.tasks]);
  const toggle = useCallback((key: string) => {
    setFolded((now) => {
      const next = new Set(now);
      if (!next.delete(key)) {
        next.add(key);
      }
      return next;
    });
  }, []);

  // ---------- what is on screen ----------

  const [view, setView] = useState({ r0: 0, r1: 40, x0: 0, x1: 2048 });
  /** The day in the middle of the timeline, kept so that a zoom or a new origin keeps it there. */
  const centre = useRef<{ day: Day; fraction: number } | null>(null);
  const timelineWidth = (el: HTMLElement) => Math.max(0, el.clientWidth - tableW);
  const scrollX = (el: HTMLElement) => Math.abs(el.scrollLeft);
  const setScrollX = (x: number) => {
    const el = scroller.current;
    if (el) {
      el.scrollLeft = rtl ? -x : x;
    }
  };

  const measure = useEffectEvent(() => {
    const el = scroller.current;
    if (!el) {
      return;
    }
    const [r0, r1] = rowWindow(el.scrollTop, el.clientHeight, rows.length);
    const [x0, x1] = columnWindow(scrollX(el), el.clientWidth);
    setView((now) => (now.r0 === r0 && now.r1 === r1 && now.x0 === x0 && now.x1 === x1 ? now : { r0, r1, x0, x1 }));
    const visible = timelineWidth(el);
    const middle = centreOf(scrollX(el), ppd, visible);
    centre.current = { day: addDays(origin, Math.floor(middle)), fraction: middle - Math.floor(middle) };
    onRange(visibleDays(origin, scrollX(el), ppd, visible, days));
  });

  // Keep the day in the middle when the zoom or the origin changes; start at today.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) {
      return;
    }
    const visible = timelineWidth(el);
    const kept = centre.current;
    const target = kept ? daysBetween(origin, kept.day) + kept.fraction : daysBetween(origin, today) + visible / ppd / 6;
    el.scrollLeft = (rtl ? -1 : 1) * scrollFor(target, ppd, visible);
    measure();
    // The table's width and the direction don't move the centre.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [origin, ppd]);

  useLayoutEffect(() => {
    measure();
  }, [rows.length, tableW]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) {
      return;
    }
    const observer = new ResizeObserver(() => {
      measure();
    });
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, []);

  useImperativeHandle(ref, () => ({
    goToday: () => {
      const el = scroller.current;
      if (el) {
        setScrollX(scrollFor(daysBetween(origin, today) + 0.5, ppd, timelineWidth(el)));
      }
    },
    page: (by) => {
      const el = scroller.current;
      if (el) {
        setScrollX(Math.max(0, scrollX(el) + by * timelineWidth(el) * 0.8));
      }
    },
  }));

  // ---------- focus and announcements ----------

  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [linkFrom, setLinkFrom] = useState<string | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [live, setLive] = useState("");
  /** A task just scheduled from the Unscheduled group, to show once it has its row among the dated. */
  const reveal = useRef<string | null>(null);
  const focusIndex = focusKey === null ? undefined : index.get(focusKey);

  const focusGrid = () => {
    scroller.current?.focus({ preventScroll: true });
  };

  const showRow = (at: number) => {
    const el = scroller.current;
    const row = rows[at];
    if (!el || !row) {
      return;
    }
    const top = at * ROW;
    if (top < el.scrollTop) {
      el.scrollTop = top;
    } else if (top + ROW + HEAD > el.scrollTop + el.clientHeight) {
      el.scrollTop = top + ROW + HEAD - el.clientHeight;
    }
    if (row.kind === "task") {
      const geo = geometry(row.task, origin, ppd);
      const visible = timelineWidth(el);
      if (geo && (geo.x1 < scrollX(el) || geo.x1 > scrollX(el) + visible - 40)) {
        setScrollX(Math.max(0, geo.x1 - visible / 3));
      }
    }
  };

  const focusRow = (at: number, by: 1 | -1 = 1) => {
    let i = Math.max(0, Math.min(rows.length - 1, at));
    while (rows[i]?.kind === "group") {
      i += by;
    }
    if (i < 0 || i >= rows.length) {
      i = Math.max(0, Math.min(rows.length - 1, at)) - by;
    }
    const row = rows[i];
    if (!row || row.kind === "group") {
      return;
    }
    setFocusKey(row.task.key);
    showRow(i);
  };

  // ---------- changes ----------

  const announceDates = (key: string, next: Dates) => {
    if (next.start !== undefined && next.due !== undefined && next.start !== next.due) {
      setLive(t("gantt-moved-dates", { key, from: fmt.full.format(asDate(next.start)), to: fmt.full.format(asDate(next.due)) }));
    } else {
      const day = next.due ?? next.start;
      if (day !== undefined) {
        setLive(t("gantt-moved-day", { key, date: fmt.full.format(asDate(day)) }));
      }
    }
  };

  /** Asks for new dates; `then` runs once they are made. */
  const change = (task: Movable, next: Dates, then: () => void) => {
    const before: Dates = { start: task.start_date, due: task.due_date };
    if (!sameDates(before, next)) {
      void onDates(task, next).then((done) => {
        if (done) {
          then();
        }
      });
    }
  };

  const unlink = (dependency: ScheduleDependency) => {
    void onUnlink(dependency).then((done) => {
      if (done) {
        setLive(t("gantt-link-removed", { from: dependency.from, to: dependency.to }));
      }
    });
  };

  const link = (from: string, to: string) => {
    if (from === to) {
      return;
    }
    const existing = linkBetween(schedule.dependencies, from, to);
    if (existing) {
      unlink(existing);
    } else {
      void onLink(from, to).then((done) => {
        if (done) {
          setLive(t("gantt-link-made", { from, to }));
        }
      });
    }
  };

  const zoomBy = (by: -1 | 1) => {
    const next = zoomStep(zoom, by);
    if (next) {
      onZoom(next);
      setLive(t("gantt-zoomed", { zoom: next }));
    }
  };

  // ---------- keyboard: a treegrid whose focus stays on the scroll container ----------

  const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) {
      return;
    }
    const at = focusIndex ?? -1;
    const row = rows[at];
    const task = row?.kind === "task" ? row.task : undefined;
    const movable: Movable | undefined = row && row.kind !== "group" ? row.task : undefined;
    const forward = (event.key === "ArrowRight") !== rtl ? 1 : -1;
    const ctrl = event.ctrlKey || event.metaKey;
    const plain = !ctrl && !event.altKey;
    const horizontal = event.key === "ArrowLeft" || event.key === "ArrowRight";
    if (event.key === "ArrowDown" && plain) {
      focusRow(at + 1, 1);
    } else if (event.key === "ArrowUp" && plain) {
      focusRow(Math.max(0, at - 1), -1);
    } else if (event.key === "PageDown") {
      focusRow(at + PAGE_ROWS, 1);
    } else if (event.key === "PageUp") {
      focusRow(Math.max(0, at - PAGE_ROWS), -1);
    } else if (event.key === "Home" && ctrl) {
      focusRow(0, 1);
    } else if (event.key === "End" && ctrl) {
      focusRow(rows.length - 1, -1);
    } else if (horizontal && event.altKey && !ctrl && task && task.shape !== "summary") {
      const next = draggedDates(datesOf(task), event.shiftKey ? "end" : "move", forward);
      change(task, next, () => {
        announceDates(task.key, next);
      });
    } else if (horizontal && plain && row?.kind === "task" && row.hasKids) {
      if ((forward > 0) !== row.open) {
        toggle(row.task.key);
      }
    } else if (event.key === "F2" && plain && movable) {
      setEditing({ key: movable.key, field: event.shiftKey ? "due" : "start" });
    } else if ((event.key === "l" || event.key === "L") && plain && !event.shiftKey && task) {
      setLinkFrom(task.key);
      setLive(t("gantt-link-start", { key: task.key }));
    } else if (event.key === "Enter" && plain && linkFrom !== null && task) {
      link(linkFrom, task.key);
      setLinkFrom(null);
    } else if (event.key === "Enter" && plain && movable) {
      onOpen(movable.key);
    } else if (event.key === "Escape" && linkFrom !== null) {
      setLinkFrom(null);
      setLive(t("gantt-link-cancelled"));
    } else if ((event.key === "+" || event.key === "=") && !ctrl) {
      zoomBy(-1);
    } else if (event.key === "-" && !ctrl) {
      zoomBy(1);
    } else {
      return;
    }
    event.preventDefault();
  };

  // ---------- pointer: moving, resizing, linking, scheduling ----------

  const [drag, setDrag] = useState<Drag | null>(null);
  /**
   * The drag as the pointer last left it; the pointer's handlers keep it
   * between renders, so that moves that change nothing drawn don't render.
   */
  const dragRef = useRef<Drag | null>(null);
  const changeDrag = (next: Drag | null) => {
    dragRef.current = next;
    setDrag(next);
  };
  /** The drag now: the pointer's latest, unless a new drag began since. */
  const currentDrag = () => (dragRef.current !== null && dragRef.current.id === drag?.id ? dragRef.current : drag);

  const onBarDown = useCallback((event: ReactPointerEvent, key: string, mode: Drag["mode"]) => {
    if (event.button !== 0) {
      return;
    }
    event.stopPropagation();
    event.preventDefault();
    setFocusKey(key);
    focusChart(event.currentTarget);
    setDrag({ id: event.timeStamp, key, mode, clientX: event.clientX, clientY: event.clientY, moving: false, days: 0, x: 0, y: 0 });
  }, []);

  /** The pointer in the timeline, logical; and whether it is over the timeline rather than the table. */
  const pointerAt = (clientX: number, clientY: number) => {
    const box = timeline.current?.getBoundingClientRect();
    const frame = scroller.current?.getBoundingClientRect();
    if (!box || !frame) {
      return { x: 0, y: 0, over: false };
    }
    // Physical positions from the pointer, turned logical here.
    const x = rtl ? box.right - clientX : clientX - box.left;
    const intoFrame = rtl ? frame.right - clientX : clientX - frame.left;
    const over = intoFrame > tableW && intoFrame < frame.width && clientY > frame.top + HEAD && clientY < frame.bottom;
    return { x, y: clientY - box.top, over };
  };

  const onPointerMove = useEffectEvent((event: PointerEvent) => {
    const now = currentDrag();
    if (!now) {
      return;
    }
    const moving = now.moving || Math.hypot(event.clientX - now.clientX, event.clientY - now.clientY) > DRAG_THRESHOLD;
    const at = pointerAt(event.clientX, event.clientY);
    const days = Math.round(((rtl ? now.clientX - event.clientX : event.clientX - now.clientX) / ppd));
    let target: string | undefined;
    if (now.mode === "link") {
      const row = rows[Math.floor(at.y / ROW)];
      if (row?.kind === "task" && row.task.key !== now.key) {
        const geo = geometry(row.task, origin, ppd);
        if (geo && at.x >= geo.x1 - 12 && at.x <= geo.x2 + 12) {
          target = row.task.key;
        }
      }
    }
    const day = now.mode === "schedule" && at.over ? dayAt(origin, at.x, ppd) : undefined;
    const next = { ...now, moving, days, x: at.x, y: at.y, target, day };
    const visible = now.mode === "link" || now.mode === "schedule";
    if (moving !== now.moving || days !== now.days || target !== now.target || day !== now.day || (visible && (at.x !== now.x || at.y !== now.y))) {
      changeDrag(next);
    } else {
      dragRef.current = next;
    }
  });

  const onPointerUp = useEffectEvent(() => {
    const now = currentDrag();
    changeDrag(null);
    if (!now?.moving) {
      return;
    }
    if (now.mode === "link") {
      if (now.target !== undefined) {
        link(now.key, now.target);
      }
      return;
    }
    if (now.mode === "schedule") {
      const task = schedule.unscheduled.find((candidate) => candidate.key === now.key);
      if (task && now.day !== undefined) {
        const day = now.day;
        change(task, task.milestone ? { start: undefined, due: day } : { start: day, due: undefined }, () => {
          setLive(t("gantt-scheduled", { key: task.key, date: fmt.full.format(asDate(day)) }));
          setFocusKey(task.key);
          reveal.current = task.key;
        });
      }
      return;
    }
    const task = byKey.get(now.key);
    if (task && now.days !== 0) {
      const next = draggedDates(datesOf(task), now.mode, now.days);
      change(task, next, () => {
        announceDates(task.key, next);
      });
    }
  });

  const scrollToRow = useEffectEvent((at: number) => {
    showRow(at);
  });
  useEffect(() => {
    const key = reveal.current;
    const at = key === null ? undefined : index.get(key);
    if (at !== undefined && rows[at]?.kind === "task") {
      reveal.current = null;
      scrollToRow(at);
    }
  }, [index, rows]);

  const onCancel = useEffectEvent(() => {
    changeDrag(null);
  });

  const dragging = drag !== null;
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const move = (event: PointerEvent) => {
      onPointerMove(event);
    };
    const up = () => {
      onPointerUp();
    };
    const cancel = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCancel();
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

  const onUnscheduledDown = useCallback((event: ReactPointerEvent, key: string) => {
    if (event.button !== 0 || (event.target instanceof Element && event.target.closest("button, input") !== null)) {
      return;
    }
    event.preventDefault();
    setFocusKey(key);
    focusChart(event.currentTarget);
    setDrag({ id: event.timeStamp, key, mode: "schedule", clientX: event.clientX, clientY: event.clientY, moving: false, days: 0, x: 0, y: 0 });
  }, []);

  const onRowClick = useCallback((key: string, from: Element) => {
    setFocusKey(key);
    focusChart(from);
  }, []);
  const onEdit = useCallback((key: string, field: Editing["field"]) => {
    setFocusKey(key);
    setEditing({ key, field });
  }, []);
  const onEditDone = (task: Movable, field: Editing["field"], value: string | null | undefined, then: "stay" | "next") => {
    if (value !== undefined) {
      const before: Dates = { start: task.start_date, due: task.due_date };
      const next: Dates = field === "start" ? { ...before, start: value ?? undefined } : { ...before, due: value ?? undefined };
      if (next.start !== undefined && next.due !== undefined && next.start > next.due) {
        if (field === "start") {
          next.due = next.start;
        } else {
          next.start = next.due;
        }
      }
      change(task, next, () => {
        announceDates(task.key, next);
      });
    }
    if (then === "next" && field === "start") {
      setEditing({ key: task.key, field: "due" });
    } else {
      setEditing(null);
      focusGrid();
    }
  };

  // ---------- resizing the table and its columns ----------

  const startResize = (event: ReactPointerEvent<HTMLElement>, column: Column | "table") => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const x0 = event.clientX;
    const cell = event.currentTarget.parentElement;
    const w0 = column === "table" ? tableW : (cell?.getBoundingClientRect().width ?? 80);
    let last = w0;
    const move = (ev: PointerEvent) => {
      const delta = rtl ? x0 - ev.clientX : ev.clientX - x0;
      if (column === "table") {
        last = Math.round(Math.max(TABLE_MIN, Math.min(TABLE_MAX, w0 + delta)));
        setLiveTable(last);
      } else {
        last = Math.round(Math.max(COLUMN_MIN, Math.min(COLUMN_MAX, w0 + delta)));
        setLiveWidths({ [column]: last });
      }
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      if (column === "table") {
        onTable(last);
        setLiveTable(null);
      } else {
        onWidth(column, last);
        setLiveWidths({});
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };

  const resizeByKey = (event: KeyboardEvent<HTMLElement>, column: Column | "table") => {
    const step = event.key === "ArrowRight" ? 16 : event.key === "ArrowLeft" ? -16 : 0;
    if (step === 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const by = rtl ? -step : step;
    if (column === "table") {
      onTable(Math.max(TABLE_MIN, Math.min(TABLE_MAX, tableW + by)));
    } else {
      const cell = event.currentTarget.parentElement;
      const current = widths[column] ?? cell?.getBoundingClientRect().width ?? 80;
      onWidth(column, Math.round(Math.max(COLUMN_MIN, Math.min(COLUMN_MAX, current + by))));
    }
  };

  // ---------- drawing ----------

  const { r0, r1, x0, x1 } = view;
  const [topUnit, lowUnit] = UNITS[zoom];
  const topCells = headerCells(origin, ppd, topUnit, x0, x1, width, weekStart);
  const lowCells = headerCells(origin, ppd, lowUnit, x0, x1, width, weekStart);
  const todayX = dayX(origin, today, ppd);
  const height = rows.length * ROW;
  const shown = rows.slice(r0, r1);
  const dependencies = useMemo(() => visibleDependencies(schedule.dependencies, index, r0, r1), [schedule.dependencies, index, r0, r1]);

  const cellLabel = (cell: HeaderCell, top: boolean) => {
    switch (cell.unit) {
      case "day":
        return fmt.day.format(asDate(cell.first));
      case "week":
        return fmt.week.format(asDate(cell.first));
      case "month":
        return top ? fmt.monthYear.format(asDate(cell.first)) : fmt.month.format(asDate(cell.first));
      case "quarter":
        return t("gantt-quarter", { quarter: Math.floor((Number(cell.first.slice(5, 7)) - 1) / 3) + 1 });
      case "year":
        return fmt.year.format(asDate(cell.first));
    }
  };

  /** A task's dates while it is dragged. */
  const draggedOf = (task: ScheduledTask): Dates | undefined =>
    drag?.moving === true && drag.key === task.key && drag.mode !== "link" && drag.mode !== "schedule" ? draggedDates(datesOf(task), drag.mode, drag.days) : undefined;
  const geoOf = (task: ScheduledTask) => geometry(task, origin, ppd, draggedOf(task));

  const tooltip = (() => {
    if (!drag?.moving || drag.mode === "link") {
      return null;
    }
    if (drag.mode === "schedule") {
      return drag.day === undefined ? null : { x: drag.x, y: drag.y - 34, text: [t("gantt-drag-day", { date: fmt.full.format(asDate(drag.day)) })] };
    }
    const task = byKey.get(drag.key);
    const at = index.get(drag.key);
    if (!task || at === undefined) {
      return null;
    }
    const next = draggedDates(datesOf(task), drag.mode, drag.days);
    const first = next.start ?? next.due;
    const last = next.due ?? next.start;
    if (first === undefined || last === undefined) {
      return null;
    }
    const text = [first === last ? t("gantt-drag-day", { date: fmt.cell(first) }) : t("gantt-drag-dates", { from: fmt.cell(first), to: fmt.cell(last) })];
    if (moveDependents && task.dependents > 0 && endsLater(datesOf(task), next)) {
      text.push(t("gantt-drag-dependents", { count: task.dependents }));
    }
    return { x: drag.x, y: at * ROW - 4 - 20 * text.length, text };
  })();

  const linkDraft = (() => {
    if (drag?.mode !== "link" || !drag.moving) {
      return null;
    }
    const task = byKey.get(drag.key);
    const at = index.get(drag.key);
    const geo = task && geometry(task, origin, ppd);
    if (!geo || at === undefined) {
      return null;
    }
    return { x1: geo.x2, y1: at * ROW + ROW / 2, x2: drag.x, y2: drag.y };
  })();

  const sizes = {
    "--gantt-table": `${String(tableW)}px`,
    ...Object.fromEntries(SIZED.filter((column) => widths[column] !== undefined).map((column) => [`--gantt-col-${column}`, `${String(widths[column])}px`])),
  } as CSSProperties;

  const activeRow = focusKey !== null && focusIndex !== undefined && focusIndex >= r0 && focusIndex < r1 ? rowId(focusKey) : undefined;
  const hintId = `${ids}-hint`;

  return (
    <div className={styles.chart} style={sizes} dir={direction} data-gantt="" data-zoom={zoom} data-dragging={drag?.moving === true ? drag.mode : undefined}>
      <p id={hintId} className={styles.hidden}>
        {t("gantt-keys-hint")}
      </p>
      <div
        ref={scroller}
        className={styles.scroll}
        role="treegrid"
        aria-label={t("gantt-label")}
        aria-describedby={hintId}
        aria-rowcount={rows.length + 1}
        aria-activedescendant={activeRow}
        aria-multiselectable={false}
        tabIndex={0}
        onScroll={() => {
          measure();
        }}
        onKeyDown={onKey}
        onFocus={(event) => {
          // Tabbing in selects the first task; a click selects its own and
          // mustn't scroll the chart away under the pointer.
          if (event.target === event.currentTarget && focusKey === null) {
            const first = rows.find((row) => row.kind !== "group");
            if (first) {
              setFocusKey(first.task.key);
            }
          }
        }}
      >
        <div className={styles.canvas} style={{ inlineSize: tableW + width, blockSize: HEAD + height }}>
          <div className={styles.corner} role="row" aria-rowindex={1}>
            <span role="columnheader" className={styles.cell}>
              {t("gantt-col-key")}
              <ResizeHandle column="key" label={t("gantt-resize-column", { column: t("gantt-col-key") })} value={widths.key} onDown={startResize} onKey={resizeByKey} />
            </span>
            <span role="columnheader" className={styles.cell}>
              {t("gantt-col-title")}
            </span>
            <span role="columnheader" className={classes(styles.cell, styles.num)}>
              {t("gantt-col-start")}
              <ResizeHandle column="start" label={t("gantt-resize-column", { column: t("gantt-col-start") })} value={widths.start} onDown={startResize} onKey={resizeByKey} />
            </span>
            <span role="columnheader" className={classes(styles.cell, styles.num)}>
              {t("gantt-col-due")}
              <ResizeHandle column="due" label={t("gantt-resize-column", { column: t("gantt-col-due") })} value={widths.due} onDown={startResize} onKey={resizeByKey} />
            </span>
            <span role="columnheader" className={classes(styles.cell, styles.num)} aria-label={t("gantt-col-progress-label")}>
              {t("gantt-col-progress")}
              <ResizeHandle column="progress" label={t("gantt-resize-column", { column: t("gantt-col-progress-label") })} value={widths.progress} onDown={startResize} onKey={resizeByKey} />
            </span>
            <div
              className={styles.tableHandle}
              role="separator"
              tabIndex={0}
              aria-orientation="vertical"
              aria-label={t("gantt-resize-table")}
              aria-valuenow={tableW}
              aria-valuemin={TABLE_MIN}
              aria-valuemax={TABLE_MAX}
              onPointerDown={(event) => {
                startResize(event, "table");
              }}
              onKeyDown={(event) => {
                resizeByKey(event, "table");
              }}
            />
          </div>

          <div className={styles.head} aria-hidden="true" style={{ inlineSize: width }}>
            <div className={styles.headRow}>
              {topCells.map((cell) => (
                <div key={cell.first} className={classes(styles.headCell, styles.headTop)} style={{ insetInlineStart: cell.x, inlineSize: cell.width }}>
                  <span>{cellLabel(cell, true)}</span>
                </div>
              ))}
            </div>
            <div className={styles.headRow}>
              {lowCells.map((cell) => (
                <div
                  key={cell.first}
                  className={classes(styles.headCell, cell.unit === "day" && weekend.has(weekday(cell.day)) && styles.weekendHead, cell.unit === "day" && cell.day === today && styles.todayHead)}
                  style={{ insetInlineStart: cell.x, inlineSize: cell.width }}
                  data-today={cell.unit === "day" && cell.day === today ? "" : undefined}
                >
                  {cellLabel(cell, false)}
                </div>
              ))}
            </div>
          </div>

          <div className={styles.table} role="rowgroup" style={{ blockSize: height }}>
            {shown.map((row, k) => {
              const at = r0 + k;
              if (row.kind === "group") {
                return (
                  <div key="group" className={classes(styles.row, styles.group)} role="row" aria-rowindex={at + 2} style={{ insetBlockStart: at * ROW }}>
                    <span role="gridcell">{t("gantt-unscheduled", { count: row.count })}</span>
                  </div>
                );
              }
              return (
                <TableRow
                  key={row.task.key}
                  id={rowId(row.task.key)}
                  row={row}
                  at={at}
                  focused={focusKey === row.task.key}
                  open={openTask === row.task.key}
                  linking={linkFrom === row.task.key}
                  editing={editing?.key === row.task.key ? editing.field : undefined}
                  problems={problems.get(row.task.key)}
                  fmt={fmt}
                  problemText={problemText}
                  onClick={onRowClick}
                  onToggle={toggle}
                  onEdit={onEdit}
                  onEditDone={onEditDone}
                  onUnscheduledDown={onUnscheduledDown}
                  onOpen={onOpen}
                />
              );
            })}
          </div>

          <div ref={timeline} className={styles.timeline} style={{ inlineSize: width, blockSize: height }}>
            <svg className={styles.svg} width={width} height={Math.max(height, 1)} style={rtl ? { transform: "scaleX(-1)" } : undefined} aria-hidden="true">
              <defs>
                <marker id={`${ids}-arrow`} viewBox="0 0 6 6" refX="6" refY="3" markerWidth="6" markerHeight="6" orient="auto">
                  <path d="M0,0 L6,3 L0,6 z" className={styles.arrowhead} />
                </marker>
                <marker id={`${ids}-arrow-warn`} viewBox="0 0 6 6" refX="6" refY="3" markerWidth="6" markerHeight="6" orient="auto">
                  <path d="M0,0 L6,3 L0,6 z" className={classes(styles.arrowhead, styles.warn)} />
                </marker>
              </defs>
              {(zoom === "day" || zoom === "week") && (
                <Weekends origin={origin} ppd={ppd} x0={x0} x1={Math.min(x1, width)} height={height} weekend={weekend} />
              )}
              {focusIndex !== undefined && focusIndex >= r0 && focusIndex < r1 && <rect className={styles.focusRow} x={x0} width={Math.max(0, Math.min(x1, width) - x0)} y={focusIndex * ROW} height={ROW} />}
              {drag?.mode === "schedule" && drag.day !== undefined && (
                <rect className={styles.dropDay} x={dayX(origin, drag.day, ppd)} width={Math.max(2, ppd)} y={0} height={height} />
              )}
              <line className={styles.today} x1={todayX + ppd / 2} x2={todayX + ppd / 2} y1={0} y2={height} />
              <g className={styles.links}>
                {dependencies.map((dependency) => {
                  const a = byKey.get(dependency.from);
                  const b = byKey.get(dependency.to);
                  const ga = a && geoOf(a);
                  const gb = b && geoOf(b);
                  const ya = index.get(dependency.from);
                  const yb = index.get(dependency.to);
                  if (!ga || !gb || ya === undefined || yb === undefined) {
                    return null;
                  }
                  const warn = dependency.violated || gb.x1 < ga.x2;
                  const d = elbow(ga.x2, ya * ROW + ROW / 2, gb.x1, yb * ROW + ROW / 2);
                  return (
                    <g key={dependency.id} data-dependency={`${dependency.from}>${dependency.to}`}>
                      <path d={d} className={classes(styles.link, warn && styles.warn)} markerEnd={`url(#${ids}-arrow${warn ? "-warn" : ""})`} />
                      <path
                        d={d}
                        className={styles.linkHit}
                        onClick={() => {
                          unlink(dependency);
                        }}
                      >
                        <title>{t("gantt-remove-dependency", { from: dependency.from, to: dependency.to })}</title>
                      </path>
                    </g>
                  );
                })}
              </g>
              {shown.map((row, k) =>
                row.kind === "task" ? (
                  <Bar
                    key={row.task.key}
                    task={row.task}
                    geo={geoOf(row.task)}
                    y={(r0 + k) * ROW}
                    focused={focusKey === row.task.key}
                    target={drag?.target === row.task.key}
                    onDown={onBarDown}
                    onOpen={onOpen}
                  />
                ) : null,
              )}
              {linkDraft && <line className={styles.linkDraft} x1={linkDraft.x1} y1={linkDraft.y1} x2={linkDraft.x2} y2={linkDraft.y2} />}
            </svg>
            <div className={styles.notes} aria-hidden="true">
              {shown.map((row, k) => {
                if (row.kind !== "task") {
                  return null;
                }
                const geo = geoOf(row.task);
                const note = noteOf(row.task, today, t);
                if (!note || !geo) {
                  return null;
                }
                return (
                  <span key={row.task.key} className={classes(styles.note, note.danger ? styles.danger : styles.warning)} style={{ insetInlineStart: geo.x2 + 6, insetBlockStart: (r0 + k) * ROW }}>
                    {note.text}
                  </span>
                );
              })}
              {tooltip && (
                <span className={styles.tip} style={{ insetInlineStart: tooltip.x + 12, insetBlockStart: Math.max(0, tooltip.y) }} data-testid="gantt-tip">
                  {tooltip.text.map((line) => (
                    <span key={line}>{line}</span>
                  ))}
                </span>
              )}
            </div>
          </div>
        </div>
      </div>
      <div className={styles.hidden} aria-live="polite" data-testid="gantt-live">
        {live}
      </div>
    </div>
  );
});

function noteOf(task: ScheduledTask, today: Day, t: ReturnType<typeof useT>): { text: string; danger: boolean } | null {
  if (task.overdue && task.shape !== "summary" && task.due_date !== undefined) {
    return { text: t("gantt-overdue-by", { days: daysBetween(task.due_date, today) }), danger: true };
  }
  if (task.shape === "no_deadline" && !task.closed) {
    return { text: t("gantt-no-deadline"), danger: false };
  }
  return null;
}

function ResizeHandle({
  column,
  label,
  value,
  onDown,
  onKey,
}: {
  column: Column;
  label: string;
  value: number | undefined;
  onDown: (event: ReactPointerEvent<HTMLElement>, column: Column) => void;
  onKey: (event: KeyboardEvent<HTMLElement>, column: Column) => void;
}) {
  return (
    <span
      className={styles.columnHandle}
      role="separator"
      tabIndex={-1}
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={COLUMN_MIN}
      aria-valuemax={COLUMN_MAX}
      data-column={column}
      onPointerDown={(event) => {
        onDown(event, column);
      }}
      onKeyDown={(event) => {
        onKey(event, column);
      }}
    />
  );
}

interface TableRowProps {
  id: string;
  row: Exclude<Row, { kind: "group" }>;
  at: number;
  focused: boolean;
  open: boolean;
  linking: boolean;
  editing: Editing["field"] | undefined;
  problems: readonly ScheduleProblem[] | undefined;
  fmt: Formats;
  problemText: (problem: ScheduleProblem) => string;
  onClick: (key: string, from: Element) => void;
  onToggle: (key: string) => void;
  onEdit: (key: string, field: Editing["field"]) => void;
  onEditDone: (task: Movable, field: Editing["field"], value: string | null | undefined, then: "stay" | "next") => void;
  onUnscheduledDown: (event: ReactPointerEvent, key: string) => void;
  onOpen: (key: string) => void;
}

const TableRow = memo(function TableRow({ id, row, at, focused, open, linking, editing, problems, fmt, problemText, onClick, onToggle, onEdit, onEditDone, onUnscheduledDown, onOpen }: TableRowProps) {
  const t = useT();
  const task: ScheduledTask | UnscheduledTask = row.task;
  const scheduled = row.kind === "task" ? row.task : undefined;
  const start = scheduled?.start_date;
  const due = scheduled?.due_date;
  const progress = scheduled ? fmt.percent.format(scheduled.progress_percent / 100) : "";
  const summary = scheduled?.summary;
  const warnings = (problems ?? []).filter(isWarning);
  /** The date field was left with Enter or Tab: its blur saves nothing more. */
  const left = useRef(false);

  let label: string;
  if (!scheduled) {
    label = t("gantt-row-unscheduled", { key: task.key, title: task.title });
  } else {
    const first = summary?.start ?? start;
    const last = summary?.end ?? due;
    if (first !== undefined && last !== undefined) {
      label = t("gantt-row-dates", { key: task.key, title: task.title, from: fmt.full.format(asDate(first)), to: fmt.full.format(asDate(last)), progress });
    } else if (last !== undefined) {
      label = t("gantt-row-due", { key: task.key, title: task.title, due: fmt.full.format(asDate(last)), progress });
    } else {
      label = t("gantt-row-start", { key: task.key, title: task.title, start: fmt.full.format(asDate(first ?? "")) });
    }
  }
  if (task.closed) {
    label = t("gantt-row-closed", { row: label });
  }

  const dateCell = (field: Editing["field"]) => {
    const value = field === "start" ? start : due;
    const shownValue = field === "start" && !value && summary ? undefined : value;
    if (editing === field) {
      return (
        <input
          type="date"
          className={styles.dateInput}
          aria-label={t(field === "start" ? "gantt-edit-start" : "gantt-edit-due", { key: task.key })}
          autoFocus
          defaultValue={value ?? ""}
          onFocus={() => {
            left.current = false;
          }}
          onClick={(event) => {
            event.stopPropagation();
          }}
          onBlur={(event) => {
            if (left.current) {
              left.current = false;
              return;
            }
            const next = event.target.value === "" ? null : event.target.value;
            onEditDone(task, field, next === (value ?? null) ? undefined : next, "stay");
          }}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === "Enter" || (event.key === "Tab" && !event.shiftKey && field === "start")) {
              event.preventDefault();
              const next = event.currentTarget.value === "" ? null : event.currentTarget.value;
              const then = event.key === "Tab" ? "next" : "stay";
              // Leaving by blur would save twice.
              left.current = true;
              onEditDone(task, field, next === (value ?? null) ? undefined : next, then);
            } else if (event.key === "Escape") {
              event.preventDefault();
              left.current = true;
              onEditDone(task, field, undefined, "stay");
            }
          }}
        />
      );
    }
    const editable = !(scheduled?.shape === "summary");
    const text = shownValue === undefined ? "" : fmt.cell(shownValue);
    return (
      <button
        type="button"
        tabIndex={-1}
        className={styles.dateButton}
        disabled={!editable}
        aria-label={t(field === "start" ? "gantt-edit-start-value" : "gantt-edit-due-value", { key: task.key, value: value === undefined ? t("gantt-none") : fmt.full.format(asDate(value)) })}
        onClick={(event) => {
          event.stopPropagation();
          onEdit(task.key, field);
        }}
      >
        {text}
      </button>
    );
  };

  return (
    <div
      id={id}
      className={classes(styles.row, focused && styles.focused, open && styles.open, linking && styles.linking, task.closed && styles.closed, !scheduled && styles.unscheduled)}
      role="row"
      aria-rowindex={at + 2}
      aria-level={row.kind === "task" ? row.level : 1}
      aria-expanded={row.kind === "task" && row.hasKids ? row.open : undefined}
      aria-selected={focused}
      aria-label={label}
      data-row={task.key}
      title={scheduled ? undefined : t("gantt-drag-unscheduled", { key: task.key })}
      style={{ insetBlockStart: at * ROW }}
      onClick={(event) => {
        onClick(task.key, event.currentTarget);
      }}
      onDoubleClick={() => {
        onOpen(task.key);
      }}
      onPointerDown={
        scheduled
          ? undefined
          : (event) => {
              onUnscheduledDown(event, task.key);
            }
      }
    >
      <span role="gridcell" className={classes(styles.cell, styles.key)}>
        {task.key}
      </span>
      <span role="gridcell" className={classes(styles.cell, styles.title)} style={{ paddingInlineStart: `${String((row.kind === "task" ? row.level - 1 : 0) * 1.25)}em` }}>
        {row.kind === "task" && row.hasKids ? (
          <button
            type="button"
            tabIndex={-1}
            className={styles.twisty}
            aria-label={t(row.open ? "gantt-collapse" : "gantt-expand", { key: task.key })}
            onClick={(event) => {
              event.stopPropagation();
              onToggle(task.key);
            }}
          >
            {row.open ? <ChevronDown className="ic" aria-hidden="true" /> : <ChevronRight className={classes("ic", styles.flip)} aria-hidden="true" />}
          </button>
        ) : (
          <span className={styles.twistySpace} aria-hidden="true" />
        )}
        <span className={styles.titleText}>{task.title}</span>
        {warnings.length > 0 && (
          <span className={styles.warnSign} title={warnings.map(problemText).join("\n")} data-problems={warnings.map((problem) => problem.kind).join(" ")}>
            <TriangleAlert className="ic" aria-hidden="true" />
            <span className={styles.hidden}>{t("gantt-problems", { count: warnings.length })}</span>
            {warnings.map((problem) => (
              <span key={`${problem.kind}-${problem.other ?? ""}`} className={styles.hidden}>
                {problemText(problem)}
              </span>
            ))}
          </span>
        )}
      </span>
      <span role="gridcell" className={classes(styles.cell, styles.num)}>
        {dateCell("start")}
      </span>
      <span role="gridcell" className={classes(styles.cell, styles.num)}>
        {dateCell("due")}
      </span>
      <span role="gridcell" className={classes(styles.cell, styles.num)}>
        {progress}
      </span>
    </div>
  );
});

/** Each shape's class. */
const SHAPE_CLASS: Readonly<Record<Geometry["shape"], string | undefined>> = {
  bar: styles.shapeBar,
  deadline: styles.shapeDeadline,
  no_deadline: styles.shapeStart,
  milestone: styles.shapeMilestone,
  summary: styles.shapeSummary,
};

interface BarProps {
  task: ScheduledTask;
  geo: Geometry | undefined;
  y: number;
  focused: boolean;
  target: boolean;
  onDown: (event: ReactPointerEvent, key: string, mode: Drag["mode"]) => void;
  onOpen: (key: string) => void;
}

const Bar = memo(
  function Bar({ task, geo, y, focused, target, onDown, onOpen }: BarProps) {
    if (!geo) {
      return null;
    }
    const w = geo.x2 - geo.x1;
    const cy = y + ROW / 2;
    const down = (mode: Drag["mode"]) => (event: ReactPointerEvent) => {
      onDown(event, task.key, mode);
    };
    const open = () => {
      onOpen(task.key);
    };
    const cls = classes(styles.bar, SHAPE_CLASS[geo.shape], focused && styles.focusedBar, target && styles.target, task.closed && styles.closedBar, task.overdue && styles.overdue);
    const handle = <circle className={styles.handle} cx={geo.x2 + 7} cy={cy} r={4} onPointerDown={down("link")} data-handle="link" />;
    if (geo.shape === "summary") {
      return (
        <g className={cls} data-bar={task.key} data-shape={geo.shape}>
          <rect x={geo.x1} y={cy - 4} width={w} height={8} rx={2} />
        </g>
      );
    }
    if (geo.shape === "milestone") {
      const cx = (geo.x1 + geo.x2) / 2;
      return (
        <g className={cls} data-bar={task.key} data-shape={geo.shape} onDoubleClick={open}>
          <rect className={styles.diamond} x={cx - 5} y={cy - 5} width={10} height={10} rx={2} transform={`rotate(45 ${String(cx)} ${String(cy)})`} onPointerDown={down("move")} />
          {handle}
        </g>
      );
    }
    if (geo.shape === "deadline") {
      const x = geo.x2;
      return (
        <g className={cls} data-bar={task.key} data-shape={geo.shape} onDoubleClick={open}>
          <path className={styles.bracket} d={`M${String(x - 9)},${String(cy - 9)} h5 a4,4 0 0 1 4,4 v10 a4,4 0 0 1 -4,4 h-5`} />
          <rect className={styles.hit} x={x - 9} y={cy - 9} width={9} height={18} onPointerDown={down("move")} data-handle="move" />
          <rect className={styles.edge} x={x - 15} y={cy - 9} width={6} height={18} onPointerDown={down("start")} data-handle="start" />
          {handle}
        </g>
      );
    }
    const edge = Math.min(6, w / 4);
    const done = Math.max(0, Math.min(100, task.progress_percent));
    return (
      <g className={cls} data-bar={task.key} data-shape={geo.shape} data-dependents={task.dependents} onDoubleClick={open}>
        <rect className={styles.body} x={geo.x1} y={cy - 9} width={w} height={18} rx={4} onPointerDown={down("move")} data-handle="move" />
        {done > 0 && geo.shape === "bar" && <rect className={styles.progress} x={geo.x1} y={cy - 9} width={(w * done) / 100} height={18} rx={4} pointerEvents="none" />}
        <rect className={styles.edge} x={geo.x1} y={cy - 9} width={edge} height={18} onPointerDown={down("start")} data-handle="start" />
        <rect className={styles.edge} x={geo.x2 - edge} y={cy - 9} width={edge} height={18} onPointerDown={down("end")} data-handle="end" />
        {handle}
      </g>
    );
  },
  (a, b) =>
    a.task === b.task &&
    a.y === b.y &&
    a.geo?.x1 === b.geo?.x1 &&
    a.geo?.x2 === b.geo?.x2 &&
    a.geo?.shape === b.geo?.shape &&
    a.focused === b.focused &&
    a.target === b.target &&
    a.onDown === b.onDown &&
    a.onOpen === b.onOpen,
);

function Weekends({ origin, ppd, x0, x1, height, weekend }: { origin: Day; ppd: number; x0: number; x1: number; height: number; weekend: ReadonlySet<number> }) {
  const first = Math.floor(x0 / ppd);
  const last = Math.ceil(x1 / ppd);
  return (
    <g>
      {weekendsBetween(origin, first, last, weekend).map((at) => (
        <rect key={at} className={styles.weekend} x={at * ppd} y={0} width={ppd} height={height} />
      ))}
    </g>
  );
}
