// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { useLocale } from "react-aria-components";

import { Segmented } from "../../components/Segmented";
import { formatDuration } from "../../data/duration";
import { failureText } from "../../data/hooks";
import { chartDays, idealLine, remaining, useBurndown, type BurndownUnit } from "../../data/sprints";
import { useDurationUnits } from "../../data/time";
import { type BurndownDay, type Sprint } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { BurndownChart, IDEAL, REMAINING, SCOPE, Swatch } from "../reports/charts";
import styles from "./Sprints.module.css";

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

/**
 * The burndown of an active or completed sprint, in tasks or in estimated
 * time: the work still open each day against the ideal line, and the scope.
 */
export function Burndown({ sprint }: { sprint: Sprint }) {
  const t = useT();
  const { locale } = useLocale();
  const units = useDurationUnits();
  const burndown = useBurndown(sprint.id);
  const [unit, setUnit] = useState<BurndownUnit>("tasks");
  if (burndown.error) {
    return (
      <p className={styles.note} role="alert">
        {t("burndown-error", { message: failureText(t, burndown.error) })}
      </p>
    );
  }
  const data = burndown.data;
  if (!data) {
    return <p className={styles.note} aria-busy="true" />;
  }
  const number = new Intl.NumberFormat(locale);
  const format = (n: number) => (unit === "tasks" ? number.format(n) : formatDuration(Math.round(n), units));
  const total = (day: BurndownDay) => (unit === "tasks" ? day.total_tasks : day.total_estimate_minutes);
  const days = chartDays(data.days, sprint.start_date, sprint.end_date);
  const ideal = idealLine(unit === "tasks" ? data.start_tasks : data.start_estimate_minutes, days.length);
  const shortDate = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  const longDate = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });
  const last = data.days[data.days.length - 1];
  const daysLeft = sprint.state === "active" && sprint.end_date !== undefined ? days.filter((each) => each.day === undefined).length : 0;
  const summary =
    last === undefined
      ? t("burndown-empty")
      : sprint.state === "active"
        ? t("burndown-summary-active", { open: format(remaining(last, unit)), total: format(total(last)), days: daysLeft })
        : t("burndown-summary-closed", { open: format(remaining(last, unit)), total: format(total(last)) });
  return (
    <section className={styles.burndown} aria-label={t("burndown-title", { name: sprint.name })}>
      <div className={styles.burndownHead}>
        <h3 className={styles.subTitle}>{t("burndown-heading")}</h3>
        <Segmented label={t("burndown-unit")} value={unit} onChange={setUnit} segments={(["tasks", "estimate"] as const).map((each) => ({ id: each, label: t(`burndown-unit-${each}`) }))} />
      </div>
      <BurndownChart
        remaining={days.map((each) => (each.day === undefined ? undefined : remaining(each.day, unit)))}
        scope={days.map((each) => (each.day === undefined ? undefined : total(each.day)))}
        ideal={ideal}
        labels={days.map((each) => shortDate.format(utc(each.date)))}
        label={t("burndown-title", { name: sprint.name })}
        summary={summary}
        format={format}
        describe={(i) => {
          const each = days[i];
          if (each === undefined) {
            return null;
          }
          const date = longDate.format(utc(each.date));
          return each.day === undefined
            ? t("burndown-day-ahead", { date, ideal: format(ideal[i] ?? 0) })
            : t("burndown-day", { date, open: format(remaining(each.day, unit)), total: format(total(each.day)), ideal: format(ideal[i] ?? 0) });
        }}
        legend={
          <>
            <li>
              <Swatch colour={REMAINING} line />
              {t("burndown-legend-remaining")}
            </li>
            <li>
              <Swatch colour={IDEAL} line />
              {t("burndown-legend-ideal")}
            </li>
            <li>
              <Swatch colour={SCOPE} line />
              {t("burndown-legend-scope")}
            </li>
          </>
        }
      />
    </section>
  );
}
