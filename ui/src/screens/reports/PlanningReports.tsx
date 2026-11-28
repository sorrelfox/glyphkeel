// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Download } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { formatDuration } from "../../data/duration";
import { errorToast, failureText, useCall, useOp } from "../../data/hooks";
import { forecastLine, loadMeasure, ON_TARGET_BINS, ratioBins, saveMeasure, type MeasureChoice } from "../../data/planning";
import { useDurationUnits } from "../../data/time";
import { useToday } from "../../data/zone";
import { type BurnupReport, type EstimateGroup, type EstimateGroupBy, type EstimatesReport, type EstimateTask, type PlanMeasure, type VelocityReport } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { QueryInput } from "../list/QueryInput";
import { BurnupChart, COMMITTED, COMPLETED, AVERAGE, DashedSwatch, HistogramChart, SCOPE, Swatch, VelocityChart } from "./charts";
import { daysBetween } from "./chartMath";
import styles from "./FlowReports.module.css";

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

function addDays(day: string, days: number): string {
  const d = utc(day);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Fluent marks placeables with invisible isolation characters; a file name must not carry them. */
function plain(text: string): string {
  return text.replace(/[⁨⁩]/g, "");
}

/** The most days a burn-up chart runs past its last day, to show the end and the forecast. */
const AHEAD_MAX = 120;

/** How work is written in a measure: a count of tasks, hours, or the field's number. */
export function useMeasureFormat(measure: PlanMeasure): (n: number) => string {
  const { locale } = useLocale();
  if (measure === "estimate") {
    const hours = new Intl.NumberFormat(locale, { style: "unit", unit: "hour", unitDisplay: "short", maximumFractionDigits: 1 });
    return (n) => hours.format(n);
  }
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: measure === "tasks" ? 0 : 2 });
  return (n) => number.format(n);
}

/** A ratio of spent to estimated time, such as 1.4×. */
export function useRatioFormat(): (n: number | undefined) => string {
  const t = useT();
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 2 });
  return (n) => (n === undefined ? t("flow-no-value") : t("plan-ratio", { ratio: number.format(n) }));
}

function LegendItem({ colour, line = false, dashed = false, children }: { colour: string; line?: boolean; dashed?: boolean; children: ReactNode }) {
  return (
    <li>
      {dashed ? <DashedSwatch colour={colour} /> : <Swatch colour={colour} line={line} />}
      {children}
    </li>
  );
}

function exported(t: Translate, toasts: ReturnType<typeof useToasts>, file: string, csv: Promise<string | undefined>) {
  csv
    .then((text) => saveExport(`${plain(file)}.csv`, text ?? "", "text/csv"))
    .then((done) => {
      if (done) {
        toasts.show({ tone: "info", text: t("flow-exported") });
      }
    })
    .catch((error: unknown) => {
      toasts.show(errorToast(t, error));
    });
}

/** The measure's segmented choice and, for a field, the number field; both remembered per project. */
function MeasurePicker({ project, choice, onChange }: { project: string; choice: MeasureChoice; onChange: (next: MeasureChoice) => void }) {
  const t = useT();
  const fields = useOp("field.list", { project }, { enabled: project !== "" });
  const numbers = (fields.data?.fields ?? []).filter((field) => field.kind === "integer" || field.kind === "decimal");
  const first = numbers[0]?.query_name;
  return (
    <>
      <Segmented<PlanMeasure>
        label={t("plan-measure")}
        value={choice.measure}
        onChange={(measure) => {
          onChange(measure === "field" ? (first === undefined ? choice : { measure, field: choice.field ?? first }) : { measure });
        }}
        segments={[
          { id: "tasks", label: t("plan-measure-tasks") },
          { id: "estimate", label: t("plan-measure-estimate") },
          ...(numbers.length > 0 ? [{ id: "field" as const, label: t("plan-measure-field") }] : []),
        ]}
      />
      {choice.measure === "field" && numbers.length > 0 && (
        <Select
          className={styles.select}
          label={t("plan-field")}
          value={choice.field ?? first ?? ""}
          onChange={(field) => {
            onChange({ measure: "field", field });
          }}
          options={numbers.map((field) => ({ id: field.query_name, label: field.name }))}
        />
      )}
    </>
  );
}

function ProjectPicker({ value, onChange }: { value: string; onChange: (key: string) => void }) {
  const t = useT();
  const projects = useOp("project.list", {});
  return (
    <span data-plan-project>
      <Select
        className={styles.select}
        label={t("report-project")}
        value={value}
        onChange={onChange}
        options={(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))}
      />
    </span>
  );
}

/** The project a planning report starts on: the one asked for, else the first. */
function useStartProject(asked: string | undefined): [string, (key: string) => void] {
  const projects = useOp("project.list", {});
  const [chosen, setChosen] = useState<string | undefined>(asked);
  return [chosen ?? projects.data?.items[0]?.key ?? "", setChosen];
}

// ---------------------------------------------------------------------
// Velocity

/** The velocity tab: a project's completed sprints, committed against completed work. */
export function VelocityPanel({ project: asked, asTable }: { project: string | undefined; asTable: boolean }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const [project, setProject] = useStartProject(asked);
  const [choice, setChoice] = useState<MeasureChoice>(() => loadMeasure(project));
  const input = { project, measure: choice.measure, ...(choice.field === undefined ? {} : { field: choice.field }) };
  const report = useOp("report.velocity", input, { enabled: project !== "" });
  return (
    <>
      <div className={styles.controls}>
        <ProjectPicker
          value={project}
          onChange={(key) => {
            setProject(key);
            setChoice(loadMeasure(key));
          }}
        />
        <MeasurePicker
          project={project}
          choice={choice}
          onChange={(next) => {
            setChoice(next);
            saveMeasure(project, next);
          }}
        />
        <div className="grow" />
        <Button
          className="btn flat"
          isDisabled={!report.data}
          onPress={() => {
            exported(t, toasts, t("plan-file-velocity", { project }), call("report.velocity", { ...input, csv: true }).then((r) => r.csv));
          }}
        >
          <Download className="ic" aria-hidden="true" />
          <span>{t("flow-export")}</span>
        </Button>
      </div>
      {report.isError && (
        <p className="faint" role="alert" data-flow-error>
          {failureText(t, report.error)}
        </p>
      )}
      {report.data && <Velocity report={report.data} asTable={asTable} />}
    </>
  );
}

/** Velocity as a chart or a table; dashboards show it too. */
export function Velocity({ report, asTable }: { report: VelocityReport; asTable: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const format = useMeasureFormat(report.measure);
  const number = new Intl.NumberFormat(locale);
  const rows = report.sprints;
  const label = t("plan-chart-velocity", { project: report.project });
  if (rows.length === 0) {
    return <p className={classes("faint", styles.note)}>{t("plan-velocity-none")}</p>;
  }
  const summary =
    report.average === undefined
      ? t("plan-velocity-none")
      : t("plan-summary-velocity", { average: format(report.average), count: report.average_of });
  const upcoming = report.upcoming.find((each) => each.sprint.state === "planned") ?? report.upcoming[0];
  const footer = (
    <>
      {upcoming !== undefined && report.average !== undefined && (
        <p className={classes("muted", styles.note)} data-plan-hint>
          {t("plan-hint", { sprint: upcoming.sprint.name, planned: format(upcoming.planned), average: format(report.average) })}
        </p>
      )}
    </>
  );
  if (asTable) {
    return (
      <>
        <table className={styles.table} aria-label={label} data-flow-table="velocity">
          <thead>
            <tr>
              <th scope="col">{t("plan-sprint")}</th>
              <th scope="col" className={styles.number}>{t("plan-committed")}</th>
              <th scope="col" className={styles.number}>{t("plan-completed")}</th>
              <th scope="col" className={styles.number}>{t("plan-average")}</th>
              <th scope="col" className={styles.number}>{t("plan-tasks-done")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.sprint.id}>
                <th scope="row">{r.sprint.name}</th>
                <td className={classes("num", styles.number)}>{format(r.committed)}</td>
                <td className={classes("num", styles.number)}>{format(r.completed)}</td>
                <td className={classes("num", styles.number)}>{format(r.average)}</td>
                <td className={classes("num", styles.number)}>{t("plan-of", { done: number.format(r.completed_tasks), total: number.format(r.committed_tasks) })}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {footer}
      </>
    );
  }
  return (
    <>
      <VelocityChart
        rows={rows}
        labels={rows.map((r) => r.sprint.name)}
        label={label}
        summary={summary}
        format={format}
        describe={(i) => {
          const r = rows[i];
          return r ? t("plan-point-velocity", { sprint: r.sprint.name, committed: format(r.committed), completed: format(r.completed), average: format(r.average) }) : null;
        }}
        legend={
          <>
            <LegendItem colour={COMMITTED}>{t("plan-committed")}</LegendItem>
            <LegendItem colour={COMPLETED}>{t("plan-completed")}</LegendItem>
            <LegendItem colour={AVERAGE} line>
              {t("plan-average")}
            </LegendItem>
          </>
        }
      />
      {footer}
    </>
  );
}

// ---------------------------------------------------------------------
// Burn-up

/** The key of a version or sprint in the burn-up's choice. */
type Target = `version:${string}` | `sprint:${string}`;

/** The burn-up tab: a version's or a sprint's scope and completed work, day by day. */
export function BurnupPanel({ project: asked, version, sprint, asTable }: { project: string | undefined; version: string | undefined; sprint: string | undefined; asTable: boolean }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const [project, setProject] = useStartProject(asked);
  const versions = useOp("version.list", { project }, { enabled: project !== "" });
  const sprints = useOp("sprint.list", { project }, { enabled: project !== "" });
  const [chosen, setChosen] = useState<Target | null>(version !== undefined ? `version:${version}` : sprint !== undefined ? `sprint:${sprint}` : null);
  const [choice, setChoice] = useState<MeasureChoice>({ measure: "tasks" });
  const started = (sprints.data?.items ?? []).filter((each) => each.state !== "planned");
  const fallback: Target | null = versions.data?.items[0] ? `version:${versions.data.items[0].id}` : started[0] ? `sprint:${started[0].id}` : null;
  const target = chosen ?? fallback;
  const [kind, id] = target === null ? ["", ""] : (target.split(/:(.*)/s) as [string, string]);
  const input = { ...(kind === "version" ? { version: id } : { sprint: id }), project, measure: choice.measure, ...(choice.field === undefined ? {} : { field: choice.field }) };
  const report = useOp("report.burnup", input, { enabled: target !== null && project !== "" });
  const options = [
    ...(versions.data?.items ?? []).map((each) => ({ id: `version:${each.id}`, label: t("plan-target-version", { name: each.name }) })),
    ...started.map((each) => ({ id: `sprint:${each.id}`, label: t("plan-target-sprint", { name: each.name }) })),
  ];
  const known = options.some((each) => each.id === target) || versions.isPending || sprints.isPending;
  return (
    <>
      <div className={styles.controls}>
        <ProjectPicker
          value={project}
          onChange={(key) => {
            setProject(key);
            setChosen(null);
            setChoice({ measure: "tasks" });
          }}
        />
        {options.length > 0 && (
          <Select<string>
            className={styles.select}
            label={t("plan-target")}
            value={known && target !== null ? target : (options[0]?.id ?? "")}
            onChange={(next) => {
              setChosen(next as Target);
            }}
            options={options}
          />
        )}
        <MeasurePicker project={project} choice={choice} onChange={setChoice} />
        <div className="grow" />
        <Button
          className="btn flat"
          isDisabled={!report.data}
          onPress={() => {
            const name = report.data?.version?.name ?? report.data?.sprint?.name ?? "";
            exported(t, toasts, t("plan-file-burnup", { name }), call("report.burnup", { ...input, csv: true }).then((r) => r.csv));
          }}
        >
          <Download className="ic" aria-hidden="true" />
          <span>{t("flow-export")}</span>
        </Button>
      </div>
      {options.length === 0 && versions.data && sprints.data && <p className={classes("faint", styles.note)}>{t("plan-burnup-nothing")}</p>}
      {report.isError && (
        <p className="faint" role="alert" data-flow-error>
          {failureText(t, report.error)}
        </p>
      )}
      {report.data && <Burnup report={report.data} asTable={asTable} />}
    </>
  );
}

/** A burn-up as a chart or a table; dashboards, the roadmap and the sprints page show it too. */
export function Burnup({ report, asTable, heading }: { report: BurnupReport; asTable: boolean; heading?: ReactNode }) {
  const t = useT();
  const { locale } = useLocale();
  const format = useMeasureFormat(report.measure);
  const shortDate = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  const longDate = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });
  const name = report.version?.name ?? report.sprint?.name ?? "";
  const label = t("plan-chart-burnup", { name });
  const days = report.days;
  const last = days.at(-1);
  // The chart runs on to the planned end and the forecast day, within reason.
  const ends = [report.end, report.forecast?.date].flatMap((d) => (d === undefined ? [] : [d]));
  const until = ends.reduce((max, d) => (d > max ? d : max), report.to);
  const ahead = Math.min(AHEAD_MAX, Math.max(0, daysBetween(report.to, until)));
  const dates = [...days.map((d) => d.date), ...Array.from({ length: ahead }, (_v, i) => addDays(report.to, i + 1))];
  const forecastText = report.forecast === undefined ? null : t("plan-forecast", { date: longDate.format(utc(report.forecast.date)) });
  const summary =
    last === undefined
      ? t("plan-burnup-empty")
      : report.forecast === undefined
        ? t("plan-summary-burnup", { completed: format(last.completed), scope: format(last.scope) })
        : t("plan-summary-burnup-forecast", { completed: format(last.completed), scope: format(last.scope), date: longDate.format(utc(report.forecast.date)) });
  if (asTable) {
    return (
      <>
        {heading}
        <table className={styles.table} aria-label={label} data-flow-table="burnup">
          <thead>
            <tr>
              <th scope="col">{t("burndown-day-column")}</th>
              <th scope="col" className={styles.number}>{t("plan-scope")}</th>
              <th scope="col" className={styles.number}>{t("plan-completed")}</th>
            </tr>
          </thead>
          <tbody>
            {days.map((d) => (
              <tr key={d.date}>
                <th scope="row" className="num">{longDate.format(utc(d.date))}</th>
                <td className={classes("num", styles.number)}>{format(d.scope)}</td>
                <td className={classes("num", styles.number)}>{format(d.completed)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {forecastText !== null && (
          <p className={classes("muted", styles.note)} data-plan-forecast>
            {forecastText}
          </p>
        )}
      </>
    );
  }
  return (
    <>
      {heading}
      <BurnupChart
        scope={dates.map((_d, i) => days[i]?.scope)}
        completed={dates.map((_d, i) => days[i]?.completed)}
        forecast={forecastLine(report.from, report.forecast, last?.scope ?? 0, dates.length)}
        labels={dates.map((d) => shortDate.format(utc(d)))}
        label={label}
        summary={summary}
        format={format}
        describe={(i) => {
          const date = dates[i];
          const d = days[i];
          if (date === undefined) {
            return null;
          }
          return d === undefined
            ? t("plan-point-burnup-ahead", { date: longDate.format(utc(date)) })
            : t("plan-point-burnup", { date: longDate.format(utc(date)), completed: format(d.completed), scope: format(d.scope) });
        }}
        legend={
          <>
            <LegendItem colour={SCOPE} line>
              {t("plan-scope")}
            </LegendItem>
            <LegendItem colour={COMPLETED} line>
              {t("plan-completed")}
            </LegendItem>
            {report.forecast !== undefined && (
              <LegendItem colour={AVERAGE} dashed>
                {t("plan-forecast-line")}
              </LegendItem>
            )}
          </>
        }
      />
      {forecastText !== null && (
        <p className={classes("muted", styles.note)} data-plan-forecast>
          {forecastText}
        </p>
      )}
    </>
  );
}

// ---------------------------------------------------------------------
// Estimate accuracy

const GROUPS: readonly EstimateGroupBy[] = ["none", "type", "label", "project", "work_kind", "month"];
const RANGES = [30, 90, 365] as const;

/** The estimate accuracy tab: closed tasks' logged time against their estimates. */
export function EstimatesPanel({ project: asked, asTable }: { project: string | undefined; asTable: boolean }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const today = useToday();
  const projects = useOp("project.list", {});
  const [project, setProject] = useState(asked ?? "");
  const [range, setRange] = useState({ from: addDays(today, -89), to: today });
  const [groupBy, setGroupBy] = useState<EstimateGroupBy>("type");
  const [queryText, setQueryText] = useState("");
  const [query, setQuery] = useState("");
  const input = { ...range, group_by: groupBy, tasks: true, ...(project === "" ? {} : { project }), ...(query === "" ? {} : { query }) };
  const report = useOp("report.estimates", input);
  const setDay = (which: "from" | "to", value: string) => {
    if (value !== "") {
      setRange(which === "from" ? { from: value, to: range.to < value ? value : range.to } : { from: range.from > value ? value : range.from, to: value });
    }
  };
  return (
    <>
      <div className={styles.controls}>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-from")} value={range.from} onChange={(event) => { setDay("from", event.target.value); }} />
        </span>
        <span className="muted" aria-hidden="true">
          –
        </span>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-to")} value={range.to} onChange={(event) => { setDay("to", event.target.value); }} />
        </span>
        {RANGES.map((days) => (
          <Button
            key={days}
            className="btn flat sm"
            onPress={() => {
              setRange({ from: addDays(today, -(days - 1)), to: today });
            }}
          >
            {t("plan-last-days", { days })}
          </Button>
        ))}
      </div>
      <div className={styles.controls}>
        <Select
          className={styles.select}
          label={t("report-project")}
          value={project === "" ? "*" : project}
          onChange={(key) => {
            setProject(key === "*" ? "" : key);
          }}
          options={[{ id: "*", label: t("report-all-projects") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))]}
        />
        <Select<EstimateGroupBy>
          className={styles.select}
          label={t("report-group-by")}
          value={groupBy}
          onChange={setGroupBy}
          options={GROUPS.map((each) => ({ id: each, label: t(`plan-group-${each}`) }))}
        />
        <div className={styles.queryField}>
          <QueryInput
            project={project === "" ? undefined : project}
            value={queryText}
            onChange={(text) => {
              setQueryText(text);
              if (text.trim() === "") {
                setQuery("");
              }
            }}
            onSubmit={() => {
              setQuery(queryText.trim());
            }}
            label={t("flow-query")}
            placeholder={t("flow-query-placeholder")}
          />
        </div>
        <Button
          className="btn flat"
          isDisabled={!report.data}
          onPress={() => {
            exported(t, toasts, t("plan-file-estimates", { from: range.from, to: range.to }), call("report.estimates", { ...input, csv: true }).then((r) => r.csv));
          }}
        >
          <Download className="ic" aria-hidden="true" />
          <span>{t("flow-export")}</span>
        </Button>
      </div>
      {report.isError && (
        <p className="faint" role="alert" data-flow-error>
          {failureText(t, report.error)}
        </p>
      )}
      {report.data && <Estimates report={report.data} asTable={asTable} />}
    </>
  );
}

function groupName(t: Translate, report: EstimatesReport, group: EstimateGroup): string {
  if (group.name !== "") {
    return group.name;
  }
  return t(`plan-group-none-${report.group_by}`);
}

/** Estimate accuracy: the figures, the ratio histogram or its table, the groups and the outliers; `compact` shows the figures and groups only (dashboards). */
export function Estimates({ report, asTable, compact = false }: { report: EstimatesReport; asTable: boolean; compact?: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const units = useDurationUnits();
  const ratio = useRatioFormat();
  const number = new Intl.NumberFormat(locale);
  const percent = new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 });
  const total = report.total;
  const label = t("plan-chart-estimates", { from: report.from, to: report.to });
  const left = report.without_estimate + report.without_time;
  const excluded = left > 0 ? <p className={classes("faint", styles.note)}>{t("plan-excluded", { estimate: report.without_estimate, time: report.without_time })}</p> : null;
  if (total.count === 0) {
    return (
      <>
        <p className={classes("faint", styles.note)}>{t("plan-estimates-none")}</p>
        {excluded}
      </>
    );
  }
  const figures: [string, string][] = [
    ["plan-stat-tasks", number.format(total.count)],
    ["plan-stat-median", ratio(total.median_ratio)],
    ["plan-stat-within", total.within_share === undefined ? t("flow-no-value") : percent.format(total.within_share)],
    ["plan-stat-over", number.format(total.over)],
    ["plan-stat-under", number.format(total.under)],
    ["plan-stat-estimated", formatDuration(total.estimate_minutes, units)],
    ["plan-stat-spent", formatDuration(total.spent_minutes, units)],
  ];
  const groups = report.groups;
  const groupTable =
    groups.length === 0 ? null : (
      <table className={styles.table} aria-label={t("plan-groups", { by: t(`plan-group-${report.group_by}`) })} data-flow-table="estimate-groups">
        <thead>
          <tr>
            <th scope="col">{t(`plan-group-${report.group_by}`)}</th>
            <th scope="col" className={styles.number}>{t("plan-stat-tasks")}</th>
            <th scope="col" className={styles.number}>{t("plan-stat-median")}</th>
            <th scope="col" className={styles.number}>{t("plan-stat-within")}</th>
            {!compact && <th scope="col" className={styles.number}>{t("plan-stat-estimated")}</th>}
            {!compact && <th scope="col" className={styles.number}>{t("plan-stat-spent")}</th>}
          </tr>
        </thead>
        <tbody>
          {groups.map((g) => (
            <tr key={g.key} data-group={g.key}>
              <th scope="row">{groupName(t, report, g)}</th>
              <td className={classes("num", styles.number)}>{number.format(g.count)}</td>
              <td className={classes("num", styles.number)}>{ratio(g.median_ratio)}</td>
              <td className={classes("num", styles.number)}>{g.within_share === undefined ? t("flow-no-value") : percent.format(g.within_share)}</td>
              {!compact && <td className={classes("num", styles.number)}>{formatDuration(g.estimate_minutes, units)}</td>}
              {!compact && <td className={classes("num", styles.number)}>{formatDuration(g.spent_minutes, units)}</td>}
            </tr>
          ))}
        </tbody>
      </table>
    );
  const stats = (
    <dl className={styles.figures} data-plan-figures>
      {(compact ? figures.slice(0, 3) : figures).map(([id, value]) => (
        <div key={id} className={styles.figure2} data-stat={id}>
          <dt className="muted">{t(id)}</dt>
          <dd className="num">{value}</dd>
        </div>
      ))}
    </dl>
  );
  if (compact) {
    return (
      <>
        {stats}
        {groupTable}
      </>
    );
  }
  const tasks = report.tasks ?? total.outliers;
  const bins = ratioBins(tasks);
  const binNames = ["plan-bin-1", "plan-bin-2", "plan-bin-3", "plan-bin-4", "plan-bin-5", "plan-bin-6"].map((id) => t(id));
  const openTask = (task: EstimateTask) => void navigate({ to: "/open/$key", params: { key: task.key } });
  return (
    <>
      {stats}
      {excluded}
      {asTable ? (
        <table className={styles.table} aria-label={label} data-flow-table="estimates">
          <thead>
            <tr>
              <th scope="col">{t("plan-ratio-range")}</th>
              <th scope="col" className={styles.number}>{t("plan-stat-tasks")}</th>
            </tr>
          </thead>
          <tbody>
            {bins.map((n, i) => (
              <tr key={binNames[i]}>
                <th scope="row">{binNames[i]}</th>
                <td className={classes("num", styles.number)}>{number.format(n)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <HistogramChart
          counts={bins}
          marked={ON_TARGET_BINS}
          labels={binNames}
          label={label}
          summary={t("plan-summary-estimates", { count: total.count, median: ratio(total.median_ratio), within: total.within_share === undefined ? t("flow-no-value") : percent.format(total.within_share) })}
          describe={(i) => t("plan-point-bin", { range: binNames[i] ?? "", count: bins[i] ?? 0 })}
          legend={
            <>
              <LegendItem colour={COMPLETED}>{t("plan-on-target")}</LegendItem>
              <LegendItem colour={AVERAGE}>{t("plan-off-target")}</LegendItem>
            </>
          }
        />
      )}
      {groupTable}
      <h3 className={styles.statsTitle}>{t("plan-outliers")}</h3>
      <table className={styles.table} data-flow-table="outliers" aria-label={t("plan-outliers")}>
        <thead>
          <tr>
            <th scope="col">{t("flow-task")}</th>
            <th scope="col">{t("plan-type")}</th>
            <th scope="col" className={styles.number}>{t("plan-stat-estimated")}</th>
            <th scope="col" className={styles.number}>{t("plan-stat-spent")}</th>
            <th scope="col" className={styles.number}>{t("plan-ratio-column")}</th>
          </tr>
        </thead>
        <tbody>
          {total.outliers.map((task) => (
            <tr key={task.key} data-outlier={task.key}>
              <td>
                <Button
                  className="linkbtn"
                  onPress={() => {
                    openTask(task);
                  }}
                >
                  <span className={classes("key", styles.rowKey)}>{task.key}</span>
                  {task.title}
                </Button>
              </td>
              <td>{task.type}</td>
              <td className={classes("num", styles.number)}>{formatDuration(task.estimate_minutes, units)}</td>
              <td className={classes("num", styles.number)}>{formatDuration(task.spent_minutes, units)}</td>
              <td className={classes("num", styles.number)}>{ratio(task.ratio)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
