// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Download } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Tab, TabList, TabPanel, Tabs, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { errorToast, failureText, useCall, useOp } from "../../data/hooks";
import { useToday } from "../../data/zone";
import {
  type Category,
  type CreatedClosedReport,
  type CumulativeFlowReport,
  type CycleTimeReport,
  type FlowBand,
  type FlowBucket,
  type FlowGroupBy,
  type FlowReportInput,
  type FlowStats,
  type WeekStart,
} from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { isoDay } from "../board/model";
import { presetRange, startOfWeek } from "../time/TimeReport";
import { bandColour, CLOSED, CREATED, CreatedClosedChart, CumulativeFlowChart, CycleTimeChart, OPEN, Swatch } from "./charts";
import { QueryInput } from "../list/QueryInput";
import styles from "./FlowReports.module.css";

/** The list's key for "all projects", "any type", "any label" and "any version"; no name has this form. */
const ANY = "*";
/** The most tasks the cycle time table lists; the export has them all. */
const TASK_ROWS = 500;

type Tab = "created-closed" | "cumulative" | "cycle";
const TABS: readonly Tab[] = ["created-closed", "cumulative", "cycle"];
type Actor = "" | "me" | "agent";
type Metric = "cycle" | "lead";

const RANGES = ["last-30-days", "last-12-weeks", "last-12-months", "this-month", "last-month", "this-year"] as const;
type Range = (typeof RANGES)[number];

function addDays(day: string, days: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return isoDay(new Date(y ?? 1970, (m ?? 1) - 1, (d ?? 1) + days));
}

/** A preset's days, and the period length that suits it. */
export function flowRange(range: Range, today: string, weekStart: WeekStart): { from: string; to: string; bucket: FlowBucket } {
  const [y, m] = today.split("-").map(Number);
  switch (range) {
    case "last-30-days":
      return { from: addDays(today, -29), to: today, bucket: "day" };
    case "last-12-weeks":
      return { from: addDays(startOfWeek(today, weekStart), -77), to: today, bucket: "week" };
    case "last-12-months":
      return { from: isoDay(new Date(y ?? 1970, (m ?? 1) - 12, 1)), to: today, bucket: "month" };
    case "this-month":
    case "last-month": {
      const [from, to] = presetRange(range, today, weekStart);
      return { from, to, bucket: "week" };
    }
    case "this-year": {
      const [from, to] = presetRange(range, today, weekStart);
      return { from, to, bucket: "month" };
    }
  }
}

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

/** Fluent marks placeables with invisible isolation characters; a file name must not carry them. */
function plain(text: string): string {
  return text.replace(/[⁨⁩]/g, "");
}

/** How a period is named: short on the axis, in full in tables and read-outs. */
export function usePeriodNames(bucket: FlowBucket): { short: (start: string) => string; full: (start: string, end: string) => string } {
  const { locale } = useLocale();
  const day = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  const month = new Intl.DateTimeFormat(locale, { month: "short", year: "numeric", timeZone: "UTC" });
  const longDay = new Intl.DateTimeFormat(locale, { weekday: "short", month: "short", day: "numeric", year: "numeric", timeZone: "UTC" });
  return {
    short: (start) => (bucket === "month" ? month.format(utc(start)) : day.format(utc(start))),
    full: (start, end) => {
      if (bucket === "day" || start === end) {
        return longDay.format(utc(start));
      }
      if (bucket === "month" && start.endsWith("-01") && end.slice(0, 7) === start.slice(0, 7)) {
        return new Intl.DateTimeFormat(locale, { month: "long", year: "numeric", timeZone: "UTC" }).format(utc(start));
      }
      return day.formatRange(utc(start), utc(end));
    },
  };
}

/** A band's name: the status as written, or its category's name. */
function bandName(t: Translate, band: FlowBand, groupBy: FlowGroupBy): string {
  if (groupBy === "status") {
    return band.name;
  }
  const ids: Record<Category, string> = { todo: "flow-category-todo", in_progress: "flow-category-in-progress", done: "flow-category-done", cancelled: "flow-category-cancelled" };
  return t(ids[band.category]);
}

/**
 * The flow reports, from the tasks' history: tasks created against tasks
 * closed, the cumulative flow by status, and cycle and lead time, per day,
 * week or month in the person's time zone; for all projects or one (with or
 * without its sub-projects), a type, a label, a version or a query (custom
 * fields as `cf.<name>`); by anyone, people
 * or agents. Each chart reads by keyboard and has its table; each report
 * saves as CSV.
 */
export function FlowReportsScreen() {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const settings = useOp("settings.get", {});
  const projects = useOp("project.list", {});
  const today = useToday();
  const weekStart = settings.data?.week_start ?? "monday";
  const [tab, setTab] = useState<Tab>("created-closed");
  const [range, setRange] = useState<{ from: string; to: string } | null>(null);
  const [bucket, setBucket] = useState<FlowBucket>("week");
  const [project, setProject] = useState("");
  const [subprojects, setSubprojects] = useState(true);
  const [taskType, setTaskType] = useState("");
  const [label, setLabel] = useState("");
  const [version, setVersion] = useState("");
  const [queryText, setQueryText] = useState("");
  const [query, setQuery] = useState("");
  const [actor, setActor] = useState<Actor>("");
  const [groupBy, setGroupBy] = useState<FlowGroupBy>("status");
  const [metric, setMetric] = useState<Metric>("cycle");
  const [asTable, setAsTable] = useState(false);
  const chosenProject = useOp("project.get", { project }, { enabled: project !== "" });
  const chosenVersions = useOp("version.list", { project }, { enabled: project !== "" });

  const input: FlowReportInput = {
    bucket,
    ...(range ?? {}),
    ...(project === "" ? {} : { project, include_subprojects: subprojects }),
    ...(taskType === "" ? {} : { type: taskType }),
    ...(label === "" ? {} : { label }),
    ...(version === "" ? {} : { fix_version: version }),
    ...(query === "" ? {} : { query }),
  };
  const withActor: FlowReportInput = { ...input, ...(actor === "" ? {} : { actor }) };
  const created = useOp("report.created_closed", withActor, { enabled: tab === "created-closed" });
  const flow = useOp("report.cumulative_flow", { ...input, group_by: groupBy }, { enabled: tab === "cumulative" });
  const cycle = useOp("report.cycle_time", { ...withActor, tasks: true }, { enabled: tab === "cycle" });
  const current = tab === "created-closed" ? created : tab === "cumulative" ? flow : cycle;
  const from = range?.from ?? current.data?.from ?? today;
  const to = range?.to ?? current.data?.to ?? today;

  const setDay = (which: "from" | "to", value: string) => {
    if (value === "") {
      return;
    }
    setRange(which === "from" ? { from: value, to: to < value ? value : to } : { from: from > value ? value : from, to: value });
  };
  const exportCsv = () => {
    const answer =
      tab === "created-closed"
        ? call("report.created_closed", { ...withActor, csv: true })
        : tab === "cumulative"
          ? call("report.cumulative_flow", { ...input, group_by: groupBy, csv: true })
          : call("report.cycle_time", { ...withActor, csv: true });
    const file = tab === "created-closed" ? "flow-file-created-closed" : tab === "cumulative" ? "flow-file-cumulative" : "flow-file-cycle";
    answer
      .then((report) => saveExport(`${plain(t(file, { from: report.from, to: report.to }))}.csv`, report.csv ?? "", "text/csv"))
      .then((done) => {
        if (done) {
          toasts.show({ tone: "info", text: t("flow-exported") });
        }
      })
      .catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
  };

  const types = [...new Set((chosenProject.data?.types ?? []).map((each) => each.name))];
  const labels = [...new Set((chosenProject.data?.labels ?? []).map((each) => each.name))];
  const versions = [...new Set((chosenVersions.data?.items ?? []).map((each) => each.name))];
  const tabLabel: Record<Tab, string> = { "created-closed": "flow-tab-created-closed", cumulative: "flow-tab-cumulative", cycle: "flow-tab-cycle" };

  return (
    <section className={styles.screen} aria-label={t("view-reports")}>
      <div className={styles.controls}>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-from")} value={from} onChange={(event) => { setDay("from", event.target.value); }} />
        </span>
        <span className="muted" aria-hidden="true">
          –
        </span>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-to")} value={to} onChange={(event) => { setDay("to", event.target.value); }} />
        </span>
        {RANGES.map((each) => (
          <Button
            key={each}
            className="btn flat sm"
            onPress={() => {
              const chosen = flowRange(each, today, weekStart);
              setRange({ from: chosen.from, to: chosen.to });
              setBucket(chosen.bucket);
            }}
          >
            {t(each.startsWith("last-") && each !== "last-month" ? `flow-${each}` : `report-${each}`)}
          </Button>
        ))}
      </div>
      <div className={styles.controls}>
        <Segmented
          label={t("flow-bucket")}
          value={bucket}
          onChange={setBucket}
          segments={[
            { id: "day", label: t("flow-bucket-day") },
            { id: "week", label: t("flow-bucket-week") },
            { id: "month", label: t("flow-bucket-month") },
          ]}
        />
        <Select
          className={styles.select}
          label={t("report-project")}
          value={project === "" ? ANY : project}
          onChange={(key) => {
            setProject(key === ANY ? "" : key);
            setTaskType("");
            setLabel("");
            setVersion("");
          }}
          options={[{ id: ANY, label: t("report-all-projects") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))]}
        />
        {project !== "" && (
          <Switch isSelected={subprojects} onChange={setSubprojects}>
            {t("report-subprojects")}
          </Switch>
        )}
        {project !== "" && types.length > 0 && (
          <Select
            className={styles.select}
            label={t("flow-type")}
            value={taskType === "" ? ANY : taskType}
            onChange={(key) => {
              setTaskType(key === ANY ? "" : key);
            }}
            options={[{ id: ANY, label: t("flow-any-type") }, ...types.map((name) => ({ id: name, label: name }))]}
          />
        )}
        {project !== "" && labels.length > 0 && (
          <Select
            className={styles.select}
            label={t("flow-label")}
            value={label === "" ? ANY : label}
            onChange={(key) => {
              setLabel(key === ANY ? "" : key);
            }}
            options={[{ id: ANY, label: t("flow-any-label") }, ...labels.map((name) => ({ id: name, label: name }))]}
          />
        )}
        {project !== "" && versions.length > 0 && (
          <Select
            className={styles.select}
            label={t("flow-version")}
            value={version === "" ? ANY : version}
            onChange={(key) => {
              setVersion(key === ANY ? "" : key);
            }}
            options={[{ id: ANY, label: t("flow-any-version") }, ...versions.map((name) => ({ id: name, label: name }))]}
          />
        )}
        <div className={styles.queryField} data-flow-query>
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
      </div>
      <Tabs
        selectedKey={tab}
        onSelectionChange={(key) => {
          setTab(TABS.find((each) => each === key) ?? "created-closed");
        }}
        className={classes(styles.tabs)}
      >
        <div className={styles.tabBar}>
          <TabList className="tabs" aria-label={t("flow-tabs")}>
            {TABS.map((each) => (
              <Tab key={each} id={each} className={({ isSelected }) => classes("tab", isSelected && "on")}>
                {t(tabLabel[each])}
              </Tab>
            ))}
          </TabList>
          <div className="grow" />
          {tab !== "cumulative" && (
            <Select
              className={styles.select}
              label={t("flow-actor")}
              value={actor === "" ? ANY : actor}
              onChange={(key) => {
                setActor(key === "me" || key === "agent" ? key : "");
              }}
              options={[
                { id: ANY, label: t("flow-actor-anyone") },
                { id: "me", label: t("flow-actor-people") },
                { id: "agent", label: t("flow-actor-agents") },
              ]}
            />
          )}
          {tab === "cumulative" && (
            <Segmented
              label={t("flow-group-by")}
              value={groupBy}
              onChange={setGroupBy}
              segments={[
                { id: "status", label: t("flow-by-status") },
                { id: "category", label: t("flow-by-category") },
              ]}
            />
          )}
          {tab === "cycle" && (
            <Segmented
              label={t("flow-metric")}
              value={metric}
              onChange={setMetric}
              segments={[
                { id: "cycle", label: t("flow-cycle-time") },
                { id: "lead", label: t("flow-lead-time") },
              ]}
            />
          )}
          <Switch isSelected={asTable} onChange={setAsTable}>
            {t("flow-as-table")}
          </Switch>
          <Button className="btn flat" isDisabled={!current.data} onPress={exportCsv}>
            <Download className="ic" aria-hidden="true" />
            <span>{t("flow-export")}</span>
          </Button>
        </div>
        {current.isError && (
          <p className="faint" role="alert" data-flow-error>
            {failureText(t, current.error)}
          </p>
        )}
        <TabPanel id="created-closed" className={classes(styles.panel)}>
          {created.data && <CreatedClosed report={created.data} asTable={asTable} />}
        </TabPanel>
        <TabPanel id="cumulative" className={classes(styles.panel)}>
          {flow.data && <CumulativeFlow report={flow.data} asTable={asTable} />}
        </TabPanel>
        <TabPanel id="cycle" className={classes(styles.panel)}>
          {cycle.data && <CycleTime report={cycle.data} asTable={asTable} metric={metric} />}
        </TabPanel>
      </Tabs>
    </section>
  );
}

function LegendItem({ colour, line = false, children }: { colour: string; line?: boolean; children: ReactNode }) {
  return (
    <li>
      <Swatch colour={colour} line={line} />
      {children}
    </li>
  );
}

/** Tasks created against closed, as a chart or a table; dashboards show it too. */
export function CreatedClosed({ report, asTable }: { report: CreatedClosedReport; asTable: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const names = usePeriodNames(report.bucket);
  const rows = report.items;
  const last = rows.at(-1);
  const summary = t("flow-summary-created-closed", { created: report.created, closed: report.closed, open: last?.open ?? report.open_before });
  const describe = (i: number) => {
    const r = rows[i];
    return r ? t("flow-point-created-closed", { period: names.full(r.start, r.end), created: r.created, closed: r.closed, reopened: r.reopened, open: r.open }) : null;
  };
  if (asTable) {
    return (
      <table className={styles.table} aria-label={t("flow-chart-created-closed", { from: report.from, to: report.to })} data-flow-table="created-closed">
        <thead>
          <tr>
            <th scope="col">{t("flow-period")}</th>
            <th scope="col" className={styles.number}>{t("flow-created")}</th>
            <th scope="col" className={styles.number}>{t("flow-closed")}</th>
            <th scope="col" className={styles.number}>{t("flow-reopened")}</th>
            <th scope="col" className={styles.number}>{t("flow-open")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.start}>
              <th scope="row" className="num">{names.full(r.start, r.end)}</th>
              <td className={classes("num", styles.number)}>{number.format(r.created)}</td>
              <td className={classes("num", styles.number)}>{number.format(r.closed)}</td>
              <td className={classes("num", styles.number)}>{number.format(r.reopened)}</td>
              <td className={classes("num", styles.number)}>{number.format(r.open)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">{t("flow-total")}</th>
            <td className={classes("num", styles.number)}>{number.format(report.created)}</td>
            <td className={classes("num", styles.number)}>{number.format(report.closed)}</td>
            <td className={classes("num", styles.number)}>{number.format(report.reopened)}</td>
            <td className={classes("num", styles.number)}>{number.format(last?.open ?? 0)}</td>
          </tr>
        </tfoot>
      </table>
    );
  }
  return (
    <>
      <CreatedClosedChart
        rows={rows}
        labels={rows.map((r) => names.short(r.start))}
        label={t("flow-chart-created-closed", { from: report.from, to: report.to })}
        summary={summary}
        describe={describe}
        legend={
          <>
            <LegendItem colour={CREATED}>{t("flow-created")}</LegendItem>
            <LegendItem colour={CLOSED}>{t("flow-closed")}</LegendItem>
            <LegendItem colour={OPEN} line>
              {t("flow-open")}
            </LegendItem>
          </>
        }
      />
      {report.open_before > 0 && <p className={classes("faint", styles.note)}>{t("flow-before", { count: report.open_before })}</p>}
    </>
  );
}

/** The tasks in each status per period, as a chart or a table; dashboards show it too. */
export function CumulativeFlow({ report, asTable }: { report: CumulativeFlowReport; asTable: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const names = usePeriodNames(report.bucket);
  const rows = report.items;
  const total = (counts: readonly number[]) => counts.reduce((sum, n) => sum + n, 0);
  const name = (band: FlowBand) => bandName(t, band, report.group_by);
  const summary = t("flow-summary-cumulative", { count: total(rows.at(-1)?.counts ?? []) });
  const describe = (i: number) => {
    const r = rows[i];
    if (!r) {
      return null;
    }
    return (
      <>
        <span>{t("flow-point-cumulative", { period: names.full(r.start, r.end), count: total(r.counts) })}</span>
        {report.bands.map((band, b) => (
          <span key={band.key} className={styles.readoutItem}>
            <Swatch colour={bandColour(band)} />
            {t("flow-point-band", { name: name(band), count: r.counts[b] ?? 0 })}
          </span>
        ))}
      </>
    );
  };
  if (rows.every((r) => total(r.counts) === 0)) {
    return <p className={classes("faint", styles.note)}>{t("flow-none")}</p>;
  }
  if (asTable) {
    return (
      <table className={styles.table} aria-label={t("flow-chart-cumulative", { from: report.from, to: report.to })} data-flow-table="cumulative">
        <thead>
          <tr>
            <th scope="col">{t("flow-period")}</th>
            {report.bands.map((band) => (
              <th key={band.key} scope="col" className={styles.number}>
                {name(band)}
              </th>
            ))}
            <th scope="col" className={styles.number}>{t("flow-total")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.start}>
              <th scope="row" className="num">{names.full(r.start, r.end)}</th>
              {r.counts.map((n, b) => (
                <td key={report.bands[b]?.key ?? b} className={classes("num", styles.number)}>
                  {number.format(n)}
                </td>
              ))}
              <td className={classes("num", styles.number)}>{number.format(total(r.counts))}</td>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }
  return (
    <CumulativeFlowChart
      rows={rows}
      bands={report.bands}
      labels={rows.map((r) => names.short(r.start))}
      label={t("flow-chart-cumulative", { from: report.from, to: report.to })}
      summary={summary}
      describe={describe}
      legend={report.bands.map((band) => (
        <LegendItem key={band.key} colour={bandColour(band)}>
          {name(band)}
        </LegendItem>
      ))}
    />
  );
}

function Stats({ title, help, stats, days }: { title: string; help: string; stats: FlowStats; days: (n: number | undefined) => string }) {
  const t = useT();
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const figures: [string, string][] = [
    ["flow-stat-count", number.format(stats.count)],
    ["flow-stat-median", days(stats.p50)],
    ["flow-stat-p85", days(stats.p85)],
    ["flow-stat-p95", days(stats.p95)],
    ["flow-stat-mean", days(stats.mean)],
    ["flow-stat-min", days(stats.min)],
    ["flow-stat-max", days(stats.max)],
  ];
  return (
    <section className={styles.stats} aria-label={title}>
      <h3 className={styles.statsTitle}>{title}</h3>
      <p className={classes("faint", styles.note)}>{help}</p>
      <dl className={styles.figures}>
        {figures.map(([id, value]) => (
          <div key={id} className={styles.figure2} data-stat={id}>
            <dt className="muted">{t(id)}</dt>
            <dd className="num">{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

/** Cycle and lead time, as a chart with their figures or as tables; `compact` leaves the figures out (dashboards). */
export function CycleTime({ report, asTable, metric, compact = false }: { report: CycleTimeReport; asTable: boolean; metric: Metric; compact?: boolean }) {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const number = new Intl.NumberFormat(locale);
  const dayFormat = new Intl.NumberFormat(locale, { style: "unit", unit: "day", unitDisplay: "long", maximumFractionDigits: 1 });
  const days = (n: number | undefined) => (n === undefined ? t("flow-no-value") : dayFormat.format(n));
  const closedFormat = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", year: "numeric", timeZone: "UTC" });
  const names = usePeriodNames(report.bucket);
  const rows = report.items;
  const tasks = report.tasks ?? [];
  const stats = metric === "cycle" ? report.cycle : report.lead;
  const summary = t("flow-summary-cycle", { count: report.lead.count, median: days(report.cycle.p50) });
  const describe = (i: number) => {
    const r = rows[i];
    return r ? t("flow-point-cycle", { period: names.full(r.start, r.end), count: r.lead.count, cycle: days(r.cycle.p50), lead: days(r.lead.p50) }) : null;
  };
  const figures = compact ? null : (
    <div className={styles.statsRow}>
      <Stats title={t("flow-cycle-time")} help={t("flow-cycle-help")} stats={report.cycle} days={days} />
      <Stats title={t("flow-lead-time")} help={t("flow-lead-help")} stats={report.lead} days={days} />
    </div>
  );
  if (report.lead.count === 0) {
    return <p className={classes("faint", styles.note)}>{t("flow-none-closed")}</p>;
  }
  if (asTable) {
    const shown = tasks.slice(-TASK_ROWS).reverse();
    return (
      <>
        {figures}
        <table className={styles.table} aria-label={t("flow-chart-cycle", { from: report.from, to: report.to })} data-flow-table="cycle">
          <thead>
            <tr>
              <th scope="col">{t("flow-period")}</th>
              <th scope="col" className={styles.number}>{t("flow-stat-count")}</th>
              <th scope="col" className={styles.number}>{t("flow-cycle-time")}</th>
              <th scope="col" className={styles.number}>{t("flow-lead-time")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.start}>
                <th scope="row" className="num">{names.full(r.start, r.end)}</th>
                <td className={classes("num", styles.number)}>{number.format(r.lead.count)}</td>
                <td className={classes("num", styles.number)}>{days(r.cycle.p50)}</td>
                <td className={classes("num", styles.number)}>{days(r.lead.p50)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className={classes("faint", styles.note)}>{t("flow-tasks-shown", { shown: shown.length, total: tasks.length })}</p>
        <table className={styles.table} data-flow-table="cycle-tasks">
          <thead>
            <tr>
              <th scope="col">{t("flow-task")}</th>
              <th scope="col">{t("flow-closed-on")}</th>
              <th scope="col" className={styles.number}>{t("flow-cycle-time")}</th>
              <th scope="col" className={styles.number}>{t("flow-lead-time")}</th>
              <th scope="col" className={styles.number}>{t("flow-reopened")}</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((task) => (
              <tr key={task.key}>
                <td>
                  <Button className="linkbtn" onPress={() => void navigate({ to: "/open/$key", params: { key: task.key } })}>
                    <span className={classes("key", styles.rowKey)}>{task.key}</span>
                    {task.title}
                  </Button>
                </td>
                <td className="num">{closedFormat.format(utc(task.closed_day))}</td>
                <td className={classes("num", styles.number)}>{task.cycle_days === undefined ? <span className="faint">{t("flow-not-started")}</span> : days(task.cycle_days)}</td>
                <td className={classes("num", styles.number)}>{days(task.lead_days)}</td>
                <td className={classes("num", styles.number)}>{number.format(task.reopened)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </>
    );
  }
  return (
    <>
      <CycleTimeChart
        rows={rows}
        tasks={tasks}
        metric={metric}
        from={report.from}
        to={report.to}
        labels={rows.map((r) => names.short(r.start))}
        label={t("flow-chart-cycle", { from: report.from, to: report.to })}
        summary={summary}
        describe={describe}
        median={stats.p50}
        p85={stats.p85}
        legend={
          <>
            <LegendItem colour="var(--hue-teal)">{metric === "cycle" ? t("flow-cycle-time") : t("flow-lead-time")}</LegendItem>
            <LegendItem colour="var(--accent)" line>
              {t("flow-line-median")}
            </LegendItem>
            <LegendItem colour="var(--hue-orange)" line>
              {t("flow-line-p85")}
            </LegendItem>
          </>
        }
      />
      {figures}
    </>
  );
}
