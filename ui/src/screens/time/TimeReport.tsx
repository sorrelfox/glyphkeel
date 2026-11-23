// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { useOp } from "../../data/hooks";
import { useDurations } from "../../data/time";
import { type TimeGroupBy, type WeekStart } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { isoDay } from "../board/model";
import styles from "./TimeReport.module.css";

/** The list's key for "all projects"; no project key has this form. */
const ALL_PROJECTS = "*";

const GROUPS = ["day", "task", "project"] as const satisfies readonly TimeGroupBy[];

function addDays(day: string, days: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return isoDay(new Date(y ?? 1970, (m ?? 1) - 1, (d ?? 1) + days));
}

/** The first day of the week that `day` falls in. */
export function startOfWeek(day: string, weekStart: WeekStart): string {
  const [y, m, d] = day.split("-").map(Number);
  const weekday = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1).getDay();
  const first = weekStart === "sunday" ? 0 : weekStart === "saturday" ? 6 : 1;
  return addDays(day, -((weekday - first + 7) % 7));
}

/** The ranges offered with one click: this and last week, this and last month. */
export function presetRange(preset: "this-week" | "last-week" | "this-month" | "last-month", today: string, weekStart: WeekStart): [string, string] {
  const [y, m] = today.split("-").map(Number);
  const year = y ?? 1970;
  const month = (m ?? 1) - 1;
  switch (preset) {
    case "this-week": {
      const from = startOfWeek(today, weekStart);
      return [from, addDays(from, 6)];
    }
    case "last-week": {
      const from = addDays(startOfWeek(today, weekStart), -7);
      return [from, addDays(from, 6)];
    }
    case "this-month":
      return [isoDay(new Date(year, month, 1)), isoDay(new Date(year, month + 1, 0))];
    case "last-month":
      return [isoDay(new Date(year, month - 1, 1)), isoDay(new Date(year, month, 0))];
  }
}

const PRESETS = ["this-week", "last-week", "this-month", "last-month"] as const;

/**
 * The time report: the time logged over a range of days, grouped by day,
 * task or project, with the total; optionally for one project and its
 * sub-projects. A task's row opens the task.
 */
export function TimeReportScreen() {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const durations = useDurations();
  const settings = useOp("settings.get", {});
  const projects = useOp("project.list", {});
  const today = isoDay(new Date());
  const weekStart = settings.data?.week_start ?? "monday";
  const [range, setRange] = useState<[string, string] | null>(null);
  const [from, to] = range ?? presetRange("this-week", today, weekStart);
  const [groupBy, setGroupBy] = useState<TimeGroupBy>("task");
  const [project, setProject] = useState("");
  const report = useOp("time.report", { from, to, group_by: groupBy, ...(project === "" ? {} : { project }) });
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" });
  const most = Math.max(1, ...(report.data?.items ?? []).map((row) => row.minutes));
  const setDay = (which: 0 | 1, value: string) => {
    if (value === "") {
      return;
    }
    const next: [string, string] = which === 0 ? [value, to < value ? value : to] : [from > value ? value : from, value];
    setRange(next);
  };
  return (
    <section className={styles.screen} aria-label={t("view-time-report")}>
      <div className={styles.controls}>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-from")} value={from} onChange={(event) => { setDay(0, event.target.value); }} />
        </span>
        <span className="muted">–</span>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-to")} value={to} onChange={(event) => { setDay(1, event.target.value); }} />
        </span>
        {PRESETS.map((preset) => (
          <Button key={preset} className="btn flat sm" onPress={() => { setRange(presetRange(preset, today, weekStart)); }}>
            {t(`report-${preset}`)}
          </Button>
        ))}
        <div className="grow" />
        <Select
          className={styles.select}
          label={t("report-project")}
          value={project === "" ? ALL_PROJECTS : project}
          onChange={(key) => {
            setProject(key === ALL_PROJECTS ? "" : key);
          }}
          options={[{ id: ALL_PROJECTS, label: t("report-all-projects") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path }))]}
        />
        <div className="seg" role="radiogroup" aria-label={t("report-group-by")}>
          {GROUPS.map((group) => (
            <button key={group} type="button" role="radio" aria-checked={groupBy === group} className={classes(groupBy === group && "on")} onClick={() => { setGroupBy(group); }}>
              {t(`report-by-${group}`)}
            </button>
          ))}
        </div>
      </div>
      {report.isError && <p className="faint">{report.error.error.message}</p>}
      {report.data && report.data.items.length === 0 && <p className={classes("faint", styles.empty)}>{t("report-none")}</p>}
      {report.data && report.data.items.length > 0 && (
        <table className={styles.table} aria-label={t("report-table", { from, to })}>
          <thead>
            <tr>
              <th scope="col">{t(`report-by-${groupBy}`)}</th>
              <th scope="col" className={styles.number}>
                {t("report-entries")}
              </th>
              <th scope="col" className={styles.number}>
                {t("report-time")}
              </th>
              <th scope="col" className={styles.barCell} aria-label={t("report-share")} />
            </tr>
          </thead>
          <tbody>
            {report.data.items.map((row) => (
              <tr key={row.key} data-report-row={row.key}>
                <td>
                  {groupBy === "day" ? (
                    <span className="num">{weekday.format(new Date(`${row.key}T00:00:00Z`))}</span>
                  ) : groupBy === "task" ? (
                    <Button className="linkbtn" onPress={() => void navigate({ to: "/open/$key", params: { key: row.key } })}>
                      <span className={classes("key", styles.rowKey)}>{row.key}</span>
                      {row.title}
                    </Button>
                  ) : (
                    <>
                      <span className={classes("key", styles.rowKey)}>{row.key}</span>
                      {row.title}
                    </>
                  )}
                </td>
                <td className={classes("num muted", styles.number)}>{row.entry_count}</td>
                <td className={classes("num", styles.number)}>{durations.format(row.minutes)}</td>
                <td className={styles.barCell}>
                  <div className="meter" role="presentation">
                    <i style={{ width: `${String(Math.round((row.minutes / most) * 100))}%` }} />
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr>
              <td>{t("report-total")}</td>
              <td className={classes("num", styles.number)}>{report.data.items.reduce((sum, row) => sum + row.entry_count, 0)}</td>
              <td className={classes("num", styles.number)} data-report-total>
                {durations.format(report.data.total_minutes)}
              </td>
              <td />
            </tr>
          </tfoot>
        </table>
      )}
    </section>
  );
}
