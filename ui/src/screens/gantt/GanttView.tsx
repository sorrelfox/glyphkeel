// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { keysOf } from "../../actions/actions";
import { classes } from "../../classes";
import { Kbd } from "../../components/Kbd";
import { QueryField, type QueryFieldHandle, type Reading } from "../../components/QueryField/QueryField";
import { Segmented } from "../../components/Segmented";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { useCommand, useOp, useUndo } from "../../data/hooks";
import { useListNames, useParsedQuery, useQueryFields } from "../../data/list";
import { useSchedule } from "../../data/schedule";
import { type QueryProblem, type ScheduleDependency } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useStatus, useTimerTask } from "../../shell/context";
import { asDate, type Day } from "../calendar/model";
import { DEFAULT_TABLE, Gantt, type GanttHandle, type Movable } from "./Gantt";
import { DEFAULT_LAYOUT, loadLayout, saveLayout, type Column, type GanttLayout } from "./layout";
import { datesUpdate, endsLater, type Dates } from "./model";
import { ZOOMS, chartSpan, type Zoom } from "./scale";
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

/**
 * A project's Gantt chart with its toolbar: zoom, today, paging, the filter
 * query and the sub-projects switch. Dates change through `task.update`,
 * with "also move dependents" when the person turned it on, as one change
 * that Undo takes back; dependencies are `blocks` links.
 */
export function GanttView({ project, openTask, onOpen }: GanttViewProps) {
  const t = useT();
  const { locale } = useLocale();
  const problemText = useProblemText();
  const projectInfo = useOp("project.get", { project });
  const projectId = projectInfo.data?.id;

  // The person's layout, per project; read once the project's ID is known.
  const [layout, setLayout] = useState<GanttLayout>(DEFAULT_LAYOUT);
  const [layoutOf, setLayoutOf] = useState<string | undefined>(undefined);
  if (projectId !== undefined && layoutOf !== projectId) {
    setLayoutOf(projectId);
    setLayout(loadLayout(projectId));
  }
  const changeLayout = (change: Partial<GanttLayout>) => {
    const next = { ...layout, ...change, widths: { ...layout.widths, ...change.widths } };
    setLayout(next);
    if (projectId !== undefined) {
      saveLayout(projectId, next);
    }
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

  const scheduledCount = schedule?.tasks.length ?? 0;
  const unscheduledCount = schedule?.unscheduled.length ?? 0;
  const overdueCount = schedule?.tasks.filter((task) => task.overdue && task.shape !== "summary").length ?? 0;
  useStatus({ summary: t("gantt-status", { scheduled: scheduledCount, unscheduled: unscheduledCount, overdue: overdueCount }) });
  useTimerTask(openTask ?? null);

  const [range, setRange] = useState<{ from: Day; to: Day } | null>(null);
  const rangeText = range ? new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).formatRange(asDate(range.from), asDate(range.to)) : "";

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
            moveDependents={layout.moveDependents}
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
          />
        </>
      )}
    </div>
  );
}
