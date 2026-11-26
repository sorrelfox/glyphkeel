// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { Bot, Download } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { errorToast, useCall, useOp } from "../../data/hooks";
import { useDurations } from "../../data/time";
import { useToday } from "../../data/zone";
import { type TimeGroupBy, type TimeReportCsv, type TimeReportInput, type TimeReportRow, type WeekStart } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { isoDay } from "../board/model";
import styles from "./TimeReport.module.css";

/** The list's key for "all projects"; no project key has this form. */
const ALL_PROJECTS = "*";

const GROUPS = ["day", "week", "month", "task", "project", "person"] as const satisfies readonly TimeGroupBy[];

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

const PRESETS = ["this-week", "last-week", "this-month", "last-month", "this-year"] as const;
export type Preset = (typeof PRESETS)[number];

/** The ranges offered with one click: this and last week, this and last month, this year. */
export function presetRange(preset: Preset, today: string, weekStart: WeekStart): [string, string] {
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
    case "this-year":
      return [isoDay(new Date(year, 0, 1)), isoDay(new Date(year, 11, 31))];
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

/**
 * The time report: the time logged over a range of days, grouped by day,
 * week, month, task, project or person, with the total; optionally for one
 * project, with or without its sub-projects. A task's row opens the task.
 * The groups or the entries can be saved as CSV.
 */
export function TimeReportScreen() {
  const t = useT();
  const { locale } = useLocale();
  const navigate = useNavigate();
  const call = useCall();
  const toasts = useToasts();
  const durations = useDurations();
  const settings = useOp("settings.get", {});
  const projects = useOp("project.list", {});
  const today = useToday();
  const weekStart = settings.data?.week_start ?? "monday";
  const [range, setRange] = useState<[string, string] | null>(null);
  const [from, to] = range ?? presetRange("this-week", today, weekStart);
  const [groupBy, setGroupBy] = useState<TimeGroupBy>("task");
  const [project, setProject] = useState("");
  const [subprojects, setSubprojects] = useState(true);
  const input: TimeReportInput = { from, to, group_by: groupBy, ...(project === "" ? {} : { project, include_subprojects: subprojects }) };
  const report = useOp("time.report", input);
  const dayFormat = new Intl.DateTimeFormat(locale, { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" });
  const weekFormat = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  const monthFormat = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric", timeZone: "UTC" });
  const count = new Intl.NumberFormat(locale);
  const most = Math.max(1, ...(report.data?.items ?? []).map((row) => row.minutes));
  const setDay = (which: 0 | 1, value: string) => {
    if (value === "") {
      return;
    }
    const next: [string, string] = which === 0 ? [value, to < value ? value : to] : [from > value ? value : from, value];
    setRange(next);
  };
  const exportCsv = (kind: TimeReportCsv) => {
    call("time.report", { ...input, csv: kind })
      .then((answer) => {
        const name = plain(t(kind === "entries" ? "report-file-entries" : "report-file-groups", { from, to }));
        return saveExport(`${name}.csv`, answer.csv ?? "", "text/csv").then((done) => {
          if (done) {
            toasts.show({ tone: "info", text: t("report-exported") });
          }
        });
      })
      .catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
  };
  const label = (row: TimeReportRow) => {
    switch (groupBy) {
      case "day":
        return <span className="num">{dayFormat.format(utc(row.key))}</span>;
      case "week":
        return <span className="num">{weekFormat.formatRange(utc(row.key), utc(addDays(row.key, 6)))}</span>;
      case "month":
        return <span className="num">{monthFormat.format(utc(`${row.key}-01`))}</span>;
      case "task":
        return (
          <Button className="linkbtn" onPress={() => void navigate({ to: "/open/$key", params: { key: row.key } })}>
            <span className={classes("key", styles.rowKey)}>{row.key}</span>
            {row.title}
          </Button>
        );
      case "project":
        return (
          <>
            <span className={classes("key", styles.rowKey)}>{row.key}</span>
            {row.title}
          </>
        );
      case "person":
        return (
          <span className={styles.person}>
            {row.title}
            {row.person?.kind === "agent" && (
              <span className={classes("faint", styles.agent)}>
                <Bot className="ic s" aria-hidden="true" />
                {t("report-agent")}
              </span>
            )}
          </span>
        );
    }
  };
  return (
    <section className={styles.screen} aria-label={t("view-time-report")}>
      <div className={styles.controls}>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-from")} value={from} onChange={(event) => { setDay(0, event.target.value); }} />
        </span>
        <span className="muted" aria-hidden="true">
          –
        </span>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("report-to")} value={to} onChange={(event) => { setDay(1, event.target.value); }} />
        </span>
        {PRESETS.map((preset) => (
          <Button key={preset} className="btn flat sm" onPress={() => { setRange(presetRange(preset, today, weekStart)); }}>
            {t(`report-${preset}`)}
          </Button>
        ))}
      </div>
      <div className={styles.controls}>
        <Select
          className={styles.select}
          label={t("report-project")}
          value={project === "" ? ALL_PROJECTS : project}
          onChange={(key) => {
            setProject(key === ALL_PROJECTS ? "" : key);
          }}
          options={[{ id: ALL_PROJECTS, label: t("report-all-projects") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.key, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))]}
        />
        {project !== "" && (
          <Switch isSelected={subprojects} onChange={setSubprojects}>
            {t("report-subprojects")}
          </Switch>
        )}
        <Select
          className={styles.select}
          label={t("report-group-by")}
          value={groupBy}
          onChange={(key) => {
            setGroupBy(GROUPS.find((group) => group === key) ?? "task");
          }}
          options={GROUPS.map((group) => ({ id: group, label: t(`report-by-${group}`) }))}
        />
        <div className="grow" />
        <MenuTrigger>
          <Button className="btn flat" isDisabled={!report.data || report.data.items.length === 0}>
            <Download className="ic" aria-hidden="true" />
            <span>{t("report-export")}</span>
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu
              className="menu"
              aria-label={t("report-export")}
              onAction={(id) => {
                exportCsv(id === "entries" ? "entries" : "groups");
              }}
            >
              <MenuItem id="groups" className="item">
                {t("report-export-groups")}
              </MenuItem>
              <MenuItem id="entries" className="item">
                {t("report-export-entries")}
              </MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
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
                <td>{label(row)}</td>
                <td className={classes("num muted", styles.number)}>{count.format(row.entry_count)}</td>
                <td className={classes("num", styles.number)}>{durations.format(row.minutes)}</td>
                <td className={styles.barCell}>
                  <div className="meter" role="presentation">
                    <i style={{ inlineSize: `${String(Math.round((row.minutes / most) * 100))}%` }} />
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr>
              <td>{t("report-total")}</td>
              <td className={classes("num", styles.number)}>{count.format(report.data.items.reduce((sum, row) => sum + row.entry_count, 0))}</td>
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
