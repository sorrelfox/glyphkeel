// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { formatDuration } from "../../data/duration";
import { failureText, useOp } from "../../data/hooks";
import { useDurationUnits } from "../../data/time";
import { type Hue, type Widget, type WidgetValue } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { MarkdownReader } from "../../markdown/Reader";
import { CreatedClosed, CumulativeFlow, CycleTime } from "../reports/FlowReports";
import { describeChange } from "../safety/ActivityScreen";
import { BurndownFigure } from "../sprints/Burndown";
import { useMomentFormat } from "../task/Activity";
import styles from "./Dashboards.module.css";

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

/** A widget's title: the one given, else its kind's own. */
export function widgetTitle(t: Translate, widget: Widget): string {
  if (widget.title !== undefined && widget.title !== "") {
    return widget.title;
  }
  // The starter dashboard's first widget, which counts the open tasks.
  if (widget.kind === "query_count" && widget.settings.query?.trim() === "is:open") {
    return t("widget-open-tasks");
  }
  if (widget.kind === "tasks_by_field") {
    const field = widget.settings.field ?? "status";
    switch (field) {
      case "status":
      case "priority":
      case "type":
      case "label":
        return t(`widget-tasks-by-${field}`);
      default:
        return t("widget-tasks-by", { field: field.replace(/^cf\./, "") });
    }
  }
  return t(`widget-kind-${widget.kind}`);
}

/** A bar's name: the priority in words, the value as it is, or none. */
function barName(t: Translate, field: string, value: string | undefined): string {
  if (value === undefined) {
    return t("widget-no-value");
  }
  return field === "priority" ? t(`priority-${value}`) : value;
}

interface BodyProps {
  value: WidgetValue;
  title: string;
  asTable: boolean;
  onOpenTask: (key: string) => void;
  onOpenQuery: (query: string) => void;
}

/** What a widget shows, by its kind; the engine computed it. */
export function WidgetBody({ value, title, asTable, onOpenTask, onOpenQuery }: BodyProps) {
  const t = useT();
  if (value.error !== undefined) {
    return (
      <p className={styles.problem} role="alert" data-widget-error>
        {t("widget-error", { message: failureText(t, value.error) })}
      </p>
    );
  }
  const widget = value.widget;
  switch (widget.kind) {
    case "query_count":
      return <Count value={value} onOpenQuery={onOpenQuery} />;
    case "query_list":
    case "due_soon":
    case "overdue":
      return <TaskList value={value} onOpenTask={onOpenTask} />;
    case "tasks_by_field":
      return <Bars value={value} title={title} onOpenQuery={onOpenQuery} />;
    case "created_closed":
      return value.created_closed ? <CreatedClosed report={value.created_closed} asTable={asTable} /> : null;
    case "cumulative_flow":
      return value.cumulative_flow ? <CumulativeFlow report={value.cumulative_flow} asTable={asTable} /> : null;
    case "cycle_time":
      return value.cycle_time ? <CycleTime report={value.cycle_time} asTable={asTable} metric="cycle" compact /> : null;
    case "burndown":
      return value.burndown ? <BurndownFigure sprint={value.burndown.sprint} data={value.burndown} asTable={asTable} heading={<span className="muted grow">{value.burndown.sprint.name}</span>} /> : null;
    case "time_report":
      return <Time value={value} />;
    case "version_progress":
      return <Versions value={value} />;
    case "activity":
      return <Changes value={value} onOpenTask={onOpenTask} />;
    case "automation_failures":
      return <Failures value={value} onOpenTask={onOpenTask} />;
    case "note":
      return <Note id={widget.id} text={value.text ?? ""} title={title} onOpenTask={onOpenTask} />;
  }
}

function Count({ value, onOpenQuery }: { value: WidgetValue; onOpenQuery: (query: string) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const count = value.count ?? 0;
  const query = value.query;
  return (
    <div className={styles.count}>
      <span className={classes("num", styles.figure)} data-widget-count>
        {new Intl.NumberFormat(locale).format(count)}
      </span>
      <span className="muted">{t("widget-tasks", { count })}</span>
      {query !== undefined && (
        <Button
          className="linkbtn"
          onPress={() => {
            onOpenQuery(query);
          }}
        >
          {t("widget-open-list")}
        </Button>
      )}
    </div>
  );
}

function TaskList({ value, onOpenTask }: { value: WidgetValue; onOpenTask: (key: string) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const tasks = value.tasks ?? [];
  const date = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });
  if (tasks.length === 0) {
    return <p className={classes("muted", styles.quiet)}>{t("widget-no-tasks")}</p>;
  }
  const more = (value.count ?? tasks.length) - tasks.length;
  return (
    <>
      <ul className={styles.rows}>
        {tasks.map((task) => (
          <li key={task.key} data-widget-task={task.key}>
            <Button
              className={classes(styles.row)}
              onPress={() => {
                onOpenTask(task.key);
              }}
            >
              <span className={classes("key", styles.key)}>{task.key}</span>
              <span className={classes("grow", styles.rowTitle)}>{task.title}</span>
              {task.due_date !== undefined && <span className={classes("muted num", styles.rowNote)}>{t("widget-due", { date: date.format(utc(task.due_date)) })}</span>}
            </Button>
          </li>
        ))}
      </ul>
      {more > 0 && <p className={classes("muted", styles.quiet)}>{t("widget-more", { count: more })}</p>}
    </>
  );
}

function hueColour(hue: Hue | undefined): string {
  return hue === undefined ? "var(--accent)" : `var(--hue-${hue})`;
}

/**
 * Tasks by field: one bar per value, longest for the most tasks. The bars
 * are a table, so they read without seeing them; each value opens its tasks.
 */
function Bars({ value, title, onOpenQuery }: { value: WidgetValue; title: string; onOpenQuery: (query: string) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const bars = value.bars ?? [];
  const field = value.widget.settings.field ?? "status";
  const most = Math.max(1, ...bars.map((bar) => bar.count));
  if (bars.length === 0) {
    return <p className={classes("muted", styles.quiet)}>{t("widget-no-tasks")}</p>;
  }
  return (
    <table className={styles.bars} aria-label={title} data-widget-bars>
      <tbody>
        {bars.map((bar) => {
          const name = barName(t, field, bar.value);
          const query = bar.query;
          return (
            <tr key={bar.value ?? ""} data-bar-value={bar.value ?? ""}>
              <th scope="row" className={styles.barName}>
                {query === undefined ? (
                  name
                ) : (
                  <Button
                    className="linkbtn"
                    onPress={() => {
                      onOpenQuery(query);
                    }}
                  >
                    {name}
                  </Button>
                )}
              </th>
              <td className={styles.barCell}>
                <span className={styles.bar} style={{ inlineSize: `${String((bar.count / most) * 100)}%`, backgroundColor: hueColour(bar.hue) }} aria-hidden="true" />
              </td>
              <td className={classes("num", styles.barCount)}>{number.format(bar.count)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function Time({ value }: { value: WidgetValue }) {
  const t = useT();
  const { locale } = useLocale();
  const units = useDurationUnits();
  const report = value.time;
  if (report === undefined) {
    return null;
  }
  const period = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" }).formatRange(utc(report.from), utc(report.to));
  const most = Math.max(1, ...report.items.map((row) => row.minutes));
  return (
    <>
      <p className={styles.count}>
        <span className={classes("num", styles.figure)}>{formatDuration(report.total_minutes, units)}</span>
        <span className="muted">{t("widget-time-total", { total: formatDuration(report.total_minutes, units), period })}</span>
      </p>
      {report.items.length === 0 ? (
        <p className={classes("muted", styles.quiet)}>{t("widget-no-time")}</p>
      ) : (
        <table className={styles.bars} aria-label={t("widget-kind-time_report")}>
          <tbody>
            {report.items.map((row) => (
              <tr key={`${row.key}-${row.person?.name ?? ""}`}>
                <th scope="row" className={styles.barName}>
                  {row.title !== "" ? row.title : row.key !== "" ? row.key : (row.person?.name ?? t("widget-no-value"))}
                </th>
                <td className={styles.barCell}>
                  <span className={styles.bar} style={{ inlineSize: `${String((row.minutes / most) * 100)}%` }} aria-hidden="true" />
                </td>
                <td className={classes("num", styles.barCount)}>{formatDuration(row.minutes, units)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  );
}

function Versions({ value }: { value: WidgetValue }) {
  const t = useT();
  const { locale } = useLocale();
  const percent = new Intl.NumberFormat(locale, { style: "percent" });
  const versions = value.versions ?? [];
  if (versions.length === 0) {
    return <p className={classes("muted", styles.quiet)}>{t("widget-no-versions")}</p>;
  }
  return (
    <ul className={styles.rows}>
      {versions.map((version) => (
        <li key={version.id} className={styles.version}>
          <span className={styles.rowTitle}>{version.name}</span>
          <div className="meter" role="progressbar" aria-label={t("widget-version-label", { name: version.name })} aria-valuemin={0} aria-valuemax={100} aria-valuenow={version.progress.percent}>
            <i style={{ inlineSize: `${String(version.progress.percent)}%` }} />
          </div>
          <span className="muted num">{t("widget-version-progress", { closed: version.progress.closed, total: version.progress.total, percent: percent.format(version.progress.percent / 100) })}</span>
        </li>
      ))}
    </ul>
  );
}

function Changes({ value, onOpenTask }: { value: WidgetValue; onOpenTask: (key: string) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const moment = useMomentFormat();
  const entries = value.activity ?? [];
  if (entries.length === 0) {
    return <p className={classes("muted", styles.quiet)}>{t("widget-no-activity")}</p>;
  }
  return (
    <ul className={styles.rows}>
      {entries.map((entry) => {
        const change = describeChange(t, entry, locale);
        const task = entry.task;
        return (
          <li key={`${String(entry.seq)}-${entry.id}`} className={styles.change}>
            <span className={styles.changeWhat}>
              <span>{entry.actor.kind === "user" ? t("comment-me") : entry.actor.name}</span>
              <span className="muted">{change.what}</span>
              {task !== undefined ? (
                <Button
                  className="linkbtn"
                  onPress={() => {
                    onOpenTask(task.key);
                  }}
                >
                  <span className="key">{task.key}</span> {task.title}
                </Button>
              ) : (
                change.item !== undefined && <span>{change.item}</span>
              )}
            </span>
            <time className={classes("muted num", styles.rowNote)} dateTime={entry.at}>
              {moment(entry.at)}
            </time>
          </li>
        );
      })}
    </ul>
  );
}

function Failures({ value, onOpenTask }: { value: WidgetValue; onOpenTask: (key: string) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const runs = value.runs ?? [];
  if (runs.length === 0) {
    return <p className={classes("muted", styles.quiet)}>{t("widget-no-failures")}</p>;
  }
  return (
    <ul className={styles.rows}>
      {runs.map((run) => {
        const task = run.task;
        return (
          <li key={run.id} className={styles.change}>
            <span className={styles.changeWhat}>
              <span>{t("widget-run", { rule: run.rule_name, message: run.error === undefined ? "" : failureText(t, run.error) })}</span>
              {task !== undefined && (
                <Button
                  className="linkbtn"
                  onPress={() => {
                    onOpenTask(task);
                  }}
                >
                  <span className="key">{task}</span>
                </Button>
              )}
            </span>
            <time className={classes("muted num", styles.rowNote)} dateTime={run.at}>
              {moment(run.at)}
            </time>
          </li>
        );
      })}
    </ul>
  );
}

/** A note, rendered by the engine like any Markdown text. */
function Note({ id, text, title, onOpenTask }: { id: string; text: string; title: string; onOpenTask: (key: string) => void }) {
  const rendered = useOp("markdown.render", { documents: [{ source: text, document_id: id }] });
  const html = rendered.data?.items[0]?.html;
  if (html === undefined) {
    return null;
  }
  return <MarkdownReader html={html} onOpenTask={onOpenTask} compact label={title} />;
}
