// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { ChevronDown, ChevronLeft, ChevronRight, Download } from "lucide-react";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { keysOf } from "../../actions/actions";
import { classes } from "../../classes";
import { Kbd } from "../../components/Kbd";
import { QueryField, type QueryFieldHandle, type Reading } from "../../components/QueryField/QueryField";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCommand, useOp, useUndo } from "../../data/hooks";
import { useListNames, useParsedQuery, useQueryFields } from "../../data/list";
import { useGanttLayout, useSchedule } from "../../data/schedule";
import { type GanttGrouping, type QueryProblem, type ReadOnlyReason, type Schedule, type ScheduleDependency } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { useStatus, useTimerTask } from "../../shell/context";
import { isoDay } from "../board/model";
import { asDate, firstDayOfWeek, type Day } from "../calendar/model";
import { chartSvg, exportSpan, lightPalette, printSvg, svgToPng } from "./export";
import { DEFAULT_TABLE, Gantt, groupText, headerText, noteOf, useFormats, type GanttHandle, type Movable } from "./Gantt";
import { GROUPINGS, layoutFrom, layoutUpdate, type Column, type GanttLayout } from "./layout";
import { datesUpdate, endsLater, type Dates, type GroupOrder } from "./model";
import { ZOOMS, chartSpan, weekendDays, type Zoom } from "./scale";
import styles from "./Gantt.module.css";

/** How long typing pauses before the query runs, as in the list. */
const RUN_DELAY_MS = 250;

interface GanttViewProps {
  project: string;
  openTask: string | undefined;
  onOpen: (key: string) => void;
}

/** The engine's message for a query mistake, in the person's language. */
function useProblemText(): (problem: QueryProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

/** A screen that only takes touch: phones and tablets without a mouse or pen. */
const TOUCH_ONLY = "(pointer: coarse) and (not (any-pointer: fine))";

/** Whether the screen only takes touch, following changes (a mouse plugged in). */
function useTouchOnly(): boolean {
  return useSyncExternalStore(
    (changed) => {
      const query = window.matchMedia(TOUCH_ONLY);
      query.addEventListener("change", changed);
      return () => {
        query.removeEventListener("change", changed);
      };
    },
    () => window.matchMedia(TOUCH_ONLY).matches,
    () => false,
  );
}

/**
 * A project's Gantt chart with its toolbar: zoom, today, paging, the filter
 * query, the sub-projects switch, grouping and export. Dates change through
 * `task.update`, with "also move dependents" when the person turned it on,
 * as one change that Undo takes back; dependencies are `blocks` links. The
 * person's layout is read once from the engine, before the chart draws.
 */
export function GanttView(props: GanttViewProps) {
  const own = useGanttLayout(props.project);
  if (!own.layout && !own.error) {
    return <div className={styles.screen} aria-busy="true" />;
  }
  return <LoadedGantt {...props} saved={layoutFrom(own.layout?.layout)} />;
}

function LoadedGantt({ project, openTask, onOpen, saved }: GanttViewProps & { saved: GanttLayout }) {
  const t = useT();
  const { locale, direction } = useLocale();
  const problemText = useProblemText();
  const projectInfo = useOp("project.get", { project });
  const touchOnly = useTouchOnly();

  // The person's layout, kept by the engine; the chart follows it at once.
  const [layout, setLayout] = useState<GanttLayout>(saved);
  const keepLayout = useCommand("gantt.layout.update");
  const changeLayout = (change: Partial<GanttLayout>) => {
    setLayout((now) => ({ ...now, ...change, widths: { ...now.widths, ...change.widths } }));
    keepLayout.mutate(layoutUpdate(project, change));
  };

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
  const chart = useRef<GanttHandle>(null);

  const data = useSchedule({ project, includeSubprojects, query: running });
  const schedule = data.schedule;
  const span = schedule ? chartSpan(schedule.range, schedule.today, layout.zoom) : undefined;

  // Groups by status follow the board's order; labels sort in the person's language.
  const statuses = projectInfo.data?.statuses;
  const groupOrder = useMemo<GroupOrder>(() => {
    const collator = new Intl.Collator(locale, { numeric: true });
    return {
      statuses: new Map((statuses ?? []).map((status, at) => [status.id, at])),
      hues: new Map((statuses ?? []).map((status) => [status.id, status.hue])),
      compare: (a, b) => collator.compare(a, b),
    };
  }, [locale, statuses]);

  const scheduledCount = schedule?.tasks.length ?? 0;
  const unscheduledCount = schedule?.unscheduled.length ?? 0;
  const overdueCount = schedule?.tasks.filter((task) => task.overdue && task.shape !== "summary").length ?? 0;
  useStatus({
    summary: t("gantt-status", { scheduled: scheduledCount, unscheduled: unscheduledCount }),
    alert: overdueCount > 0 ? t("gantt-status-overdue", { count: overdueCount }) : undefined,
    note: touchOnly ? t("gantt-touch-read-only") : undefined,
  });
  useTimerTask(openTask ?? null);

  const [range, setRange] = useState<{ from: Day; to: Day } | null>(null);
  const rangeFormat = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
  const rangeText = range ? rangeFormat.formatRange(asDate(range.from), asDate(range.to)) : "";

  // ---------- changes ----------

  const update = useCommand("task.update");
  const relate = useCommand("relation.add");
  const unrelate = useCommand("relation.remove");
  const toasts = useToasts();
  const { undo } = useUndo();
  const full = new Intl.DateTimeFormat(locale, { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });

  // Errors show as toasts (the command's own); the chart learns only whether it worked.
  const done = (run: Promise<unknown>) => run.then(() => true).catch(() => false);

  const onDates = (task: Movable, next: Dates) => {
    const before: Dates = { start: task.start_date, due: task.due_date };
    const changes = datesUpdate(before, next);
    if (!changes) {
      return Promise.resolve(false);
    }
    const alsoDependents = layout.moveDependents && (task.dependents ?? 0) > 0 && endsLater(before, next);
    return done(update.mutateAsync(
      { key: task.key, ...changes, expected_version: task.version, ...(alsoDependents ? { move_dependents: true } : {}) },
      {
        onSuccess: (output) => {
          const moved = output.moved?.length ?? 0;
          const first = next.start ?? next.due;
          const last = next.due ?? next.start;
          let message: string;
          if (moved > 0) {
            message = t("gantt-moved-with-dependents", { key: task.key, count: moved });
          } else if (first !== undefined && last !== undefined && first !== last) {
            message = t("gantt-moved-dates", { key: task.key, from: full.format(asDate(first)), to: full.format(asDate(last)) });
          } else {
            message = t("gantt-moved-day", { key: task.key, date: first === undefined ? "" : full.format(asDate(first)) });
          }
          toasts.show({ tone: "info", text: message, action: { label: t("toast-undo"), run: undo } });
        },
      },
    ));
  };

  const onLink = (from: string, to: string) =>
    done(relate.mutateAsync(
      { key: from, kind: "blocks", other: to },
      {
        onSuccess: () => {
          toasts.show({ tone: "info", text: t("gantt-link-made", { from, to }), action: { label: t("toast-undo"), run: undo } });
        },
      },
    ));

  const onUnlink = (dependency: ScheduleDependency) =>
    done(unrelate.mutateAsync(
      { key: dependency.from, kind: "blocks", other: dependency.to },
      {
        onSuccess: () => {
          toasts.show({ tone: "info", text: t("gantt-link-removed", { from: dependency.from, to: dependency.to }), action: { label: t("toast-undo"), run: undo } });
        },
      },
    ));

  const onRefused = (key: string, reason: ReadOnlyReason) => {
    toasts.show({ tone: "error", text: t("gantt-read-only", { key, reason }) });
  };

  // ---------- printing and pictures ----------

  const fmt = useFormats(locale, schedule?.today ?? isoDay(new Date()));
  const picture = (shown: Schedule) => {
    const rows = chart.current?.rows() ?? [];
    const { from, to } = exportSpan(shown, layout.zoom);
    const name = projectInfo.data?.name ?? project;
    return chartSvg({
      schedule: shown,
      rows,
      zoom: layout.zoom,
      rtl: direction === "rtl",
      weekStart: firstDayOfWeek(locale),
      weekend: weekendDays(locale),
      palette: lightPalette(),
      font: getComputedStyle(document.body).fontFamily,
      text: {
        title: t("gantt-export-title", { project: name, range: rangeFormat.formatRange(asDate(from), asDate(to)) }),
        columns: { key: t("gantt-col-key"), title: t("gantt-col-title"), start: t("gantt-col-start"), due: t("gantt-col-due"), progress: t("gantt-col-progress") },
        cell: (cell, top) => headerText(cell, top, fmt, t),
        day: (day) => fmt.cell(day),
        percent: (share) => fmt.percent.format(share),
        group: (row) => groupText(t, row),
        note: (task) => noteOf(task, shown.today, t, fmt.percent) ?? undefined,
      },
    });
  };
  const exportAs = (kind: "png" | "print") => {
    if (!schedule) {
      return;
    }
    const { svg, width, height } = picture(schedule);
    if (kind === "print") {
      printSvg(svg);
      return;
    }
    // A file name, without the marks that keep a placeable's direction on screen.
    const file = t("gantt-export-file", { project, date: isoDay(new Date()) }).replace(/[\u2068\u2069]/g, "");
    svgToPng(svg, width, height)
      .then((bytes) => saveExport(file, bytes, "image/png"))
      .then((saved) => {
        if (saved) {
          toasts.show({ tone: "info", text: t("gantt-exported", { file }) });
        }
      })
      .catch((error: unknown) => {
        toasts.show(error instanceof Error && (error.message === "canvas" || error.message === "png") ? { tone: "error", text: t("gantt-export-failed") } : errorToast(t, error));
      });
  };

  // ---------- drawing ----------

  const filterChord = keysOf("view.filter")[0];
  const queryReading: Reading | undefined = reading ? { text, tokens: reading.tokens, error: reading.error } : undefined;
  const problem = reading?.error;
  const focusChart = () => {
    document.querySelector<HTMLElement>(`[data-gantt-view="${CSS.escape(project)}"] [role=treegrid]`)?.focus();
  };

  return (
    <div className={styles.screen} data-gantt-view={project}>
      <div className={classes("toolbar", styles.toolbar)}>
        <div className={styles.toolbarItem}>
          <Segmented
            label={t("gantt-zoom")}
            segments={ZOOMS.map((zoom) => ({ id: zoom, label: t(`gantt-zoom-${zoom}`) }))}
            value={layout.zoom}
            onChange={(zoom: Zoom) => {
              changeLayout({ zoom });
            }}
          />
        </div>
        <div className={styles.toolbarItem}>
          <Button className={classes("btn flat icon", styles.flip)} aria-label={t("gantt-earlier")} onPress={() => chart.current?.page(-1)}>
            <ChevronLeft className="ic" aria-hidden="true" />
          </Button>
          <Button className="btn" onPress={() => chart.current?.goToday()}>
            {t("gantt-today")}
          </Button>
          <Button className={classes("btn flat icon", styles.flip)} aria-label={t("gantt-later")} onPress={() => chart.current?.page(1)}>
            <ChevronRight className="ic" aria-hidden="true" />
          </Button>
          <h2 className={styles.range} data-testid="gantt-range">
            {rangeText}
          </h2>
        </div>
        <QueryField
          ref={field}
          value={text}
          onChange={setText}
          reading={queryReading}
          problem={problem ? problemText(problem) : undefined}
          fields={fields}
          names={names}
          label={t("gantt-query-label")}
          placeholder={t("gantt-query-placeholder")}
          onSubmit={focusChart}
          onEscape={focusChart}
          shortcut={filterChord && <Kbd chord={filterChord} />}
        />
        <div className={styles.toolbarItem}>
          <Switch isSelected={includeSubprojects} onChange={setIncludeSubprojects}>
            {t("gantt-include-subprojects")}
          </Switch>
        </div>
        {!touchOnly && (
          <div className={styles.toolbarItem}>
            <Switch
              isSelected={layout.moveDependents}
              onChange={(on) => {
                changeLayout({ moveDependents: on });
              }}
            >
              {t("gantt-move-dependents")}
            </Switch>
          </div>
        )}
        <div className={styles.toolbarItem}>
          <GroupMenu
            value={layout.groupBy}
            onChange={(groupBy) => {
              changeLayout({ groupBy });
            }}
          />
          <MenuTrigger>
            <Button className="btn flat" isDisabled={!schedule}>
              <Download className="ic" aria-hidden="true" />
              <span className="hide-narrow">{t("gantt-export")}</span>
            </Button>
            <Popover className="pop menuPopover" placement="bottom end" offset={4}>
              <Menu
                className="menu"
                aria-label={t("gantt-export")}
                onAction={(id) => {
                  exportAs(id === "print" ? "print" : "png");
                }}
              >
                <MenuItem id="png" className="item">
                  {t("gantt-export-png")}
                </MenuItem>
                <MenuItem id="print" className="item">
                  {t("gantt-export-print")}
                </MenuItem>
              </Menu>
            </Popover>
          </MenuTrigger>
        </div>
      </div>
      {data.error !== null && (
        <p className={styles.note} role="alert">
          {t("gantt-error", { message: data.error.error.message })}
        </p>
      )}
      {schedule && span && (
        <>
          {schedule.tasks.length === 0 && schedule.unscheduled.length === 0 && (
            <p className={styles.note} role="status">
              {t("gantt-empty")}
            </p>
          )}
          <Gantt
            ref={chart}
            schedule={schedule}
            origin={span.origin}
            days={span.days}
            zoom={layout.zoom}
            table={layout.table ?? DEFAULT_TABLE}
            widths={layout.widths}
            moveDependents={layout.moveDependents && !touchOnly}
            groupBy={layout.groupBy}
            groupOrder={groupOrder}
            readOnly={touchOnly}
            openTask={openTask}
            onDates={onDates}
            onLink={onLink}
            onUnlink={onUnlink}
            onOpen={onOpen}
            onZoom={(zoom) => {
              changeLayout({ zoom });
            }}
            onTable={(table) => {
              changeLayout({ table });
            }}
            onWidth={(column: Column, width: number) => {
              changeLayout({ widths: { [column]: width } });
            }}
            onRange={setRange}
            onRefused={onRefused}
          />
        </>
      )}
    </div>
  );
}

/** The Group menu: by parent (the tree), label, status, or none. */
function GroupMenu({ value, onChange }: { value: GanttGrouping; onChange: (grouping: GanttGrouping) => void }) {
  const t = useT();
  const name = (grouping: GanttGrouping) => t(`gantt-group-by-${grouping}`);
  return (
    <MenuTrigger>
      <Button className="btn flat" aria-label={t("gantt-group-button", { grouping: name(value) })}>
        <span className="muted hide-narrow">{t("gantt-group")}</span>
        <span>{name(value)}</span>
        <ChevronDown className="ic s muted" aria-hidden="true" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu
          className="menu"
          aria-label={t("gantt-group")}
          selectionMode="single"
          selectedKeys={[value]}
          onAction={(id) => {
            const chosen = GROUPINGS.find((grouping) => grouping === id);
            if (chosen) {
              onChange(chosen);
            }
          }}
        >
          {GROUPINGS.map((grouping) => (
            <MenuItem key={grouping} id={grouping} className="item">
              {name(grouping)}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
